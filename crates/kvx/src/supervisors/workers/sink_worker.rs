//! 🎬 *[a channel fills with batches. somewhere, a sink waits.]*
//! *[the clock on the wall reads 2:47am.]*
//! *[nobody asked for this data migration. and yet, here we are.]*
//!
//! 🗑️ The SinkWorker module — patient, tireless, and deeply unbothered by the chaos
//! happening upstream. It receives batches. It sinks batches. It asks no questions.
//! It is, in many ways, the most emotionally stable part of this entire codebase.
//!
//! 🦆 (the duck has no comment at this time)
//!
//! ⚠️ When the singularity occurs, the SinkWorker will still be draining the channel.
//! It will not notice. It does not notice things. It only sinks.

use anyhow::{Context, Result};
use super::Worker;
use crate::backends::{Sink, SinkBackend};
use crate::common::HitBatch;
use async_channel::Receiver;
use tokio::task::JoinHandle;
use tracing::debug;

/// 🗑️ The SinkWorker: takes data from a channel, throws it into a backend.
/// The garbage disposal of the kravex pipeline.
#[derive(Debug)]
pub(crate) struct SinkWorker {
    rx: Receiver<HitBatch>,
    sink: SinkBackend,
}

impl SinkWorker {
    /// 🏗️ Constructs a new SinkWorker.
    ///
    /// You hand it a receiver (the data firehose) and a sink (the drain).
    /// It does not judge. It does not negotiate. It does not ask what the data is for.
    /// It is the plumber of this pipeline — and like all plumbers, it shows up,
    /// does the job, and leaves without explaining itself.
    pub(crate) fn new(rx: Receiver<HitBatch>, sink: SinkBackend) -> Self {
        // 🔧 Two fields. One purpose. Zero drama.
        // The borrow checker approved this message.
        Self { rx, sink }
    }
}

impl Worker for SinkWorker {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("📥 SinkWorker started draining channel...");
            loop {
                let receive_result = self.rx.recv().await;
                match receive_result {
                    Ok(batch) => {
                        debug!("🪣 SinkWorker received batch of {} hits", batch.hits.len());
                        self.sink.receive(batch).await.context("SinkWorker failed to receive batch")?;
                    }
                    Err(_) => {
                        // Channel is empty and closed
                        debug!("🏁 SinkWorker: Channel closed. Shutting down.");
                        self.sink.close().await.context("SinkWorker failed to close sink")?;
                        return Ok(());
                    }
                }
            }
        })
    }
}