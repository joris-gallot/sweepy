use anyhow::{Context, Result};
use clap::Parser;
use oxc_allocator::Allocator;

use oxc_ast::ast::*;
use oxc_parser::Parser as OxcParser;

use oxc_span::SourceType;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "kniprs-lite")]
struct Args {
  /// Project root to scan
  #[arg(short, long, default_value = ".")]
  root: PathBuf,

  /// entrypoints (can pass multiple). If not provided, tries common defaults.
  #[arg(short, long)]
  entry: Vec<PathBuf>,
}

#[derive(Debug)]
struct ParsedFile {
  path: PathBuf,
  imports: Vec<ImportInfo>, // list of imports in this file
  exports: Vec<String>,     // exported symbol names (including "default")
}

#[derive(Debug, Clone)]
struct ImportInfo {
  source: String,          // module specifier as written: "./greet"
  specifiers: Vec<String>, // imported names; empty => likely a side-effect import or `import * as ns`
  has_namespace: bool,     // true if `import * as ns from ...`
  has_default: bool,       // true if `import def from ...`
}

/// try file extensions/indices in this order
const CANDIDATE_EXTS: &[&str] = &[".ts", ".tsx", ".js", ".jsx"];

fn main() -> Result<()> {
  let args = Args::parse();

  let files = collect_source_files(&args.root)?;
  println!("Found {} source files", files.len());

  // parse each file and extract imports/exports
  let mut parsed_map: HashMap<PathBuf, ParsedFile> = HashMap::new();

  for path in &files {
    match parse_and_extract(path) {
      Ok(pf) => {
        parsed_map.insert(pf.path.clone(), pf);
      }
      Err(e) => {
        eprintln!("Failed to parse {}: {:?}", path.display(), e);
      }
    }
  }

  // Build quick lookup of files set for resolution
  let file_set: HashSet<PathBuf> = parsed_map.keys().cloned().collect();

  // Resolve imports to targets (only for relative imports)
  let mut graph: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
  // Also build reverse mapping: module -> importers with specifiers
  let mut import_usage: HashMap<PathBuf, Vec<(PathBuf, ImportInfo)>> = HashMap::new();

  for (path, pf) in &parsed_map {
    for imp in &pf.imports {
      if is_relative(&imp.source) {
        if let Some(target) = resolve_relative_import(path, &imp.source, &file_set) {
          graph
            .entry(path.clone())
            .or_default()
            .insert(target.clone());
          import_usage
            .entry(target)
            .or_default()
            .push((path.clone(), imp.clone()));
        } else {
          // not resolved: maybe missing extension or external; ignore for now
          // println!("Could not resolve {} from {}", imp.source, path.display());
        }
      } else {
        // non-relative (package name) -> treat as external dependency
      }
    }
  }

  let entrypoints = if !args.entry.is_empty() {
    args.entry.clone()
  } else {
    println!("No entrypoints provided, trying defaults...");
    find_default_entrypoints(&args.root, &file_set)
  };

  // compute reachable files by DFS/BFS from entrypoints
  let reachable = compute_reachable(&entrypoints, &graph, &file_set);

  let unused_files: Vec<&PathBuf> = file_set.difference(&reachable).collect();
  println!("\nUnused files (not reachable from entrypoints):");
  for f in &unused_files {
    println!("  - {}", f.display());
  }

  // naive exports usage detection:
  // For each file/module M and for each export name E, check if any importer imports E explicitly OR wildcard or default as appropriate.
  println!("\nChecking exports usage (naïf)...");
  for (module_path, pf) in &parsed_map {
    let importers = import_usage.get(module_path);
    for export in &pf.exports {
      let mut used = false;

      if let Some(importers) = importers {
        for (importer_path, import_info) in importers {
          if import_info.has_namespace {
            // namespace import `import * as ns from './mod'` -> assume may use anything
            used = true;
            break;
          }
          if export == "default" && import_info.has_default {
            used = true;
            break;
          }
          if import_info.specifiers.iter().any(|s| s == export) {
            used = true;
            break;
          }
        }
      }

      if !used {
        println!("Unused export: {} -> {}", module_path.display(), export);
      }
    }
  }

  Ok(())
}

/// Collect all ts/tsx/js/jsx files under root
fn collect_source_files(root: &Path) -> Result<Vec<PathBuf>> {
  let mut files = Vec::new();
  for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
    let p = entry.path();

    if let Some(ext) = p.extension().and_then(OsStr::to_str)
      && matches!(ext, "ts" | "tsx" | "js" | "jsx")
    {
      files.push(p.to_path_buf());
    }
  }
  Ok(files)
}

/// Parse file with oxc and extract imports & exports
fn parse_and_extract(path: &Path) -> Result<ParsedFile> {
  let source = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
  let allocator = Allocator::default();
  let source_type = SourceType::from_path(path).unwrap_or(SourceType::ts());
  let parser = OxcParser::new(&allocator, &source, source_type);
  let parsed = parser.parse();
  if !parsed.errors.is_empty() {
    // we still try to continue, but warn
    eprintln!(
      "Parser errors in {}: {} error(s)",
      path.display(),
      parsed.errors.len()
    );
  }
  let program = parsed.program;

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
                // named.imported might be Some or None (for shorthand)
                // use imported if present, otherwise local
                let name = named.imported.name().to_string();

                // named.local.name.to_string()
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
        // export { a, b as c } from "...";  OR export function f() {} OR export const x = ...
        if let Some(src) = &export.source {
          // re-export from other module: e.g. export { x } from './mod'
          // we capture the specifiers so they will be matched to importers of this module later
          for spec in &export.specifiers {
            let exported = if let Some(id_name) = spec.exported.identifier_name() {
              id_name.to_string()
            } else if let Some(local_name) = spec.local.identifier_name() {
              local_name.to_string()
            } else {
              "<unknown>".to_string()
            };

            exports.push(exported);
          }
        } else {
          // local export
          if let Some(decl) = &export.declaration {
            match decl {
              Declaration::FunctionDeclaration(fd) => {
                if let Some(id) = &fd.id {
                  exports.push(id.name.to_string());
                }
              }

              Declaration::VariableDeclaration(vd) => {
                for declarator in &vd.declarations {
                  let exported = if let Some(id_name) = declarator.id.get_identifier_name() {
                    id_name.to_string()
                  } else {
                    "<unknown>".to_string()
                  };
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

          // named specifiers: export { a as b }
          for spec in &export.specifiers {
            let exported = if let Some(id_name) = spec.exported.identifier_name() {
              id_name.to_string()
            } else if let Some(local_name) = spec.local.identifier_name() {
              local_name.to_string()
            } else {
              "<unknown>".to_string()
            };
            exports.push(exported);
          }
        }
      }

      Statement::ExportDefaultDeclaration(_) => {
        exports.push("default".to_string());
      }

      Statement::ExportAllDeclaration(_) => {
        // export * from '...'
        // we put a wildcard marker
        exports.push("*".to_string());
      }

      _ => {}
    }
  }

  Ok(ParsedFile {
    path: path.to_path_buf(),
    imports,
    exports,
  })
}

/// Check if a module specifier is relative
fn is_relative(spec: &str) -> bool {
  spec.starts_with('.') // ./ or ../
}

/// Try to resolve a relative import string to one of the files in file_set.
/// Returns Some(resolved_path) or None
fn resolve_relative_import(
  from: &Path,
  spec: &str,
  file_set: &HashSet<PathBuf>,
) -> Option<PathBuf> {
  // base dir of `from`
  let base = from.parent().unwrap_or(Path::new("."));
  let candidate = base.join(spec);

  // Try exact file with candidate + ext
  for ext in CANDIDATE_EXTS {
    let p = candidate.with_extension(ext.trim_start_matches('.'));
    if file_set.contains(&p) {
      return Some(p);
    }
  }

  // If candidate already has extension and matches
  if file_set.contains(&candidate) {
    return Some(candidate);
  }

  // Try candidate + /index.ext
  for ext in CANDIDATE_EXTS {
    let p = candidate.join(format!("index{}", ext));
    if file_set.contains(&p) {
      return Some(p);
    }
  }

  // Try adding .ts/.tsx etc to the spec string explicitly (if spec includes a dot)
  for ext in CANDIDATE_EXTS {
    let p = PathBuf::from(format!("{}{}", candidate.display(), ext));
    if file_set.contains(&p) {
      return Some(p);
    }
  }

  None
}

/// Find reasonable default entrypoints inside project root using file_set
fn find_default_entrypoints(root: &Path, file_set: &HashSet<PathBuf>) -> Vec<PathBuf> {
  let candidates = vec![
    root.join("src").join("index.ts"),
    root.join("src").join("index.tsx"),
    root.join("index.ts"),
    root.join("index.tsx"),
    root.join("src").join("main.ts"),
    root.join("src").join("main.tsx"),
  ];

  candidates
    .into_iter()
    .filter(|p| file_set.contains(p))
    .collect()
}

/// compute reachable set of files from the given entrypoints using the directed graph
fn compute_reachable(
  entrypoints: &[PathBuf],
  graph: &HashMap<PathBuf, HashSet<PathBuf>>,
  file_set: &HashSet<PathBuf>,
) -> HashSet<PathBuf> {
  let mut visited = HashSet::new();
  let mut stack = Vec::new();

  for ep in entrypoints {
    if file_set.contains(ep) {
      visited.insert(ep.clone());
      stack.push(ep.clone());
    }
  }

  while let Some(node) = stack.pop() {
    if let Some(neighbors) = graph.get(&node) {
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
