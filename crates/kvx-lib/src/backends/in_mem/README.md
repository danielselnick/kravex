

# In-Memory Backend

In-memory Source and Sink implementations for testing.

## Source

Vec-backed source that yields pre-loaded barrels via a `VecDeque<Barrel>`. Pages are popped front on each `pump()` call (FIFO). When the queue is empty, returns `None`.

### Constructors

- **`new()`** — Async. Loads the classic 4-doc sacred corpus (`{"doc":1}` through `{"doc":4}`) as a single newline-delimited barrel. Backward-compatible default.
- **`with_barrels(barrels: Vec<Barrel>)`** — Sync. Accepts arbitrary barrel data for injection into the pipeline. Enables integration tests that exercise specific tapper paths (e.g., ES PIT responses for PitToBulk, NDJSON feeds for NdJsonToBulk) without needing real backends.

## Sink

Vec-backed sink that collects drained drums behind `Arc<Mutex<Vec<String>>>`. Used in unit tests to capture output for assertion. Cloneable — clone before passing to the pipeline, then inspect the clone after the pipeline completes.

## Key Concepts

- **Test-only**: Not intended for production use
- **Deterministic**: No I/O, no network, no filesystem
- **Format-agnostic source**: `with_barrels()` accepts any barrel format — the tapper/manifold resolution determines how barrels are processed
- **Paired with JsonArrayManifold**: When used as a sink via `SinkConfig::InMemory`, resolves to JsonArrayManifold for drum assembly

## Knowledge Graph

```
InMemorySource → Source trait → SourceBackend::InMemory
InMemorySource::new() → 4-doc sacred corpus (single barrel)
InMemorySource::with_barrels() → custom barrel injection (multi-barrel)
InMemorySink → Sink trait → SinkBackend::InMemory
InMemory backends → used by integration tests (InMemory→InMemory, ES→ES in-memory)
with_barrels() + ES SourceConfig → PitToBulk tapper path (ES→ES integration test)
```
