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
