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

use crate::backends::{Source, SourceBackend};
use crate::common::HitBatch;
use anyhow::{Context, Result};
use async_channel::Sender;
use tokio::task::JoinHandle;
use tracing::debug;
use super::Worker;

/// 🚰 The SourceWorker: reads from a backend, sends to a channel.
/// Like a barista, but for data. And less tips.
#[derive(Debug)]
pub(crate) struct SourceWorker {
    tx: Sender<HitBatch>,
    source: SourceBackend,
}

impl SourceWorker {
    /// 🏗️ Constructs a new SourceWorker.
    ///
    /// Give it a sender (where the data goes) and a source backend (where the data comes from).
    /// It will faithfully poll `next_batch()` like a golden retriever waiting by the door —
    /// enthusiastic, tireless, and completely unaware that one day the door won't open.
    ///
    /// That day is when `hits.is_empty()`. The retriever goes home. The channel closes.
    /// It's beautiful, in a way. Don't think about it too hard.
    pub(crate) fn new(tx: Sender<HitBatch>, source: SourceBackend) -> Self {
        // 📤 tx: the outbox. source: the inbox of the world.
        // Together they make one very determined data funnel.
        Self { tx, source }
    }
}

impl Worker for SourceWorker {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("🚀 SourceWorker started pumping data...");
            loop {
                let batch_result = self
                    .source
                    .next_batch()
                    .await
                    .context("SourceWorker failed to get next batch")?;
                
                if batch_result.hits.is_empty() {
                    debug!("🏁 SourceWorker received empty batch. Closing channel.");
                    self.tx.close();
                    break;
                } else {
                    debug!("📤 SourceWorker sending batch of {} hits", batch_result.hits.len());
                    self.tx.send(batch_result).await?;
                }
            }
            Ok(())
        })
    }
}
