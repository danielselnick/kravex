# Summary

Search migrations for devs with better things to do. No tuning. No babysitting. Just migrate and move on. Now with 6 phases of refactoring and graceful shutdown because we're professionals.

# Description

Kravex automatically optimizes search migrations — no knobs, no guesswork. Smart throttling adapts to destination limits (429 backoff/ramp). Smart cutovers provide retries, validation, recovery, pause, and resume. The singularity will arrive before we hit v1.0.

# Knowledge Graph

- **Workspace root**: Cargo workspace with members `crates/kvx`, `crates/kvx-cli`
- **`kvx`**: Core library — throttling, cutover logic, retry/recovery, adaptive throughput, graceful shutdown
- **`kvx-cli`**: CLI binary — user-facing entry point wrapping `kvx`
- **Edition**: 2024
- **Test count**: 110 total (101 kvx + 9 kvx-cli)
- **Architecture version**: v17 (post 6-phase refactor)

## Phase Summary (6-phase refactor, complete)
| Phase | Change |
|---|---|
| 1 | `controllers/` → `throttlers/`; `ThrottleConfig` moved from `backends/common_config.rs` to `throttlers.rs` |
| 2 | `sink.send()` → `sink.drain()`; `source.next_page()` → `source.pump(doc_count_hint)`; `set_page_size_hint()` eliminated |
| 3 | `supervisors/workers/` flattened → `workers/` (top-level) |
| 4 | `ThrottleAppConfig` with `[throttle.source]`/`[throttle.sink]`; `source`/`sink` in `AppConfig` |
| 5 | `ThrottleControllerBackend` gains `Clone` + `from_config()`; `app_config.rs` split to module dir; `build_backend()` factory on config enums |
| 6 | `CancellationToken` (tokio-util); workers use `tokio::select!`; `kvx::stop()` public API |

# Key Concepts

- Zero-config optimization
- Adaptive throttle (429 backoff/ramp, PID feedback loop)
- Smart cutovers (retry, validation, recovery, pause, resume)
- Graceful shutdown via `CancellationToken` + `kvx::stop()`
- Clean TOML schema: connection config in `[source.X]`/`[sink.X]`, sizing in `[throttle.source]`/`[throttle.sink]`

## TOML Config Schema (current)
```toml
[runtime]
queue_capacity = 8
sink_parallelism = 8

[source.File]
file_name = "input.json"

[sink.Elasticsearch]
url = "http://localhost:9200"
index = "test-123"

[throttle.source]
max_batch_size_bytes = 131072
max_batch_size_docs = 10000

[throttle.sink]
max_request_size_bytes = 131072
```

# Development

## Prerequisites

- Rust toolchain (edition 2024)
- VS Code with [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) extension

## VS Code Tasks & Launch

`.vscode/tasks.json` provides:
- `cargo build --workspace` (default build — `Ctrl+Shift+B`)
- `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace`
- `cargo build -p kvx-cli` (targeted build for launch configs)

`.vscode/launch.json` provides:
- **Debug kvx-cli** (`F5`) — LLDB debugger attached
- **Run kvx-cli (no debug)** (`Ctrl+F5`)

# Notes for future reference

- POC/MVP stage — API surface is unstable
- Keep dependency footprint minimal
- Throttle config lives in `[throttle.source]`/`[throttle.sink]` — never in backend-specific sections
- Backend sections (`[source.X]`/`[sink.X]`) are pure connection config: URL, credentials, index
- mod.rs files are BANNED — named module files only

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: workspace with two crates, placeholder implementations
- `.vscode/` configured with build tasks and LLDB launch configs for `kvx-cli`
- v17 (current): 6-phase refactor complete. Clean trait APIs (`pump`/`drain`), consolidated throttle config, flattened module hierarchy, graceful cancellation. 110 tests passing.
