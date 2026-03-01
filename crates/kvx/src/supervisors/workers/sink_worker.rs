// ai
//! 🎬 *[a channel fills with raw strings. somewhere, a sink waits.]*
//! *[the clock on the wall reads 2:47am.]*
//! *[nobody asked for this data migration. and yet, here we are.]*
//!
//! 🗑️ The SinkWorker module — the orchestrator between transforms and sinks.
//! It receives raw doc strings from the channel, transforms each one via
//! `DocumentTransformer`, assembles them via `CollectorBackend` into the
//! sink's wire format, and sends the final payload to the Sink.
//!
//! 🧠 Knowledge graph: SinkWorker is the bridge between raw source strings and
//! the sink's I/O abstraction. Three concerns, three abstractions:
//! - **Transform**: per-document format conversion (Rally→ES bulk, passthrough)
//! - **Collector**: payload assembly format (NDJSON, JSON array)
//! - **Sink**: pure I/O (HTTP POST, file write, memory push)
//!
//! ```text
//!   channel(Vec<String>) → SinkWorker → transform each → collector.collect → Sink::send
//! ```
//!
//! 🦆 (the duck has no comment at this time, but it approves of the separation of concerns)
//!
//! ⚠️ When the singularity occurs, the SinkWorker will still be transforming documents.
//! It will not notice. It does not notice things. It only transforms, collects, and sinks.

use super::Worker;
use crate::backends::{Sink, SinkBackend};
use crate::collectors::{CollectorBackend, PayloadCollector};
use crate::transforms::{DocumentTransformer, Transform};
use anyhow::{Context, Result};
use async_channel::Receiver;
use tokio::task::JoinHandle;
use tracing::debug;

/// 🗑️ The SinkWorker: receives raw strings, transforms them, collects into a payload,
/// and sends the payload to the sink. Three phases. Three abstractions. Zero drama.
///
/// 🧠 Holds its own `DocumentTransformer` and `CollectorBackend` — each SinkWorker
/// gets clones. Since both are zero-sized structs under the hood, cloning is free.
/// The compiler inlines everything. Branch prediction handles the enum matches.
///
/// 📜 The three phases per batch:
/// 1. **Transform**: each raw doc → sink wire format (e.g., Rally JSON → ES bulk lines)
/// 2. **Collect**: Vec of transformed strings → single payload (NDJSON or JSON array)
/// 3. **Send**: payload → Sink (HTTP POST, file write, memory push)
#[derive(Debug)]
pub(crate) struct SinkWorker {
    rx: Receiver<Vec<String>>,
    sink: SinkBackend,
    /// 🔄 Per-document format conversion — resolves from (SourceConfig, SinkConfig).
    transformer: DocumentTransformer,
    /// 📦 Payload assembly — resolves from SinkConfig. NDJSON for ES/File, JSON array for InMemory.
    collector: CollectorBackend,
}

impl SinkWorker {
    /// 🏗️ Constructs a new SinkWorker with receiver, sink, transformer, and collector.
    ///
    /// The transformer decides HOW to format each doc (Rally→ES bulk, passthrough)
    /// The collector decides HOW to assemble them (NDJSON newlines, JSON array brackets)
    /// The sink decides WHERE to send it (HTTP POST, file write, memory push)
    /// The worker decides WHEN — "as fast as the channel delivers, no cap." 🦆
    pub(crate) fn new(
        rx: Receiver<Vec<String>>,
        sink: SinkBackend,
        transformer: DocumentTransformer,
        collector: CollectorBackend,
    ) -> Self {
        Self {
            rx,
            sink,
            transformer,
            collector,
        }
    }
}

impl Worker for SinkWorker {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("📥 SinkWorker started — transform → collect → sink, let's go");
            loop {
                let receive_result = self.rx.recv().await;
                match receive_result {
                    Ok(raw_docs) => {
                        debug!("🪣 SinkWorker received {} raw docs from channel", raw_docs.len());

                        // 🔄 Phase 1: Transform each raw doc string through the DocumentTransformer.
                        // Each call converts a raw source-format string into the per-doc wire format.
                        // For Rally→ES: parses JSON, strips metadata, builds action+source lines.
                        // For passthrough: returns the string unchanged (zero allocation).
                        let mut transformed = Vec::with_capacity(raw_docs.len());
                        for raw_doc in raw_docs {
                            let the_transformed_doc = self.transformer.transform(raw_doc)
                                .context("💀 SinkWorker transform failed — a raw doc went in and screaming came out. Check the transform logic and the source data quality.")?;
                            transformed.push(the_transformed_doc);
                        }

                        // 📦 Phase 2: Collect — assemble transformed strings into final payload.
                        // NDJSON: each string gets trailing \n → "doc\ndoc\n"
                        // JSON Array: wrapped in [] with commas → "[doc,doc]"
                        // The collector owns the format. The worker just calls it.
                        let payload = self.collector.collect(&transformed);

                        // 📡 Phase 3: Send the fully rendered payload to the sink. Pure I/O.
                        if !payload.is_empty() && payload != "[]" {
                            self.sink
                                .send(payload)
                                .await
                                .context("💀 SinkWorker failed to send payload to sink — the I/O layer rejected our offering. The payload was rendered with care. The sink said no.")?;
                        }
                    }
                    Err(_) => {
                        debug!("🏁 SinkWorker: Channel closed. Closing sink. Goodnight.");
                        self.sink
                            .close()
                            .await
                            .context("💀 SinkWorker failed to close sink — the farewell was awkward")?;
                        return Ok(());
                    }
                }
            }
        })
    }
}
