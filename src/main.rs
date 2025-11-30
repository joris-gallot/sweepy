mod analyzer;

use analyzer::ProjectAnalyzer;
use anyhow::Result;
use clap::Parser;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "sweepy")]
struct Args {
  #[arg(short, long, default_value = ".")]
  root: PathBuf,

  #[arg(short, long)]
  entry: Vec<PathBuf>,
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

fn main() -> Result<()> {
  let args = Args::parse();
  let sources = collect_source_files(&args.root)?;
  let sources_ref: HashMap<PathBuf, &str> = sources
    .iter()
    .map(|(p, c)| (p.clone(), c.as_str()))
    .collect();

  let analyzer = ProjectAnalyzer::from_sources(&sources_ref)?;

  let entrypoints = if !args.entry.is_empty() {
    args.entry.clone()
  } else {
    println!("No entrypoints provided, trying defaults...");
    let file_set: HashSet<PathBuf> = analyzer.files.keys().cloned().collect();
    find_default_entrypoints(&args.root, &file_set)
  };

  let reachable = analyzer.compute_reachable(&entrypoints);
  println!("Reachable files:");
  for f in &reachable {
    println!("  - {}", f.display());
  }

  let unused_exports = analyzer.find_unused_exports();
  println!("\nUnused exports:");
  for (module, export) in unused_exports {
    println!("  - {} -> {}", module.display(), export);
  }

  Ok(())
}

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
