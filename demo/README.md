# Demo

End-to-end demonstration of Kravex migrating data through a two-leg pipeline:

```
dataset.json  →  Elasticsearch  →  OpenSearch
                   (port 19200)     (port 19201)
```

Supports three datasets: **geonames** (11.4M docs), **noaa** (33.6M docs), **pmc** (574K docs).

## Architecture

`demo.sh` is the entry point. It checks prerequisites, builds kvx-cli in release mode, then hands off to `demo.py` — a polling orchestrator that manages Docker infrastructure and monitors migration progress.

Kravex execution lives in standalone shell scripts that you run in separate terminals:

```
demo.sh                →  builds binary, launches orchestrator
demo.py                →  manages containers, creates indices, polls doc counts
file_to_esdb.sh [ds]   →  runs kravex: File → Elasticsearch
esdb_to_osdb.sh [ds]   →  runs kravex: Elasticsearch → OpenSearch
```

## Prerequisites

| Requirement | Why |
|---|---|
| [Docker Desktop](https://docs.docker.com/get-docker/) | Runs ES + OpenSearch containers |
| [uv](https://docs.astral.sh/uv/) | Python package/environment manager |
| [Rust toolchain](https://rustup.rs/) | Builds kvx-cli (auto-built by demo.sh if missing) |
| A dataset (see below) | Automatically downloaded if missing |

**Minimum RAM**: 16 GB recommended (two search clusters @ 2 GB heap each + OS overhead).

**Docker Desktop resources**: Ensure Docker has at least 6 GB memory allocated (Settings → Resources).

## Quick Start

```bash
# Terminal 1 — orchestrator
./demo/demo.sh
```

The orchestrator will:
1. Build kvx-cli (if needed)
2. Present dataset selection (auto-detects what's available, offers download)
3. Start Docker containers
4. Create indices with dataset-specific mappings
5. Prompt you to run each leg script

```bash
# Terminal 2 — when prompted for Leg 1
./demo/file_to_esdb.sh geonames

# Terminal 2 — when prompted for Leg 2
./demo/esdb_to_osdb.sh geonames
```

The orchestrator polls the destination index and detects completion automatically.

## Datasets

| Dataset | Docs | Size (decompressed) | Source |
|---|---|---|---|
| geonames | 11,396,503 | ~3.5 GB | Geographic features (cities, mountains, lakes) |
| noaa | 33,659,481 | ~9 GB | NOAA weather station observations |
| pmc | 574,199 | ~1.6 GB | PubMed Central journal articles |

All datasets come from [Elastic Rally tracks](https://github.com/elastic/rally-tracks). See `DATA_ATTRIBUTION.md` for licensing.

The demo will auto-detect available datasets and offer to download missing ones. Manual download:

```bash
mkdir -p datasets
curl -L -o datasets/geonames.json.bz2 \
  https://rally-tracks.elastic.co/geonames/documents-2.json.bz2
bunzip2 -k datasets/geonames.json.bz2
```

## Standalone Usage

The leg scripts accept an optional dataset argument (defaults to `geonames`):

```bash
# Assuming containers are already running and indices exist:
./demo/file_to_esdb.sh         # geonames (default)
./demo/file_to_esdb.sh noaa    # noaa
./demo/esdb_to_osdb.sh pmc     # pmc
```

## What It Does

1. **Build** — `demo.sh` builds kvx-cli in release mode (skips if already built)
2. **Dataset selection** — Auto-detects available datasets, offers download for missing ones
3. **Pre-flight checks** — Verifies dataset, Docker connectivity
4. **Port conflict resolution** — Detects containers/processes on ports 19200/19201, offers to stop them
5. **Container startup** — Launches Elasticsearch 7.10.2 and OpenSearch 3.5.0 via docker-compose
6. **Index creation** — Creates index with dataset-specific mapping and optimized settings
7. **Leg 1: File → Elasticsearch** — You run `file_to_esdb.sh`; orchestrator polls ES
8. **Leg 2: Elasticsearch → OpenSearch** — You run `esdb_to_osdb.sh`; orchestrator polls OS
9. **Validation** — Compares doc counts across clusters
10. **Summary** — Prints timing table with per-step durations and doc counts
11. **Cleanup prompt** — Offers to tear down containers or leave them running

## Files

| File | Purpose |
|---|---|
| `demo.sh` | Entry point — checks uv/cargo, builds kvx-cli, delegates to `demo.py` |
| `demo.py` | Orchestrator — manages containers, creates indices, polls for completion |
| `file_to_esdb.sh` | Runs kravex for Leg 1 (File → ES). Accepts dataset arg. |
| `esdb_to_osdb.sh` | Runs kravex for Leg 2 (ES → OS). Accepts dataset arg. |
| `kvx_utils.py` | Shared utilities — cluster health, index CRUD, binary discovery |
| `pyproject.toml` | Python dependencies for `uv` (`requests`) |
| `docker-compose-demo.yml` | ES 8.15.0 (19200) + OpenSearch 3.5.0 (19201) |
| `demo_file_to_esdb_*.toml` | Kravex configs: File → Elasticsearch (per dataset) |
| `demo_esdb_to_osdb_*.toml` | Kravex configs: Elasticsearch → OpenSearch (per dataset) |
| `reset_index.sh` | Standalone utility to reset an ES/OS index |
| `DATA_ATTRIBUTION.md` | Dataset licensing and attribution |
| `results/` | Historical benchmark results (JSON) |

## Ports

| Port | Service | Why non-standard |
|---|---|---|
| 19200 | Elasticsearch 7.10.2 | Avoids collision with dev `docker-compose.yml` on 9200 |
| 19201 | OpenSearch 3.5.0 | Avoids collision with dev `docker-compose.yml` on 9201 |

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
The demo auto-detects and offers to download datasets. For manual download, see the Datasets section above.

**Containers OOM-killed**
Increase Docker Desktop memory allocation to at least 6 GB (Settings → Resources → Memory).

**kvx-cli binary not found (from leg scripts)**
Run `./demo/demo.sh` first — it handles the build. Or manually: `cargo build --release -p kvx-cli`

## Manual Cleanup

If the demo exits unexpectedly without cleaning up:
```bash
docker compose -f demo/docker-compose-demo.yml down -v
```
