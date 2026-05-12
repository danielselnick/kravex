# Memory — Kravex Project

## Recent Changes (2026-03-26)

### Cluster Stats in Progress Display
- **Feature**: Added source/sink cluster metrics (CPU%, MEM%) to progress display
- **Files created**: `crates/kvx-lib/src/progress/cluster_stats.rs`, `crates/kvx-lib/src/progress/README.md`
- **Files modified**: `crates/kvx-lib/src/progress/renderer.rs`, `crates/kvx-lib/src/progress/mod.rs`, `crates/kvx-lib/src/foreman.rs`
- **Architecture**: `ClusterStatsPoller` is a stateless struct (URL + auth + HTTP client). `fetch()` spawns a background tokio task hitting `_nodes/stats/os,jvm`. Renderer checks `is_finished()` each tick — non-blocking. No coupling to foreman pipeline logic.
- **Display**: 4-column mode when any ES poller exists (cols: drain rate, cumulative, source cluster, sink cluster). Classic 2-column when no pollers. `...` shown while first fetch is in-flight.
- **Auth**: `ClusterAuth` enum — `Basic`, `ApiKey`, `None`. Resolved from ES config at poller construction.
- **Tests**: 11 progress tests total (3 cluster_stats, 8 renderer). 188 total tests passing.
- `spawn_progress_reporter()` now takes `&AppConfig` — matches on source/sink config variants to build pollers

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

---
## 2026-03-26 — Fix Silent Document Loss in ES Bulk Response Handling

### Problem
- Demo showed 11,396,501 / 11,396,503 docs — 2 missing
- Root cause: `submit_bulk_request()` only checked HTTP status code, not response body
- ES `_bulk` API returns 200 even when individual documents are rejected (mapping errors, version conflicts, etc.)
- Per-item failures are only visible in the response body's `errors` boolean and `items` array

### What was done
- Added `BulkResponse`, `BulkItemWrapper`, `BulkItemResult`, `BulkItemError` serde types for deserializing bulk response
- `submit_bulk_request()` now parses response body: checks `errors` boolean, counts per-item failures, logs sampled reasons
- **Per-item retry**: extracted `send_bulk_post()` (HTTP plumbing) and `extract_failed_pairs()` (NDJSON line correlation) helpers
- Retry loop: up to 10 retries, each round only sends the failed NDJSON action/doc pairs — successful docs never re-sent
- NDJSON correlation: pair `i` = lines `2i` and `2i+1`, response item `i` tells if pair `i` failed
- Safety bail: if NDJSON line count ≠ 2 × response item count, skip retry (can't correlate)
- Fast path: `errors: false` skips item iteration; empty body treated as success
- Added 8 new tests (Group C.5 + C.6): clean response, errors response, total failure, weird JSON, non-JSON, empty body, partial-retry-then-success, retry exhaustion
- Added stall detection to `demo.py` `poll_until_doc_count()` — if doc count unchanged for 60s, reports stalled

### Test results
- 183 tests passed, 0 failed (29 elasticsearch_sink tests including 8 new)

### Files modified
- `crates/kvx-lib/src/backends/elasticsearch/elasticsearch_sink.rs` — bulk response types + parsing + per-item retry + 8 tests
- `crates/kvx-lib/src/backends/elasticsearch/README.md` — documented bulk response validation and per-item retry
- `demo/demo.py` — stall detection in polling loop
- `memory.md` — this entry

### Key design decisions
- Per-item retry prevents duplicate documents in File→ES flows (auto-generated `_id`s)
- Per-item retry is also safe for ES→ES flows (explicit `_id`s make retry idempotent)
- 10 retries max — persistent rejections (mapping errors) won't loop forever
- Cap sampled error reasons at 5 (avoid OOM on catastrophic bulk failure)
- Empty body = success (backward compat with mocks/proxies)
- Non-JSON body = error (catches reverse proxy HTML error barrels)

---
## 2026-03-26 — ElasticsearchSource Implementation (PIT + search_after)

### What was done
- Added `index: String` (required) field to `ElasticsearchSourceConfig` in config.rs
- Implemented full `ElasticsearchSource` with PIT + search_after pagination in elasticsearch_source.rs
- Source lifecycle: open PIT → _search with search_after cursor → close PIT on EOF
- HTTP client: 10s connect timeout, 30s response timeout (mirrors sink pattern)
- Auth: API key > basic auth > anonymous (same hierarchy as sink)
- Startup validation: ping cluster, verify index exists (HEAD request)
- Returns raw _search response envelope as Page — PitToBulk/PitToJson Tappers extract hits downstream
- Added 6 unit tests (exhaustion, auth priority, request body structure)
- Updated elasticsearch README.md with full config table and knowledge graph
- Fixed 3 existing tests in casts/mod.rs and lib.rs (missing `index` field)

### Test results
- 175 tests passed, 0 failed

### Files modified
- `crates/kvx-lib/src/backends/elasticsearch/config.rs` — added `index: String` field
- `crates/kvx-lib/src/backends/elasticsearch/elasticsearch_source.rs` — full implementation
- `crates/kvx-lib/src/backends/elasticsearch/README.md` — updated docs
- `crates/kvx-lib/src/casts/mod.rs` — fixed test struct initializers (added index field)
- `crates/kvx-lib/src/lib.rs` — fixed test struct initializer (added index field)

### Architecture notes
- PIT requires ES 7.10+
- PIT ID can rotate between responses — always use the latest from response body
- close_pit() is best-effort (PIT auto-expires after keep_alive=5m)
- Source does NOT apply regulator/throttle — backpressure comes from channel bounds
- Sort by `_doc` ascending — most efficient for bulk reads (no scoring)
