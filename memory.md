# Memory — Kravex Project

## Recent Changes (2026-03-26)

### Demo Harness Created
- **`/demo/`** folder with full end-to-end demo: File → ES → OpenSearch
- Files: `demo.sh`, `demo.py`, `pyproject.toml`, `docker-compose-demo.yml`, `demo_file_to_esdb.toml`, `demo_esdb_to_osdb.toml`, `README.md`
- Demo ports: **19200** (ES), **19201** (OS) — avoids dev ports 9200/9201
- Uses `uv` for Python env, `docker` SDK for container inspection, `osascript` for terminal launch on macOS
- Completion detection: marker file (`/tmp/kravex_demo_*.done`) + doc count polling fallback
- Geonames mappings embedded in `demo.py` — no external mapping files needed

### Dataset Directory Renamed
- `benchmark/data/` → `datasets/` (project root)
- Updated: `.gitignore`, `configs/kvx_file_to_esdb.toml`, `configs/kvx_file_to_meilisearch.toml`, `benchmark/scripts/python_migrate.py`, `benchmark/scripts/run_benchmarks.py`, `benchmark/scripts/setup.py`
- Both benchmark scripts use `DATA_DIR = REPO_ROOT / "datasets"` now

### Root README Updated
- Added "Demo" section with `./demo/demo.sh` quick start
- Project structure updated: shows `datasets/`, updated `demo/` description

## Architecture Notes

### Backend Config Patterns
- OpenSearch uses `Elasticsearch` backend (same wire protocol, same config enum variant)
- Source `common_config` is NOT flattened (needs sub-table in TOML)
- Sink `common_config` IS flattened (same level as url/index)
- Binary name: `kvx-cli` (from `crates/kvx-cli/`)

### Data Files (gitignored)
- `datasets/geonames.json` — 11.4M docs, ~3.5 GB
- `datasets/noaa.json` — 33.6M docs
- `datasets/pmc.json` — 574K docs
- Downloaded via `benchmark/scripts/setup.py` from elastic/rally-tracks

### Docker Compose Ports
- Dev: ES=9200, OS=9201, Meilisearch=7700
- Demo: ES=19200, OS=19201
