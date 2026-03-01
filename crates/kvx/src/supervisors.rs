//! 🎬 *[camera pans across a dimly lit server room]*
//! 🎬 *[dramatic orchestral music swells]*
//! 🎬 "In a world where workers toil endlessly..."
//! 🎬 "One supervisor dared to manage them all."
//! 🎬 *[record scratch]* 🦆
//!
//! 📦 The Supervisor module — part middle manager, part helicopter parent,
//! part that one project manager who schedules a meeting to plan the next meeting.
//!
//! ⚠️ DO NOT MAKE THIS PUB EVER
//! ⚠️ YOU HAVE BEEN WARNED
//! 💀 WORKERS ARE SUPERVISORS PRIVATE LITTLE MINIONS WHOM THE WORLD FORGOT ABOUT
//! 🔒 Like Fight Club, but for async tasks. First rule: you don't pub the workers.

mod workers;
// -- 🔧 but of course you can tell the supervisor how to manage their minions
// -- it's like a parenting book — everyone has opinions, might as well take config for it
pub mod config;
use crate::app_config::AppConfig;
use crate::supervisors::workers::Worker;
use crate::transforms::DocumentTransformer;
use anyhow::{Context, Result};

/// 📦 The Supervisor: because even async tasks need someone hovering over them
/// asking "is it done yet?" every 5 milliseconds.
///
/// 🏗️ Built with the same care and attention as IKEA furniture —
/// looks good in the docs, wobbly in production.
pub(crate) struct Supervisor {
    /// 🔧 The sacred scrolls of configuration, passed down from main()
    /// through the ancient ritual of .clone()
    app_config: AppConfig,
}

impl Supervisor {
    /// 🚀 Birth of a Supervisor. It's like a baby, but less crying.
    /// Actually no, there's plenty of crying. Mostly from the developer.
    pub(crate) fn new(app_config: AppConfig) -> Self {
        // -- 🐛 "My therapist says I should let go of control"
        // -- — said no supervisor ever
        Self { app_config }
    }
}

impl Supervisor {
    /// 🧵 Unleash the workers! Now with transform powers.
    ///
    /// 🧠 Knowledge graph: the pipeline flow is now:
    /// ```text
    /// Source → Vec<String> → channel → SinkWorker(transform + binary collect) → Sink(I/O)
    /// ```
    /// Each SinkWorker gets its own clone of the `DocumentTransformer`.
    /// Since transforms are zero-sized structs, cloning is free. The compiler laughs.
    pub(crate) async fn start_workers(
        &self,
        source_backend: crate::backends::SourceBackend,
        sink_backends: Vec<crate::backends::SinkBackend>,
        transformer: DocumentTransformer,
    ) -> Result<()> {
        // 📬 Channel carries Vec<String> — raw doc strings from source to sink workers.
        let (tx, rx) = async_channel::bounded(self.app_config.runtime.queue_capacity);

        let mut worker_handles = Vec::with_capacity(sink_backends.len() + 1);

        // 🗑️ Spawn N sink workers, each with its own transformer clone and shared receiver.
        for sink_backend in sink_backends {
            let sink_worker =
                workers::SinkWorker::new(rx.clone(), sink_backend, transformer.clone());
            worker_handles.push(sink_worker.start());
        }

        // 🚰 Spawn the source worker — it pumps raw strings into the channel.
        let source_worker = workers::SourceWorker::new(tx.clone(), source_backend);
        worker_handles.push(source_worker.start());

        let results = futures::future::join_all(worker_handles).await;
        for result in results {
            // 🤯 result?? — the outer `?` unwraps the JoinHandle, the inner `?` unwraps the work.
            result??;
        }

        Ok(())
    }
}
