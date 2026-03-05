# Summary

Core library for kravex — the data migration engine. 3-stage pipeline: Pumper (async I/O) → Joiner (sync CPU, std::thread) → Drainer (async I/O). Cow-powered zero-copy, Manifold-based payload assembly, and a clean config ownership model.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, memchr, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Dev-Dependencies**: wiremock (mock HTTP server for sink/source tests), criterion (benchmarks), tempfile
- **Edition**: 2024
- **Modules**:
  - `app_config` — `AppConfig`, `RuntimeConfig`, `SourceConfig`, `SinkConfig` (Figment-based config loading; owns all top-level config enums)
  - `backends` — backend wiring + re-exports; includes `CommonSinkConfig`, `CommonSourceConfig` (backend-shared config primitives)
  - `backends/common_config` — `CommonSinkConfig`, `CommonSourceConfig` (live here to avoid circular dep with `app_config`)
  - `backends/{source,sink}` — `Source`/`Sink` traits + `SourceBackend`/`SinkBackend` enums
  - `backends/elasticsearch/{elasticsearch_source,elasticsearch_sink}` — ES backend impls
  - `backends/file/{file_source,file_sink}` — file backend impls
  - `backends/in_mem/{in_mem_source,in_mem_sink}` — in-memory test backend
  - `casts` — `Caster` trait + `DocumentCaster` enum (NdJsonToBulk, Passthrough) + from_configs resolver
  - `manifolds` — `Manifold` trait + `ManifoldBackend` enum (NdjsonManifold, JsonArrayManifold)
  - `workers` — `Worker` trait, `Pumper` (async), `Joiner` (std::thread), `Drainer` (async)
  - `foreman` — pipeline orchestration (Foreman spawns+joins all workers)
  - `progress` — TUI metrics

## Pipeline Architecture (current — 3-stage: Pumper → Joiner → Drainer)
```
Source.next_page() → Option<String> (raw page)
  → ch1 (async_channel, bounded, MPMC)
  → Joiner(s) on std::thread (recv_blocking → buffer → manifold.join(buffer, caster) → send_blocking)
  → ch2 (async_channel, bounded, MPMC)
  → Drainer(s) on tokio (recv → sink.send)
  → Sink (HTTP POST, file write, memory push)
```

### Why 3 stages?
- **Pumper**: async — blocked on I/O (source reads, HTTP, file)
- **Joiner**: sync std::thread — CPU-bound (JSON parsing, casting, manifold join, buffering)
- **Drainer**: async — blocked on I/O (sink writes, HTTP, file)

Separating CPU work onto OS threads prevents starving tokio's async I/O workers.

## Module Dependency Graph
```
lib.rs ──► app_config (RuntimeConfig, SourceConfig, SinkConfig)
  │              │
  │              ▼
  │         backends ──► backends/common_config (CommonSinkConfig, CommonSourceConfig)
  │              │              ↑ (imported by backend-specific configs to embed)
  │              ▼
  │         foreman ──► workers (Pumper, Joiner, Drainer)
  │              │         │         │
  ├──► casts    ◄──────────┘ (Joiner holds DocumentCaster)
  └──► manifolds ◄─────────┘ (Joiner holds ManifoldBackend)
```

# Key Concepts

- **Sources return `Option<String>`**: one raw page per call, content uninterpreted. `None` = EOF
- **Sinks are I/O-only**: accept a fully rendered payload `String`, send it
- **Joiner buffers raw pages** by byte size, flushes via Manifold when buffer approaches `max_request_size_bytes`
- **Caster** (`DocumentCaster`): per-page format conversion (NdJsonToBulk, Passthrough)
- **Manifold** (`ManifoldBackend`): cast + assemble in one shot:
  - ES/File → `NdjsonManifold`: items joined with `\n`, trailing `\n`
  - InMemory → `JsonArrayManifold`: `[item,item,item]`, zero serde
- **All abstractions follow the same pattern**: trait → concrete impls → enum dispatcher → from_config resolver
- **Joiner threads**: CPU-bound work on `std::thread`, not tokio. Uses `recv_blocking()`/`send_blocking()` on async_channel
- **Drainer is thin**: just recv from ch2, send to sink. No buffering, no casting, no manifold

## Resolution Tables

### Caster Resolution (from SourceConfig × SinkConfig)
| SourceConfig | SinkConfig | Resolves to |
|---|---|---|
| File | Elasticsearch | `NdJsonToBulk` |
| File | File | `Passthrough` |
| InMemory | InMemory | `Passthrough` |
| Elasticsearch | File | `Passthrough` |

### Manifold Resolution (from SinkConfig)
| SinkConfig | Manifold | Wire Format |
|---|---|---|
| Elasticsearch | `NdjsonManifold` | `item\nitem\n` |
| File | `NdjsonManifold` | `item\nitem\n` |
| InMemory | `JsonArrayManifold` | `[item,item]` |

## Responsibility Boundaries

| Component | Responsibility |
|---|---|
| Source | Read raw page, return `Option<String>`. Format-ignorant. |
| ch1 | Carry `String` (raw pages) between Pumper and Joiners. MPMC bounded. |
| Joiner | Buffer pages by byte size, flush via `manifold.join(buffer, caster)`. CPU-bound on std::thread. |
| ch2 | Carry `String` (assembled payloads) between Joiners and Drainers. MPMC bounded. |
| Drainer | Pure I/O relay: recv payload from ch2, send to sink. Async on tokio. |
| Caster | Per-page format conversion (NdJsonToBulk, Passthrough) |
| Manifold | Cast all buffered feeds + assemble wire-format payload |
| Sink | Pure I/O: HTTP POST, file write, memory push |

## RuntimeConfig Fields

| Field | Default | Description |
|---|---|---|
| `queue_capacity` | 10 | ch1 bounded capacity (pumper → joiners) |
| `payload_channel_capacity` | 10 | ch2 bounded capacity (joiners → drainers) |
| `sink_parallelism` | 1 | Number of drainer/sink workers |
| `joiner_parallelism` | cpu_count - 1 (min 1) | Number of joiner threads (std::thread) |

# Notes for future reference

- POC/MVP stage — API surface is unstable
- Joiner threads use `std::thread::spawn`, not `tokio::task::spawn_blocking` — full control over OS threads
- Joiner does NOT implement the `Worker` trait (which returns `tokio::task::JoinHandle`); has its own `start()` → `std::thread::JoinHandle`
- Foreman manages both async (tokio JoinHandle) and sync (std::thread JoinHandle) workers
- Pipeline cascade: pumper done → ch1 closes → joiners flush+exit → ch2 closes → drainers exit
- `BUFFER_EPSILON_BYTES` = 64 KiB headroom; lives in joiner.rs now (moved from drainer.rs)
- Casters and manifolds are zero-sized structs — cloning per-joiner is free
- `SinkConfig::max_request_size_bytes()` helper is on `SinkConfig` in `app_config.rs`
- **Config ownership**: `RuntimeConfig`/`SourceConfig`/`SinkConfig` → `app_config.rs`; `CommonSinkConfig`/`CommonSourceConfig` → `backends/common_config.rs`
- ES bulk action line includes `_id` only; `_index`/`routing` set by sink URL
- `escape_json_string()` avoids serde round-trip for action line construction
- `channel_data.rs` still empty — to be removed
- ES sink no longer buffers — SinkWorker handles all buffering via byte-size threshold + epsilon
- `BUFFER_EPSILON_BYTES` = 64 KiB headroom to avoid exceeding max request size after transformation
- **FileSource I/O model**: reads 128 KiB chunks from raw `File`, scans with `memchr` SIMD, remainder bytes stashed between pages. No BufReader. `\n` byte `0x0A` is safe to scan in raw UTF-8 bytes.
- **Benchmarks**: criterion-based (`cargo bench --bench file_source_bench`). Compares buffered 128K chunk reading vs BufReader+read_line baseline. Reports MB/s and docs/s.
- **Dev deps**: criterion (async_tokio), tempfile
- Backend code split: each backend type has its own `{type}_source.rs` / `{type}_sink.rs`
- Core Source/Sink traits in `backends/source.rs` and `backends/sink.rs`
- Transforms and composers are Clone+Copy (zero-sized structs) — each SinkWorker gets its own copy
- `SinkConfig::max_request_size_bytes()` helper is now on `SinkConfig` in `app_config.rs`
- **Config ownership**: `RuntimeConfig`/`SourceConfig`/`SinkConfig` → `app_config.rs`; `CommonSinkConfig`/`CommonSourceConfig` → `backends/common_config.rs` (re-exported from `backends`)
- `supervisors/config.rs` — **deleted**. No backwards-compat shim remains. All callers updated.
- **3-stage pipeline**: Pumper (async) → ch1 → Joiner(s) (std::thread, CPU-bound: buffer+cast+join) → ch2 → Drainer(s) (async, thin I/O relay). `joiner_parallelism` and `payload_channel_capacity` in RuntimeConfig.
- S3 source backend not yet implemented

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`
- v1 transforms: IngestTransform/EgressTransform + Hit intermediate — **superseded**
- v2 transforms: direct pair functions + dead traits — **superseded**
- v3 transforms: mirrors backends pattern. `Transform` trait → concrete struct impls → `DocumentTransformer` enum dispatch → `from_configs()` resolver
- v4 pipeline refactor: Sources return `Vec<String>`, Sinks are I/O-only (`send(payload)`), SinkWorker does transform + binary collect. Hit/HitBatch phased out of pipeline.
- v5 collectors: Extracted payload assembly into `PayloadCollector` trait + `NdjsonCollector`/`JsonArrayCollector` — **superseded by v10 composers**
- v6-v9 backend file splits: separated backend implementations into dedicated files with re-export shims
- v10 raw pages + composers (current): Source returns `Option<String>` (raw page), Transform returns `Vec<Cow<str>>` (zero-copy), Composer replaces Collector (transform+assemble in one shot), SinkWorker buffers by byte size. 31 tests passing.
- v11 config migration (complete): `RuntimeConfig`/`SourceConfig`/`SinkConfig` → `app_config.rs`; `CommonSinkConfig`/`CommonSourceConfig` → `backends/common_config.rs`; `supervisors/config.rs` deleted; all callers updated. 31 tests passing.
- v12 buffered chunk reading + tests + benchmarks: FileSource reads 128 KiB chunks via raw `tokio::fs::File` (no BufReader), scans for newlines with `memchr` (SIMD-accelerated), stashes remainder bytes between `next_page()` calls. 8 unit tests. Criterion benchmarks: **~1.09 GiB/s** buffered vs ~196 MiB/s read_line (**5.7x throughput**), **~9.97M docs/s** vs ~2.38M docs/s (**4.2x docs/s**). 36 tests passing.
- v12 elasticsearch sink tests: 21 unit tests via wiremock. Covers constructor, bulk POST, close(), edge cases.
- v12 joiner threads: 3-stage pipeline — Pumper (async) → ch1 → Joiner(s) (std::thread, CPU-bound) → ch2 → Drainer(s) (async, thin I/O relay). Drainer simplified to recv+send.
- S3 source backend not yet implemented
