# Kravex

Search migrations for teams with better things to do.

Kravex is an open-source migration engine for Elasticsearch (5–8) and OpenSearch (1–3). Any version, any direction. Managed services, self-hosted, on-prem — if it speaks the ES bulk API, Kravex can move it. OpenSearch clusters work through the Elasticsearch backend since both share the same wire protocol.

## What it does

- Migrates index data between Elasticsearch and OpenSearch clusters (same backend handles both)
- Adaptive throttling — automatically backs off on 429s and ramps back up
- Smart cutovers — retry, validation, recovery, pause, and resume
- Zero tuning required — no knobs, no guesswork

## Demo

Run the full end-to-end pipeline (File → Elasticsearch → OpenSearch) with a single command:

```bash
./demo/demo.sh
```

Requires Docker Desktop and [uv](https://docs.astral.sh/uv/). Spins up isolated ES + OpenSearch containers, ingests the geonames dataset (11.4M docs), migrates it across clusters, and reports timing metrics. See [demo/README.md](demo/README.md) for details.

## Quickstart

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (edition 2024)
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose

### 1. Start an Elasticsearch cluster

```bash
docker compose --profile bench up -d es8
```

Wait for it to be healthy:

```bash
docker compose ps
```

### 2. Seed some test data

Load documents into Elasticsearch:

```bash
curl -s -H "Content-Type: application/json" \
  -XPOST "http://localhost:9200/employees/_bulk?pretty" --data-binary @input.json
```

Verify:

```bash
curl -s "http://localhost:9200/employees/_count" | jq .count
```

### 3. Configure the migration

Create a config file (or use an existing one from `configs/`):

```toml
# kvx.toml — File to Elasticsearch

[source_config.File]
file_name = "input.json"

[sink_config.Elasticsearch]
url = "http://localhost:9200"
index = "employees"
```

For an **Elasticsearch-to-Elasticsearch** migration (or ES-to-OpenSearch — same config, just change the URL):

```toml
# kvx_es_to_es.toml

[source_config.Elasticsearch]
url = "http://localhost:9200"
index = "employees"

[sink_config.Elasticsearch]
url = "http://localhost:9201"
index = "employees"
```

### 4. Build and run

```bash
cargo build --workspace
cargo run -p kvx-cli -- kvx.toml
```

### 5. Verify the migration

```bash
curl -s "http://localhost:9200/employees/_count" | jq .count
```

## Architecture

Kravex uses a plumbing metaphor throughout. The entire pipeline is modeled as water flowing through pipes — sources are faucets, sinks are drains, and everything in between controls the flow.

```
Pumper (async) → ch1 → Joiner pool (std::thread) → ch2 → Drainer pool (async) → Sink
                            ↑                                    |
                        FlowKnob ← Governor ← Regulator ← ch3
```

### Terminology

| Component | Plumbing analogy | What it does |
|-----------|-----------------|--------------|
| **Source** | Faucet | Produces raw data one page at a time. Maximally ignorant of format — just emits bytes. |
| **Pumper** | The handle you turn | Async tokio worker. Calls `source.pump()` in a loop, feeds raw pages into ch1 until EOF. |
| **Caster** | Pipe fitting | Stateless transformer. Takes a raw page and casts it into the format the sink expects (e.g., PIT response → bulk NDJSON). |
| **Manifold** | Collector pipe | Orchestrates cast-and-join. Buffers individual drafts from the Caster and assembles them into wire-format payloads sized to the current flow rate. |
| **Joiner** | The junction | CPU-bound `std::thread` worker. Sits between Pumper and Drainer. Receives raw pages from ch1, casts via Caster, buffers via Manifold, flushes assembled payloads to ch2. |
| **Drainer** | The drain | Async tokio worker. Receives assembled payloads from ch2 and writes them to the Sink with retry logic and exponential backoff. |
| **Sink** | Drain pipe | Pure I/O, zero logic. Accepts a fully rendered payload and sends it. Does not buffer, does not transform. |
| **Foreman** | The plumber | Pipeline orchestrator. Wires up all channels, spawns all workers, and waits for completion. |
| **Governor** | The foreman's clipboard | Receives gauge readings on ch3, feeds them to the active Regulator, writes the result to the FlowKnob. Orchestrates the feedback loop. |
| **Regulator** | Pressure valve | Dynamically adjusts payload sizing based on feedback. Variants: `ThroughputSeeker` (hill-climbing optimizer), `CpuPressure` (PID controller), `Static` (fixed value). |
| **PressureGauge** | Pressure meter | Background tokio task. Polls the sink cluster's `_nodes/stats` endpoint, feeds readings to the active Regulator. |
| **FlowKnob** | The valve handle | `Arc<AtomicUsize>` shared between Governor (writes) and Joiners (reads). Controls how large each payload gets before flushing. |

### Channels

| Channel | Carries | From → To |
|---------|---------|-----------|
| **ch1** | Raw pages (`Barrel`) | Pumper → Joiner pool |
| **ch2** | Assembled payloads (`Payload`) | Joiner pool → Drainer pool |
| **ch3** | Latency/error readings (`GaugeReading`) | Drainers → Governor |

### Why the thread split?

Joiners run on `std::thread` (not tokio) because casting and payload assembly are CPU-bound. Pumpers and Drainers are async because they do network I/O. This keeps the tokio runtime free for I/O and prevents CPU work from starving network operations.

## A note on the code

The codebase follows a set of comedy conventions. This is intentional.

- **Comments** prefixed with `// --` are humor comments. They're there for humans reading the source.
- **Variable names** are deliberately expressive — `let my_therapist_says_move_on` instead of `retry_count`, `fn send_it_and_pray()` instead of `fn submit()`.
- **Error messages** are written as micro-fiction, designed to be informative and entertaining at 3am during an incident.
- **Module doc comments** open like TV show cold opens — set the scene, create tension, then explain what the module does.
- **Test names** read like episode titles — `the_one_where_the_config_file_doesnt_exist`, `sink_worker_survives_the_apocalypse`.
- **Emojis** are used functionally in log output and comments (🚀 info, ⚠️ warn, 💀 error, 🦆 no reason whatsoever).

The comedy is part of the project's identity. If you're reading the source and something makes you laugh, that's working as intended.

## Supported backends

| Backend | Source | Sink | Notes |
|---------|--------|------|-------|
| Elasticsearch 5–8 | Yes | Yes | |
| OpenSearch 1–3 | Yes | Yes | Uses the Elasticsearch backend — same bulk API wire protocol |
| File (JSON/NDJSON) | Yes | Yes | |
| InMemory | Yes | Yes | Test-only, not for production use |

## Project structure

```
kravex/
├── crates/
│   ├── kvx/          # Core library
│   │   └── src/
│   │       ├── backends/       # Source + Sink implementations (ES, File, InMemory)
│   │       ├── casts/          # Barrel-to-Draft transformers
│   │       ├── manifolds/      # Draft-to-Payload assemblers
│   │       ├── regulators/     # Adaptive throttle controllers (hill-climbing, PID, static)
│   │       ├── workers/        # Pumper, Joiner, Drainer, Governor
│   │       └── foreman.rs      # Pipeline orchestrator
│   └── kvx-cli/      # CLI binary wrapping kvx
│   └── kvx-api/      # REST API binary wrapping kvx
├── ee/                # Enterprise features (separate license)
├── configs/           # Example TOML configurations
├── datasets/          # Benchmark & demo data (gitignored — downloaded at runtime)
├── benchmark/         # Benchmark scripts and attribution
├── demo/              # End-to-end demo harness (File → ES → OpenSearch)
└── docker-compose.yml # Local ES + OpenSearch (profile: demo | bench)
```

## Configuration reference

All configuration lives in a single TOML file.

### `[runtime]`

| Key | Aliases | Description | Default |
|-----|---------|-------------|---------|
| `pumper_to_joiner_capacity` | `channel_size`, `queue_capacity` | Channel capacity (ch1) between Pumper and Joiner pool | `joiner_parallelism × 4` |
| `joiner_to_drainer_capacity` | `payload_channel_capacity` | Channel capacity (ch2) between Joiner pool and Drainer pool | `joiner_parallelism × 4` |
| `joiner_parallelism` | `num_joiner_workers` | Number of CPU-bound Joiner threads | `num_cpus - 1` |
| `sink_parallelism` | `num_sink_workers` | Number of concurrent Drainer workers | `joiner_parallelism × 12` |

### `[source_config]`

Source backend is specified as a sub-table: `[source_config.Elasticsearch]`, `[source_config.File]`.

Common source config fields (nested under the backend sub-table):

| Key | Description |
|-----|-------------|
| `max_batch_size_bytes` | Maximum batch size in bytes per page (optional) |
| `max_batch_size_docs` | Maximum batch size in documents per page (optional) |

#### `[source_config.Elasticsearch]`

| Key | Description |
|-----|-------------|
| `url` | Cluster URL (e.g., `http://localhost:9200`) |
| `index` | Index name to read from |
| `username` | Basic auth username (optional) |
| `password` | Basic auth password (optional) |
| `api_key` | API key authentication (optional) |

#### `[source_config.File]`

| Key | Description |
|-----|-------------|
| `file_name` | Path to input file (JSON or NDJSON) |

### `[sink_config]`

Sink backend is specified as a sub-table: `[sink_config.Elasticsearch]`, `[sink_config.File]`.

| Key | Description |
|-----|-------------|
| `max_request_size_bytes` | Maximum request payload size in bytes (optional, flattened at backend level) |

#### `[sink_config.Elasticsearch]`

| Key | Description |
|-----|-------------|
| `url` | Cluster URL — works for both Elasticsearch and OpenSearch |
| `index` | Index name to write to |
| `username` | Basic auth username (optional) |
| `password` | Basic auth password (optional) |
| `api_key` | API key authentication (optional) |

#### `[sink_config.File]`

| Key | Description |
|-----|-------------|
| `file_name` | Path to output file |

### `[drainer]`

| Key | Description |
|-----|-------------|
| `max_retries` | Maximum retry attempts per failed drain |
| `initial_backoff_ms` | Initial backoff duration in milliseconds |
| `backoff_multiplier` | Multiplier applied to backoff on each retry |
| `max_backoff_ms` | Maximum backoff cap in milliseconds |

### `[governor]`

Controls how Kravex adapts payload sizing during a migration. Specified as an enum-style sub-table — pick one:
Legacy `[flow_master.*]` sections are still accepted for backward compatibility.

#### `[governor.Static]`

Fixed payload size. No adaptation.

| Key | Description |
|-----|-------------|
| `output_bytes` | Fixed payload size in bytes |

#### `[governor.Throughput]`

Hill-climbing optimizer. Automatically finds the optimal payload size by measuring throughput and adjusting.

| Key | Description |
|-----|-------------|
| `min_request_size_bytes` | Floor for payload size |
| `initial_output_bytes` | Starting payload size |
| `window_duration_secs` | Measurement window duration |
| `improvement_threshold_pct` | Minimum improvement to keep climbing |
| `degradation_threshold_pct` | Degradation threshold to reverse direction |
| `re_explore_after_windows` | Windows of stability before re-exploring |

#### `[governor.Latency]`

PID controller targeting latency.

#### `[governor.CPU]`

PID controller targeting CPU pressure on the sink cluster.

## Development

### VS Code

`.vscode/tasks.json` provides:
- `cargo build --workspace` (default build — `Ctrl+Shift+B`)
- `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace`
- `cargo build -p kvx-cli` (targeted build for launch configs)

`.vscode/launch.json` provides:
- **Debug kvx-cli** (`F5`) — LLDB debugger attached via [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb)
- **Run kvx-cli (no debug)** (`Ctrl+F5`)

### Benchmarks

Sample JSON corpora (NOAA weather, Geonames, PubMed Central) are used for benchmarking. See [benchmark/DATA_ATTRIBUTION.md](benchmark/DATA_ATTRIBUTION.md) for licensing and attribution.

## License

[Business Source License 1.1](LICENSE) (BSL 1.1).

**Free to use** — run Kravex for your own migrations, internal tooling, CI pipelines, side projects, whatever. No limits.

**Not free to resell** — you can't use Kravex as part of a product or service you sell to others. That means no hosting it as a competing migration platform, and no using it as the engine behind paid consulting or managed migration services. If that's your use case, [reach out](mailto:daniel@kravex.net) for a commercial license.

**Goes fully open source** — on **2029-03-15**, this version automatically converts to [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0).

Enterprise features under `/ee/` require a separate license. Contact [daniel@kravex.net](mailto:daniel@kravex.net).
