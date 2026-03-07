// ai
//! 🎬 *[INT. VALVE CONTROL ROOM — THE DIALS SPIN]*
//! *[A single entity sits at the controls. It receives readings. It adjusts the flow.]*
//! *[Not too fast. Not too slow. The Goldilocks of data throughput.]*
//! *["I am the FlowMaster," it announces. "And this porridge is JUST right."]* 🔧📡🦆
//!
//! 📦 FlowMaster — the unified regulator worker that listens to GaugeReading signals
//! and adjusts the FlowKnob that Joiners read to size their payloads.
//!
//! 🧠 Knowledge graph:
//! ```text
//! Drainer(s) --[ch3: GaugeReading::LatencyMs]--> FlowMaster
//!   → regulator.regulate(reading, dt) → new flow rate (bytes)
//!     → FlowKnob: Arc<AtomicUsize> (effective max_request_size_bytes)
//!       → Joiner reads flow knob on every flush check
//! ```
//!
//! 🔄 Shutdown: all Drainers exit → their tx3 clones drop → ch3 closes → FlowMaster exits.
//! Pure RAII cascade. No `.close()` calls. No `.abort()`. Just vibes and reference counting.
//!
//! ⚠️ The singularity will self-regulate without channels. We use async_channel and cope.

use std::sync::atomic::Ordering;
use std::time::SystemTime;

use anyhow::Result;
use async_channel::Receiver;
use tokio::task::JoinHandle;
use tracing::{debug, info};

use crate::GaugeReading;
use crate::regulators::{Regulate, Regulators};
use crate::regulators::pressure_gauge::FlowKnob;
use super::Worker;

/// 🎛️ The FlowMaster: receives gauge readings, feeds a PID regulator, adjusts the FlowKnob.
///
/// Like a DJ reading the room and adjusting the volume — except the room is a cluster,
/// the music is bulk payloads, and nobody asked for this metaphor. 🎧🦆
pub struct FlowMaster {
    /// 📥 Channel receiver — GaugeReading signals from Drainers (latency) or future CPU poller
    rx: Receiver<GaugeReading>,
    /// 🎛️ The regulator — PID math that converts readings into flow rates
    regulator: Regulators,
    /// 🔧 The shared atomic valve — Joiners read this to size their payloads
    the_flow_knob: FlowKnob,
}

impl FlowMaster {
    /// 🏗️ Construct a FlowMaster — a receiver, a regulator, and a knob to turn. 🔧
    ///
    /// "In the beginning there was a channel, a PID, and an atomic.
    ///  And the FlowMaster said: let there be regulated throughput." — Genesis 2:1 (Tokio Edition) 🦆
    pub fn new(
        rx: Receiver<GaugeReading>,
        regulator: Regulators,
        the_flow_knob: FlowKnob,
    ) -> Self {
        Self {
            rx,
            regulator,
            the_flow_knob,
        }
    }
}

impl Worker for FlowMaster {
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            info!("🎛️ FlowMaster online — listening for gauge readings, regulating the flow");
            let mut the_last_time_we_checked = SystemTime::now();

            loop {
                match self.rx.recv().await {
                    Ok(the_gauge_reading) => {
                        let since_the_last_time_we_checked = SystemTime::now()
                            .duration_since(the_last_time_we_checked)
                            .unwrap_or_default();

                        let the_new_flow = self.regulator.regulate(
                            the_gauge_reading,
                            since_the_last_time_we_checked,
                        );

                        the_last_time_we_checked = SystemTime::now();

                        // 🔧 Store the regulated output to the FlowKnob — Joiners will pick it up
                        let the_old_flow = self.the_flow_knob.swap(
                            the_new_flow as usize,
                            Ordering::Relaxed,
                        );

                        debug!(
                            "🎛️ FlowMaster: regulated {} → {} bytes (Δ{})",
                            the_old_flow,
                            the_new_flow as usize,
                            (the_new_flow as i64) - (the_old_flow as i64)
                        );
                    }
                    Err(_) => {
                        // 🏁 All senders dropped — ch3 closed — Drainers are done. Time to rest.
                        info!("🏁 FlowMaster: ch3 closed. All drainers done. Regulation complete. Goodnight. 💤");
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
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use crate::regulators::{ByteValue, CpuPressure};

    /// 🧪 The one where FlowMaster receives a reading and adjusts the knob.
    /// Like a thermostat that actually listens. Unlike my office thermostat. 🌡️🦆
    #[tokio::test]
    async fn the_one_where_flow_master_adjusts_the_knob() {
        // 🔧 Set up: PID with 200ms setpoint, 128KiB min, 64MiB max, start at 4MiB
        let the_knob: FlowKnob = Arc::new(AtomicUsize::new(4_194_304));
        let the_regulator = Regulators::CpuPressure(CpuPressure::new(
            200.0, 131_072.0, 67_108_864.0, 4_194_304.0,
        ));

        let (tx, rx) = async_channel::bounded(16);
        let the_flow_master = FlowMaster::new(rx, the_regulator, the_knob.clone());

        // 🚀 Spawn FlowMaster
        let the_handle = the_flow_master.start();

        // 📡 Send a low-latency reading — PID should increase flow (headroom)
        tx.send(GaugeReading::LatencyMs(50)).await.unwrap();

        // 💤 Give FlowMaster a moment to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let the_knob_value = the_knob.load(Ordering::Relaxed);
        assert!(
            the_knob_value > 0,
            "🎯 FlowKnob should have a positive value after regulation — got {}",
            the_knob_value
        );

        // 🏁 Drop sender → ch3 closes → FlowMaster exits
        drop(tx);
        the_handle.await.unwrap().unwrap();
    }

    /// 🧪 The one where FlowMaster exits cleanly when ch3 closes.
    /// Pure RAII shutdown — no abort needed. The channel said "goodbye" and FlowMaster listened. 🚪
    #[tokio::test]
    async fn the_one_where_flow_master_exits_when_channel_closes() {
        let the_knob: FlowKnob = Arc::new(AtomicUsize::new(1_000_000));
        let the_regulator = Regulators::Static(ByteValue::new(42.0));

        let (tx, rx) = async_channel::bounded(16);
        let the_flow_master = FlowMaster::new(rx, the_regulator, the_knob);

        let the_handle = the_flow_master.start();

        // 🏁 Immediately drop sender — FlowMaster should exit gracefully
        drop(tx);

        let honestly_who_knows = the_handle.await.unwrap();
        assert!(
            honestly_who_knows.is_ok(),
            "🎯 FlowMaster should exit Ok when channel closes — got {:?}",
            honestly_who_knows
        );
    }

    /// 🧪 The one where FlowMaster stores the PID output after multiple readings.
    /// Feed it high latency → flow should decrease. Like a bouncer at a club:
    /// "Too crowded? Slow the line." 🚪🦆
    #[tokio::test]
    async fn the_one_where_high_latency_reduces_the_flow() {
        let the_initial_flow = 4_194_304_usize;
        let the_knob: FlowKnob = Arc::new(AtomicUsize::new(the_initial_flow));
        let the_regulator = Regulators::CpuPressure(CpuPressure::new(
            200.0, 131_072.0, 67_108_864.0, the_initial_flow as f64,
        ));

        let (tx, rx) = async_channel::bounded(256);
        let the_flow_master = FlowMaster::new(rx, the_regulator, the_knob.clone());
        let the_handle = the_flow_master.start();

        // 📡 Send sustained high-latency readings — PID should reduce flow
        for _ in 0..20 {
            tx.send(GaugeReading::LatencyMs(500)).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // 💤 Let FlowMaster process all readings
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let the_final_flow = the_knob.load(Ordering::Relaxed);
        assert!(
            the_final_flow < the_initial_flow,
            "🎯 High latency (500ms vs 200ms setpoint) should reduce flow — got {} (started at {})",
            the_final_flow, the_initial_flow
        );

        drop(tx);
        the_handle.await.unwrap().unwrap();
    }
}
