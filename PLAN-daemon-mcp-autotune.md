# Kravex Daemon — REST API + MCP for LLM-Driven Autotuning

## Context

Kravex is currently a CLI batch job: load TOML config → run pipeline → exit. The goal is to make it a **daemon** with a REST API so an LLM (via MCP) can iteratively run dry-run migrations, measure throughput, adjust config parameters, and converge on optimal settings. **Autotune via LLM-in-the-loop.**

The problem: every deployment has different hardware, network topology, cluster sizing, and data shapes. There is no universal "best config." Today, tuning requires a human running migrations, eyeballing metrics, tweaking TOML, and repeating. This is exactly the kind of tedious optimization loop an LLM excels at — fast iteration, pattern recognition across runs, no fatigue at 3am.

## Architecture Overview

```
Claude Code ←stdio→ MCP Server ←in-process→ REST API (axum) → kvx::run()
                                                ↕
                                         DaemonState (Arc)
                                         ├─ PipelineHandle
                                         │  ├─ JoinHandle
                                         │  ├─ CancellationToken
                                         │  ├─ FlowKnob (Arc<AtomicUsize>)
                                         │  └─ PipelineMetrics (Arc, atomics)
                                         ├─ Current AppConfig
                                         └─ RunHistory (last N results)
```

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| New crate vs subcommand | **New `kvx-daemon` crate** | Clean separation: kvx-cli = batch, kvx-daemon = server. No axum/MCP bloat in CLI. |
| HTTP framework | **axum 0.8** | Tokio-native (no runtime conflict), tower/hyper ecosystem, lightweight |
| MCP SDK | **rmcp (Rust MCP SDK)** | Same-process, shared state, no HTTP hop for MCP→daemon calls |
| Pipeline concurrency | **Single pipeline** | Autotuning is sequential: run → measure → adjust → repeat |
| Cancellation | **tokio_util::CancellationToken** | Pumper checks token → stops → RAII cascade kills all workers |
| Metrics | **New `Arc<PipelineMetrics>` with AtomicU64** | Per-run, resets between runs, lock-free reads |

---

## Phase 1: Core Plumbing (kvx crate changes)

Minimal, surgical changes to the kvx library to support daemon control. **No behavior change for kvx-cli.**

### 1a. Add `Serialize` to all config types

Every config struct gets `#[derive(Serialize)]` added alongside existing `Deserialize`. This enables the daemon to serialize configs as JSON responses.

**Files (one-line change each — add `Serialize` to derive macro):**

| File | Structs |
|------|---------|
| `crates/kvx/src/config.rs` | `AppConfig`, `RuntimeConfig` |
| `crates/kvx/src/backends/config.rs` | `SourceConfig`, `SinkConfig`, `CommonSourceConfig`, `CommonSinkConfig` |
| `crates/kvx/src/backends/elasticsearch/config.rs` | `ElasticsearchSourceConfig`, `ElasticsearchSinkConfig` |
| `crates/kvx/src/backends/file/config.rs` | `FileSourceConfig`, `FileSinkConfig` |
| `crates/kvx/src/regulators/config.rs` | `FlowMasterConfig`, `StaticRegulatorConfig`, `LatencyRegulatorConfig`, `CpuRegulatorConfig` |
| `crates/kvx/src/workers/drainer.rs` | `DrainerConfig` |

Also add `use serde::Serialize;` to any file that doesn't already import it.

### 1b. PipelineMetrics struct

New struct for daemon-queryable metrics. Lives in `crates/kvx/src/lib.rs` (or a new `metrics.rs` if it gets large).

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct PipelineMetrics {
    pub docs_drained: AtomicU64,
    pub bytes_drained: AtomicU64,
    pub pages_pumped: AtomicU64,
    pub drain_latency_sum_nanos: AtomicU64,
    pub drain_count: AtomicU64,
    pub errors: AtomicU64,
}

impl PipelineMetrics {
    pub fn new() -> Self { /* all zeros */ }
    pub fn avg_drain_latency_ms(&self) -> f64 { /* sum_nanos / count / 1_000_000 */ }
    pub fn snapshot(&self) -> MetricsSnapshot { /* read all atomics once */ }
}

pub struct MetricsSnapshot {
    pub docs_drained: u64,
    pub bytes_drained: u64,
    pub pages_pumped: u64,
    pub avg_drain_latency_ms: f64,
    pub errors: u64,
}
```

**Instrumentation points:**
- **Pumper**: `metrics.pages_pumped.fetch_add(1, Relaxed)` after each successful pump
- **Drainer**: `metrics.docs_drained.fetch_add(doc_count, Relaxed)` + `bytes_drained` + `drain_latency_sum_nanos` + `drain_count` after each successful drain
- **Drainer**: `metrics.errors.fetch_add(1, Relaxed)` on drain failure

### 1c. CancellationToken for graceful shutdown

Add `tokio-util` as a workspace dependency.

**Pumper changes:**
```rust
pub struct Pumper {
    tx: Sender<Page>,
    source: SourceBackend,
    cancel_token: Option<CancellationToken>,  // NEW
    metrics: Option<Arc<PipelineMetrics>>,     // NEW
}
```

In Pumper's pump loop, use `tokio::select!` to race cancellation against source calls:
```rust
loop {
    let page = tokio::select! {
        _ = async {
            match &self.cancel_token {
                Some(token) => token.cancelled().await,
                None => std::future::pending().await,
            }
        } => {
            info!("🛑 Pumper: cancellation received. Ceasing operations.");
            break;
        }
        result = self.source.pump() => result?,
    };
    // ... send page on channel
}
```

This triggers the existing RAII cascade: tx1 drops → ch1 closes → joiners exit → ch2 closes → drainers exit → ch3 closes → FlowMaster exits. **Zero changes to Foreman, Joiner, Drainer, FlowMaster.**

### 1d. PipelineHandle return type

New `run()` signature and return type:

```rust
pub struct PipelineHandle {
    pub join_handle: JoinHandle<Result<()>>,
    pub flow_knob: FlowKnob,
    pub metrics: Arc<PipelineMetrics>,
    pub cancel_token: CancellationToken,
}

// New entry point for daemon use (returns handle, doesn't await)
pub async fn start(app_config: AppConfig) -> Result<PipelineHandle> {
    // ... same setup as run() ...
    // spawns foreman.start_workers() as a task
    // returns PipelineHandle immediately
}

// Existing run() preserved for CLI (calls start() + awaits)
pub async fn run(app_config: AppConfig) -> Result<()> {
    let handle = start(app_config).await?;
    handle.join_handle.await??;
    Ok(())
}
```

### 1e. Thread metrics + cancel_token through Foreman

`Foreman::start_workers()` gains two new parameters:
```rust
pub async fn start_workers(
    &self,
    source_backend: SourceBackend,
    sink_backends: Vec<SinkBackend>,
    caster: PageToEntriesCaster,
    manifold: ManifoldBackend,
    the_flow_knob: FlowKnob,
    the_flow_master_config: &FlowMasterConfig,
    the_sink_max_request_size_bytes: usize,
    cancel_token: Option<CancellationToken>,       // NEW
    metrics: Option<Arc<PipelineMetrics>>,          // NEW
) -> Result<()>
```

Passes `cancel_token` to Pumper, `metrics` to Pumper + each Drainer.

**Files modified in Phase 1:**
- `Cargo.toml` (workspace) — add `tokio-util`
- `crates/kvx/Cargo.toml` — add `tokio-util`
- `crates/kvx/src/lib.rs` — PipelineMetrics, PipelineHandle, start(), updated run()
- `crates/kvx/src/config.rs` — Serialize derive
- `crates/kvx/src/foreman.rs` — thread cancel_token + metrics
- `crates/kvx/src/workers/drainer.rs` — DrainerConfig Serialize, metrics instrumentation
- `crates/kvx/src/workers/pumper.rs` (or equivalent) — cancel_token + metrics
- `crates/kvx/src/backends/config.rs` — Serialize derives
- `crates/kvx/src/backends/elasticsearch/config.rs` — Serialize derives
- `crates/kvx/src/backends/file/config.rs` — Serialize derives
- `crates/kvx/src/regulators/config.rs` — Serialize derives

**Verification:** `cargo test -p kvx` — all 107 existing tests pass unchanged.

---

## Phase 2: Daemon HTTP Server (new `kvx-daemon` crate)

### Crate structure
```
crates/kvx-daemon/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs           # CLI: clap args, launch server
    ├── server.rs          # axum router setup
    ├── state.rs           # DaemonState, PipelineState enum
    └── handlers/
        ├── mod.rs         # re-exports
        ├── pipeline.rs    # start, stop, status, metrics, config, dry-run
        └── health.rs      # GET /health
```

### Cargo.toml
```toml
[package]
name = "kvx-daemon"
version = "0.1.0"
edition = "2024"

[dependencies]
kvx = { path = "../kvx" }
tokio = { workspace = true }
tokio-util = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
axum = "0.8"
uuid = { version = "1", features = ["v4"] }
clap = { version = "4", features = ["derive"] }
```

### DaemonState

```rust
pub enum PipelineState {
    Idle,
    Starting,
    Running {
        run_id: Uuid,
        config: AppConfig,
        handle: PipelineHandle,
        started_at: Instant,
    },
    Stopping { run_id: Uuid },
    Completed {
        run_id: Uuid,
        metrics: MetricsSnapshot,
        elapsed: Duration,
    },
    Failed {
        run_id: Uuid,
        error: String,
        metrics: Option<MetricsSnapshot>,
    },
}

pub struct DaemonState {
    pub pipeline: Mutex<PipelineState>,
    pub run_history: Mutex<Vec<RunResult>>,
}

pub struct RunResult {
    pub run_id: Uuid,
    pub config: AppConfig,
    pub metrics: MetricsSnapshot,
    pub elapsed: Duration,
    pub error: Option<String>,
}
```

### REST API Endpoints

#### `POST /pipeline/start`
```
Request:  AppConfig as JSON
Response: { "run_id": "uuid", "status": "started" }
Error:    409 if pipeline already running
```
- Locks state, checks Idle/Completed/Failed
- Calls `kvx::start(config)` → gets PipelineHandle
- Sets state to Running
- Spawns background task that awaits join_handle, transitions to Completed/Failed

#### `POST /pipeline/stop`
```
Response: { "status": "stopping" }
Error:    409 if not running
```
- Locks state, calls `cancel_token.cancel()`
- Sets state to Stopping
- Background task (from start) handles transition to Completed

#### `GET /pipeline/status`
```json
{
  "state": "Running",
  "run_id": "uuid",
  "elapsed_secs": 42.5,
  "metrics": {
    "docs_drained": 1500000,
    "bytes_drained": 3221225472,
    "pages_pumped": 150,
    "docs_per_sec": 35294.0,
    "bytes_per_sec": 75788800.0,
    "avg_drain_latency_ms": 185.0,
    "errors": 0
  },
  "current_flow_knob_bytes": 4194304
}
```

#### `GET /pipeline/metrics`
Same as status but metrics-only, for lightweight polling.

#### `PUT /pipeline/config/flow`
```
Request:  { "output_bytes": 2097152 }
Response: { "previous_bytes": 4194304, "new_bytes": 2097152 }
Error:    409 if not running
```
- Writes directly to FlowKnob `Arc<AtomicUsize>`

#### `GET /pipeline/config`
```
Response: Current AppConfig as JSON (requires Serialize)
Error:    409 if idle with no previous run
```

#### `POST /pipeline/dry-run`
```
Request:  { "config": { ... }, "duration_secs": 30 }
Response: { "run_id": "uuid", "status": "started", "auto_stop_after_secs": 30 }
```
- Same as start, but spawns a timer task that cancels after N seconds
- On completion, result saved to `run_history`

#### `GET /pipeline/history`
```json
[
  {
    "run_id": "uuid",
    "config": { ... },
    "metrics": { ... },
    "elapsed_secs": 30.0,
    "error": null
  }
]
```

#### `GET /health`
```json
{ "status": "ok", "uptime_secs": 3600.0 }
```

### Server startup

```rust
// main.rs
#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let state = Arc::new(DaemonState::new());
    let app = server::create_router(state);
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    axum::serve(listener, app).await?;
}
```

Default bind: `0.0.0.0:8420`

**Verification:** `cargo run -p kvx-daemon -- serve` + curl all endpoints.

---

## Phase 3: Dry Run + Auto-Stop (follow-up session)

- `POST /pipeline/dry-run` spawns pipeline + timer that cancels after N seconds
- On completion, `MetricsSnapshot` + `AppConfig` saved to `run_history`
- `GET /pipeline/history` returns all past runs for LLM comparison

---

## Phase 4: MCP Server (follow-up session)

### In-process MCP, shared state

MCP server runs in the same `kvx-daemon` process using `rmcp` Rust SDK. Shares `Arc<DaemonState>` directly — no REST calls, no HTTP hop.

### MCP Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `kravex_start_pipeline` | Start migration | `config: AppConfig` |
| `kravex_stop_pipeline` | Stop running pipeline | — |
| `kravex_get_status` | State + metrics | — |
| `kravex_get_metrics` | Detailed metrics | — |
| `kravex_set_flow` | Adjust flow knob | `output_bytes: u64` |
| `kravex_get_config` | Current config | — |
| `kravex_dry_run` | Time-limited run | `config, duration_secs` |
| `kravex_get_history` | Past run results | — |

### Daemon CLI modes
```bash
kvx-daemon serve              # HTTP server only (port 8420)
kvx-daemon serve --mcp        # HTTP + MCP stdio
kvx-daemon mcp                # MCP stdio only (for Claude Code)
```

### Claude Code integration
```bash
claude mcp add kvx -- cargo run -p kvx-daemon -- mcp
```

### File structure addition
```
crates/kvx-daemon/src/
    mcp/
        mod.rs             # MCP server setup, stdio transport
        tools.rs           # Tool definitions, handlers
```

---

## Phase 5: Autotuning Polish (follow-up session)

### LLM Autotuning Loop

```
1. LLM calls kravex_dry_run(config_v1, 30s)
2. LLM polls kravex_get_metrics() every 5s during run
3. After 30s, auto-stop → metrics saved to history
4. LLM analyzes: docs/s, bytes/s, latency, errors
5. LLM adjusts config (parallelism, flow_master mode, batch sizes)
6. LLM calls kravex_dry_run(config_v2, 30s)
7. Repeat until convergence (docs/s stops improving)
8. LLM calls kravex_start_pipeline(best_config) for real run
```

### Tunable Search Space

| Parameter | Range | Effect |
|-----------|-------|--------|
| `runtime.sink_parallelism` | 1–64 | More parallel drain workers |
| `runtime.joiner_parallelism` | 1–(cpu_count-1) | More CPU threads for cast+join |
| `flow_master` mode | Static / Latency | Fixed vs adaptive payload sizing |
| `flow_master.Static.output_bytes` | 1MiB–64MiB | Fixed payload size |
| `flow_master.Latency.set_point_latency_ms` | 100–500 | PID target latency |
| `sink.max_request_size_bytes` | 1MiB–64MiB | Hard ceiling per request |
| `source.max_batch_size_docs` | 1K–50K | Docs per source page |

### Decision Heuristics (for LLM system prompt)

- `docs/s` low + latency low → **source-bound**: increase batch sizes
- `docs/s` low + latency high → **sink-bound**: reduce parallelism, try Latency mode
- `errors > 0` (429s) → **too aggressive**: lower parallelism, raise latency setpoint
- `docs/s` plateaus across iterations → **found ceiling**, lock in config

### Compound autotuning tool

`kravex_autotune_iteration`: start dry run → poll metrics at intervals → stop → return full result. Reduces LLM round-trips.

---

## What Changes Mid-Run vs Requires Restart

| Parameter | Mid-Run? | Mechanism |
|-----------|----------|-----------|
| FlowKnob (output bytes) | ✅ Yes | `Arc<AtomicUsize>` swap via `PUT /pipeline/config/flow` |
| PID setpoint / gains | ❌ No | Pipeline restart required |
| Parallelism (sink, joiner) | ❌ No | Baked at channel/thread creation |
| Channel capacities | ❌ No | Baked at channel creation |
| Source / Sink connection | ❌ No | Pipeline restart required |
| Drainer retry config | ❌ No | Pipeline restart required |

---

## Implementation Order (This Session)

### Step 1: Phase 1a — Serialize derives
Add `Serialize` to all config structs. Verify with `cargo check`.

### Step 2: Phase 1b — PipelineMetrics
Create struct, wire into Drainer + Pumper.

### Step 3: Phase 1c — CancellationToken
Add tokio-util dep, thread token through Pumper.

### Step 4: Phase 1d — PipelineHandle + start()
New `start()` function, update `run()` to use it.

### Step 5: Phase 1e — Update Foreman signature
Thread metrics + cancel_token through `start_workers()`.

### Step 6: Verify — `cargo test -p kvx`
All 107 tests must pass.

### Step 7: Phase 2 — kvx-daemon crate
Scaffold crate, implement DaemonState + axum handlers.

### Step 8: Verify — curl against running daemon
Manual testing of all endpoints.

---

## Potential Challenges

| Challenge | Mitigation |
|-----------|------------|
| **Cancellation latency** (source blocked on long HTTP call) | `tokio::select!` in Pumper races cancellation vs source call |
| **Joiner threads vs CancellationToken** | Not needed — joiners exit via RAII cascade when ch1 closes |
| **Concurrent start requests** | `Mutex<PipelineState>` with state validation inside lock |
| **InMemory Serialize edge case** | `SourceConfig::InMemory(())` — `()` serializes as `null`, acceptable |
| **rmcp SDK maturity** | Fallback: manual JSON-RPC 2.0 over stdio (MCP is just JSON-RPC) |
