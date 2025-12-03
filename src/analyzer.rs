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
  pub source: String,          // module specifier as written: "./foo"
  pub specifiers: Vec<String>, // imported names; empty => likely a side-effect import or `import * as ns`
  pub has_namespace: bool,     // true if `import * as ns from ...`
  pub has_default: bool,       // true if `import def from ...`
}

#[derive(Debug, Clone)]
pub struct ExportInfo {
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
  pub imports: Vec<ImportInfo>,
  pub exports: Vec<ExportItem>,
}

pub struct ProjectAnalyzer {
  pub files: HashMap<PathBuf, ParsedFile>,
  pub graph: HashMap<PathBuf, HashSet<PathBuf>>,
  pub import_usage: HashMap<PathBuf, Vec<(PathBuf, ImportInfo)>>,
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

      for export in &pf.exports {
        match export {
          ExportItem::All(specifier_path) => {
            if let Some(spec) = specifier_path.to_str()
              && let Some(target) = resolve_relative_import_from_set(path, spec, &file_set)
            {
              graph
                .entry(path.clone())
                .or_default()
                .insert(target.clone());
              true
            } else {
              false
            }
          }
          ExportItem::Named(exp) => {
            if let Some(src) = &exp.source {
              if let Some(spec) = src.to_str()
                && let Some(target) = resolve_relative_import_from_set(path, spec, &file_set)
              {
                graph
                  .entry(path.clone())
                  .or_default()
                  .insert(target.clone());
                true
              } else {
                false
              }
            } else {
              false
            }
          }
        };
      }
    }

    Ok(Self {
      files,
      graph,
      import_usage,
    })
  }

  /// Compute reachable files from entrypoints
  pub fn compute_reachable(&self, entrypoints: Vec<PathBuf>) -> HashSet<PathBuf> {
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
                      && let Some(target) =
                        resolve_relative_import_from_set(other_module_path, spec, &file_set)
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
                  && let Some(target) =
                    resolve_relative_import_from_set(other_module_path, spec, &file_set)
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
  let pf = extract_imports_exports(&program);
  Ok(pf)
}

fn normalize_soft(path: &Path) -> PathBuf {
  PathBuf::from(path).clean()
}

fn extract_imports_exports(program: &Program) -> ParsedFile {
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

  ParsedFile { imports, exports }
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
  let candidate = normalize_soft(Path::new(spec));

  let candidate = if is_relative(spec) {
    let from_dir = from.parent().unwrap_or(Path::new(""));
    normalize_soft(&from_dir.join(&candidate))
  } else {
    candidate
  };

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
  use super::*;
  use std::collections::HashMap;
  use std::path::PathBuf;

  /// Test project builder for creating in-memory test projects
  struct TestProject {
    sources: HashMap<PathBuf, String>,
    entries: Vec<PathBuf>,
  }

  impl TestProject {
    fn new() -> Self {
      Self {
        sources: HashMap::new(),
        entries: Vec::new(),
      }
    }

    fn add_file(mut self, path: &str, content: &str) -> Self {
      self
        .sources
        .insert(PathBuf::from(path), content.to_string());
      self
    }

    fn entry(mut self, path: &str) -> Self {
      self.entries.push(PathBuf::from(path));
      self
    }

    fn build(self) -> (ProjectAnalyzer, Vec<PathBuf>) {
      let sources_ref: HashMap<PathBuf, &str> = self
        .sources
        .iter()
        .map(|(p, c)| (p.clone(), c.as_str()))
        .collect();

      let analyzer = ProjectAnalyzer::from_sources(&sources_ref).expect("Failed to build analyzer");

      (analyzer, self.entries)
    }
  }

  fn assert_reachable(analyzer: &ProjectAnalyzer, entries: &[PathBuf], expected: &[&str]) {
    let reachable = analyzer.compute_reachable(entries.to_vec());
    for path in expected {
      assert!(
        reachable.contains(&PathBuf::from(path)),
        "Expected {} to be reachable",
        path
      );
    }
  }

  fn assert_unused(analyzer: &ProjectAnalyzer, expected: Vec<(&str, &str)>) {
    let mut unused = analyzer.find_unused_exports();
    let mut expected_sorted: Vec<_> = expected
      .into_iter()
      .map(|(file, name)| (PathBuf::from(file), name.to_string()))
      .collect();
    expected_sorted.sort();
    unused.sort();
    assert_eq!(unused, expected_sorted);
  }

  // ===== Basic Named Exports =====
  mod basic {
    use super::*;

    #[test]
    fn named_exports_all_unused() {
      let project = TestProject::new()
        .add_file("index.ts", "console.log('entry');")
        .add_file(
          "utils.ts",
          "export const foo = 1;\nexport function bar() {}",
        )
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "bar"), ("utils.ts", "foo")]);
    }

    #[test]
    fn named_exports_some_used() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "bar")]);
    }

    #[test]
    fn named_exports_all_used() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo, bar } from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![]);
    }

    #[test]
    fn type_only_imports() {
      let project = TestProject::new()
        .add_file(
          "index.ts",
          "import type { MyType } from './types'; import { foo } from './types';",
        )
        .add_file(
          "types.ts",
          "export type MyType = string;\nexport const foo = 1;\nexport const bar = 2;",
        )
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "types.ts"]);
      assert_unused(&analyzer, vec![("types.ts", "bar")]);
    }
  }

  // ===== Default Exports =====
  mod default_exports {
    use super::*;

    #[test]
    fn default_export_unused() {
      let project = TestProject::new()
        .add_file("index.ts", "console.log('entry');")
        .add_file("utils.ts", "export default function foo() {}")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "default")]);
    }

    #[test]
    fn default_export_used() {
      let project = TestProject::new()
        .add_file("index.ts", "import foo from './utils';")
        .add_file("utils.ts", "export default function foo() {}")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![]);
    }

    #[test]
    fn mixed_default_and_named() {
      let project = TestProject::new()
        .add_file("index.ts", "import foo, { bar } from './utils';")
        .add_file(
          "utils.ts",
          "export default function foo() {}\nexport const bar = 1;\nexport const baz = 2;",
        )
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "baz")]);
    }
  }

  // ===== Namespace Imports =====
  mod namespace {
    use super::*;

    #[test]
    fn namespace_import_marks_all_used() {
      let project = TestProject::new()
        .add_file("index.ts", "import * as utils from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![]);
    }
  }

  // ===== Re-exports =====
  mod reexports {
    use super::*;

    #[test]
    fn reexport_all_unused() {
      let project = TestProject::new()
        .add_file("index.ts", "console.log('entry');")
        .add_file("barrel.ts", "export * from './utils';")
        .add_file("utils.ts", "export const foo = 1;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "foo")]);
    }

    #[test]
    fn reexport_all_used() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './barrel';")
        .add_file("barrel.ts", "export * from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "barrel.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "bar")]);
    }

    #[test]
    fn reexport_named() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './barrel';")
        .add_file("barrel.ts", "export { foo, bar } from './utils';")
        .add_file(
          "utils.ts",
          "export const foo = 1;\nexport const bar = 2;\nexport const baz = 3;",
        )
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "barrel.ts", "utils.ts"]);
      assert_unused(
        &analyzer,
        vec![
          ("barrel.ts", "bar"),
          ("utils.ts", "bar"),
          ("utils.ts", "baz"),
        ],
      );
    }

    #[test]
    fn reexport_with_alias() {
      let project = TestProject::new()
        .add_file("index.ts", "import { myFoo } from './barrel';")
        .add_file("barrel.ts", "export { foo as myFoo } from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "barrel.ts", "utils.ts"]);
      // The alias export creates a new export name, so 'foo' from utils is not directly used
      // Only 'bar' remains unused since 'myFoo' is imported (but myFoo doesn't exist in utils)
      assert_unused(&analyzer, vec![("utils.ts", "bar"), ("utils.ts", "foo")]);
    }

    #[test]
    fn namespace_import_via_reexport() {
      let project = TestProject::new()
        .add_file("index.ts", "import * as utils from './barrel';")
        .add_file("barrel.ts", "export * from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "barrel.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![]);
    }
  }

  // ===== Path Resolution =====
  mod path_resolution {
    use super::*;

    #[test]
    fn deep_relative_path() {
      let project = TestProject::new()
        .add_file("index.ts", "import './deep/folder/module';")
        .add_file(
          "deep/folder/module.ts",
          "import { foo } from '../../utils';",
        )
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(
        &analyzer,
        &entries,
        &["index.ts", "deep/folder/module.ts", "utils.ts"],
      );
      assert_unused(&analyzer, vec![("utils.ts", "bar")]);
    }

    #[test]
    fn import_with_js_extension() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './utils.js';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "bar")]);
    }

    #[test]
    fn index_file_resolution() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './utils/index';")
        .add_file(
          "utils/index.ts",
          "export const foo = 1;\nexport const bar = 2;",
        )
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils/index.ts"]);
      assert_unused(&analyzer, vec![("utils/index.ts", "bar")]);
    }
  }

  // ===== Side Effects =====
  mod side_effects {
    use super::*;

    #[test]
    fn side_effect_import_makes_file_reachable() {
      let project = TestProject::new()
        .add_file("index.ts", "import './setup';")
        .add_file(
          "setup.ts",
          "console.log('setup');\nexport const config = {};",
        )
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "setup.ts"]);
      assert_unused(&analyzer, vec![("setup.ts", "config")]);
    }
  }

  // ===== Multi-entry =====
  mod multi_entry {
    use super::*;

    #[test]
    fn multiple_entrypoints() {
      let project = TestProject::new()
        .add_file("entry1.ts", "import { foo } from './utils';")
        .add_file("entry2.ts", "import { bar } from './utils';")
        .add_file(
          "utils.ts",
          "export const foo = 1;\nexport const bar = 2;\nexport const baz = 3;",
        )
        .entry("entry1.ts")
        .entry("entry2.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["entry1.ts", "entry2.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "baz")]);
    }
  }

  // ===== Import Aliases =====
  mod aliases {
    use super::*;

    #[test]
    fn import_with_alias() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo as myFoo } from './utils';")
        .add_file("utils.ts", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.ts"]);
      assert_unused(&analyzer, vec![("utils.ts", "bar")]);
    }
  }

  // ===== Mixed Extensions =====
  mod mixed_extensions {
    use super::*;

    #[test]
    fn js_and_ts_files() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './utils.js';")
        .add_file("utils.js", "export const foo = 1;\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "utils.js"]);
      assert_unused(&analyzer, vec![("utils.js", "bar")]);
    }

    #[test]
    fn jsx_file() {
      let project = TestProject::new()
        .add_file("index.tsx", "import { Component } from './component.jsx';")
        .add_file(
          "component.jsx",
          "export const Component = () => {};\nexport const Other = () => {};",
        )
        .entry("index.tsx");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.tsx", "component.jsx"]);
      assert_unused(&analyzer, vec![("component.jsx", "Other")]);
    }
  }

  // ===== Circular Dependencies =====
  mod circular {
    use super::*;

    #[test]
    fn circular_imports() {
      let project = TestProject::new()
        .add_file("index.ts", "import { foo } from './a';")
        .add_file("a.ts", "import { bar } from './b';\nexport const foo = 1;")
        .add_file("b.ts", "import { foo } from './a';\nexport const bar = 2;")
        .entry("index.ts");

      let (analyzer, entries) = project.build();

      assert_reachable(&analyzer, &entries, &["index.ts", "a.ts", "b.ts"]);
      assert_unused(&analyzer, vec![]);
    }
  }
}
