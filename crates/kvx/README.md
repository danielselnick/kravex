# Summary

Core library for kravex — the data migration engine.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Edition**: 2024
- **Modules**: `backends` (Source/Sink traits + impls), `common` (Hit/HitBatch), `transforms` (IngestTransform/EgressTransform), `supervisors` (pipeline orchestration), `progress` (TUI metrics), `app_config` (Figment-based config)

## Module Dependency Graph
```
app_config ──► supervisors ──► backends ──► common
                    │                         ▲
                    ▼                         │
               transforms ───────────────────┘
```

# Key Concepts

- **Adaptive throttling**: 429 backoff/ramp
- **Zero-config optimization**: sensible defaults, optional overrides
- **Cutover management**: pause/resume/retry/validation
- **Intermediate format**: `Hit` struct IS the intermediate — all source formats convert TO it (IngestTransform), all sink formats convert FROM it (EgressTransform). Reduces N×N converters to 2N.
- **Monomorphized transforms**: zero-sized marker types (RallyS3Json, ElasticsearchBulk, RawJsonPassthrough) — compiler generates specialized code per format pair. No vtables.
- **Ethos pattern**: backend/format owns its own config and transform. Enums point at implementations, not the other way around.

## Transform Architecture
```
Source Formats       Intermediate        Sink Formats
┌──────────────┐    ┌──────────┐    ┌──────────────┐
│ Rally S3 JSON│─┐  │   Hit    │  ┌─│ ES Bulk API  │
│ ES Dump      │─┼─▶│ id,index │─▶├─│ JSON Lines   │
│ Raw JSON     │─┘  │ routing  │  └─│ S3 Objects   │
└──────────────┘    │ source_buf│    └──────────────┘
N IngestTransforms  └──────────┘    N EgressTransforms
                Total: 2N (not N²)
```

# Notes for future reference

- POC/MVP stage — API surface is unstable and expected to change
- Public config groups execution knobs under `[runtime]`
- `Hit` struct (common.rs) serves dual purpose: pipeline data carrier AND intermediate transform format
- Transforms operate on individual Hits, not batches — batch-level optimization can be added later
- Rally S3 ingest strips 6 metadata fields (_rallyAPIMajor/Minor, _ref, _refObjectUUID, _objectVersion, _CreatedAt)
- ES bulk egress produces two-line NDJSON (action + source), no trailing newline (sink handles that)
- Passthrough transform exists for testing and raw file-to-file copies

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`, no public API defined yet
- Transform layer added: `IngestTransform` and `EgressTransform` traits with 3 implementations (RallyS3Json, ElasticsearchBulk, RawJsonPassthrough)
- Transforms are NOT yet wired into the Source/Sink pipeline — next step is integrating them into SourceWorker/SinkWorker
- S3 source backend not yet implemented — need to find ethos project reference for download patterns
- `channel_data.rs` still empty — could be repurposed or removed
- 19 tests passing, including full pipeline integration test (Rally JSON → Hit → ES Bulk)
