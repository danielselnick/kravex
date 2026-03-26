# Demo

End-to-end demonstration of Kravex migrating data through a two-leg pipeline:

```
geonames.json  →  Elasticsearch  →  OpenSearch
  (11.4M docs)      (port 19200)     (port 19201)
```

## Architecture

`demo.sh` is the entry point. It checks prerequisites, builds kvx-cli in release mode, then hands off to `demo.py` — a polling orchestrator that manages Docker infrastructure and monitors migration progress.

Kravex execution lives in standalone shell scripts that you run in separate terminals:

```
demo.sh            →  builds binary, launches orchestrator
demo.py            →  manages containers, creates indices, polls doc counts
file_to_esdb.sh    →  runs kravex: File → Elasticsearch
esdb_to_osdb.sh    →  runs kravex: Elasticsearch → OpenSearch
```

## Prerequisites

| Requirement | Why |
|---|---|
| [Docker Desktop](https://docs.docker.com/get-docker/) | Runs ES + OpenSearch containers |
| [uv](https://docs.astral.sh/uv/) | Python package/environment manager |
| [Rust toolchain](https://rustup.rs/) | Builds kvx-cli (auto-built by demo.sh if missing) |
| `datasets/geonames.json` | 11.4M geographic features (~3.5 GB) |

**Minimum RAM**: 16 GB recommended (two search clusters @ 2 GB heap each + OS overhead).

**Docker Desktop resources**: Ensure Docker has at least 6 GB memory allocated (Settings → Resources).

## Quick Start

```bash
# Terminal 1 — orchestrator
./demo/demo.sh
```

The orchestrator will:
1. Build kvx-cli (if needed)
2. Start Docker containers
3. Create indices
4. Prompt you to run each leg script

```bash
# Terminal 2 — when prompted for Leg 1
./demo/file_to_esdb.sh

# Terminal 2 — when prompted for Leg 2
./demo/esdb_to_osdb.sh
```

The orchestrator polls the destination index and detects completion automatically.

## Standalone Usage

The leg scripts can be run independently (e.g., for re-running a single leg):

```bash
# Assuming containers are already running and indices exist:
./demo/file_to_esdb.sh    # Leg 1 only
./demo/esdb_to_osdb.sh    # Leg 2 only
```

## What It Does

1. **Build** — `demo.sh` builds kvx-cli in release mode (skips if already built)
2. **Pre-flight checks** — Verifies dataset, Docker connectivity
3. **Port conflict resolution** — Detects containers/processes on ports 19200/19201, offers to stop them
4. **Container startup** — Launches Elasticsearch 8.15.0 and OpenSearch 2.13.0 via docker-compose
5. **Index creation** — Creates `geonames` index with optimized settings (no replicas, refresh disabled)
6. **Leg 1: File → Elasticsearch** — You run `file_to_esdb.sh`; orchestrator polls ES for 11.4M docs
7. **Leg 2: Elasticsearch → OpenSearch** — You run `esdb_to_osdb.sh`; orchestrator polls OS for 11.4M docs
8. **Validation** — Compares doc counts across clusters
9. **Summary** — Prints timing table with per-step durations and doc counts
10. **Cleanup prompt** — Offers to tear down containers or leave them running

## Files

| File | Purpose |
|---|---|
| `demo.sh` | Entry point — checks uv/cargo, builds kvx-cli, delegates to `demo.py` |
| `demo.py` | Orchestrator — manages containers, creates indices, polls for completion |
| `file_to_esdb.sh` | Runs kravex for Leg 1 (File → Elasticsearch) |
| `esdb_to_osdb.sh` | Runs kravex for Leg 2 (Elasticsearch → OpenSearch) |
| `pyproject.toml` | Python dependencies for `uv` (`requests`) |
| `docker-compose-demo.yml` | ES 8.15.0 (19200) + OpenSearch 2.13.0 (19201) |
| `demo_file_to_esdb.toml` | Kravex config: File → Elasticsearch |
| `demo_esdb_to_osdb.toml` | Kravex config: Elasticsearch → OpenSearch |

## Ports

| Port | Service | Why non-standard |
|---|---|---|
| 19200 | Elasticsearch 8.15.0 | Avoids collision with dev `docker-compose.yml` on 9200 |
| 19201 | OpenSearch 2.13.0 | Avoids collision with dev `docker-compose.yml` on 9201 |

## Dataset

The geonames dataset (11.4M docs, ~3.5 GB decompressed) is not checked into git.

Download it:
```bash
python benchmark/scripts/setup.py
```

Or manually:
```bash
mkdir -p datasets
curl -o datasets/geonames.json.bz2 \
  https://rally-tracks.elastic.co/geonames/documents.json.bz2
bunzip2 -k datasets/geonames.json.bz2
```

## Troubleshooting

**Docker not running**
```
Cannot connect to Docker daemon.
```
Start Docker Desktop and try again.

**Ports in use**
```
Port 19200 (Elasticsearch) is in use by a non-Docker process
```
Find and stop the process: `lsof -i :19200`

**Dataset missing**
```
Dataset not found: /path/to/datasets/geonames.json
```
Run `python benchmark/scripts/setup.py` or see the download instructions above.

**Containers OOM-killed**
Increase Docker Desktop memory allocation to at least 6 GB (Settings → Resources → Memory).

**kvx-cli binary not found (from leg scripts)**
Run `./demo/demo.sh` first — it handles the build. Or manually: `cargo build --release -p kvx-cli`

## Manual Cleanup

If the demo exits unexpectedly without cleaning up:
```bash
docker compose -f demo/docker-compose-demo.yml down -v
```
