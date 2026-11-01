# Kravex

Search migrations for teams with better things to do.

Kravex is a Rust workspace for moving data through search stacks with a single, config-driven engine. The current codebase is centered on a library crate (`crates/kvx-lib`) and a CLI (`crates/kvx-cli`), with a demo harness under `demo/` for end-to-end validation.

## What it does

- Migrates data between Elasticsearch and OpenSearch using the same bulk/API conventions
- Supports file-backed and in-memory backends for testing and local workflows
- Uses adaptive throttling and flow-control logic to back off on 429s and recover smoothly
- Runs from a TOML config file and supports environment overrides via `KVX_*` variables
- Includes a demo pipeline that provisions local Docker services and exercises end-to-end migration

## Current repository layout

```text
kravex/
├── README.md
├── LICENSE
├── Cargo.toml
├── docker-compose.yml            # local ES + OpenSearch dev setup
├── configs/                      # example TOML configs
├── crates/
│   ├── kvx-cli/                  # CLI entrypoint
│   └── kvx-lib/                  # migration engine
├── demo/                         # full pipeline demo, dataset downloads, smoke tests
├── ee/                           # enterprise-only features, separate license
├── sample_data/                  # repo-local fixtures and dataset material
├── .github/                      # CI workflow metadata
├── .idea/ .obsidian/ .zed/        # editor-local workspace state
└── architecture.drawio.png       # architecture diagram
```

This repository does not contain a top-level `benchmark/` directory or a checked-in `input.json` file. The demo and sample data live under `demo/` and `sample_data/`.

## Demo

Run the end-to-end helper for a File → Elasticsearch → OpenSearch workflow:

```bash
./demo/demo.sh
```

This uses Docker, builds the CLI if needed, downloads demo datasets when missing, creates local clusters, and validates the migration path.

Requirements:

- Rust toolchain
- Docker Desktop / Docker Engine + Docker Compose
- `uv` for the Python-based demo orchestration

## Quickstart

### Prerequisites

- Rust toolchain (edition 2024)
- Docker and Docker Compose
- Optional: `uv` for the demo workflow

### 1. Start the local search services

```bash
docker compose up -d es8 opensearch
```

This repo's `docker-compose.yml` defines the development stack used by the default local setup:

- Elasticsearch 8.15.0 on `http://localhost:9200`
- OpenSearch 2.19.0 on `http://localhost:9201`

### 2. Pick a configuration

The repo includes example configs under `configs/`:

```bash
ls configs
```

Examples:

- `configs/kvx_file.toml` — file-based migration example
- `configs/kvx_es_to_es.toml` — Elasticsearch-to-Elasticsearch (or ES → OpenSearch) example

### 3. Run the CLI

```bash
cargo run -p kvx-cli -- configs/kvx_es_to_es.toml
```

If you want to run without a config file, the CLI also defaults to `kvx.toml` in the current directory when present.

## Architecture

Kravex models the migration pipeline as a plumbing system:

```text
Pumper (async) → ch1 → Refiner pool (std::thread) → ch2 → Drainer pool (async) → Sink
                           ↑                                      |
                       FlowKnob ← Governor ← Regulator ← ch3
```

The engine separates:

- Source backends: emit data from Elasticsearch, files, or in-memory fixtures
- Taps: transform raw records into sink-ready payloads
- Manifolds: group and buffer transformed payloads into drums
- Refiners: CPU-bound assembly work
- Drainers: async sink workers with retry/backoff behavior
- Governor and regulators: adapt throughput and batching under load

## Supported backends

| Backend | Source | Sink | Notes |
| --- | --- | --- | --- |
| Elasticsearch | Yes | Yes | Direct bulk API compatibility |
| OpenSearch | Yes | Yes | Same protocol family as Elasticsearch |
| File (JSON/NDJSON) | Yes | Yes | Useful for local testing and batch workflows |
| InMemory | Yes | Yes | Primarily for tests and internal fixtures |

## Configuration

All runtime behavior is driven by a TOML config and merged with environment variables using the `KVX_` prefix.

Example:

```toml
[runtime]
pumper_to_refiner_capacity = 4
sink_parallelism = 4

[source_config.Elasticsearch]
url = "http://localhost:9200"
index = "employees"

[sink_config.Elasticsearch]
url = "http://localhost:9201"
index = "employees"
max_drum_size_bytes = 4194304
```

The CLI and library also support config structs for the sink/source backends, drainer retry behavior, and governor tuning.

## Development

### Workspace

```bash
cargo build --workspace
cargo test --workspace
```

### Demo workflow

```bash
uv run --project demo demo/demo.py --smoke
```

The smoke test is intended for CI and uses the demo harness without requiring interactive prompts.

## License

Kravex is distributed under the Business Source License 1.1 (`LICENSE`).

Enterprise-only features under `/ee/` require a separate commercial arrangement and are not included in the standard open-source license terms.

## Notes

- The project intentionally includes humorous comments and variable names in the codebase.
- The repo is a working Rust migration engine, not a static documentation-only project.
- The most important operational docs are in the project code, config examples, and the `demo/` harness.
