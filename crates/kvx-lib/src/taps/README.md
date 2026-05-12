

# Taps

Feed format transformation layer. A Tapper converts raw feeds from a Source into sink-ready documents.

## Trait

| Trait | Method | Returns | Purpose |
|---|---|---|---|
| `Tapper` | `tap(barrel)` | `Result<Vec<Draft>>` | Transform one barrel into sink-ready drafts |

## Dispatcher Enum

`BarrelToDraftsTapper` — routes to concrete tapper based on source/sink config combination.

## Concrete Tappers

| Tapper | Source → Sink | Transformation |
|---|---|---|
| `Passthrough` | Any → same format | Identity — feed passes through unchanged |
| `NdJsonToBulk` | File → Elasticsearch | Wraps each NDJSON line with a `_bulk` action line |
| `PitToBulk` | Elasticsearch → Elasticsearch | Extracts hits from PIT search response, emits `_bulk` NDJSON |

## Resolution

Tapper selection is determined by the **source x sink config** combination at startup via `from_configs()`.

## Key Concepts

- **Stateless**: Tappers hold no state — pure transformation
- **Zero-sized**: All tappers are zero-sized structs (Clone + Copy for free)

## Knowledge Graph

```
Tapper trait → BarrelToDraftsTapper enum → Passthrough | NdJsonToBulk | PitToBulk
BarrelToDraftsTapper → resolved by from_configs(SourceConfig, SinkConfig)
Tapper → consumed by Manifold during join()
```
