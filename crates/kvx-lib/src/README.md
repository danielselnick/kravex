

# kvx Source Root

Core library source for kravex — the zero-config search migration engine.

## Module Map

| Module | Purpose |
|---|---|
| `config` | Configuration hierarchy — AppConfig, RuntimeConfig, SourceConfig, SinkConfig |
| `backends` | I/O abstraction — Source/Sink traits, backend-specific implementations |
| `taps` | Feed transformation — Tapper trait, format conversion between source and sink |
| `manifolds` | Drum assembly — cast feeds into docs, buffer and flush as wire-format drums |
| `workers` | Pipeline stages — Pumper (async read), Joiner (sync CPU), Drainer (async write) |
| `regulators` | Adaptive throttling — PID controller, pressure gauges, flow control |
| `foreman` | Orchestration — spawns and joins all pipeline workers |
| `progress` | TUI metrics and progress reporting |
| `lib.rs` | Entry point — wires up config, regulators, foreman, shutdown |

## Pipeline Vocabulary

| Term | Definition |
|---|---|
| **Feed** | Raw result barrel from a Source |
| **Cast** | Transform a feed into sink-ready doc(s) |
| **Drum** | Wire-format string ready for the sink |
| **Pump** | Read the next feed from a source |
| **Drain** | Write a drum to a sink |
| **Lint hygiene** | Keep module contracts and docs aligned with static analysis expectations |

## Architecture

```
Source.pump() → ch1 → Joiner(cast+join) → ch2 → Drainer(drain)
```

Three-stage pipeline: async I/O → sync CPU → async I/O. Channels are bounded async_channel (MPMC).

## Knowledge Graph

```
lib.rs → AppConfig → Foreman → Workers (Pumper, Joiner, Drainer)
lib.rs → Regulators → Manometer + Governor → FlowKnob
Foreman → Source (via Pumper), Sink (via Drainer)
Joiner → Tapper + Manifold (cast feeds, assemble drums)
Quality loop → Compiler + Lints + Tests → stable migration behavior
```
