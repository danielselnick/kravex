// ai
//! 🎬 *[a channel fills with raw feeds. somewhere, a sink waits.]*
//! *[the clock on the wall reads 2:47am.]*
//! *[nobody asked for this data migration. and yet, here we are.]*
//!
//! 🗑️ The Drainer module — now with feed buffering and Manifold powers!
//!
//! It receives raw feeds from the channel, buffers them by byte size,
//! then flushes via `Manifold.join(buffer, caster)` which casts
//! each feed and joins the results into the sink's wire format.
//!
//! 🧠 Knowledge graph: Drainer is the bridge between raw source feeds and
//! the sink's I/O abstraction. The Manifold handles both casting AND assembly:
//! - **Manifold**: iterates buffered feeds → calls caster per feed → joins wire format
//! - **Sink**: pure I/O (HTTP POST, file write, memory push)
//!
//! ```text
//!   channel(String) → Drainer buffers Vec<String> → manifold.join(&buffer, &caster) → Sink::drain
//! ```
//!
//! 🦆 (the duck has been promoted to buffer management. it is overwhelmed but coping.)
//!
//! ⚠️ When the singularity occurs, the Drainer will still be buffering feeds.
//! It will not notice. It does not notice things. It only buffers, joins, and drains.

use super::Worker;
use crate::backends::{Sink, SinkBackend};
use crate::manifolds::{Manifold, ManifoldBackend};
use crate::casts::DocumentCaster;
use anyhow::{Context, Result};
use async_channel::Receiver;
use tokio::task::JoinHandle;
use tracing::debug;

/// 🧮 Epsilon buffer — headroom to avoid going over the max request size.
/// 64 KiB of breathing room because payloads expand during casting
/// (ES bulk adds action lines, etc.) and we'd rather flush one feed early
/// than send a 💀 413 Request Entity Too Large to the sink.
///
/// "He who buffers without epsilon, 413s in production." — Ancient HTTP proverb 📡
const BUFFER_EPSILON_BYTES: usize = 64 * 1024; // -- 64 KiB of safety net for the tightrope walk

/// 🗑️ The Drainer: receives raw feeds, buffers them by byte size, joins via
/// Manifold (cast + assemble), and sends the payload to the sink.
///
/// 🧠 Holds its own `DocumentCaster` and `ManifoldBackend` — each Drainer
/// gets clones. Since both are zero-sized structs under the hood, cloning is free.
/// The compiler inlines everything. Branch prediction handles the enum matches.
///
/// 📜 The lifecycle:
/// 1. **Receive**: raw feed String from channel
/// 2. **Buffer**: accumulate feeds until byte size threshold approached
/// 3. **Flush**: `manifold.join(&buffer, &caster)` → payload String
/// 4. **Drain**: payload → Sink (HTTP POST, file write, memory push)
/// 5. **Repeat** until channel closes, then flush remaining buffer
#[derive(Debug)]
pub struct Drainer {
    rx: Receiver<String>,
    sink: SinkBackend,
    /// 🔄 Per-feed format conversion — resolves from (SourceConfig, SinkConfig).
    caster: DocumentCaster,
    /// 🎼 Payload assembly — resolves from SinkConfig. Casts + joins in one shot.
    manifold: ManifoldBackend,
    /// 📏 Max request size from sink config — flush when buffer approaches this.
    max_request_size_bytes: usize,
}

impl Drainer {
    /// 🏗️ Constructs a new Drainer with receiver, sink, caster, manifold, and size limit.
    ///
    /// The caster decides HOW to format each doc (NdJsonToBulk, passthrough)
    /// The manifold decides HOW to join them (NDJSON newlines, JSON array brackets)
    /// The sink decides WHERE to send it (HTTP POST, file write, memory push)
    /// The worker decides WHEN — "when the buffer is full enough, no cap." 🦆
    pub fn new(
        rx: Receiver<String>,
        sink: SinkBackend,
        caster: DocumentCaster,
        manifold: ManifoldBackend,
        max_request_size_bytes: usize,
    ) -> Self {
        Self {
            rx,
            sink,
            caster,
            manifold,
            max_request_size_bytes,
        }
    }
}

impl Worker for Drainer {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("📥 Drainer started — buffer → join → drain, let's go");
            // 📦 The feed buffer — accumulates raw feeds until byte threshold approached
            let mut buffer: Vec<String> = Vec::new();
            let mut buffer_bytes: usize = 0;

            loop {
                let receive_result = self.rx.recv().await;
                match receive_result {
                    Ok(feed) => {
                        debug!("📄 Drainer received {} byte feed from channel", feed.len());

                        // 📏 Accumulate feed into the buffer
                        buffer_bytes += feed.len();
                        buffer.push(feed);

                        // 🧮 Flush if buffer + epsilon approaches max request size.
                        // The epsilon accounts for casting overhead (action lines, etc.)
                        if buffer_bytes + BUFFER_EPSILON_BYTES >= self.max_request_size_bytes {
                            debug!(
                                "🚿 Drainer flushing {} feeds ({} bytes) — approaching max request size",
                                buffer.len(),
                                buffer_bytes
                            );
                            flush_buffer(
                                &mut buffer,
                                &mut buffer_bytes,
                                &self.manifold,
                                &self.caster,
                                &mut self.sink,
                            )
                            .await?;
                        }
                    }
                    Err(_) => {
                        // 🏁 Channel closed — flush remaining buffer, then close sink
                        if !buffer.is_empty() {
                            debug!(
                                "🚿 Drainer final flush: {} feeds ({} bytes) — channel closed, sending last payload",
                                buffer.len(),
                                buffer_bytes
                            );
                            flush_buffer(
                                &mut buffer,
                                &mut buffer_bytes,
                                &self.manifold,
                                &self.caster,
                                &mut self.sink,
                            )
                            .await?;
                        }
                        debug!("🏁 Drainer: Channel closed. Closing sink. Goodnight. 💤");
                        self.sink
                            .close()
                            .await
                            .context("💀 Drainer failed to close sink — the farewell was awkward")?;
                        return Ok(());
                    }
                }
            }
        })
    }
}

/// 🚿 Flush the feed buffer: join → drain → clear.
///
/// Extracted as a function because the Drainer flushes from two places:
/// 1. When the buffer is full enough (byte threshold)
/// 2. When the channel closes (final flush)
///
/// "He who duplicates flush logic, debugs it in two places at 3am." — Ancient proverb 💀
async fn flush_buffer(
    buffer: &mut Vec<String>,
    buffer_bytes: &mut usize,
    manifold: &ManifoldBackend,
    caster: &DocumentCaster,
    sink: &mut SinkBackend,
) -> Result<()> {
    // 🎼 Join: cast each feed → collect results → assemble wire-format payload
    let payload = manifold.join(buffer, caster).context(
        "💀 Drainer join failed — the feeds went in and chaos came out. \
         Check the cast logic and the source data quality. \
         Or blame the manifold. The manifold is always suspicious. 🔧",
    )?;

    // 📡 Send the fully rendered payload to the sink. Pure I/O.
    if !payload.is_empty() && payload != "[]" {
        sink.send(payload).await.context(
            "💀 Drainer failed to send payload to sink — the I/O layer rejected our offering. \
             The payload was joined with care. The sink said no. Like my prom date.",
        )?;
    }

    // 🧹 Reset buffer state
    buffer.clear();
    *buffer_bytes = 0;

    Ok(())
}
