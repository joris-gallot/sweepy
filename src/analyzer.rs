use anyhow::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser as OxcParser;
use oxc_span::SourceType;
use path_clean::PathClean;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ImportInfo {
  pub source: String,          // module specifier as written: "./greet"
  pub specifiers: Vec<String>, // imported names; empty => likely a side-effect import or `import * as ns`
  pub has_namespace: bool,     // true if `import * as ns from ...`
  pub has_default: bool,       // true if `import def from ...`
}

#[derive(Debug, Clone)]
struct ExportInfo {
  pub name: String,
  pub source: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ExportItem {
  Named(ExportInfo),
  All(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
  pub path: PathBuf,
  pub imports: Vec<ImportInfo>,
  pub exports: Vec<ExportItem>,
}

pub struct ProjectAnalyzer {
  pub files: HashMap<PathBuf, ParsedFile>,
  pub graph: HashMap<PathBuf, HashSet<PathBuf>>,
  pub import_usage: HashMap<PathBuf, Vec<(PathBuf, ImportInfo)>>,
  pub file_set: HashSet<PathBuf>,
}

impl ProjectAnalyzer {
  pub fn from_sources(sources: &HashMap<PathBuf, &str>) -> Result<Self> {
    let mut files = HashMap::new();

    for (path, content) in sources {
      let clean = normalize_soft(path);
      let pf = parse_module_from_path(&clean, content)?;

      files.insert(clean, pf);
    }

    let file_set: HashSet<PathBuf> = files.keys().cloned().collect();

    let mut graph: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
    let mut import_usage: HashMap<PathBuf, Vec<(PathBuf, ImportInfo)>> = HashMap::new();

    for (path, pf) in &files {
      for imp in &pf.imports {
        if is_relative(&imp.source)
          && let Some(target) = resolve_relative_import_from_set(&imp.source, &file_set)
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

      for export in &pf.exports {
        if let ExportItem::All(specifier_path) = export
          && let Some(spec) = specifier_path.to_str()
          && let Some(target) = resolve_relative_import_from_set(spec, &file_set)
        {
          graph
            .entry(path.clone())
            .or_default()
            .insert(target.clone());
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
  pub fn compute_reachable(&self, entrypoints: &Vec<PathBuf>) -> HashSet<PathBuf> {
    let file_set: HashSet<PathBuf> = self.files.keys().cloned().collect();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    let normalized_entrypoints: Vec<PathBuf> =
      entrypoints.iter().map(|ep| normalize_soft(ep)).collect();

    for ep in normalized_entrypoints {
      if file_set.contains(&ep) {
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

  pub fn find_unused_exports(&self) -> Vec<(PathBuf, String)> {
    let file_set: HashSet<PathBuf> = self.files.keys().cloned().collect();
    let mut unused_set: HashSet<(PathBuf, String)> = HashSet::new();

    for (module_path, pf) in &self.files {
      for export in &pf.exports {
        if let ExportItem::Named(exp) = export {
          let mut used = false;

          let mut reexporters: Vec<PathBuf> = Vec::new();

          for (other_module_path, other_pf) in &self.files {
            if other_module_path != module_path {
              for other_export in &other_pf.exports {
                let has_named_reexport = if let ExportItem::Named(other_exp) = other_export {
                  if let Some(src) = &other_exp.source {
                    if let Some(spec) = src.to_str()
                      && let Some(target) = resolve_relative_import_from_set(spec, &file_set)
                      && &target == module_path
                      && other_exp.name == exp.name
                    {
                      true
                    } else {
                      false
                    }
                  } else {
                    false
                  }
                } else {
                  false
                };

                let has_all_reexport = if let ExportItem::All(specifier_path) = other_export
                  && let Some(spec) = specifier_path.to_str()
                  && let Some(target) = resolve_relative_import_from_set(spec, &file_set)
                  && &target == module_path
                {
                  true
                } else {
                  false
                };

                if has_named_reexport || has_all_reexport {
                  reexporters.push(other_module_path.clone());
                }
              }
            }
          }

          for reexporter_path in reexporters {
            if let Some(importers) = self.import_usage.get(&reexporter_path) {
              for (_importer_path, import_info) in importers {
                if import_info.has_namespace
                  || (exp.name == "default" && import_info.has_default)
                  || import_info.specifiers.iter().any(|s| s == &exp.name)
                {
                  used = true;
                  break;
                }
              }
            }
            if used {
              break;
            }
          }

          if let Some(importers) = self.import_usage.get(module_path) {
            for (_importer_path, import_info) in importers {
              if import_info.has_namespace
                || (exp.name == "default" && import_info.has_default)
                || import_info.specifiers.iter().any(|s| s == &exp.name)
              {
                used = true;
                break;
              }
            }
          }

          if !used {
            unused_set.insert((module_path.clone(), exp.name.clone()));
          }
        }
      }
    }

    let mut unused_vec: Vec<_> = unused_set.into_iter().collect();
    unused_vec.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    unused_vec
  }
}

/// Parse a source string in memory
fn parse_module_from_path(path: &Path, source: &str) -> Result<ParsedFile> {
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

fn normalize_soft(path: &Path) -> PathBuf {
  PathBuf::from(path).clean()
}

fn extract_imports_exports(path: &Path, program: &Program) -> ParsedFile {
  let mut imports: Vec<ImportInfo> = Vec::new();
  let mut exports: Vec<ExportItem> = Vec::new();

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

            let source_path = PathBuf::from(src.value.to_string());

            exports.push(ExportItem::Named(ExportInfo {
              name: exported,
              source: Some(source_path),
            }));
          }
        } else {
          if let Some(decl) = &export.declaration {
            match decl {
              Declaration::TSInterfaceDeclaration(int) => {
                exports.push(ExportItem::Named(ExportInfo {
                  name: int.id.name.to_string(),
                  source: None,
                }));
              }
              Declaration::TSTypeAliasDeclaration(ta) => {
                exports.push(ExportItem::Named(ExportInfo {
                  name: ta.id.name.to_string(),
                  source: None,
                }));
              }
              Declaration::TSEnumDeclaration(en) => {
                exports.push(ExportItem::Named(ExportInfo {
                  name: en.id.name.to_string(),
                  source: None,
                }));
              }
              Declaration::TSModuleDeclaration(md) => {
                exports.push(ExportItem::Named(ExportInfo {
                  name: md.id.name().to_string(),
                  source: None,
                }));
              }
              Declaration::FunctionDeclaration(fd) => {
                if let Some(id) = &fd.id {
                  exports.push(ExportItem::Named(ExportInfo {
                    name: id.name.to_string(),
                    source: None,
                  }));
                }
              }
              Declaration::VariableDeclaration(vd) => {
                for declarator in &vd.declarations {
                  let exported = declarator
                    .id
                    .get_identifier_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                  exports.push(ExportItem::Named(ExportInfo {
                    name: exported,
                    source: None,
                  }));
                }
              }
              Declaration::ClassDeclaration(cd) => {
                if let Some(id) = &cd.id {
                  exports.push(ExportItem::Named(ExportInfo {
                    name: id.name.to_string(),
                    source: None,
                  }));
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

            exports.push(ExportItem::Named(ExportInfo {
              name: exported,
              source: None,
            }));
          }
        }
      }

      Statement::ExportDefaultDeclaration(_) => {
        exports.push(ExportItem::Named(ExportInfo {
          name: "default".to_string(),
          source: None,
        }));
      }

      Statement::ExportAllDeclaration(export_all) => {
        exports.push(ExportItem::All(PathBuf::from(
          export_all.source.value.to_string(),
        )));
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
  spec: &str,
  file_set: &HashSet<PathBuf>,
) -> Option<PathBuf> {
  let candidate = normalize_soft(Path::new(spec));

  const CANDIDATE_EXTS: &[&str] = &[".ts", ".tsx", ".js", ".jsx"];

  for ext in CANDIDATE_EXTS {
    let with_extension = candidate.with_extension(ext.trim_start_matches('.'));

    if file_set.contains(&with_extension) {
      return Some(with_extension);
    }
  }

  if file_set.contains(&candidate) {
    return Some(candidate);
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

    sources.insert(
      PathBuf::from("./path-to-exports/exports-named.ts"),
      exports_named(),
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(!reachable.contains(&PathBuf::from("path-to-exports/exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(
      unused_exports,
      vec![
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "Baz".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyAbstractClass".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyEnum".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyInterface".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyNamespace".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyType".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "bar".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "foo".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myArrowFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myAsyncFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myConstEnum".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myDeclaredFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myGeneratorFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myIntersectionType".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myOverloadedFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myTuple".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
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
        import { foo, bar, type MyInterface } from "./path-to-exports/exports-named";
        import type { MyType } from "./path-to-exports/exports-named";
        console.log("Hello World");
      "#,
    );

    sources.insert(
      PathBuf::from("./path-to-exports/exports-named.ts"),
      exports_named(),
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(reachable.contains(&PathBuf::from("path-to-exports/exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(
      unused_exports,
      vec![
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "Baz".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyAbstractClass".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyEnum".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "MyNamespace".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myArrowFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myAsyncFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myConstEnum".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myDeclaredFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myGeneratorFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myIntersectionType".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myOverloadedFunction".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
          "myTuple".to_string()
        ),
        (
          PathBuf::from("path-to-exports/exports-named.ts"),
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
        import * as exportsNamed from "./path-to-exports/exports-named";
        console.log("Hello World");
      "#,
    );

    sources.insert(
      PathBuf::from("./path-to-exports/exports-named.ts"),
      exports_named(),
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(reachable.contains(&PathBuf::from("path-to-exports/exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(unused_exports, vec![]);
  }

  #[test]
  fn test_import_all_exports_all() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        import * as all from "./exports-all";
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("exports-named.ts"), exports_named());
    sources.insert(
      PathBuf::from("./exports-all.ts"),
      r#"
        export * from './exports-named'
        export const extra = 'extra'
      "#,
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(reachable.contains(&PathBuf::from("exports-all.ts")));
    assert!(reachable.contains(&PathBuf::from("exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(unused_exports, vec![]);
  }

  #[test]
  fn test_import_some_exports_all() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        import { foo, extra } from "./exports-all";
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("exports-named.ts"), exports_named());
    sources.insert(
      PathBuf::from("./exports-all.ts"),
      r#"
        export * from './exports-named'
        export const extra = 'extra'
      "#,
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(reachable.contains(&PathBuf::from("exports-all.ts")));
    assert!(reachable.contains(&PathBuf::from("exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(
      unused_exports,
      vec![
        (PathBuf::from("exports-named.ts"), "Baz".to_string()),
        (
          PathBuf::from("exports-named.ts"),
          "MyAbstractClass".to_string()
        ),
        (PathBuf::from("exports-named.ts"), "MyEnum".to_string()),
        (PathBuf::from("exports-named.ts"), "MyInterface".to_string()),
        (PathBuf::from("exports-named.ts"), "MyNamespace".to_string()),
        (PathBuf::from("exports-named.ts"), "MyType".to_string()),
        (PathBuf::from("exports-named.ts"), "bar".to_string()),
        (
          PathBuf::from("exports-named.ts"),
          "myArrowFunction".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myAsyncFunction".to_string()
        ),
        (PathBuf::from("exports-named.ts"), "myConstEnum".to_string()),
        (
          PathBuf::from("exports-named.ts"),
          "myDeclaredFunction".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myGeneratorFunction".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myIntersectionType".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myOverloadedFunction".to_string()
        ),
        (PathBuf::from("exports-named.ts"), "myTuple".to_string()),
        (PathBuf::from("exports-named.ts"), "myUnionType".to_string()),
      ]
    );
  }

  #[test]
  fn test_import_some_exports_some() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        import { foo, extra } from "./exports-all";
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("exports-named.ts"), exports_named());
    sources.insert(
      PathBuf::from("./exports-all.ts"),
      r#"
        export { foo, bar, Baz } from './exports-named'
        export const extra = 'extra'
      "#,
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(reachable.contains(&PathBuf::from("exports-all.ts")));
    assert!(reachable.contains(&PathBuf::from("exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();
    assert_eq!(
      unused_exports,
      vec![
        (PathBuf::from("exports-named.ts"), "Baz".to_string()),
        (PathBuf::from("exports-named.ts"), "bar".to_string()),
      ]
    );
  }

  #[test]
  fn test_unused_exports_all() {
    let mut sources = HashMap::new();

    sources.insert(
      PathBuf::from("./index.ts"),
      r#"
        console.log("Hello World");
      "#,
    );

    sources.insert(PathBuf::from("exports-named.ts"), exports_named());
    sources.insert(
      PathBuf::from("./exports-all.ts"),
      r#"
        export * from './exports-named'
        export const extra = 'extra'
      "#,
    );

    let sources_ref: HashMap<PathBuf, &str> =
      sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

    let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
    let entrypoints = vec![PathBuf::from("./index.ts")];
    let reachable = analyzer.compute_reachable(&entrypoints);

    assert!(reachable.contains(&PathBuf::from("index.ts")));
    assert!(!reachable.contains(&PathBuf::from("exports-all.ts")));
    assert!(!reachable.contains(&PathBuf::from("exports-named.ts")));

    let unused_exports = analyzer.find_unused_exports();

    assert_eq!(
      unused_exports,
      vec![
        (PathBuf::from("exports-all.ts"), "extra".to_string()),
        (PathBuf::from("exports-named.ts"), "Baz".to_string()),
        (
          PathBuf::from("exports-named.ts"),
          "MyAbstractClass".to_string()
        ),
        (PathBuf::from("exports-named.ts"), "MyEnum".to_string()),
        (PathBuf::from("exports-named.ts"), "MyInterface".to_string()),
        (PathBuf::from("exports-named.ts"), "MyNamespace".to_string()),
        (PathBuf::from("exports-named.ts"), "MyType".to_string()),
        (PathBuf::from("exports-named.ts"), "bar".to_string()),
        (PathBuf::from("exports-named.ts"), "foo".to_string()),
        (
          PathBuf::from("exports-named.ts"),
          "myArrowFunction".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myAsyncFunction".to_string()
        ),
        (PathBuf::from("exports-named.ts"), "myConstEnum".to_string()),
        (
          PathBuf::from("exports-named.ts"),
          "myDeclaredFunction".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myGeneratorFunction".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myIntersectionType".to_string()
        ),
        (
          PathBuf::from("exports-named.ts"),
          "myOverloadedFunction".to_string()
        ),
        (PathBuf::from("exports-named.ts"), "myTuple".to_string()),
        (PathBuf::from("exports-named.ts"), "myUnionType".to_string()),
      ]
    );
  }

  // #[test]
  // fn tessss() {
  //   let mut sources = HashMap::new();

  //   sources.insert(
  //     PathBuf::from("./index.ts"),
  //     r#"
  //       import { foo } from "./exports-named";';
  //       console.log("Hello World");
  //     "#,
  //   );

  //   sources.insert(PathBuf::from("exports-named.ts"), exports_named());
  //   sources.insert(
  //     PathBuf::from("./exports-all.ts"),
  //     r#"
  //       export * from './exports-named'
  //       export const extra = 'extra'
  //     "#,
  //   );

  //   let sources_ref: HashMap<PathBuf, &str> =
  //     sources.iter().map(|(p, c)| (p.clone(), *c)).collect();

  //   let analyzer = ProjectAnalyzer::from_sources(&sources_ref).unwrap();
  //   let entrypoints = vec![PathBuf::from("./index.ts")];
  //   let reachable = analyzer.compute_reachable(&entrypoints);

  //   assert!(reachable.contains(&PathBuf::from("index.ts")));
  //   assert!(!reachable.contains(&PathBuf::from("./exports-all.ts")));
  //   assert!(!reachable.contains(&PathBuf::from("exports-named.ts")));

  //   let unused_exports = analyzer.find_unused_exports();

  //   assert_eq!(
  //     unused_exports,
  //     vec![
  //       (PathBuf::from("./exports-all.ts"), "Baz".to_string()),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "MyAbstractClass".to_string()
  //       ),
  //       (PathBuf::from("./exports-all.ts"), "MyEnum".to_string()),
  //       (PathBuf::from("./exports-all.ts"), "MyInterface".to_string()),
  //       (PathBuf::from("./exports-all.ts"), "MyNamespace".to_string()),
  //       (PathBuf::from("./exports-all.ts"), "MyType".to_string()),
  //       (PathBuf::from("./exports-all.ts"), "bar".to_string()),
  //       (PathBuf::from("./exports-all.ts"), "extra".to_string()),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "myArrowFunction".to_string()
  //       ),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "myAsyncFunction".to_string()
  //       ),
  //       (PathBuf::from("./exports-all.ts"), "myConstEnum".to_string()),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "myDeclaredFunction".to_string()
  //       ),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "myGeneratorFunction".to_string()
  //       ),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "myIntersectionType".to_string()
  //       ),
  //       (
  //         PathBuf::from("./exports-all.ts"),
  //         "myOverloadedFunction".to_string()
  //       ),
  //       (PathBuf::from("./exports-all.ts"), "myTuple".to_string()),
  //       (PathBuf::from("./exports-all.ts"), "myUnionType".to_string()),
  //     ]
  //   );
  // }
}
