# Summary

CLI interface for kravex — run migrations from the command line.

# Description

`kvx-cli` wraps the `kvx` core library and exposes it as a terminal tool. Intended as the primary user-facing entry point for running, monitoring, and managing search migrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx-cli`
- **Dependencies**: `kvx` (path = `../kvx`)
- **Edition**: 2024
- **Binary crate**

# Key Concepts

- Thin CLI layer over `kvx` core
- Will surface throttle/cutover/progress to the terminal

# Notes for future reference

- POC/MVP stage — currently a placeholder `main.rs`
- CLI argument parsing library not yet chosen
- VS Code launch configs (`F5` / `Ctrl+F5`) target this binary via CodeLLDB

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: "Hello, world!" placeholder in `main.rs`
- `.vscode/launch.json` debug/run configurations point to this crate
