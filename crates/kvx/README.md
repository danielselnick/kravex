# Summary

Core library for kravex — the data migration engine.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: None (yet)
- **Edition**: 2024

# Key Concepts

- Adaptive throttling (429 backoff/ramp)
- Zero-config optimization
- Cutover management (pause/resume/retry/validation)

# Notes for future reference

- POC/MVP stage — API surface is unstable and expected to change
- No external dependencies introduced yet; keep the dependency footprint minimal
- Public config now groups execution knobs under `[runtime]` instead of exposing internal supervisor/worker terminology

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`, no public API defined yet
