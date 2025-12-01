use napi_derive::napi;
mod analyzer;

use analyzer::ProjectAnalyzer;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[napi(object)]
pub struct UnusedExport {
  pub file: String,
  pub name: String,
}

#[napi(object)]
pub struct SweepyResult {
  pub reachable_files: Vec<String>,
  pub unused_exports: Vec<UnusedExport>,
}

/// Collect all ts/tsx/js/jsx files under root
fn collect_source_files(root: &Path) -> Result<HashMap<PathBuf, String>> {
  let mut files = HashMap::new();
  for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
    let p = entry.path();
    if let Some(ext) = p.extension().and_then(|s| s.to_str())
      && matches!(ext, "ts" | "tsx" | "js" | "jsx")
    {
      let content = fs::read_to_string(p)?;
      files.insert(p.to_path_buf(), content);
    }
  }
  Ok(files)
}

#[napi]
pub fn sweepy(_root: String, entries: Vec<String>) -> SweepyResult {
  let root = PathBuf::from(_root);

  let sources = collect_source_files(&root).expect("Failed to collect source files");
  let sources_ref: HashMap<PathBuf, &str> = sources
    .iter()
    .map(|(p, c)| {
      let relative_path = p.strip_prefix(&root).unwrap_or(p).to_path_buf();
      (relative_path, c.as_str())
    })
    .collect();

  let analyzer = ProjectAnalyzer::from_sources(&sources_ref).expect("Failed to analyze project");
  let entrypoints: Vec<PathBuf> = entries
    .iter()
    .map(|e| {
      let p = PathBuf::from(e);
      p.strip_prefix(&root).unwrap_or(&p).to_path_buf()
    })
    .collect();

  let reachable = analyzer.compute_reachable(entrypoints);
  let unused_exports_raw = analyzer.find_unused_exports();

  let mut reachable_files: Vec<String> = reachable
    .into_iter()
    .map(|p| p.to_string_lossy().to_string())
    .collect();

  reachable_files.sort();

  let mut unused_exports: Vec<UnusedExport> = unused_exports_raw
    .into_iter()
    .map(|(path, name)| UnusedExport {
      file: path.to_string_lossy().to_string(),
      name,
    })
    .collect();

  // sort unused exports by file and then by name
  unused_exports.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.name.cmp(&b.name)));

  SweepyResult {
    reachable_files,
    unused_exports,
  }
}
