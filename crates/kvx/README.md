# Summary

Core library for kravex — the data migration engine. Now with raw pages, Cow-powered zero-copy, and Composer-based payload assembly.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Edition**: 2024
- **Modules**: `backends` (backend wiring + re-exports), `backends/{source,sink}` (core traits + backend enums), `backends/elasticsearch/{elasticsearch_source,elasticsearch_sink}`, `backends/file/{file_source,file_sink}`, `backends/in_mem/{in_mem_source,in_mem_sink}`, `composers` (Composer trait + NdjsonComposer/JsonArrayComposer), `common` (Hit/HitBatch — legacy dead code), `transforms` (Transform trait + Cow-based impls), `supervisors` (pipeline orchestration), `progress` (TUI metrics), `app_config` (Figment-based config)

## Pipeline Architecture (current — Raw Pages + Composer)
```
Source.next_page() → Option<String> (raw page)
  → channel(String)
  → SinkWorker buffers Vec<String> (by byte size threshold)
  → Composer.compose(&buffer, &transformer) → final payload String
  → Sink.send(payload)
```

## Module Dependency Graph
```
lib.rs ──► supervisors ──► backends (Source/Sink traits)
  │              │
  │              ▼
  │         workers (SourceWorker, SinkWorker)
  │              │
  ├──► transforms ◄── Composer (calls transformer per page)
  └──► composers  ◄── SinkWorker (holds ComposerBackend + DocumentTransformer)
```

# Key Concepts

- **Sources return `Option<String>`**: one raw page per call, content uninterpreted. `None` = EOF. Source is maximally ignorant — it's a faucet, not a chef.
- **Sinks are I/O-only**: accept a fully rendered payload `String`, send it (HTTP POST, file write, memory push)
- **SinkWorker buffers raw pages** by byte size, flushes via Composer when buffer approaches `max_request_size_bytes`
- **Transform** (`DocumentTransformer`): per-page format conversion. Returns `Vec<Cow<str>>` items:
  - `Cow::Borrowed` = zero-copy passthrough (no allocation!)
  - `Cow::Owned` = format conversion (Rally→ES bulk, etc.)
- **Composer** (`ComposerBackend`): transform + assemble in one shot:
  - ES/File → `NdjsonComposer`: items joined with `\n`, trailing `\n`
  - InMemory → `JsonArrayComposer`: `[item,item,item]`, zero serde
- **All abstractions follow the same pattern**: trait → concrete impls → enum dispatcher → from_config resolver
- **Zero-copy passthrough**: NDJSON→NDJSON scenarios (file-to-file) — Cow borrows from buffered pages, no per-doc allocation

## Architecture Pattern (used by backends, transforms, composers)
```
┌──────────────────┐   ┌──────────────────────┐   ┌─────────────────────┐
│ trait Source      │   │ trait Transform       │   │ trait Composer      │
│   fn next_page() │   │   fn transform(&str)  │   │   fn compose(pages) │
│   → Option<Str>  │   │   → Vec<Cow<str>>     │   │   → String          │
└────────┬─────────┘   └────────┬─────────────┘   └────────┬────────────┘
         │                      │                           │
┌────────┴─────────┐   ┌────────┴─────────────┐   ┌────────┴────────────┐
│ FileSource       │   │ RallyS3ToEs          │   │ NdjsonComposer      │
│ InMemorySource   │   │ Passthrough          │   │ JsonArrayComposer   │
│ ElasticsearchSrc │   │                      │   │                     │
└────────┬─────────┘   └────────┬─────────────┘   └────────┬────────────┘
         │                      │                           │
┌────────┴─────────┐   ┌────────┴─────────────┐   ┌────────┴────────────┐
│ enum SourceBknd  │   │ enum DocTransformer   │   │ enum ComposerBknd   │
│   match dispatch │   │   match dispatch      │   │   match dispatch    │
└──────────────────┘   └──────────────────────┘   └─────────────────────┘
```

## Resolution Tables

### Transform Resolution (from SourceConfig × SinkConfig)
| SourceConfig | SinkConfig | Resolves to |
|---|---|---|
| File | Elasticsearch | `RallyS3ToEs` — splits page by `\n`, transforms each doc |
| File | File | `Passthrough` — returns entire page as `Cow::Borrowed` |
| InMemory | InMemory | `Passthrough` |
| Elasticsearch | File | `Passthrough` |
| other | other | `panic!` at resolve time |

### Composer Resolution (from SinkConfig)
| SinkConfig | Composer | Wire Format |
|---|---|---|
| Elasticsearch | `NdjsonComposer` | `item\nitem\n` |
| File | `NdjsonComposer` | `item\nitem\n` |
| InMemory | `JsonArrayComposer` | `[item,item]` |

## Responsibility Boundaries

| Component | Responsibility |
|---|---|
| Source | Read raw page, return `Option<String>`. Format-ignorant. |
| Channel | Carry `String` (raw pages) between workers |
| SinkWorker | Buffer pages by byte size, flush via Composer |
| Composer | Transform pages (via Transformer) + assemble wire-format payload |
| Transform | Per-page → `Vec<Cow<str>>` items (Borrowed=passthrough, Owned=conversion) |
| Sink | Pure I/O: HTTP POST, file write, memory push |

# Notes for future reference

- POC/MVP stage — API surface is unstable
- `Hit`/`HitBatch` in `common.rs` are now dead code — pipeline uses raw pages throughout
- Rally S3 transform splits page by `\n`, transforms each doc individually, strips 6 top-level metadata fields; nested refs survive
- ES bulk action line includes `_id` only; `_index`/`routing` set by sink URL
- Passthrough doesn't validate or split — returns entire page as one `Cow::Borrowed` item
- `escape_json_string()` avoids serde round-trip for action line construction
- `collectors.rs` superseded by `composers.rs` — Composer handles transform+assemble in one step
- `channel_data.rs` still empty — to be removed
- ES sink no longer buffers — SinkWorker handles all buffering via byte-size threshold + epsilon
- `BUFFER_EPSILON_BYTES` = 64 KiB headroom to avoid exceeding max request size after transformation
- Backend code split: each backend type has its own `{type}_source.rs` / `{type}_sink.rs`
- Core Source/Sink traits in `backends/source.rs` and `backends/sink.rs`
- Transforms and composers are Clone+Copy (zero-sized structs) — each SinkWorker gets its own copy
- `SinkConfig::max_request_size_bytes()` helper extracts the limit regardless of sink variant

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`
- v1 transforms: IngestTransform/EgressTransform + Hit intermediate — **superseded**
- v2 transforms: direct pair functions + dead traits — **superseded**
- v3 transforms: mirrors backends pattern. `Transform` trait → concrete struct impls → `DocumentTransformer` enum dispatch → `from_configs()` resolver
- v4 pipeline refactor: Sources return `Vec<String>`, Sinks are I/O-only (`send(payload)`), SinkWorker does transform + binary collect. Hit/HitBatch phased out of pipeline.
- v5 collectors: Extracted payload assembly into `PayloadCollector` trait + `NdjsonCollector`/`JsonArrayCollector` — **superseded by v10 composers**
- v6-v9 backend file splits: separated backend implementations into dedicated files with re-export shims
- v10 raw pages + composers (current): Source returns `Option<String>` (raw page), Transform returns `Vec<Cow<str>>` (zero-copy), Composer replaces Collector (transform+assemble in one shot), SinkWorker buffers by byte size. 31 tests passing.
- S3 source backend not yet implemented
