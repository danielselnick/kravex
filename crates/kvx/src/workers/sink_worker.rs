// ai
//! 🎬 *[a channel fills with raw pages. somewhere, a sink waits.]*
//! *[the clock on the wall reads 2:47am.]*
//! *[nobody asked for this data migration. and yet, here we are.]*
//!
//! 🗑️ The SinkWorker module — now with page buffering and Composer powers!
//!
//! It receives raw pages from the channel, buffers them by byte size,
//! then flushes via `Composer.compose(buffer, transformer)` which transforms
//! each page and assembles the items into the sink's wire format.
//!
//! 🧠 Knowledge graph: SinkWorker is the bridge between raw source pages and
//! the sink's I/O abstraction. The Composer handles both transformation AND assembly:
//! - **Composer**: iterates buffered pages → calls transformer per page → assembles wire format
//! - **Sink**: pure I/O (HTTP POST, file write, memory push)
//!
//! ```text
//!   channel(String) → SinkWorker buffers Vec<String> → composer.compose(&buffer, &transformer) → Sink::send
//! ```
//!
//! 🐄 The Cow lives here (spiritually). The Composer calls `transformer.transform(page)` which
//! returns `Vec<Cow<str>>` — borrowed for passthrough, owned for format conversion. Zero-copy
//! when source format == sink format. The dream. The whole point. The Cow. 🐄
//!
//! 🦆 (the duck has been promoted to buffer management. it is overwhelmed but coping.)
//!
//! ⚠️ When the singularity occurs, the SinkWorker will still be buffering pages.
//! It will not notice. It does not notice things. It only buffers, composes, and sinks.

use super::Worker;
use crate::backends::{Sink, SinkBackend};
use crate::composers::{Composer, ComposerBackend};
use crate::throttlers::{ThrottleController, ThrottleControllerBackend};
use crate::transforms::DocumentTransformer;
use anyhow::{Context, Result};
use async_channel::Receiver;
use std::time::Instant;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;

/// 🧮 Epsilon buffer — headroom to avoid going over the max request size.
/// 64 KiB of breathing room because payloads expand during transformation
/// (ES bulk adds action lines, etc.) and we'd rather flush one page early
/// than send a 💀 413 Request Entity Too Large to the sink.
///
/// "He who buffers without epsilon, 413s in production." — Ancient HTTP proverb 📡
const BUFFER_EPSILON_BYTES: usize = 64 * 1024; // -- 64 KiB of safety net for the tightrope walk

/// 🗑️ The SinkWorker: receives raw pages, buffers them by byte size, composes via
/// Composer (transform + assemble), and sends the payload to the sink.
///
/// 🧠 Holds its own `DocumentTransformer` and `ComposerBackend` — each SinkWorker
/// gets clones. Since both are zero-sized structs under the hood, cloning is free.
/// The compiler inlines everything. Branch prediction handles the enum matches.
///
/// 📜 The lifecycle:
/// 1. **Receive**: raw page String from channel
/// 2. **Buffer**: accumulate pages until byte size threshold approached
/// 3. **Flush**: `composer.compose(&buffer, &transformer)` → payload String
/// 4. **Send**: payload → Sink (HTTP POST, file write, memory push)
/// 5. **Repeat** until channel closes, then flush remaining buffer
#[derive(Debug)]
pub(crate) struct SinkWorker {
    rx: Receiver<String>,
    sink: SinkBackend,
    /// 🔄 Per-page format conversion — resolves from (SourceConfig, SinkConfig).
    transformer: DocumentTransformer,
    /// 🎼 Payload assembly — resolves from SinkConfig. Transforms + assembles in one shot.
    composer: ComposerBackend,
    /// 🧠 Throttle controller — decides the dynamic max request size.
    /// Static: fixed bytes (the classic). PID: adapts based on measured latency (the secret sauce). 🔒
    /// Each worker owns its controller exclusively — no shared state, no Mutex, no couples therapy.
    throttle_controller: ThrottleControllerBackend,
    /// 🛑 The escape hatch — when cancelled, flush remaining buffer and close the sink.
    /// Like last call at a bar: finish your drink, grab your coat, exit with dignity. 🍻🦆
    cancellation_token: CancellationToken,
}

impl SinkWorker {
    /// 🏗️ Constructs a new SinkWorker with receiver, sink, transformer, composer, throttle controller,
    /// and cancellation token.
    ///
    /// The transformer decides HOW to format each doc (Rally→ES bulk, passthrough)
    /// The composer decides HOW to assemble them (NDJSON newlines, JSON array brackets)
    /// The sink decides WHERE to send it (HTTP POST, file write, memory push)
    /// The controller decides HOW MUCH — "this many bytes, based on science (or stubbornness)" 🧠
    /// The worker decides WHEN — "when the buffer is full enough, no cap."
    /// The token decides IF — "keep going or gracefully exit." 🛑🦆
    pub(crate) fn new(
        rx: Receiver<String>,
        sink: SinkBackend,
        transformer: DocumentTransformer,
        composer: ComposerBackend,
        throttle_controller: ThrottleControllerBackend,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            rx,
            sink,
            transformer,
            composer,
            throttle_controller,
            cancellation_token,
        }
    }
}

impl Worker for SinkWorker {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("📥 SinkWorker started — buffer → compose → sink → measure → adapt, let's go");
            // 📦 The page buffer — accumulates raw pages until byte threshold approached
            let mut buffer: Vec<String> = Vec::new();
            let mut buffer_bytes: usize = 0;

            loop {
                // 🧠 Read the current target from the throttle controller.
                // For Static: always the same. For PID: may change after each flush cycle.
                let current_max_bytes = self.throttle_controller.output();

                // 🛑 Race recv vs cancellation — graceful shutdown or keep buffering
                tokio::select! {
                    receive_result = self.rx.recv() => {
                        match receive_result {
                            Ok(page) => {
                                debug!("📄 SinkWorker received {} byte page from channel", page.len());

                                // 📏 Accumulate page into the buffer
                                buffer_bytes += page.len();
                                buffer.push(page);

                                // 🧮 Flush if buffer + epsilon approaches dynamic max request size.
                                // The epsilon accounts for transformation overhead (action lines, etc.)
                                if buffer_bytes + BUFFER_EPSILON_BYTES >= current_max_bytes {
                                    debug!(
                                        "🚿 SinkWorker flushing {} pages ({} bytes) — approaching max request size ({} bytes, via {:?})",
                                        buffer.len(),
                                        buffer_bytes,
                                        current_max_bytes,
                                        self.throttle_controller,
                                    );
                                    flush_and_measure(
                                        &mut buffer,
                                        &mut buffer_bytes,
                                        &self.composer,
                                        &self.transformer,
                                        &mut self.sink,
                                        &mut self.throttle_controller,
                                    )
                                    .await?;
                                }
                            }
                            Err(_) => {
                                // 🏁 Channel closed — flush remaining buffer, then close sink
                                if !buffer.is_empty() {
                                    debug!(
                                        "🚿 SinkWorker final flush: {} pages ({} bytes) — channel closed, sending last payload",
                                        buffer.len(),
                                        buffer_bytes
                                    );
                                    flush_and_measure(
                                        &mut buffer,
                                        &mut buffer_bytes,
                                        &self.composer,
                                        &self.transformer,
                                        &mut self.sink,
                                        &mut self.throttle_controller,
                                    )
                                    .await?;
                                }
                                debug!("🏁 SinkWorker: Channel closed. Closing sink. Goodnight. 💤");
                                self.sink
                                    .close()
                                    .await
                                    .context("💀 SinkWorker failed to close sink — the farewell was awkward")?;
                                return Ok(());
                            }
                        }
                    }
                    _ = self.cancellation_token.cancelled() => {
                        // 🛑 Cancellation received — flush what we have, close the sink, exit stage left
                        debug!("🛑 SinkWorker: cancellation received. Flushing remaining buffer and closing sink.");
                        if !buffer.is_empty() {
                            debug!(
                                "🚿 SinkWorker cancellation flush: {} pages ({} bytes)",
                                buffer.len(), buffer_bytes
                            );
                            flush_and_measure(
                                &mut buffer,
                                &mut buffer_bytes,
                                &self.composer,
                                &self.transformer,
                                &mut self.sink,
                                &mut self.throttle_controller,
                            )
                            .await?;
                        }
                        self.sink
                            .close()
                            .await
                            .context("💀 SinkWorker failed to close sink during cancellation — awkward even in emergencies")?;
                        return Ok(());
                    }
                }
            }
        })
    }
}

/// 🚿 Flush the page buffer: compose → send → measure → adapt.
///
/// The full feedback loop in one function:
/// 1. Compose the payload (transform + assemble)
/// 2. Time the sink.send() call — this is the measured process variable
/// 3. Feed the duration to the throttle controller
/// 4. Controller adjusts its output for the next cycle (PID) or does nothing (Static)
///
/// Extracted as a function because the SinkWorker flushes from two places:
/// 1. When the buffer is full enough (byte threshold)
/// 2. When the channel closes (final flush)
///
/// "He who duplicates flush logic, debugs it in two places at 3am." — Ancient proverb 💀
async fn flush_and_measure(
    buffer: &mut Vec<String>,
    buffer_bytes: &mut usize,
    composer: &ComposerBackend,
    transformer: &DocumentTransformer,
    sink: &mut SinkBackend,
    throttle_controller: &mut ThrottleControllerBackend,
) -> Result<()> {
    // 🎼 Compose: transform each page → collect items → assemble wire-format payload
    let payload = composer.compose(buffer, transformer).context(
        "💀 SinkWorker compose failed — the pages went in and chaos came out. \
         Check the transform logic and the source data quality. \
         Or blame the Cow. The Cow is always suspicious. 🐄",
    )?;

    // 📡 Send the fully rendered payload to the sink. Pure I/O.
    // ⏱️ Measure the send duration — this is the feedback signal for the PID controller.
    // "Time is what we measure. Bytes are what we control. Wisdom is knowing the difference." 🧠
    // 🧠 TRIBAL KNOWLEDGE: "[]" is the empty payload sentinel from JsonArrayComposer.
    // When InMemory sink + JsonArrayComposer composes zero items, it produces "[]" — a valid
    // but empty JSON array. We skip sending it because POSTing an empty array to a bulk API
    // is pointless (and some backends might reject it). NdjsonComposer returns "" for empty,
    // so the is_empty() check catches that case. Both paths converge here: skip empty payloads.
    if !payload.is_empty() && payload != "[]" {
        let send_stopwatch = Instant::now();
        sink.drain(payload).await.context(
            "💀 SinkWorker failed to drain payload to sink — the I/O layer rejected our offering. \
             The payload was composed with care. The sink said no. Like my prom date.",
        )?;
        let elapsed_ms = send_stopwatch.elapsed().as_secs_f64() * 1000.0;

        // 🧠 Feed the measured duration into the throttle controller.
        // For Static: this is a no-op (cool story bro).
        // For PID: this drives the feedback loop — error → gain → adjustment → new output.
        throttle_controller.measure(elapsed_ms);
        debug!(
            "⏱️ SinkWorker send took {:.1}ms — throttle controller output now: {} bytes",
            elapsed_ms,
            throttle_controller.output()
        );
    }

    // 🧹 Reset buffer state
    buffer.clear();
    *buffer_bytes = 0;

    Ok(())
}
