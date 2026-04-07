

# Casts

Feed format transformation layer. A Caster converts raw feeds from a Source into sink-ready documents.

## Trait

| Trait | Method | Returns | Purpose |
|---|---|---|---|
| `Caster` | `cast(page)` | `Result<Vec<Entry>>` | Transform one page into sink-ready entries |

## Dispatcher Enum

`PageToEntriesCaster` — routes to concrete caster based on source/sink config combination.

## Concrete Casters

| Caster | Source → Sink | Transformation |
|---|---|---|
| `Passthrough` | Any → same format | Identity — feed passes through unchanged |
| `NdJsonToBulk` | File → Elasticsearch | Wraps each NDJSON line with a `_bulk` action line |
| `NdJsonSplit` | File → Meilisearch | Splits NDJSON lines into individual entries (no bulk headers) |
| `PitToBulk` | Elasticsearch → Elasticsearch | Extracts hits from PIT search response, emits `_bulk` NDJSON |
| `PitToJson` | Elasticsearch → Meilisearch | Extracts `_source` from PIT hits as raw JSON entries (no bulk headers) |
| `NdJsonToBulk` | File → OpenObserve | Wraps each NDJSON line with a `_bulk` action line |
| `PitToBulk` | Elasticsearch → OpenObserve | Extracts hits from PIT search response, emits `_bulk` NDJSON |
| `Passthrough` | InMemory → OpenObserve | Identity — feed passes through unchanged |

## Resolution

Caster selection is determined by the **source x sink config** combination at startup via `from_configs()`.

## Key Concepts

- **Stateless**: Casters hold no state — pure transformation
- **Zero-sized**: All casters are zero-sized structs (Clone + Copy for free)

## Knowledge Graph

```
Caster trait → PageToEntriesCaster enum → Passthrough | NdJsonToBulk | NdJsonSplit | PitToBulk | PitToJson
PageToEntriesCaster → resolved by from_configs(SourceConfig, SinkConfig)
OpenObserve sink → reuses NdJsonToBulk, PitToBulk, Passthrough (ES-compatible bulk API)
Caster → consumed by Manifold during join()
```
