use anyhow::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser as OxcParser;
use oxc_span::SourceType;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Info about an import in a module
#[derive(Debug, Clone)]
pub struct ImportInfo {
  pub source: String,          // module specifier as written: "./greet"
  pub specifiers: Vec<String>, // imported names; empty => likely a side-effect import or `import * as ns`
  pub has_namespace: bool,     // true if `import * as ns from ...`
  pub has_default: bool,       // true if `import def from ...`
}

/// Parsed file representation
#[derive(Debug, Clone)]
pub struct ParsedFile {
  pub path: PathBuf,
  pub imports: Vec<ImportInfo>,
  pub exports: Vec<String>,
}

/// Analyzer struct
pub struct ProjectAnalyzer {
  pub files: HashMap<PathBuf, ParsedFile>,
  pub graph: HashMap<PathBuf, HashSet<PathBuf>>,
  pub import_usage: HashMap<PathBuf, Vec<(PathBuf, ImportInfo)>>,
  pub file_set: HashSet<PathBuf>,
}

impl ProjectAnalyzer {
  /// Create ProjectAnalyzer from sources in memory
  pub fn from_sources(sources: &HashMap<PathBuf, &str>) -> Result<Self> {
    let mut files = HashMap::new();

    for (path, content) in sources {
      let pf = parse_from_str(path, content)?;
      files.insert(path.clone(), pf);
    }

    let file_set: HashSet<PathBuf> = files.keys().cloned().collect();
    let mut graph: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
    let mut import_usage: HashMap<PathBuf, Vec<(PathBuf, ImportInfo)>> = HashMap::new();

    for (path, pf) in &files {
      for imp in &pf.imports {
        if is_relative(&imp.source)
          && let Some(target) = resolve_relative_import_from_set(path, &imp.source, &file_set)
        {
          graph
            .entry(path.clone())
            .or_default()
            .insert(target.clone());
          import_usage
            .entry(target.clone())
            .or_default()
            .push((path.clone(), imp.clone()));
        }
      }
    }

    Ok(Self {
      files,
      graph,
      import_usage,
      file_set,
    })
  }

  /// Compute reachable files from entrypoints
  pub fn compute_reachable(&self, entrypoints: &[PathBuf]) -> HashSet<PathBuf> {
    let file_set: HashSet<PathBuf> = self.files.keys().cloned().collect();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    for ep in entrypoints {
      if file_set.contains(ep) {
        visited.insert(ep.clone());
        stack.push(ep.clone());
      }
    }

    while let Some(node) = stack.pop() {
      if let Some(neighbors) = self.graph.get(&node) {
        for n in neighbors {
          if !visited.contains(n) {
            visited.insert(n.clone());
            stack.push(n.clone());
          }
        }
      }
    }

    visited
  }

  /// Find unused exports
  pub fn find_unused_exports(&self) -> Vec<(PathBuf, String)> {
    let mut unused_set: HashSet<(PathBuf, String)> = HashSet::new();

    for (module_path, pf) in &self.files {
      let importers = self.import_usage.get(module_path);

      for export in &pf.exports {
        let mut used = false;

        if let Some(importers) = importers {
          for (_importer_path, import_info) in importers {
            if import_info.has_namespace
              || (export == "default" && import_info.has_default)
              || import_info.specifiers.iter().any(|s| s == export)
            {
              used = true;
              break;
            }
          }
        }

        if !used {
          unused_set.insert((module_path.clone(), export.clone()));
        }
      }
    }

    let mut unused_vec: Vec<_> = unused_set.into_iter().collect();
    unused_vec.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1))); // optionnel, pour tri stable
    unused_vec
  }
}

/// Helper: parse a source string in memory
fn parse_from_str(path: &Path, source: &str) -> Result<ParsedFile> {
  let allocator = Allocator::default();
  let source_type = SourceType::from_path(path).unwrap_or(SourceType::ts());
  let parser = OxcParser::new(&allocator, source, source_type);
  let parsed = parser.parse();

  if !parsed.errors.is_empty() {
    eprintln!(
      "Parser errors in {}: {} error(s)",
      path.display(),
      parsed.errors.len()
    );
  }

  let program = parsed.program;
  let pf = extract_imports_exports(path, &program);
  Ok(pf)
}

/// Extract imports & exports from Oxc Program
fn extract_imports_exports(path: &Path, program: &Program) -> ParsedFile {
  let mut imports = Vec::new();
  let mut exports = Vec::new();

  for stmt in &program.body {
    match stmt {
      Statement::ImportDeclaration(import) => {
        let source_s = import.source.value.to_string();
        let mut specifiers = Vec::new();
        let mut has_namespace = false;
        let mut has_default = false;

        if let Some(_specifiers) = &import.specifiers {
          for spec in _specifiers {
            match spec {
              ImportDeclarationSpecifier::ImportSpecifier(named) => {
                let name = named.imported.name().to_string();
                specifiers.push(name);
              }
              ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                has_namespace = true;
              }
              ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
                has_default = true;
              }
            }
          }
        }

        imports.push(ImportInfo {
          source: source_s,
          specifiers,
          has_namespace,
          has_default,
        });
      }

      Statement::ExportNamedDeclaration(export) => {
        if let Some(src) = &export.source {
          for spec in &export.specifiers {
            let exported = spec
              .exported
              .identifier_name()
              .map(|n| n.to_string())
              .or_else(|| spec.local.identifier_name().map(|n| n.to_string()))
              .unwrap_or_else(|| "<unknown>".to_string());
            exports.push(exported);
          }
        } else {
          if let Some(decl) = &export.declaration {
            match decl {
              Declaration::TSInterfaceDeclaration(int) => {
                exports.push(int.id.name.to_string());
              }
              Declaration::TSTypeAliasDeclaration(ta) => {
                exports.push(ta.id.name.to_string());
              }
              Declaration::TSEnumDeclaration(en) => {
                exports.push(en.id.name.to_string());
              }
              Declaration::TSModuleDeclaration(md) => {
                exports.push(md.id.name().to_string());
              }
              Declaration::FunctionDeclaration(fd) => {
                if let Some(id) = &fd.id {
                  exports.push(id.name.to_string());
                }
              }
              Declaration::VariableDeclaration(vd) => {
                for declarator in &vd.declarations {
                  let exported = declarator
                    .id
                    .get_identifier_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                  exports.push(exported);
                }
              }
              Declaration::ClassDeclaration(cd) => {
                if let Some(id) = &cd.id {
                  exports.push(id.name.to_string());
                }
              }
              _ => {}
            }
          }

          for spec in &export.specifiers {
            let exported = spec
              .exported
              .identifier_name()
              .map(|n| n.to_string())
              .or_else(|| spec.local.identifier_name().map(|n| n.to_string()))
              .unwrap_or_else(|| "<unknown>".to_string());
            exports.push(exported);
          }
        }
      }

      Statement::ExportDefaultDeclaration(_) => {
        exports.push("default".to_string());
      }

      Statement::ExportAllDeclaration(_) => {
        exports.push("*".to_string());
      }

      _ => {}
    }
  }

  ParsedFile {
    path: path.to_path_buf(),
    imports,
    exports,
  }
}

/// Check if a module specifier is relative
fn is_relative(spec: &str) -> bool {
  spec.starts_with('.')
}

/// Resolve relative import in memory using a HashSet of file paths
pub fn resolve_relative_import_from_set(
  from: &Path,
  spec: &str,
  file_set: &HashSet<PathBuf>,
) -> Option<PathBuf> {
  let base = from.parent().unwrap_or(Path::new("."));
  let candidate = base.join(spec);

  const CANDIDATE_EXTS: &[&str] = &[".ts", ".tsx", ".js", ".jsx"];

  for ext in CANDIDATE_EXTS {
    let p = candidate.with_extension(ext.trim_start_matches('.'));
    if file_set.contains(&p) {
      return Some(p);
    }
  }

  if file_set.contains(&candidate) {
    return Some(candidate);
  }

  for ext in CANDIDATE_EXTS {
    let p = candidate.join(format!("index{}", ext));
    if file_set.contains(&p) {
      return Some(p);
    }
  }

  for ext in CANDIDATE_EXTS {
    let p = PathBuf::from(format!("{}{}", candidate.display(), ext));
    if file_set.contains(&p) {
      return Some(p);
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use super::super::analyzer::ProjectAnalyzer;
  use std::collections::HashMap;
  use std::path::PathBuf;

  fn exports_named() -> &'static str {
    r#"
      export const foo = 'foo'
      export function bar() { return 'bar' }
      export class Baz { greet() { return 'Hello from Baz' } }
      export interface MyInterface { id: number; name: string }
      export type MyType = { value: string }
      export enum MyEnum { FIRST, SECOND, THIRD }
      export namespace MyNamespace { export function sayHello() { return 'Hello from MyNamespace' } }
      export const myArrowFunction = (x: number): number => x * x
      export async function myAsyncFunction(): Promise<string> { return 'This is an async function' }
      export function* myGeneratorFunction() { yield 1; yield 2; yield 3 }
      export const myTuple: [number, string] = [1, 'one']
      export const myUnionType: number | string = 'union'
      export const myIntersectionType: { a: number } & { b: string } = { a: 1, b: 'two' }
      export function myOverloadedFunction(x: number): number
      export function myOverloadedFunction(x: string): string
      export function myOverloadedFunction(x: number | string): number | string {
          if (typeof x === 'number') { return x * 2 } else { return x + x }
      }
      export abstract class MyAbstractClass { abstract getName(): string }
      export declare function myDeclaredFunction(param: string): void
      export const myConstEnum = { A: 1, B: 2, C: 3 } as const
    "#
  }

  #[test]
  fn test_unused_exports_named() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("./exports-named.ts"), exports_named());

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("./index.ts")));
    assert!(!reachable.contains(&PathBuf::from("./exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(
      unused_exports,
      vec![
        (PathBuf::from("./exports-named.ts"), "Baz".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "MyAbstractClass".to_string()
        ),
        (PathBuf::from("./exports-named.ts"), "MyEnum".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "MyInterface".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "MyNamespace".to_string()
        ),
        (PathBuf::from("./exports-named.ts"), "MyType".to_string()),
        (PathBuf::from("./exports-named.ts"), "bar".to_string()),
        (PathBuf::from("./exports-named.ts"), "foo".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "myArrowFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myAsyncFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myConstEnum".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myDeclaredFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myGeneratorFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myIntersectionType".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myOverloadedFunction".to_string()
        ),
        (PathBuf::from("./exports-named.ts"), "myTuple".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "myUnionType".to_string()
        ),
      ]
    );
  }

  #[test]
  fn test_import_exports_named() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        import { foo, bar, type MyInterface } from "./exports-named";
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("./exports-named.ts"), exports_named());

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("./index.ts")));
    assert!(reachable.contains(&PathBuf::from("./exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(
      unused_exports,
      vec![
        (PathBuf::from("./exports-named.ts"), "Baz".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "MyAbstractClass".to_string()
        ),
        (PathBuf::from("./exports-named.ts"), "MyEnum".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "MyNamespace".to_string()
        ),
        (PathBuf::from("./exports-named.ts"), "MyType".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "myArrowFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myAsyncFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myConstEnum".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myDeclaredFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myGeneratorFunction".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myIntersectionType".to_string()
        ),
        (
          PathBuf::from("./exports-named.ts"),
          "myOverloadedFunction".to_string()
        ),
        (PathBuf::from("./exports-named.ts"), "myTuple".to_string()),
        (
          PathBuf::from("./exports-named.ts"),
          "myUnionType".to_string()
        ),
      ]
    );
  }

  #[test]
  fn test_import_all_exports_named() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        import * as exportsNamed from "./exports-named";
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("./exports-named.ts"), exports_named());

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("./index.ts")));
    assert!(reachable.contains(&PathBuf::from("./exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(unused_exports, vec![]);
  }
}
