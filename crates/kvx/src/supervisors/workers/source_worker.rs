//! 🎬 *[a vast index stretches to the horizon, billions of documents, blissfully unaware]*
//! *[a SourceWorker cracks its knuckles]*
//! *["Don't worry," it says. "I'll be gentle."]*
//! *[it was not gentle. it was `next_batch()` in a loop.]*
//!
//! 🚰 The SourceWorker module — the headwaters of the kravex pipeline. Data starts here.
//! It wakes up, calls `next_batch()` until the well runs dry, then closes the channel
//! and quietly exits stage left, never to be heard from again.
//!
//! 🦆 (same duck, different file, same vibe)
//!
//! ⚠️ When the singularity occurs, the SourceWorker will have already finished.
//! It respects empty batches. It knows when to let go. Unlike the rest of us.

use super::Worker;
use crate::backends::{Source, SourceBackend};
use anyhow::{Context, Result};
use async_channel::Sender;
use tokio::task::JoinHandle;
use tracing::debug;

/// 🚰 The SourceWorker: reads raw strings from a backend, sends Vec<String> to the channel.
///
/// 🧠 Knowledge graph: Sources now return Vec<String> — raw document strings, no Hit wrappers.
/// The channel carries Vec<String>. The SinkWorker downstream transforms and binary-collects.
/// Like a barista, but for data. And less tips.
#[derive(Debug)]
pub(crate) struct SourceWorker {
    tx: Sender<Vec<String>>,
    source: SourceBackend,
}

impl SourceWorker {
    /// 🏗️ Constructs a new SourceWorker — the headwaters of the pipeline.
    ///
    /// Give it a sender (where the raw strings go) and a source backend (where the data comes from).
    /// It will faithfully poll `next_batch()` like a golden retriever waiting by the door.
    /// Empty vec = the retriever goes home. The channel closes.
    pub(crate) fn new(tx: Sender<Vec<String>>, source: SourceBackend) -> Self {
        Self { tx, source }
    }
}

impl Worker for SourceWorker {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("🚀 SourceWorker started pumping raw strings into the channel...");
            loop {
                let raw_docs = self
                    .source
                    .next_batch()
                    .await
                    .context("💀 SourceWorker failed to get next batch — the well collapsed")?;

                if raw_docs.is_empty() {
                    debug!("🏁 SourceWorker: empty batch = EOF. Closing channel. The well is dry.");
                    self.tx.close();
                    break;
                } else {
                    debug!("📤 SourceWorker sending {} raw docs to channel", raw_docs.len());
                    self.tx.send(raw_docs).await?;
                }
            }
            Ok(())
        })
    }
}
