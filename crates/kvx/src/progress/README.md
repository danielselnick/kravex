# progress/

Mission Control for migration visibility. Real-time pipeline metrics rendered to the terminal.

## Module Structure

| File | Role |
|---|---|
| `mod.rs` | Thin API surface. `spawn_progress_reporter()` wires config → pollers → reporter. |
| `renderer.rs` | `ProgressReporter` — sliding-window rate calculation, comfy-table rendering, cluster snapshot lifecycle. |
| `cluster_stats.rs` | `ClusterStatsPoller` — async ES/OS node stats fetcher (`_nodes/stats/os,jvm`). |

## Key Concepts

- **DrainMetrics** — Lock-free atomic counters shared between N drainer tasks and one reporter task. Writers call `record_drain()`, reader loads values every 500ms tick. Relaxed ordering.
- **Sliding Window** — 5-second VecDeque of `(timestamp, bytes, docs)` samples. Rates computed as delta between newest and oldest sample in window.
- **ClusterStatsPoller** — Stateless value object holding URL, auth, and HTTP client. `fetch()` spawns a background tokio task, returns `JoinHandle<Result<ClusterSnapshot>>` immediately.
- **Render-Kicked Polling** — No persistent background task for cluster stats. The renderer's `tick()` method kicks off a new fetch when no fetch is in-flight, and harvests completed results via `is_finished()` + `.await`.
- **ClusterSnapshot** — Two numbers: `cpu_percent` and `jvm_heap_percent`, averaged across all reporting nodes.
- **ClusterAuth** — Tri-modal: `Basic`, `ApiKey`, or `None`. Resolved from ES config at poller construction.

## Display Layout

Classic 2-column mode (no ES clusters):
```
sink: <pipeline>
  <docs/min>       <~total docs>
  <MiB/s>          <cumulative bytes>
  <avg latency>    <last latency>
  <avg req size>   <last req size>
  <elapsed>        <remaining>
| [=====>----------]
```

4-column mode (ES source and/or sink):
```
sink: <pipeline>
  <docs/min>       <~total docs>     source     sink
  <MiB/s>          <cumulative>    CPU  23%   CPU  67%
  <avg latency>    <last latency>  MEM  41%   MEM  58%
  <avg req size>   <last req size>
  <elapsed>        <remaining>
| [=====>----------]
```

## Knowledge Graph

- `DrainMetrics` ← written by `workers/drainer.rs`, read by `progress/renderer.rs`
- `spawn_progress_reporter()` ← called by `foreman.rs`, receives `&AppConfig`
- `ClusterStatsPoller` ← constructed from `ElasticsearchSourceConfig` / `ElasticsearchSinkConfig` fields
- `ClusterSnapshot` ← parsed from ES/OS `_nodes/stats/os,jvm` JSON response
- `ProgressBar` (indicatif) ← owned by `ProgressReporter`, aborted by foreman after workers complete
