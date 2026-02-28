# Summary

Core library for kravex — the data migration engine.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Edition**: 2024
- **Modules**: `backends` (Source/Sink traits + impls), `common` (Hit/HitBatch — legacy, pipeline carrier), `transforms` (Transform trait + impls, same pattern as backends), `supervisors` (pipeline orchestration), `progress` (TUI metrics), `app_config` (Figment-based config)

## Module Dependency Graph
```
app_config ──► supervisors ──► backends ──► common
                    │
                    ▼
               transforms (mirrors backends pattern)
```

# Key Concepts

- **Adaptive throttling**: 429 backoff/ramp
- **Zero-config optimization**: sensible defaults, optional overrides
- **Cutover management**: pause/resume/retry/validation
- **Transform pattern mirrors backends pattern exactly**:
  - `Transform` trait (like `Source`/`Sink`)
  - Concrete impls: `RallyS3ToEs`, `Passthrough` (like `FileSource`, `InMemorySink`)
  - `DocumentTransformer` enum wraps them (like `SourceBackend`/`SinkBackend`)
  - Enum impls `Transform` via match dispatch (static dispatch inside each arm)
  - `from_configs(SourceConfig, SinkConfig)` resolves the transformer (like `from_source_config()`)
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

# Notes for future reference

- POC/MVP stage — API surface is unstable
- `Hit` struct (common.rs) still used by backends/workers pipeline — separate concern from transforms
- Rally S3 transform strips 6 top-level metadata fields; nested refs survive
- ES bulk action line includes `_id` only; `_index`/`routing` set by sink
- Passthrough doesn't validate — non-JSON passes through. Intentional.
- `escape_json_string()` avoids serde round-trip for action line construction
- `channel_data.rs` still empty — to be repurposed or removed

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`
- v1 transforms: IngestTransform/EgressTransform + Hit intermediate — **superseded**
- v2 transforms: direct pair functions + dead traits — **superseded**
- v3 transforms (current): mirrors backends pattern. `Transform` trait → concrete struct impls → `DocumentTransformer` enum dispatch → `from_configs()` resolver. Clean. Consistent. Same pattern throughout codebase.
- 18 tests passing
- S3 source backend not yet implemented — ethos project reference not found on filesystem
