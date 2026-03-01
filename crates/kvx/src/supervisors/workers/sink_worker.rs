// ai
//! 🎬 *[a channel fills with raw strings. somewhere, a sink waits.]*
//! *[the clock on the wall reads 2:47am.]*
//! *[nobody asked for this data migration. and yet, here we are.]*
//!
//! 🗑️ The SinkWorker module — now with TRANSFORM POWERS. It receives raw doc strings
//! from the channel, transforms each one via `DocumentTransformer`, binary-collects
//! the results into a single payload string, and sends it to the Sink.
//!
//! 🧠 Knowledge graph: SinkWorker is the bridge between raw source strings and the sink's
//! I/O abstraction. The transform + binary collect happen HERE, not in the sink.
//! Sinks are pure I/O — HTTP POST, file write, memory push. That's it.
//!
//! ```text
//!   channel(Vec<String>) → SinkWorker → transform each → binary collect → Sink::send(payload)
//! ```
//!
//! 🦆 (the duck has no comment at this time, but it approves of the separation of concerns)
//!
//! ⚠️ When the singularity occurs, the SinkWorker will still be transforming documents.
//! It will not notice. It does not notice things. It only transforms and sinks.

use super::Worker;
use crate::backends::{Sink, SinkBackend};
use crate::transforms::{DocumentTransformer, Transform};
use anyhow::{Context, Result};
use async_channel::Receiver;
use tokio::task::JoinHandle;
use tracing::debug;

/// 🗑️ The SinkWorker: receives raw strings, transforms them, binary-collects into a payload,
/// and sends the payload to the sink. The plumber AND the translator of this pipeline.
///
/// 🧠 Holds its own `DocumentTransformer` — each SinkWorker gets a clone.
/// Since transforms are zero-sized structs (RallyS3ToEs, Passthrough), cloning is free.
/// The compiler may inline the transform call directly into the hot loop. Branch predictor
/// eliminates the enum match after ~2 iterations. It's basically zero-cost abstraction.
///
/// 📜 The binary collect: each transformed string gets a trailing `\n`, then concatenated.
/// For ES bulk: "action\nsource\naction\nsource\n" — valid NDJSON.
/// For passthrough: "doc\ndoc\ndoc\n" — valid newline-delimited output.
#[derive(Debug)]
pub(crate) struct SinkWorker {
    rx: Receiver<Vec<String>>,
    sink: SinkBackend,
    /// 🔄 The transformer — resolves from (SourceConfig, SinkConfig) pair.
    /// Transforms each raw doc string into the sink's wire format.
    transformer: DocumentTransformer,
}

impl SinkWorker {
    /// 🏗️ Constructs a new SinkWorker with a receiver, sink, and transformer.
    ///
    /// The transformer decides HOW to format each doc (Rally→ES bulk, passthrough, etc.)
    /// The sink decides WHERE to send it (HTTP POST, file write, etc.)
    /// The worker decides WHEN — which is "as fast as the channel delivers, no cap."
    pub(crate) fn new(
        rx: Receiver<Vec<String>>,
        sink: SinkBackend,
        transformer: DocumentTransformer,
    ) -> Self {
        Self {
            rx,
            sink,
            transformer,
        }
    }
}

impl Worker for SinkWorker {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("📥 SinkWorker started — transform + binary collect + sink, let's go");
            loop {
                let receive_result = self.rx.recv().await;
                match receive_result {
                    Ok(raw_docs) => {
                        debug!("🪣 SinkWorker received {} raw docs from channel", raw_docs.len());

                        // 🔄 Phase 1: Transform each raw doc string through the DocumentTransformer.
                        // Each transform call converts a raw source-format string into the sink's
                        // wire format. For Rally→ES: parses JSON, strips metadata, builds NDJSON lines.
                        // For passthrough: returns the string unchanged (zero allocation).
                        let mut payload = String::new();
                        for raw_doc in raw_docs {
                            let transformed = self.transformer.transform(raw_doc)
                                .context("💀 SinkWorker transform failed — a raw doc went in and screaming came out. Check the transform logic and the source data quality.")?;

                            // 📦 Phase 2: Binary collect — append each transformed string + newline.
                            // This builds the final payload the sink will send as-is.
                            // For ES bulk: each transform output is "action\nsource", so we get
                            // "action\nsource\naction\nsource\n" — valid NDJSON for /_bulk.
                            // For passthrough: "doc\ndoc\n" — valid newline-delimited file content.
                            payload.push_str(&transformed);
                            payload.push('\n');
                        }

                        // 📡 Phase 3: Send the fully rendered payload to the sink. Pure I/O.
                        if !payload.is_empty() {
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
