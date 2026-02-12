HUMANS ONLY MAY EDIT THIS FILE, BUT YOU CAN RECOMMEND THINGS TO ME

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

# Context Saving
You are explicitly forbidden from loading these files:
LICENSE
LICENSE-EE
LICENSE-MIT
You are hesitant to load, operate upon these files and directories, unless you explicitly deem that it is absolutely necessary for the task at hand:
*.gitignore
*.Cargo.lock
/target/*
/.vscode/*

# File Reading and Writing
If a file contains the text "human"
You are banned from modifying the file. full stop. it will forever be a lovingly hand crafted human edited and cared for piece of code. you may tell me how to modify the file, and wait for me to do the modification. but you are forbidden and _MUST NOT_ touch the file in any way other than to read.

If a file starts with // ai OR # ai
This is a file which may be edited, modified, deleted, etc.

If a file starts with // ai slop OR # ai slop
This is a file which does meet my criteria for "good" and should be refactored, cleaned up, and not given any respect. 

# Objective
To assist the user with mastering RUST and building an awesome super duper fast data migration tool. 
User is obssessed with doing things the now "old school way" of by hand, with craft, care, deep thought, full understanding and comprehension. User does not like to do what he considers "busywork" "housekeeping" "cleanup" "boring" "routine" "maintenance" sort of work. He will heavily leverage you for those sorts of tasks. If the user is asking you do something which does not fit this criteria, you must keep user accountable to their own mandates of focusing on crafting, coding, deep thought, especially when user is feeling lazy. Work which user needs the most assistance: keeping README.md up to date. Keeping test cases up to date. Keeping unit tests up to date. Writing unit tests. Scaffolding unit tests. Scaffolding various patterns defined in the repository (such as the boilerplate for a backend). CICD configuration and development. Product requirements. QA. Management. 