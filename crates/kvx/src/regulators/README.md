
# Regulators

Adaptive throttling via feedback control. Regulators dynamically adjust payload sizing based on sink pressure signals and throughput measurements.

## Vocabulary

| Term | Definition |
|---|---|
| **Regulator** | Controls a value based on feedback signals |
| **FlowKnob** | Shared atomic value read by Joiner to size payloads |
| **FlowMaster** | Consumer of GaugeReading signals — drives the regulator, adjusts the FlowKnob |
| **GaugeReading** | Signal from the pipeline: `CpuValue`, `LatencyMs`, `DrainResult`, or `Error` |
| **DrainResult** | Gauge signal carrying `payload_bytes` and `latency_ms` from a completed drain |

## Trait

| Trait | Method | Returns | Purpose |
|---|---|---|---|
| `Regulate` | `regulate(reading, dt)` | `f64` | Given a GaugeReading and time delta, return the desired payload size in bytes |

## Dispatcher Enum

`Regulators` — routes to concrete regulator based on config. Variants: `Static`, `CpuPressure`, `ThroughputSeeker`.

## Concrete Regulators

| Regulator | Behavior | Config |
|---|---|---|
| `ByteValue` (Static) | Returns a fixed value — no regulation | `StaticRegulatorConfig` |
| `CpuPressure` (PID) | PID controller targeting a CPU or latency setpoint | `CpuRegulatorConfig` / `LatencyRegulatorConfig` |
| `ThroughputSeeker` (Hill Climbing) | Directly optimizes bytes/sec via dual-system adaptive search | `ThroughputSeekerConfig` |

## Signal Flow

```
Drainer (drain complete) → GaugeReading::DrainResult { payload_bytes, latency_ms } → FlowMaster → Regulator → FlowKnob
Drainer (error/429)      → GaugeReading::Error() → FlowMaster → Regulator → FlowKnob
Manometer (polls sink)   → GaugeReading::CpuValue(cpu_percent) → FlowMaster → Regulator → FlowKnob
```

## Key Concepts

- **ThroughputSeeker**: Dual-system design — fast circuit breaker (per-reading) + slow hill climber (5s windows)
- **Circuit Breaker**: Dual EMA crossover — fast EMA drops 20% below slow EMA → immediate halve + cooldown
- **Hill Climbing**: Windowed median comparison — step forward on improvement, reverse + shrink (×0.618) on worsening
- **Convergence**: Step size shrinks below 64 KiB → seeker holds position. Re-explores after 30 settled windows.
- **PID Controller**: Proportional-Integral-Derivative feedback loop (legacy, for CPU/latency setpoints)
- **EMA Smoothing**: Exponential moving average dampens noise in both PID and circuit breaker
- **Auto-tuned Gains**: PID gains derived from min/max payload size ratio
- **FlowKnob is atomic**: Lock-free reads from hot-path workers

## Knowledge Graph

```
Regulate trait → Regulators enum → ByteValue | CpuPressure | ThroughputSeeker
Drainer → sends DrainResult or Error via async_channel to FlowMaster
FlowMaster → receives GaugeReading → runs Regulator → writes FlowKnob
FlowKnob → read by Joiner for dynamic payload sizing
ThroughputSeeker → System 1 (circuit breaker, every reading) + System 2 (hill climber, 5s windows)
TOML → [flow_master.Throughput] | [flow_master.Latency] | [flow_master.CPU] | [flow_master.Static]
```
