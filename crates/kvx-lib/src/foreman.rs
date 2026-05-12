// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
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
//! Pumper (async, tokio) → ch1 → Refiner(s) (sync, std::thread) → ch2 → Drainer(s) (async, tokio) → Sink
//!                                                                             ↓ (latency readings)
//!                                                                            ch3
//!                                                                             ↓
//!                                                                        Governor → FlowKnob → Refiners
//! ```
//! - **ch1**: async_channel::bounded — raw barrels from source, MPMC
//! - **ch2**: async_channel::bounded — assembled drums from refiners, MPMC
//! - **ch3**: async_channel::bounded — GaugeReading from drainers to Governor (latency barrelback)
//! - **Refiners**: CPU-bound work (tapping, manifold join) on dedicated OS threads
//! - **Drainers**: I/O-bound work (sink.drain) on tokio async runtime
//! - **Governor**: receives latency readings, PID-regulates, adjusts FlowKnob
//!
//! ⚠️ DO NOT MAKE THIS PUB EVER
//! ⚠️ YOU HAVE BEEN WARNED
//! 💀 WORKERS ARE THE FOREMAN'S PRIVATE LITTLE MINIONS WHOM THE WORLD FORGOT ABOUT
//! 🔒 Like Fight Club, but for async tasks. First rule: you don't pub the workers.

use crate::config::AppConfig;
use crate::taps::BarrelToDraftsTapper;
use crate::manifolds::ManifoldBackend;
use crate::progress::{DrainMetrics, spawn_progress_reporter};
use crate::FlowKnob;
use crate::regulators::Regulators;
use crate::workers;
use crate::workers::{GovernorConfig, Worker};
use crate::GaugeReading;
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::info;

/// 📦 The Foreman: because even async tasks need someone hovering over them
/// asking "is it done yet?" every 5 milliseconds.
///
/// 🏗️ Built with the same care and attention as IKEA furniture —
/// looks good in the docs, indestructible in production (terms and conditions apply).
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
    /// 🧵 Orchestrate the 3-stage pipeline: Pumper → Refiners → Drainers (+ optional Governor).
    ///
    /// 🧠 Knowledge graph — pipeline wiring:
    /// ```text
    /// Pumper (async) --[ch1: raw barrels]--> Refiner(s) (std::thread)
    ///                                        --[ch2: drums]--> Drainer(s) (async) --> Sink
    ///                                                                 |
    ///                                                            [ch3: latency]
    ///                                                                 ↓
    ///                                                            Governor → FlowKnob → Refiners
    /// ```
    ///
    /// 🔒 Channel closure semantics (async_channel implicit close):
    /// An async_channel closes when ALL clones of its Sender (or Receiver) are dropped.
    /// This is refcount-based — every `.clone()` extends the channel's lifetime.
    /// There is no single "owner" that closes the channel; the LAST drop does it.
    /// The foreman creates both channels but is NOT a participant — it's the orchestrator.
    /// So it must drop its copies after distributing clones to the actual workers.
    ///
    /// 🔄 Shutdown cascade (all driven by implicit Sender drops, no `.close()` calls):
    /// 1. Pumper finishes → its tx1 is dropped (only Sender for ch1) → ch1 closes
    /// 2. Refiners' recv_blocking() returns Err → flush remaining → refiner threads exit → tx2 clones dropped
    /// 3. Last refiner's tx2 dropped → all Senders for ch2 gone → ch2 closes
    /// 4. Drainers' recv().await returns Err → close sinks → exit → tx3 clones dropped
    /// 5. Last drainer's tx3 dropped → ch3 closes → Governor exits
    ///
    /// "In the beginning there was main(). And main() said 'let there be workers.'
    ///  And the Foreman made it so. And it was... mostly okay." — Genesis 1:1 (Cargo edition) 🦆
    #[allow(clippy::too_many_arguments)]
    pub async fn start_workers(
        &self,
        source_backend: crate::backends::SourceBackend,
        sink_backends: Vec<crate::backends::SinkBackend>,
        tapper: BarrelToDraftsTapper,
        manifold: ManifoldBackend,
        the_flow_knob: FlowKnob,
        the_governor_config: &GovernorConfig,
        the_sink_max_drum_size_bytes: usize,
        pipeline_name: String,
        total_expected_bytes: u64,
    ) -> Result<()> {
        let the_refiner_count = self.app_config.runtime.refiner_count;

        // 📬 ch1: pumper → refiners — carries raw barrel Strings, MPMC
        // Like a conveyor belt at a sushi restaurant, but the sushi is JSON 🍣
        let (tx1, rx1) = async_channel::bounded(self.app_config.runtime.pumper_to_refiner_capacity);

        // 📬 ch2: refiners → drainers — carries assembled drum Strings, MPMC
        // The VIP lounge of the pipeline — only processed drums allowed past this point 🎟️
        let (tx2, rx2) = async_channel::bounded::<crate::Drum>(self.app_config.runtime.refiner_to_drainer_capacity);

        // 📬 ch3: drainers → governor — carries GaugeReading (latency barrelback), MPSC-ish
        // Only created for latency regulation. Static mode = no channel, no Governor, no drama 🎭
        let the_gauge_channel = match the_governor_config {
            GovernorConfig::Latency(latency_config) => {
                let (tx3, rx3) = async_channel::bounded::<GaugeReading>(256);
                let the_regulator = Regulators::from_latency_config(
                    latency_config,
                    the_sink_max_drum_size_bytes,
                );
                Some((tx3, rx3, the_regulator))
            }
            GovernorConfig::Throughput(throughput_config) => {
                let (tx3, rx3) = async_channel::bounded::<GaugeReading>(256);
                let the_regulator = Regulators::from_throughput_config(
                    throughput_config,
                    the_sink_max_drum_size_bytes,
                );
                Some((tx3, rx3, the_regulator))
            }
            GovernorConfig::Static(_) => None,
        };

        info!(
            "🏗️ Foreman assembling pipeline: 1 pumper → {} refiners → {} drainers{}",
            the_refiner_count,
            sink_backends.len(),
            if the_gauge_channel.is_some() { " + Governor" } else { "" }
        );

        // ═══════════════════════════════════════════════════════════════════
        // 🔒 CHANNEL OWNERSHIP CONTRACT
        //
        // async_channel uses refcounting: a channel stays open as long as at
        // least one Sender (or Receiver) clone exists. The channel closes
        // implicitly when the LAST clone is dropped — no explicit .close()
        // needed. This means every .clone() is a commitment: "I am keeping
        // this channel alive." The foreman creates both channels but must
        // surrender all handles to the workers, retaining NOTHING. Otherwise
        // a stale foreman handle prevents implicit closure → deadlock.
        //
        // We enforce this by:
        //   - Moving tx1 directly into the pumper (no clone, no foreman copy)
        //   - Dropping tx2, rx1, rx2, tx3 after distributing clones to workers
        //
        // The result: only workers hold channel handles. When workers exit,
        // their handles drop, channels close, downstream workers see Err,
        // and the pipeline cascades to shutdown. No .close() calls anywhere.
        // Pure RAII. The borrow checker would shed a single, proud tear. 🦀
        // ═══════════════════════════════════════════════════════════════════

        // 🧵 Spawn N refiners on dedicated OS threads (std::thread).
        // They do the CPU-heavy lifting: accumulating drafts, tapping, manifold join.
        // Each gets its own clone of rx1 and tx2.
        // Tappers and manifolds are zero-sized structs — cloning is cheaper than this comment. 🐄
        let mut the_refiner_thread_handles = Vec::with_capacity(the_refiner_count);
        for _ in 0..the_refiner_count {
            let refiner = workers::Refiner::new(
                rx1.clone(),
                tx2.clone(),
                tapper.clone(),
                manifold.clone(),
                the_flow_knob.clone(),
            );
            the_refiner_thread_handles.push(refiner.start());
        }

        // 🗑️ Foreman surrenders ch2 sender and ch1 receiver.
        // tx2: if foreman kept this, ch2 would never close (foreman's Sender outlives
        //   the refiners → drainers hang on recv() forever → deadlock). By dropping it,
        //   only refiner threads hold ch2 Senders. When the last refiner exits and drops
        //   its tx2 clone, ch2 closes, and drainers see Err on recv(). 📱
        // rx1: receivers don't affect send-side closure, but the foreman has no business
        //   holding a receiver it will never read. Clean ownership = clean conscience. 🧹
        drop(tx2);
        drop(rx1);

        // 📊 Create shared drain metrics — N drainers write, 1 reporter reads.
        // Arc<DrainMetrics> is the FlowKnob pattern applied to progress reporting.
        // No channels, no Mutex, no shutdown cascade — just atomics and vibes. 🧘
        let the_drain_metrics = Arc::new(DrainMetrics::new());

        // 🚰 Spawn N drainers on tokio — thin async relays from ch2 to sinks.
        // Each drainer gets its own sink, a clone of rx2, and optionally a clone of tx3.
        let the_gauge_tx = the_gauge_channel.as_ref().map(|(tx, _, _)| tx.clone());
        let mut the_async_worker_handles = Vec::with_capacity(sink_backends.len() + 2);
        for sink_backend in sink_backends {
            let drainer = workers::Drainer::new(
                rx2.clone(),
                sink_backend,
                self.app_config.drainer.clone(),
                the_gauge_tx.clone(),
                the_drain_metrics.clone(),
            );
            the_async_worker_handles.push(drainer.start());
        }

        // 🗑️ Foreman surrenders ch2 receiver — only drainer tasks hold rx2 clones now.
        // Same reasoning: foreman is orchestrator, not participant. No stale handles. 🧹
        drop(rx2);

        // 🗑️ Foreman surrenders ch3 sender — only drainers hold tx3 clones now.
        // When all drainers exit and drop their tx3 clones → ch3 closes → Governor exits.
        drop(the_gauge_tx);

        // 🎛️ Spawn Governor if we have a gauge channel — it consumes rx3 and adjusts FlowKnob.
        if let Some((tx3, rx3, the_regulator)) = the_gauge_channel {
            // 🗑️ Drop foreman's tx3 — only drainers should hold senders
            drop(tx3);
            let the_governor = workers::Governor::new(rx3, the_regulator, the_flow_knob.clone());
            the_async_worker_handles.push(the_governor.start());
        }

        // 🚰 Spawn the pumper — gets tx1 by MOVE (not clone).
        // tx1 is moved directly into the pumper, so no foreman copy exists.
        // When the pumper's async task exits (EOF from source), tx1 drops,
        // and since it's the ONLY Sender for ch1, ch1 closes implicitly.
        // No .close() call needed — RAII handles it. Like a self-closing door. 🚪
        let pumper = workers::Pumper::new(tx1, source_backend);
        the_async_worker_handles.push(pumper.start());

        // 📊 Spawn the progress reporter — a leaf display task that ticks every 500ms.
        // It reads DrainMetrics atomics, renders a comfy-table, and sleeps. Safe to abort.
        // Like a screensaver — decorative, informative, entirely expendable. 🖥️🦆
        let the_progress_reporter = spawn_progress_reporter(
            pipeline_name,
            the_drain_metrics.clone(),
            total_expected_bytes,
            &self.app_config,
        );

        // ⏳ Wait for all async workers (pumper + drainers + optional Governor).
        // The cascade: pumper done → ch1 closes → refiners drain+exit → ch2 closes
        //   → drainers exit → ch3 closes → Governor exits.
        // So by the time join_all returns, everyone's done. 🏁
        let the_async_results = futures::future::join_all(the_async_worker_handles).await;

        // 🗑️ Abort the progress reporter — all real workers are done, no more data to display.
        // One final tick to show the end state, then goodnight. 🌙
        the_progress_reporter.abort();
        let _ = the_progress_reporter.await;

        for result in the_async_results {
            // 🤯 result?? — outer `?` unwraps JoinHandle, inner `?` unwraps the work
            result??;
        }

        // 🧵 Join the std::thread handles — should be instant since refiners are already done
        // (ch1 closed → refiners flushed → exited before drainers could finish).
        // This is just the funeral procession. The threads are already at rest. 🪦
        for (i, handle) in the_refiner_thread_handles.into_iter().enumerate() {
            handle
                .join()
                .map_err(|the_panic_drum| {
                    anyhow::anyhow!(
                        "💀 Refiner thread {} panicked — it saw something in the JSON that broke it. \
                         The panic drum: {:?}. \
                         Like a horror movie, but the monster is malformed data.",
                        i,
                        the_panic_drum
                    )
                })?
                .context(format!(
                    "💀 Refiner thread {} returned an error — it tried its best, \
                     but the barrels fought back like a cornered raccoon 🦝",
                    i
                ))?;
        }

        Ok(())
    }
}