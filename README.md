# Kravex

Search migrations for teams with better things to do.

Kravex is an open-source migration engine for Elasticsearch and OpenSearch. ES 5–8, OpenSearch 1–3. Any version, any direction. Managed services, self-hosted, on-prem — if it speaks the ES or OpenSearch API, Kravex can move it.

## What it does

- Migrates index data between any combination of Elasticsearch and OpenSearch clusters
- Adaptive throttling — automatically backs off on 429s and ramps back up
- Smart cutovers — retry, validation, recovery, pause, and resume
- Zero tuning required — no knobs, no guesswork

## Quickstart

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (edition 2024)
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose

### 1. Start source and destination clusters

The included `docker-compose.yml` stands up Elasticsearch 8.15 on port 9200 and OpenSearch 2.19 on port 9201:

```bash
docker compose up -d es8 opensearch
```

Wait for both to be healthy:

```bash
docker compose ps
```

### 2. Seed some test data

Load documents into the source Elasticsearch cluster:

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
# kvx.toml

[runtime]
pumper_to_joiner_capacity = 8
sink_parallelism = 4

[source_config.Elasticsearch]
url = "http://localhost:9200"
index = "employees"

[sink_config.OpenSearch]
url = "http://localhost:9201"
index = "employees"
```

### 4. Build and run

```bash
cargo build --workspace
cargo run -p kvx-cli -- --config kvx.toml
```

### 5. Verify the migration

```bash
curl -s "http://localhost:9201/employees/_count" | jq .count
```

## Project structure

```
kravex/
├── crates/
│   ├── kvx/          # Core library — throttling, cutover logic, retry/recovery, adaptive throughput
│   └── kvx-cli/      # CLI binary — user-facing entry point wrapping kvx
├── configs/           # Example TOML configurations
├── benchmark/         # Benchmark data and attribution
└── docker-compose.yml # Local ES + OpenSearch + Meilisearch for development
```

## Supported backends

| Backend | Source | Sink |
|---------|--------|------|
| Elasticsearch 5–8 | Yes | Yes |
| OpenSearch 1–3 | Yes | Yes |
| Meilisearch | — | Yes |
| File (JSON) | Yes | — |

## Configuration reference

All configuration lives in a single TOML file.

### `[runtime]`

| Key | Description |
|-----|-------------|
| `pumper_to_joiner_capacity` | Internal queue capacity between source reader and sink writer |
| `sink_parallelism` | Number of concurrent sink workers |

### `[source_config]`

| Key | Description |
|-----|-------------|
| `max_batch_size_bytes` | Maximum batch size in bytes (optional) |
| `max_batch_size_docs` | Maximum batch size in documents (optional) |

Source backend is specified as a sub-table: `[source_config.Elasticsearch]`, `[source_config.File]`, etc.

### `[sink_config]`

| Key | Description |
|-----|-------------|
| `max_request_size_bytes` | Maximum request payload size in bytes (optional) |

Sink backend is specified as a sub-table: `[sink_config.Elasticsearch]`, `[sink_config.OpenSearch]`, `[sink_config.Meilisearch]`, etc.

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

## Status

POC/MVP — API surface is unstable. Dependency footprint is kept minimal.

## License

Apache 2.0 — see [LICENSE](LICENSE).
