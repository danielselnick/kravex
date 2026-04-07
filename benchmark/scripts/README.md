

# Summary
Shared Python utilities for Kravex demo and benchmark scripts.

# Description
The Switzerland of the Kravex repo. Common functions extracted from `demo/scripts/` and `benchmark/scripts/` to eliminate duplication. One library to rule them all.

# Knowledge Graph
- `kvx_utils.py` (in `benchmark/scripts/`) → used by `benchmark/scripts/*.py`
- Index ops: `create_index`, `delete_index`, `get_doc_count`, `refresh_index`, `force_merge`, `reset_index`
- Binary discovery: `find_kvx_binary`, `build_kvx_binary`, `find_or_build_kvx_binary`
- Cluster health: `check_cluster_health` (polling with timeout)
- Metrics: `MetricsSampler` (background thread, CPU/RSS via `ps`)
- Result recording: `record_result` (JSON output for Jupyter notebook)
- URL helpers: `extract_host_port`, `engine_from_url`, `get_engine_url`
- Timing: `generate_run_id`

# Key Concepts
- **No rich/console dependency**: Shared utils use plain print/exceptions — callers add their own UI
- **macOS-first metrics**: `ps -p PID -o %cpu=,rss=` — RSS in KB, converted to MB
- **Idempotent index ops**: `reset_index` = delete + create, `create_index` handles settings
- **Benchmark-optimal defaults**: 1 shard, 0 replicas, refresh=-1 for max ingest throughput

# Notes for future reference
- Import pattern: `from kvx_utils import ...` (Python auto-adds script directory to sys.path)
- `_guess_project_root()` walks up from `benchmark/scripts/` to find `Cargo.toml`
- `MetricsSampler` uses daemon threads — no zombie cleanup needed
