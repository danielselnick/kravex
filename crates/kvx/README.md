# Summary

Core library for kravex — the data migration engine.

# Description

`kvx` provides the foundational primitives for search migration: throttling, cutover logic, retry/recovery, and adaptive throughput. This crate is consumed by `kvx-cli` and any future integrations.

# Knowledge Graph

- **Workspace member**: `crates/kvx`
- **Dependents**: `kvx-cli`
- **Dependencies**: anyhow, async-channel, figment, reqwest, serde, serde_json, tokio, tracing, async-trait, futures, indicatif, comfy-table
- **Edition**: 2024
- **Modules**: `backends` (Source/Sink traits + impls), `common` (Hit/HitBatch — legacy, being phased out), `transforms` (direct pair transforms), `supervisors` (pipeline orchestration), `progress` (TUI metrics), `app_config` (Figment-based config)

## Module Dependency Graph
```
app_config ──► supervisors ──► backends ──► common
                    │
                    ▼
               transforms (standalone, no common dependency)
```

# Key Concepts

- **Adaptive throttling**: 429 backoff/ramp
- **Zero-config optimization**: sensible defaults, optional overrides
- **Cutover management**: pause/resume/retry/validation
- **Direct pair transforms**: N×N dedicated converters. Each (input, output) pair gets its own `#[inline]` function. No intermediate struct. `String` in, `String` out.
- **Three transform traits**: `InputFormat` (marker for sources), `OutputFormat` (marker for sinks), `Transform` (`fn transform(&self, String) -> Result<String>`)
- **DocumentTransformer enum**: resolved once from `(InputFormatType, OutputFormatType)` at startup. Dispatches via match in hot loop. Branch predictor handles the rest.
- **Unimplemented pairs panic at resolve time** — fail loud at startup, not silent in production.
- **Ethos pattern**: backend/format owns its own config and transform.

## Transform Architecture
```
InputFormat                      OutputFormat
┌──────────────┐                ┌──────────────┐
│ RallyS3Json  │───────────────▶│ ES Bulk API  │  rally_s3_to_es.rs
│ RawJson      │───────────────▶│ RawJson      │  passthrough.rs (zero-copy)
│ ES Dump      │──── panic! ───▶│ JsonLines    │  not yet implemented
└──────────────┘                └──────────────┘
Each arrow = one dedicated, inlined function. No intermediate struct.
```

## Implemented Transform Pairs

| Input | Output | Module | Notes |
|-------|--------|--------|-------|
| RallyS3Json | ElasticsearchBulk | `rally_s3_to_es.rs` | Extracts ObjectID→_id, strips 6 metadata fields, produces NDJSON |
| RawJson | RawJson | `passthrough.rs` | Zero-copy `Ok(raw)` — ownership transfer, no allocation |
| RawJson | JsonLines | `passthrough.rs` | Same as above |

# Notes for future reference

- POC/MVP stage — API surface is unstable and expected to change
- Public config groups execution knobs under `[runtime]`
- `Hit` struct (common.rs) still used by backends/workers pipeline — will be phased out as transforms take over
- Rally S3 transform strips 6 top-level metadata fields; nested refs (Project._ref) survive
- ES bulk action line includes `_id` only; `_index` set by sink URL or future config layer
- Passthrough doesn't validate input — not-JSON passes through. This is intentional.
- `escape_json_string()` in rally_s3_to_es.rs avoids serde round-trip for simple action lines
- `channel_data.rs` still empty — to be repurposed or removed

# Aggregated Context Memory Across Sessions for Current and Future Use

- Initial scaffold: empty `lib.rs`, no public API defined yet
- v1 transform layer: IngestTransform/EgressTransform traits + Hit intermediate (2N approach) — **superseded**
- v2 transform layer: direct pair transforms via DocumentTransformer enum dispatch (N×N approach). No Hit intermediate. `String` in, `String` out. Zero-copy passthrough.
- Transforms NOT yet wired into Source/Sink pipeline — next step: integrate into SourceWorker/SinkWorker
- S3 source backend not yet implemented — ethos project reference not found on filesystem
- 20 tests passing, including integration tests and panic test for unimplemented pairs
