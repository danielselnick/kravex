// ai
//! 🎬 *[camera pans across a dimly lit server room]*
//! 🎬 *[dramatic orchestral music swells]*
//! 🎬 "In a world where workers toil endlessly..."
//! 🎬 "One foreman dared to manage them all."
//! 🎬 *[record scratch]* 🦆
//!
//! 📦 The Foreman module — part middle manager, part helicopter parent,
//! part that one project manager who schedules a meeting to plan the next meeting.
//!
//! 🧠 Knowledge graph — the 3-stage pipeline:
//! ```text
//! Pumper (async, tokio) → ch1 → Joiner(s) (sync, std::thread) → ch2 → Drainer(s) (async, tokio) → Sink
//! ```
//! - **ch1**: async_channel::bounded — raw feeds from source, MPMC
//! - **ch2**: async_channel::bounded — assembled payloads from joiners, MPMC
//! - **Joiners**: CPU-bound work (casting, manifold join) on dedicated OS threads
//! - **Drainers**: I/O-bound work (sink.send) on tokio async runtime
//!
//! ⚠️ DO NOT MAKE THIS PUB EVER
//! ⚠️ YOU HAVE BEEN WARNED
//! 💀 WORKERS ARE THE FOREMAN'S PRIVATE LITTLE MINIONS WHOM THE WORLD FORGOT ABOUT
//! 🔒 Like Fight Club, but for async tasks. First rule: you don't pub the workers.

use crate::app_config::AppConfig;
use crate::casts::DocumentCaster;
use crate::manifolds::ManifoldBackend;
use crate::workers;
use crate::workers::Worker;
use anyhow::{Context, Result};
use tracing::info;

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
        Self { app_config }
    }
}

impl Foreman {
    /// 🧵 Orchestrate the 3-stage pipeline: Pumper → Joiners → Drainers.
    ///
    /// 🧠 Knowledge graph — pipeline wiring:
    /// ```text
    /// Pumper (async) --[ch1: raw feeds]--> Joiner(s) (std::thread)
    ///                                      --[ch2: payloads]--> Drainer(s) (async) --> Sink
    /// ```
    ///
    /// The flow cascades via channel closure:
    /// 1. Pumper finishes → closes ch1 (drops tx1)
    /// 2. Joiners see ch1 closed → flush remaining → drop tx2
    /// 3. All joiners' tx2 dropped → ch2 closes
    /// 4. Drainers see ch2 closed → close sinks → exit
    ///
    /// "In the beginning there was main(). And main() said 'let there be workers.'
    ///  And the Foreman made it so. And it was... mostly okay." — Genesis 1:1 (Cargo edition) 🦆
    pub async fn start_workers(
        &self,
        source_backend: crate::backends::SourceBackend,
        sink_backends: Vec<crate::backends::SinkBackend>,
        caster: DocumentCaster,
        manifold: ManifoldBackend,
        max_request_size_bytes: usize,
    ) -> Result<()> {
        let the_joiner_count = self.app_config.runtime.joiner_parallelism;

        // 📬 ch1: pumper → joiners — carries raw feed Strings, MPMC
        // Like a conveyor belt at a sushi restaurant, but the sushi is JSON 🍣
        let (tx1, rx1) = async_channel::bounded(self.app_config.runtime.queue_capacity);

        // 📬 ch2: joiners → drainers — carries assembled payload Strings, MPMC
        // The VIP lounge of the pipeline — only processed payloads allowed past this point 🎟️
        let (tx2, rx2) = async_channel::bounded(self.app_config.runtime.payload_channel_capacity);

        info!(
            "🏗️ Foreman assembling pipeline: 1 pumper → {} joiners → {} drainers",
            the_joiner_count,
            sink_backends.len()
        );

        // 🧵 Spawn N joiners on dedicated OS threads (std::thread).
        // They do the CPU-heavy lifting: buffering raw feeds, casting, manifold join.
        // Each gets its own clone of rx1, tx2, caster, manifold.
        // Since casters and manifolds are zero-sized structs, cloning is cheaper than this comment. 🐄
        let mut the_joiner_thread_handles = Vec::with_capacity(the_joiner_count);
        for _ in 0..the_joiner_count {
            let joiner = workers::Joiner::new(
                rx1.clone(),
                tx2.clone(),
                caster.clone(),
                manifold.clone(),
                max_request_size_bytes,
            );
            the_joiner_thread_handles.push(joiner.start());
        }
        // 🗑️ Drop the foreman's copy of tx2 — otherwise ch2 never closes because
        // the foreman holds a sender that never sends. Like having a phone but never calling.
        // When all joiner threads finish and drop their tx2 clones, ch2 closes naturally. 📱
        drop(tx2);

        // 🚰 Spawn N drainers on tokio — thin async relays from ch2 to sinks.
        // Each drainer gets its own sink and a clone of rx2.
        let mut the_async_worker_handles = Vec::with_capacity(sink_backends.len() + 1);
        for sink_backend in sink_backends {
            let drainer = workers::Drainer::new(rx2.clone(), sink_backend);
            the_async_worker_handles.push(drainer.start());
        }

        // 🚰 Spawn the pumper — it reads from the source and fills ch1 with raw feeds.
        // The pumper is the DJ of this party: it sets the tempo. When it stops, everyone goes home.
        let pumper = workers::Pumper::new(tx1.clone(), source_backend);
        the_async_worker_handles.push(pumper.start());

        // ⏳ Wait for all async workers (pumper + drainers).
        // The cascade: pumper done → ch1 closes → joiners drain+exit → ch2 closes → drainers exit.
        // So by the time join_all returns, the joiner threads should already be done. 🏁
        let the_async_results = futures::future::join_all(the_async_worker_handles).await;
        for result in the_async_results {
            // 🤯 result?? — outer `?` unwraps JoinHandle, inner `?` unwraps the work
            result??;
        }

        // 🧵 Join the std::thread handles — should be instant since joiners are already done
        // (ch1 closed → joiners flushed → exited before drainers could finish).
        // This is just the funeral procession. The threads are already at rest. 🪦
        for (i, handle) in the_joiner_thread_handles.into_iter().enumerate() {
            handle
                .join()
                .map_err(|the_panic_payload| {
                    anyhow::anyhow!(
                        "💀 Joiner thread {} panicked — it saw something in the JSON that broke it. \
                         The panic payload: {:?}. \
                         Like a horror movie, but the monster is malformed data.",
                        i,
                        the_panic_payload
                    )
                })?
                .context(format!(
                    "💀 Joiner thread {} returned an error — it tried its best, \
                     but the feeds fought back like a cornered raccoon 🦝",
                    i
                ))?;
        }

        Ok(())
    }
}
