# OpenObserve Backend

Sink-only backend for [OpenObserve](https://openobserve.ai), an observability platform with an ES-compatible bulk ingestion API.

## Concepts

- **Bulk API**: `POST /api/{org}/_bulk` — accepts NDJSON (action + data line pairs), identical to Elasticsearch's `_bulk` format.
- **Organization**: URL path component (`org`). Defaults to `"default"`.
- **Stream**: Target stream name, specified in bulk action lines via `_index`. Auto-created on first write.
- **Authentication**: Basic auth (username/password). No API key support.
- **Wire format**: NDJSON — reuses `NdjsonManifold`, no dedicated manifold needed.

## Caster Resolution

| Source | Caster | Rationale |
|--------|--------|-----------|
| File | `NdJsonToBulk` | Same bulk format as Elasticsearch |
| Elasticsearch | `PitToBulk` | Same bulk format as Elasticsearch |
| InMemory | `Passthrough` | Testing path |

## Configuration

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `url` | `String` | Yes | — | Base URL (e.g., `http://localhost:5080`) |
| `org` | `String` | No | `"default"` | Organization name |
| `stream` | `String` | Yes | — | Target stream name |
| `username` | `Option<String>` | No | `None` | Basic auth username |
| `password` | `Option<String>` | No | `None` | Basic auth password |
| `max_request_size_bytes` | `usize` | No | `10MB` | Max payload size per request |

## Module Structure

- `mod.rs` — Module root, re-exports
- `config.rs` — `OpenObserveSinkConfig` serde struct
- `open_observe_sink.rs` — `OpenObserveSink` implementation + unit tests
