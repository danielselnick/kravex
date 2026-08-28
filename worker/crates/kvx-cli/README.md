# Summary

CLI interface for kravex — run migrations from the command line.

# Description

`kvx-cli` wraps the `kvx-lib` core library and exposes it as a terminal tool. Intended as the primary user-facing entry point for running, monitoring, and managing search migrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx-cli`
- **Dependencies**: `kvx-lib` (path = `../kvx-lib`, imported as `kvx_lib`)
- **Edition**: 2024
- **Binary crate**

# Key Concepts

- Thin CLI layer over `kvx-lib` core
- Will surface throttle/cutover/progress to the terminal

# Notes

- VS Code launch configs (`F5` / `Ctrl+F5`) target this binary via CodeLLDB
