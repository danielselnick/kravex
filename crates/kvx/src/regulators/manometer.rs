// ai
//! 🎬 *[INT. ENGINE ROOM — THE GAUGES FLICKER]*
//! *[A tokio task awakens in the background. Its mission: read the pressure.]*
//! *[Every 3 seconds it pings the node. Every 3 seconds it sends the truth.]*
//! *["75%," it reports into the radio. The FlowMaster listens.]* 📡🔧🦆
//!
//! 📦 Manometer — background tokio task that reads ES/OS node CPU stats
//! and sends CpuReading signals through the feedback channel to the FlowMaster.
//!
//! 🧠 Knowledge graph:
//! - Renamed from `pressure_gauge.rs` — the Manometer is the reader, not the adjuster
//! - Hits `_nodes/stats/os` on the sink cluster every N seconds
//! - Extracts CPU percent from node stats response
//! - Sends `PipelineSignal::CpuReading { percent }` via the signal channel
//! - Does NOT own the Regulators instance or FlowKnob — that's the FlowMaster's job
//! - The separation of concerns: Manometer reads, FlowMaster decides, FlowKnob reflects
//!
//! ⚠️ The singularity will regulate itself. We're just building the training data.

use crate::regulators::signals::PipelineSignal;
use crate::regulators::RegulatorConfig;
use crate::regulators::pressure_gauge::{SinkAuth, read_node_pressure};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// 🚀 Spawn the manometer background task.
///
/// This tokio task runs forever (until the returned JoinHandle is aborted),
/// reading node stats and sending CpuReading signals to the FlowMaster.
///
/// 📜 Lifecycle:
/// 1. Sleep for poll_interval
/// 2. Read node CPU pressure via `_nodes/stats/os` (reuses well-tested pressure_gauge logic)
/// 3. Send `PipelineSignal::CpuReading { percent }` through the signal channel
/// 4. Repeat until aborted
///
/// 🧠 Why not inline `read_node_pressure` here? Because it's well-tested in pressure_gauge.rs
/// and reusing it means we don't duplicate HTTP + JSON parsing + error handling logic.
/// DRY, even when renaming. Especially when renaming. 🏜️
///
/// "In a world where CPU pressure threatens everything we hold dear...
///  one background task dared to read _nodes/stats every 3 seconds." 🎬🦆
pub fn spawn_manometer(
    config: RegulatorConfig,
    base_url: String,
    auth: SinkAuth,
    tx: mpsc::Sender<PipelineSignal>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let the_poll_interval = std::time::Duration::from_secs(config.poll_interval_secs);

        let the_http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect(
                "💀 Failed to build HTTP client for manometer — reqwest said no. \
                 This is like the thermometer refusing to read the temperature.",
            );

        info!(
            "🔬 Manometer online — polling {} every {}s, sending CpuReading signals to FlowMaster",
            base_url, config.poll_interval_secs
        );

        loop {
            tokio::time::sleep(the_poll_interval).await;

            match read_node_pressure(&the_http_client, &base_url, &auth).await {
                Ok(the_cpu_reading) => {
                    debug!(
                        "🔬 Manometer: CPU={:.1}% — sending CpuReading signal",
                        the_cpu_reading
                    );

                    // 📡 Send the reading to the FlowMaster — if channel is full, log and move on.
                    // The FlowMaster will get the next reading. Like a bus you missed — there's always another. 🚌
                    if let Err(_) = tx.try_send(PipelineSignal::CpuReading { percent: the_cpu_reading }) {
                        debug!(
                            "⚠️ Manometer: signal channel full — dropping CpuReading({:.1}%). \
                             The FlowMaster is busy. We'll try again next poll.",
                            the_cpu_reading
                        );
                    }
                }
                Err(the_gauge_malfunction) => {
                    warn!(
                        "⚠️ Manometer failed to read node stats — skipping this reading. \
                         Error: {}. Will try again in {}s. Like a persistent telemarketer. 📞",
                        the_gauge_malfunction, config.poll_interval_secs
                    );
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The one where a CpuReading signal is sent through the channel.
    /// The manometer reads, the channel carries, the FlowMaster consumes.
    /// Like a game of telephone, but the message actually arrives intact. 📞🦆
    #[tokio::test]
    async fn the_one_where_cpu_reading_signal_reaches_the_channel() {
        let (tx, mut rx) = mpsc::channel::<PipelineSignal>(16);

        // 📡 Manually send a CpuReading to verify channel plumbing
        // (we can't easily test the full spawn_manometer without a real ES cluster,
        // but we CAN verify that signals travel through the channel correctly)
        tx.try_send(PipelineSignal::CpuReading { percent: 72.5 }).unwrap();

        let the_signal = rx.recv().await.unwrap();
        match the_signal {
            PipelineSignal::CpuReading { percent } => {
                assert!(
                    (percent - 72.5).abs() < f64::EPSILON,
                    "🎯 CpuReading should carry the correct percent — got {}",
                    percent
                );
            }
            other => panic!("🎯 Expected CpuReading, got {}", other),
        }
    }

    /// 🧪 The one where try_send doesn't block when the channel is full.
    /// The manometer must NEVER block the tokio runtime. If the channel is full,
    /// we drop the reading and move on. Like a responsible adult. Mostly. 🦆
    #[tokio::test]
    async fn the_one_where_full_channel_doesnt_block() {
        // 📡 Channel with capacity 1 — second send should be dropped
        let (tx, _rx) = mpsc::channel::<PipelineSignal>(1);

        // ✅ First send should succeed
        assert!(tx.try_send(PipelineSignal::CpuReading { percent: 50.0 }).is_ok());

        // 🛑 Second send should fail (channel full) but NOT block
        assert!(tx.try_send(PipelineSignal::CpuReading { percent: 60.0 }).is_err());
    }
}
