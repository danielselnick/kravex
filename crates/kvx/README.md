# Summary

Core library for kravex — the data migration engine.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Edition**: 2024
- **Modules**: `backends` (Source/Sink traits + impls), `collectors` (PayloadCollector trait + impls), `common` (Hit/HitBatch — legacy dead code), `transforms` (Transform trait + impls), `supervisors` (pipeline orchestration), `progress` (TUI metrics), `app_config` (Figment-based config)

## Pipeline Architecture (current)
```
Source::next_batch() → Vec<String> → channel → SinkWorker:
  1. transform each doc (DocumentTransformer)
  2. collect into payload (CollectorBackend)
  3. sink.send(payload) (SinkBackend — I/O only)
```

## Module Dependency Graph
```
lib.rs ──► supervisors ──► backends (Source/Sink traits)
  │              │
  │              ▼
  │         workers (SourceWorker, SinkWorker)
  │              │
  ├──► transforms ◄── SinkWorker (holds DocumentTransformer)
  └──► collectors ◄── SinkWorker (holds CollectorBackend)
```

# Key Concepts

- **Sources return `Vec<String>`**: raw document strings, no Hit wrappers, no trailing newlines
- **Sinks are I/O-only**: accept a fully rendered payload `String`, send it (HTTP POST, file write, memory push)
- **SinkWorker orchestrates three phases**: transform → collect → send
- **Transform** (`DocumentTransformer`): per-document format conversion. Rally JSON → ES bulk lines, or passthrough.
- **Collector** (`CollectorBackend`): payload assembly format, resolved per sink type:
  - ES/File → `NdjsonCollector`: each string gets trailing `\n`
  - InMemory → `JsonArrayCollector`: `[doc,doc,doc]`, zero serde
- **All three abstractions follow the same pattern**: trait → concrete impls → enum dispatcher → from_config resolver

## Architecture Pattern (used by backends, transforms, collectors)
```
┌──────────────────┐   ┌──────────────────────┐   ┌─────────────────────┐
│ trait Source      │   │ trait Transform       │   │ trait PayloadCollect │
│   fn next_batch() │   │   fn transform()     │   │   fn collect()      │
└────────┬─────────┘   └────────┬─────────────┘   └────────┬────────────┘
         │                      │                           │
┌────────┴─────────┐   ┌────────┴─────────────┐   ┌────────┴────────────┐
│ FileSource       │   │ RallyS3ToEs          │   │ NdjsonCollector     │
│ InMemorySource   │   │ Passthrough          │   │ JsonArrayCollector  │
│ ElasticsearchSrc │   │                      │   │                     │
└────────┬─────────┘   └────────┬─────────────┘   └────────┬────────────┘
         │                      │                           │
┌────────┴─────────┐   ┌────────┴─────────────┐   ┌────────┴────────────┐
│ enum SourceBknd  │   │ enum DocTransformer   │   │ enum CollectorBknd  │
│   match dispatch │   │   match dispatch      │   │   match dispatch    │
└──────────────────┘   └──────────────────────┘   └─────────────────────┘
```

## Resolution Tables

### Transform Resolution (from SourceConfig × SinkConfig)
| SourceConfig | SinkConfig | Resolves to |
|---|---|---|
| File | Elasticsearch | `RallyS3ToEs` |
| File | File | `Passthrough` |
| InMemory | InMemory | `Passthrough` |
| Elasticsearch | File | `Passthrough` |
| other | other | `panic!` at resolve time |

### Collector Resolution (from SinkConfig)
| SinkConfig | Collector | Wire Format |
|---|---|---|
| Elasticsearch | `NdjsonCollector` | `doc\ndoc\n` |
| File | `NdjsonCollector` | `doc\ndoc\n` |
| InMemory | `JsonArrayCollector` | `[doc,doc]` |

## Responsibility Boundaries

| Component | Responsibility |
|---|---|
| Source | Read data, return `Vec<String>`, strip newlines |
| Channel | Carry `Vec<String>` between workers |
| SinkWorker | Orchestrate: transform → collect → send |
| Transform | Per-doc conversion (Rally→ES bulk, passthrough) |
| Collector | Payload assembly (NDJSON newlines, JSON array brackets) |
| Sink | Pure I/O: HTTP POST, file write, memory push |

# Notes for future reference

- POC/MVP stage — API surface is unstable
- `Hit`/`HitBatch` in `common.rs` are now dead code — pipeline uses `Vec<String>` throughout
- Rally S3 transform strips 6 top-level metadata fields; nested refs survive
- ES bulk action line includes `_id` only; `_index`/`routing` set by sink URL
- Passthrough doesn't validate — non-JSON passes through. Intentional.
- `escape_json_string()` avoids serde round-trip for action line construction
- `channel_data.rs` still empty — to be repurposed or removed
- ES sink no longer buffers — SinkWorker handles all buffering
- Transforms and collectors are Clone+Copy (zero-sized structs) — each SinkWorker gets its own copy
- JsonArrayCollector uses zero serde — manual bracket/comma concatenation

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`
- v1 transforms: IngestTransform/EgressTransform + Hit intermediate — **superseded**
- v2 transforms: direct pair functions + dead traits — **superseded**
- v3 transforms: mirrors backends pattern. `Transform` trait → concrete struct impls → `DocumentTransformer` enum dispatch → `from_configs()` resolver
- v4 pipeline refactor: Sources return `Vec<String>`, Sinks are I/O-only (`send(payload)`), SinkWorker does transform + binary collect. Hit/HitBatch phased out of pipeline.
- v5 collectors (current): Extracted payload assembly into `PayloadCollector` trait + `NdjsonCollector`/`JsonArrayCollector`. NDJSON for ES/File, JSON array for InMemory. Zero serde. 27 tests passing.
- S3 source backend not yet implemented — ethos project reference not found on filesystem
