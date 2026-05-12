

# Workers

Pipeline execution stages. Three worker types form the data flow pipeline.

## Worker Types

| Worker | Runtime | Role | I/O Model |
|---|---|---|---|
| **Pumper** | tokio (async) | Reads feeds from Source into ch1 | Async I/O bound |
| **Refiner** | std::thread (sync) | Casts + joins feeds into drums | CPU bound |
| **Drainer** | tokio (async) | Writes drums from ch2 to Sink | Async I/O bound |

## Pipeline Flow

```
Source → Pumper → [ch1] → Refiner → [ch2] → Drainer → Sink
                                                ↻ retry with backoff
```

- **ch1**: Bounded async_channel carrying raw feeds (String)
- **ch2**: Bounded async_channel carrying assembled drums (Drum)

## Traits

| Trait | Method | Returns | Purpose |
|---|---|---|---|
| `Worker` | `start()` | `JoinHandle<Result<()>>` | Spawn the worker as an async task |

Note: Refiner does NOT implement Worker — it uses std::thread, not tokio tasks.

## Shutdown Cascade

Pumper completes → ch1 closes → Refiners flush and exit → ch2 closes → Drainers exit

## Retry & Backoff

Drainer retries failed `sink.drain()` calls with configurable exponential backoff.

| Config Field | Default | Description |
|---|---|---|
| `max_retries` | 3 | Maximum retry attempts after initial failure |
| `initial_backoff_ms` | 1000 | Base backoff duration in milliseconds |
| `backoff_multiplier` | 2.0 | Exponential multiplier per retry |
| `max_backoff_ms` | 30000 | Ceiling for backoff duration |

TOML section `[drainer]` — optional, defaults apply when absent.

Backoff formula: `min(initial_backoff_ms * multiplier^attempt, max_backoff_ms)`

Total attempts = 1 (initial) + max_retries. All errors are retried uniformly; granular classification planned for a future release.

## Key Concepts

- **Three-stage separation**: Async I/O (pump) → sync CPU (cast+join) → async I/O (drain)
- **Drainer is thin + resilient**: Relay with retry — recv from ch2, send to sink with backoff
- **DrainMetrics**: Shared `Arc<DrainMetrics>` passed to Drainer constructor. After each successful `drain_with_retry`, Drainer calls `drain_metrics.record_drain(drum_bytes, latency_ms)` to atomically update shared progress counters. Separate from `gauge_tx` (Governor feedback) — this is for progress reporting
- **Refiner is stateful**: Buffers feeds by byte count, flushes the Manifold output

## Knowledge Graph

```
Foreman → spawns Pumper (1) + Refiner (N) + Drainer (N)
Pumper → Source.pump() → ch1
Refiner → ch1 → Tapper + Manifold → ch2
Drainer → ch2 → Sink.drain() with exponential backoff retry
Drainer → Arc<DrainMetrics> (progress reporting, atomic counters)
Drainer → gauge_tx (Governor latency feedback, separate concern)
Drainer config → DrainerConfig (workers/config.rs)
Refiner parallelism → RuntimeConfig.refiner_parallelism
Drainer parallelism → RuntimeConfig.sink_parallelism
```
