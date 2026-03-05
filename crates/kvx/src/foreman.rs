//! 🎬 *[camera pans across a dimly lit server room]*
//! 🎬 *[dramatic orchestral music swells]*
//! 🎬 "In a world where workers toil endlessly..."
//! 🎬 "One foreman dared to manage them all."
//! 🎬 *[record scratch]* 🦆
//!
//! 📦 The Foreman module — part middle manager, part helicopter parent,
//! part that one project manager who schedules a meeting to plan the next meeting.
//!
//! ⚠️ DO NOT MAKE THIS PUB EVER
//! ⚠️ YOU HAVE BEEN WARNED
//! 💀 WORKERS ARE THE FOREMAN'S PRIVATE LITTLE MINIONS WHOM THE WORLD FORGOT ABOUT
//! 🔒 Like Fight Club, but for async tasks. First rule: you don't pub the workers.

use crate::app_config::AppConfig;
use crate::manifolds::ManifoldBackend;
use crate::workers::Worker;
use crate::workers;
use crate::casts::DocumentCaster;
use anyhow::{Context, Result};

/// 📦 The Foreman: because even async tasks need someone hovering over them
/// asking "is it done yet?" every 5 milliseconds.
///
/// 🏗️ Built with the same care and attention as IKEA furniture —
/// looks good in the docs, wobbly in production.
pub struct Foreman {
    /// 🔧 The sacred scrolls of configuration, passed down from main()
    /// through the ancient ritual of .clone()
    app_config: AppConfig,
}

impl Foreman {
    /// 🚀 Birth of a Foreman. It's like a baby, but less crying.
    /// Actually no, there's plenty of crying. Mostly from the developer.
    pub fn new(app_config: AppConfig) -> Self {
        // -- 🐛 "My therapist says I should let go of control"
        // -- — said no foreman ever
        Self { app_config }
    }
}

impl Foreman {
    /// 🧵 Unleash the workers! Now with Manifold powers and feed buffering.
    ///
    /// 🧠 Knowledge graph: the pipeline flow is now:
    /// ```text
    /// Source.pump() → channel(String) → Drainer(buffer feeds → manifold.join → sink.drain) → Sink(I/O)
    /// ```
    /// Each Drainer gets its own clone of the `DocumentCaster` and `ManifoldBackend`.
    /// Since casters and manifolds are zero-sized structs, cloning is free.
    /// The Manifold handles both casting AND assembly — the plumbing lives there. 🔧
    pub async fn start_workers(
        &self,
        source_backend: crate::backends::SourceBackend,
        sink_backends: Vec<crate::backends::SinkBackend>,
        caster: DocumentCaster,
        manifold: ManifoldBackend,
        max_request_size_bytes: usize,
    ) -> Result<()> {
        // 📬 Channel carries String — raw feeds from source to drainer workers.
        let (tx, rx) = async_channel::bounded(self.app_config.runtime.queue_capacity);

        let mut worker_handles = Vec::with_capacity(sink_backends.len() + 1);

        // 🗑️ Spawn N drainers, each with its own caster + manifold clones.
        for sink_backend in sink_backends {
            let drainer = workers::Drainer::new(
                rx.clone(),
                sink_backend,
                caster.clone(),
                manifold.clone(),
                max_request_size_bytes,
            );
            worker_handles.push(drainer.start());
        }

        // 🚰 Spawn the pumper — it pumps raw feeds into the channel.
        let pumper = workers::Pumper::new(tx.clone(), source_backend);
        worker_handles.push(pumper.start());

        let results = futures::future::join_all(worker_handles).await;
        for result in results {
            // 🤯 result?? — the outer `?` unwraps the JoinHandle, the inner `?` unwraps the work.
            result??;
        }

        Ok(())
    }
}
