# Summary

Core library for kravex — the data migration engine.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Edition**: 2024
- **Modules**: `backends` (Source/Sink traits + impls), `common` (Hit/HitBatch — legacy dead code), `transforms` (Transform trait + impls, same pattern as backends), `supervisors` (pipeline orchestration), `progress` (TUI metrics), `app_config` (Figment-based config)

## Pipeline Architecture (current)
```
Source::next_batch() → Vec<String> → channel → SinkWorker → transform → binary collect → Sink::send(payload)
```

## Module Dependency Graph
```
lib.rs ──► supervisors ──► backends (Source/Sink traits)
  │              │
  │              ▼
  │         workers (SourceWorker, SinkWorker)
  │              │
  ▼              ▼
transforms ◄── SinkWorker (holds DocumentTransformer)
```

# Key Concepts

- **Sources return `Vec<String>`**: raw document strings, no Hit wrappers, no trailing newlines
- **Sinks are I/O-only**: accept a fully rendered payload `String`, send it (HTTP POST, file write, memory push)
- **SinkWorker does the work**: receives `Vec<String>` from channel, transforms each doc via `DocumentTransformer`, binary-collects into single payload with `\n` delimiters, sends to Sink
- **Binary collect**: each transformed string gets trailing `\n`, concatenated into one payload. For ES bulk: "action\nsource\naction\nsource\n". For passthrough: "doc\ndoc\n".
- **Transform pattern mirrors backends pattern exactly**:
  - `Transform` trait (like `Source`/`Sink`)
  - Concrete impls: `RallyS3ToEs`, `Passthrough` (like `FileSource`, `InMemorySink`)
  - `DocumentTransformer` enum wraps them (like `SourceBackend`/`SinkBackend`)
  - Enum impls `Transform` via match dispatch (static dispatch inside each arm)
  - `from_configs(SourceConfig, SinkConfig)` resolves the transformer
- **Ethos pattern**: backend/format owns its own config, struct, and impl

## Transform Architecture (mirrors backends.rs)
```
backends.rs pattern:               transforms.rs pattern:
┌──────────────────┐              ┌──────────────────────┐
│ trait Source      │              │ trait Transform       │
│   fn next_batch() │              │   fn transform()     │
└────────┬─────────┘              └────────┬─────────────┘
         │                                 │
┌────────┴─────────┐              ┌────────┴─────────────┐
│ FileSource       │              │ RallyS3ToEs          │
│ InMemorySource   │              │ Passthrough          │
│ ElasticsearchSrc │              │ (more as needed)     │
└────────┬─────────┘              └────────┬─────────────┘
         │                                 │
┌────────┴─────────┐              ┌────────┴─────────────┐
│ enum SourceBackend│              │ enum DocumentTransfmr│
│   impl Source     │              │   impl Transform     │
│   match dispatch  │              │   match dispatch      │
└──────────────────┘              └──────────────────────┘
```

## Transform Resolution (from existing config enums)

| SourceConfig | SinkConfig | Resolves to |
|---|---|---|
| File | Elasticsearch | `RallyS3ToEs` |
| File | File | `Passthrough` |
| InMemory | InMemory | `Passthrough` |
| Elasticsearch | File | `Passthrough` |
| other | other | `panic!` at resolve time |

## Responsibility Boundaries

| Component | Responsibility |
|---|---|
| Source | Read data, return `Vec<String>`, strip newlines |
| Channel | Carry `Vec<String>` between workers |
| SinkWorker | Transform + buffer + binary collect + send to sink |
| Transform | Convert raw doc string → sink wire format |
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
- Transforms are Clone+Copy (zero-sized structs) — each SinkWorker gets its own copy

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`
- v1 transforms: IngestTransform/EgressTransform + Hit intermediate — **superseded**
- v2 transforms: direct pair functions + dead traits — **superseded**
- v3 transforms: mirrors backends pattern. `Transform` trait → concrete struct impls → `DocumentTransformer` enum dispatch → `from_configs()` resolver
- v4 pipeline refactor (current): Sources return `Vec<String>`, Sinks are I/O-only (`send(payload)`), SinkWorker does transform + binary collect. Hit/HitBatch phased out of pipeline. 18 tests passing.
- S3 source backend not yet implemented — ethos project reference not found on filesystem
