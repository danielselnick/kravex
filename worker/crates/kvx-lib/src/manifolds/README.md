

# Manifolds

## Vocabulary

| Term         | Definition                                                    |
| ------------ | ------------------------------------------------------------- |
| **Feed**     | Raw result barrel from a Source                                 |
| **Doc**      | Single sink-ready string produced by casting a feed           |
| **Drum**  | Joined docs in wire format, ready for the sink                |
| **Tapper**   | Stateless: one feed in, many docs out                         |
| **Manifold** | Stateful: casts feeds → docs, buffers docs, flushes drums |
| **Refiner**   | Worker: buffers feeds, drives the Manifold, forwards drums |

## Pipeline

```
[channel 1: feeds] → Refiner → Manifold → [channel 2: drums]
                        │          │
                        │          ├─ casts feeds → docs (via Tapper)
                        │          ├─ lazy iteration over feeds, drops the feed once all docs in feed are cast
                        │          
                        │
                        └─ buffers feeds by byte count
                           sends barrel through Tapper  
                           receives docs from Manifold
                           buffers docs by set point
                           extra docs are kept around until the next flush, and not included in the current drum
                           creates drums from docs
                           forwards drums to channel 2
```

## How It Works

1. **Refiner** accumulates feeds from channel 1 until a byte threshold is reached
2. **Refiner** passes the buffered feeds to the **Manifold**
3. **Manifold** casts each feed into docs (via the **Tapper**), adding them to its doc buffer
4. When the doc buffer reaches the setpoint, the Manifold flushes it as a drum
5. Leftover feeds and docs that didn't reach the setpoint stay in the Manifold (FIFO carry-over)
6. On the next call, carried-over state is processed first, then new feeds
7. When the source is exhausted, the Refiner triggers a final flush — all remaining docs drain as one last drum

## Resolution (SinkConfig → ManifoldBackend)

| Sink | Manifold | Wire Format |
|---|---|---|
| Elasticsearch | NdjsonManifold | `item\nitem\n` |
| File | NdjsonManifold | `item\nitem\n` |
| InMemory | JsonArrayManifold | `[item, item]` |

## Key Concepts

- **Two-level buffering:** Refiner buffers feeds, Manifold buffers docs
- **Both setpoints are dynamic** — read from FlowKnob, adjusted by backpressure
- **Manifold is stateful** — carries over unconsumed feeds and docs between calls
- **Tapper is stateless** — transforms only, no buffering or joining

## Knowledge Graph

```
Refiner ──buffers──→ feeds
Refiner ──flushes──→ feeds into Manifold
Manifold ──casts via──→ Tapper (feed → docs)
Manifold ──buffers──→ docs (stateful carry-over)
Manifold ──flushes──→ drum(s) at dynamic setpoint
Refiner ──forwards──→ drums to channel 2
FlowKnob ──controls──→ both Refiner + Manifold setpoints
Governor ──adjusts──→ FlowKnob (via regulator)
```
