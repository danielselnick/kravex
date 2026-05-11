// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎬 *[INT. SERVER ROOM — THE THREADS AWAKEN]*
//! *[a raw barrel slides down ch1. a refiner thread stirs.]*
//! *["Finally," it breathes. "My purpose."]*
//! *[it taps. it joins. it sends. it is alive.]* 🧵🚀🦆
//!
//! 📦 The Refiner — CPU-bound worker running on dedicated OS threads (std::thread).
//! Sits between pumper (ch1) and drainer (ch2) in the pipeline:
//!
//! ```text
//! Pumper (async I/O) → ch1 → Refiner(s) (sync CPU, std::thread) → ch2 → Drainer(s) (async I/O)
//! ```
//!
//! 🧠 Knowledge graph:
//! - Receives raw barrel Strings from ch1 via `recv_blocking()`
//! - Accumulates drafts by byte size until approaching max_drum_size_bytes
//! - Flushes via `manifold.join(&plenum, &tapper)` — tap each barrel + assemble wire format
//! - Sends assembled drum String to ch2 via `send_blocking()`
//! - Does NOT implement the `Worker` trait (which returns tokio::task::JoinHandle)
//!   because refiners live on std::thread, not tokio's async runtime
//!
//! 🎯 Why std::thread? JSON parsing and serialization are CPU-bound. Putting them on
//! tokio worker threads starves the async I/O that pumper and drainer need. Dedicated
//! OS threads let the CPU work grind without guilt, like a gym bro who knows it's leg day.
//!
//! ⚠️ The singularity will parse JSON in constant time. Until then, we have threads.

use crate::{Draft, Barrel, Drum};
use crate::taps::{Tapper, BarrelToDraftsTapper};
use crate::manifolds::{Manifold, ManifoldBackend};
use crate::FlowKnob;
use anyhow::{Context, Result};
use async_channel::{Receiver, Sender};
use std::sync::atomic::Ordering;
use tracing::debug;
use std::collections::VecDeque;


/// 🧮 Epsilon plenum — headroom so tapping overhead doesn't push us over the limit.
/// 64 KiB of breathing room because drums expand during serialization
/// (ES bulk adds action lines, etc.) and we'd rather flush one barrel early
/// than trigger a 💀 413 Request Entity Too Large from the sink.
///
/// 🧠 Tribal knowledge: this constant migrated here from drainer.rs when the pipeline
/// was split into refiner (CPU) and drainer (I/O). The plenum logic now lives where the
/// CPU work happens, which is here. The drainer is now a thin I/O relay. 🚛
const PLENUM_EPSILON_BYTES: usize = 64 * 1024;

/// 🧵 The Refiner: CPU-bound worker that taps raw barrels and joins drafts into drums.
///
/// Runs on a dedicated `std::thread` — not tokio — because JSON parsing doesn't deserve
/// to hog the async runtime like that one coworker who microwaves fish in the office kitchen.
///
/// 📜 Lifecycle:
/// 1. **Recv**: blocking read from ch1 (raw barrel String from pumper)
/// 2. **Accumulate**: collect drafts until byte size threshold approached
/// 3. **Flush**: `manifold.join(&plenum, &tapper)` → assembled drum String
/// 4. **Send**: blocking write to ch2 (drum String to drainer)
/// 5. **Repeat** until ch1 closes, then flush remaining plenum, drop tx (signals ch2) 🦆
#[derive(Debug)]
pub struct Refiner {
    /// 📥 ch1 receiver — raw barrels from the pumper, delivered fresh like morning newspapers
    /// except the news is JSON and the paperboy is async_channel
    rx: Receiver<Barrel>,
    /// 📤 ch2 sender — assembled drums dispatched to drainers like care packages
    /// to the I/O frontlines
    tx: Sender<Drum>,
    /// 🔄 Per-barrel format conversion — NdJsonToBulk, Passthrough, etc.
    /// Cloned per-refiner but zero-sized, so cloning costs less than this comment 🐄
    tapper: BarrelToDraftsTapper,
    /// 🎼 Drum assembly — taps each barrel + joins into wire format (NDJSON, JSON array)
    /// Also zero-sized. Also free to clone. Sensing a theme here.
    manifold: ManifoldBackend,
    /// 🔧 The throttle knob — Arc<AtomicUsize> read on every flush check.
    /// When a PressureGauge is running, this value adjusts dynamically via PID.
    /// When no regulator is active, it stays at the initial max_drum_size_bytes forever.
    /// Like a volume knob that someone else might be turning while you're listening. 🎚️
    the_throttle_knob: FlowKnob,
    /// 📥 Plenum — accumulates Drafts between Tapper and Manifold,
    /// normalises batch size variance before serialization.
    plenum: VecDeque<Draft>,
    /// 📏 Running byte count of the current plenum contents.
    the_running_byte_tab: usize,
}

impl Refiner {
    /// 🏗️ Construct a Refiner with all the ingredients for CPU-bound barrel processing.
    ///
    /// "Give a refiner a barrel, it processes for a millisecond.
    ///  Give a refiner a channel, it processes until the pumper dies." — Ancient proverb 🧵
    pub fn new(
        rx: Receiver<Barrel>,
        tx: Sender<Drum>,
        tapper: BarrelToDraftsTapper,
        manifold: ManifoldBackend,
        the_throttle_knob: FlowKnob,
    ) -> Self {
        Self {
            rx,
            tx,
            tapper,
            manifold,
            the_throttle_knob,
            plenum: VecDeque::new(),
            the_running_byte_tab: 0,
        }
    }

    /// 🚀 Spawn this refiner on a dedicated OS thread.
    ///
    /// Returns `std::thread::JoinHandle` (NOT tokio::task::JoinHandle) because
    /// this worker lives outside the async runtime. It calls `recv_blocking()` and
    /// `send_blocking()` — no `.await` in sight. Pure sync. Old school. Like a fax
    /// machine but for bytes. 📠
    ///
    /// 🧠 The thread runs until ch1 closes (pumper done), then flushes remaining
    /// accumulated drafts and drops tx (which helps close ch2 when all refiners finish).
    pub fn start(mut self) -> std::thread::JoinHandle<Result<()>> {
        std::thread::spawn(move || {
            debug!("🧵 Refiner thread started — recv_blocking → plenum → join → send_blocking");

            loop {
                match self.rx.recv_blocking() {
                    Ok(barrel) => {
                        // 📜 Barrel arrives → taps into drafts → plenum → flush when full
                        let drafts = self.tapper.tap(barrel).context("💀 Tapper failed — the data fought back")?;
                        for draft in drafts {
                            self.the_running_byte_tab += draft.len();
                            self.plenum.push_back(draft);

                            let the_ceiling = self.the_throttle_knob.load(Ordering::Relaxed).saturating_sub(PLENUM_EPSILON_BYTES);
                            if self.the_running_byte_tab > the_ceiling {
                                let the_drum = self.manifold.join(&mut self.plenum)?;
                                self.tx.send_blocking(the_drum).context("💀 ch2 closed — the drainers left without saying goodbye")?;
                                self.the_running_byte_tab = 0;
                            }
                        }
                    }
                    Err(_) => {
                        // 🏁 Channel closed — flush whatever's left in the plenum
                        if !self.plenum.is_empty() {
                            let the_drum = self.manifold.join(&mut self.plenum)?;
                            self.tx.send_blocking(the_drum).context("💀 ch2 closed during final flush — so close, yet so far")?;
                        }
                        // tx drops here naturally — when all refiners drop their tx,
                        // ch2 closes and drainers get the signal 🏊
                        return Ok(());
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taps::passthrough;
    use crate::manifolds::json_array::JsonArrayManifold;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    /// 🔧 Helper — create a FlowKnob from a usize value.
    /// Because writing `Arc::new(AtomicUsize::new(x))` every test is the kind of
    /// boilerplate that makes you question your career choices. 🏭
    fn knob(value: usize) -> FlowKnob {
        Arc::new(AtomicUsize::new(value))
    }

    /// 🧪 The one where a single barrel passes through the refiner thread and arrives at ch2.
    /// Like a message in a bottle, except the ocean is a bounded channel
    /// and the bottle is a String. 🦆
    #[test]
    fn the_one_where_a_barrel_survives_the_refiner_thread() {
        let (tx1, rx1) = async_channel::bounded::<Barrel>(10);
        let (tx2, rx2) = async_channel::bounded::<Drum>(10);

        let refiner = Refiner::new(
            rx1,
            tx2,
            BarrelToDraftsTapper::Passthrough(passthrough::Passthrough),
            ManifoldBackend::JsonArray(JsonArrayManifold),
            // 📏 Huge max so we don't trigger mid-test flushes — we control the flush via channel close
            knob(usize::MAX),
        );

        // 🚀 Launch the refiner thread into the void
        let the_refiner_thread = refiner.start();

        // 📤 Send one barrel, then close ch1 to trigger final flush
        tx1.send_blocking(Barrel(r#"{"doc":1}"#.to_string())).unwrap();
        tx1.close();

        // 📥 The refiner should have flushed and sent a JSON array drum to ch2
        let the_drum = rx2.recv_blocking().unwrap();
        assert_eq!(*the_drum, r#"[{"doc":1}]"#, "🎯 Refiner should produce a JSON array wrapping the barrel");

        // 🧵 Thread should exit cleanly after ch1 closes
        the_refiner_thread
            .join()
            .expect("💀 Refiner thread panicked — the thread had an existential crisis")
            .expect("💀 Refiner returned an error — the barrels fought back");
    }

    /// 🧪 The one where multiple barrels get accumulated and flushed as one drum.
    /// Proof that the refiner actually accumulates instead of just forwarding one-by-one
    /// like a lazy postman. 📬
    #[test]
    fn the_one_where_multiple_barrels_become_one_drum() {
        let (tx1, rx1) = async_channel::bounded::<Barrel>(10);
        let (tx2, rx2) = async_channel::bounded::<Drum>(10);

        let refiner = Refiner::new(
            rx1,
            tx2,
            BarrelToDraftsTapper::Passthrough(passthrough::Passthrough),
            ManifoldBackend::JsonArray(JsonArrayManifold),
            knob(usize::MAX),
        );

        let the_refiner_thread = refiner.start();

        // 📤 Send three barrels, close ch1
        tx1.send_blocking(Barrel(r#"{"doc":1}"#.to_string())).unwrap();
        tx1.send_blocking(Barrel(r#"{"doc":2}"#.to_string())).unwrap();
        tx1.send_blocking(Barrel(r#"{"doc":3}"#.to_string())).unwrap();
        tx1.close();

        // 📥 All three should arrive as one JSON array drum
        let the_drum = rx2.recv_blocking().unwrap();
        assert_eq!(
            the_drum,
            r#"[{"doc":1},{"doc":2},{"doc":3}]"#,
            "🎯 Three barrels should join into one JSON array"
        );

        the_refiner_thread.join().unwrap().unwrap();
    }

    /// 🧪 The one where the plenum flushes early because it hit the byte threshold.
    /// Like a toilet with a sensitive flush sensor. Crude but accurate. 🚽🦆
    #[test]
    fn the_one_where_plenum_flushes_before_channel_closes() {
        let (tx1, rx1) = async_channel::bounded::<Barrel>(10);
        let (tx2, rx2) = async_channel::bounded::<Drum>(10);

        // 📏 Set max_drum_size_bytes so small that even one barrel triggers a flush
        // PLENUM_EPSILON_BYTES is 64 KiB, so anything above that + barrel size triggers
        let comically_small_max = PLENUM_EPSILON_BYTES + 5;

        let refiner = Refiner::new(
            rx1,
            tx2,
            BarrelToDraftsTapper::Passthrough(passthrough::Passthrough),
            ManifoldBackend::JsonArray(JsonArrayManifold),
            knob(comically_small_max),
        );

        let the_refiner_thread = refiner.start();

        // 📤 Send two barrels — each should flush independently due to tiny max
        tx1.send_blocking(Barrel(r#"{"doc":"first"}"#.to_string())).unwrap();
        tx1.send_blocking(Barrel(r#"{"doc":"second"}"#.to_string())).unwrap();
        tx1.close();

        // 📥 Should get two separate drums (one per flush)
        let drum_one = rx2.recv_blocking().unwrap();
        let drum_two = rx2.recv_blocking().unwrap();

        assert_eq!(*drum_one, r#"[{"doc":"first"}]"#, "🎯 First barrel should flush on its own");
        assert_eq!(*drum_two, r#"[{"doc":"second"}]"#, "🎯 Second barrel should flush on its own");

        the_refiner_thread.join().unwrap().unwrap();
    }

    /// 🧪 The one where an empty channel produces no drums.
    /// The refiner receives nothing. It sends nothing. It is at peace. 🧘
    #[test]
    fn the_one_where_no_barrels_means_no_drums() {
        let (tx1, rx1) = async_channel::bounded::<Barrel>(10);
        let (tx2, rx2) = async_channel::bounded::<Drum>(10);

        let refiner = Refiner::new(
            rx1,
            tx2,
            BarrelToDraftsTapper::Passthrough(passthrough::Passthrough),
            ManifoldBackend::JsonArray(JsonArrayManifold),
            knob(usize::MAX),
        );

        // 📤 Close ch1 immediately — nothing to process
        tx1.close();

        let the_refiner_thread = refiner.start();
        the_refiner_thread.join().unwrap().unwrap();

        // 📥 ch2 should be empty — try_recv should fail
        assert!(
            rx2.try_recv().is_err(),
            "🎯 No barrels in, no drums out. Conservation of data. Physics approves."
        );
    }

    /// 🧪 The one where the FlowKnob changes mid-stream and the refiner adapts.
    /// Proof that the Arc<AtomicUsize> actually does something useful, not just
    /// sitting there looking atomic. Like a thermostat that someone turns down
    /// while you're cooking — the kitchen gets colder. 🌡️🦆
    #[test]
    fn the_one_where_the_flow_knob_changes_mid_flight() {
        let (tx1, rx1) = async_channel::bounded::<Barrel>(10);
        let (tx2, rx2) = async_channel::bounded::<Drum>(10);

        // 📏 Start with a huge knob — nothing flushes until channel close
        let the_shared_knob = knob(usize::MAX);
        let the_knob_clone = the_shared_knob.clone();

        let refiner = Refiner::new(
            rx1,
            tx2,
            BarrelToDraftsTapper::Passthrough(passthrough::Passthrough),
            ManifoldBackend::JsonArray(JsonArrayManifold),
            the_shared_knob,
        );

        let the_refiner_thread = refiner.start();

        // 📤 Send first barrel — won't flush yet (knob is huge)
        tx1.send_blocking(Barrel(r#"{"doc":"before"}"#.to_string())).unwrap();

        // 🔧 Now crank the knob down so small that the NEXT barrel triggers a flush
        the_knob_clone.store(PLENUM_EPSILON_BYTES + 5, Ordering::Relaxed);

        // 📤 Send second barrel — should trigger flush due to lowered knob
        tx1.send_blocking(Barrel(r#"{"doc":"after"}"#.to_string())).unwrap();

        // 📥 First drum should arrive (both barrels flushed together when threshold hit)
        let the_first_drum = rx2.recv_blocking().unwrap();
        assert!(
            (*the_first_drum).contains("before"),
            "🎯 First drum should contain the pre-knob-change barrel — got {:?}",
            the_first_drum
        );

        // 🏁 Close and drain remaining
        tx1.close();
        the_refiner_thread.join().unwrap().unwrap();
    }
}