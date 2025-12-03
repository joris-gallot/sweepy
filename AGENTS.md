# Sweepy Agents

Sweepy is a Rust-powered analyzer that surfaces unused exports in JavaScript and TypeScript projects, distributed as a napi-built native module.

## Architecture
- `src/`: Cargo crate containing the analyzer logic and napi entry point used to build the binary.
- `__test__/`: JavaScript test suite that validates the compiled binary’s behaviour.

## Core analysis flow
- `collect_source_files`: Walks the project tree, loading every `.ts`, `.tsx`, `.js`, and `.jsx` module into memory for analysis.
- `ProjectAnalyzer::from_sources`: Parses modules, records import/export relationships, and prepares the dependency graph.
- `ProjectAnalyzer::compute_reachable`: Traces the dependency graph from the provided entry points to determine which files are reachable.
- `ProjectAnalyzer::find_unused_exports`: Compares declared exports against the import usage map to surface unused symbols.
