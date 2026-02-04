# Kravex — AI Agent Instructions

> CLAUDE.md symlinks here. This is the canonical file.

## Project Overview

**Kravex**: Zero-config search migration engine. Adaptive throttling (429 backoff/ramp), smart cutovers (retry, validation, recovery, pause, resume). No tuning, no babysitting.

- **Status**: POC/MVP — API surface unstable
- **Language**: Rust, edition 2024
- **Workspace resolver**: 3

## Repository Structure

```
kravex/
├── Cargo.toml              # Workspace root (members: crates/kvx, crates/kvx-cli)
├── AGENTS.md               # THIS FILE — canonical AI instructions
├── CLAUDE.md -> AGENTS.md  # Symlink
├── README.md               # Root docs
├── LICENSE / LICENSE-EE / LICENSE-MIT
├── .vscode/                # VS Code launch + tasks configs
│   ├── launch.json         # LLDB debug (F5) / run (Ctrl+F5) for kvx-cli
│   └── tasks.json          # cargo build/check/test/clippy workspace tasks
└── crates/
    ├── kvx/                # Core library
    │   ├── Cargo.toml
    │   ├── README.md
    │   └── src/lib.rs      # Empty — awaiting core implementation
    └── kvx-cli/            # CLI binary
        ├── Cargo.toml
        ├── README.md
        └── src/main.rs     # Placeholder (Hello, world!)
```

## Crate Dependency Graph

```
kvx-cli v0.1.0
  └── kvx v0.1.0 (path = "../kvx")
        └── (no external deps)
```

## Build & Dev Commands

| Command | Purpose |
|---|---|
| `cargo build --workspace` | Build all (Ctrl+Shift+B in VS Code) |
| `cargo check --workspace` | Type-check all |
| `cargo test --workspace` | Run all tests |
| `cargo clippy --workspace` | Lint all |
| `cargo build -p kvx-cli` | Build CLI only (used by launch configs) |

VS Code: F5 = debug kvx-cli (LLDB), Ctrl+F5 = run without debug. Requires CodeLLDB extension.

## README.md Usage

This list is comprehensive and kept up to date (you must update if needed) list of all README.md within this solution:
- README.md
- crates/kvx/README.md
- crates/kvx-cli/README.md

**Rules**:
- If a `Cargo.toml` exists, a `README.md` MUST exist in the same directory
- You MUST proactively read, create, update, and delete README.md files and their contents
- Contents MUST be concise, terse, compacted; emphasis on preserving a knowledge graph
- Shared format:

```
# Summary
# Description
# Knowledge Graph
# Key Concepts
# Notes for future reference
# Aggregated Context Memory Across Sessions for Current and Future Use
```

## Context Saving

**Forbidden files** (never load):
- LICENSE
- LICENSE-EE
- LICENSE-MIT

**Avoid loading** (unless absolutely necessary):
- *.gitignore
- *.Cargo.lock
- /target/*
- /.vscode/*

## Conventions

- Keep external dependency footprint minimal
- Prefer zero-config / convention-over-configuration patterns
- Core logic in `kvx` crate; `kvx-cli` is a thin wrapper
- No CI/CD configured yet — all builds via Cargo + VS Code tasks
- No CLI arg parser chosen yet for `kvx-cli`

## Architecture Notes

- **kvx** (lib): Will contain adaptive throttling, cutover management, retry/recovery, validation, pause/resume primitives
- **kvx-cli** (bin): Terminal-facing layer exposing kvx capabilities, progress reporting

## Aggregated Context Memory Across Sessions

- Initial scaffold: workspace with two crates, placeholder implementations
- `.vscode/` configured with build tasks and LLDB launch configs for `kvx-cli`
- Edition 2024, resolver 3 — latest Rust standards
- Triple-license structure (standard + enterprise edition)
- CLAUDE.md is a symlink to AGENTS.md — always edit AGENTS.md
