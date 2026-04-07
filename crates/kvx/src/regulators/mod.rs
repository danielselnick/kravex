// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎬 *[INT. CONTROL TOWER — DAWN BREAKS]*
//! *[Below, the pipeline roars. Data flows like a river. But rivers flood.]*
//! *[The regulators stir. They exist for this. To hold the line.]*
//! *["Not too fast," they murmur. "Not too slow. Just right."]*  🔧📡🦆
//!
//! 📦 Regulators — the throttle control layer between pipeline velocity and cluster health.
//!
//! 🧠 Knowledge graph:
//! ```text
//! Drainer sends GaugeReading::DrainResult { payload_bytes, latency_ms }
//!   → FlowMaster receives on ch3
//!     → Regulator.regulate(reading, dt) → new flow rate (bytes)
//!       → FlowKnob: Arc<AtomicUsize> (effective max_request_size_bytes)
//!         → Joiner reads flow knob on every flush check
//! ```
//!
//! - `Regulate` trait: `fn regulate(&mut self, reading: GaugeReading, dt: Duration) -> f64`
//! - `Regulators` enum: dispatches to Pid, ByteValue (static), or ThroughputSeeker
//! - All gauge readings originate from drains
//!
//! ⚠️ The singularity will self-regulate. We're just practicing.

pub mod config;
pub mod pid_controller;
pub mod static_regulator;
pub mod throughput_seeker;

use std::time::Duration;

pub use config::StaticRegulatorConfig;
pub use config::LatencyRegulatorConfig;
pub use config::ThroughputSeekerConfig;
pub use pid_controller::PidController;
pub use static_regulator::ByteValue;
pub use throughput_seeker::ThroughputSeeker;

use crate::GaugeReading;

// ============================================================
// 🎛️ Regulate trait — the contract for all regulators
// ============================================================

/// 🔄 The Regulate trait — it needs to regulate itself hehehe.
///
/// Takes a pressure reading and time delta, returns an adjusted output.
/// Like a thermostat, but for bytes. And the house is on fire. 🔥
pub trait Regulate {
    /// 🔄 Feed a reading, get an adjusted output.
    /// - `reading`: the measured value (CPU %, latency ms, whatever)
    /// - `since_last_checked_ms`: time since last call in milliseconds
    /// - Returns: new output value (bytes, typically)
    fn regulate(&mut self, reading: GaugeReading, since_last_checked_ms: Duration) -> f64;
}

// ============================================================
// 🎭 Regulators enum — the dispatcher
// ============================================================

/// 🎭 Regulators — enum dispatcher for concrete regulator implementations.
///
/// Like a union of thermostats: one does PID, one returns a constant.
/// Knock knock. *Who's there?* Match arm. *Match arm wh—*
/// `Regulators::Static(v) => v.regulate(r, dt)` 🚪
#[derive(Debug, Clone)]
pub enum Regulators {
    /// 📏 Fixed value — no regulation, just vibes
    Static(ByteValue),
    /// 🎛️ PID controller — regulates based on a setpoint (latency, etc.)
    Pid(PidController),
    /// 🏔️ Throughput-seeking hill climber — directly optimizes bytes/sec
    ThroughputSeeker(ThroughputSeeker),
}

impl Regulators {
    /// 🏗️ Create a Regulators instance from latency config.
    /// PID math is generic — setpoint becomes target latency,
    /// error direction: high reading = overloaded → reduce output. 🎛️
    ///
    /// 📏 `sink_max_request_size_bytes` is the hard ceiling from the sink config —
    /// the PID won't suggest payloads bigger than what the sink can physically accept. 🦆
    pub fn from_latency_config(config: &LatencyRegulatorConfig, sink_max_request_size_bytes: usize) -> Self {
        Regulators::Pid(PidController::new(
            config.set_point_latency_ms as f64,
            config.min_request_size_bytes as f64,
            sink_max_request_size_bytes as f64,
            config.initial_output_bytes as f64,
        ))
    }

    /// 🏗️ Create a Regulators instance from throughput seeker config.
    /// No PID, no setpoints, no guessing — just climb toward peak throughput.
    /// Like a GPS that optimizes for "fastest route" instead of "shortest distance." 🏔️🦆
    pub fn from_throughput_config(config: &ThroughputSeekerConfig, sink_max_request_size_bytes: usize) -> Self {
        Regulators::ThroughputSeeker(ThroughputSeeker::new(
            config,
            sink_max_request_size_bytes as f64,
        ))
    }
}

impl Regulate for Regulators {
    fn regulate(&mut self, reading: GaugeReading, since_last_checked_ms: Duration) -> f64 {
        match self {
            Regulators::Static(the_byte_value) => the_byte_value.regulate(reading, since_last_checked_ms),
            Regulators::Pid(the_pid) => the_pid.regulate(reading, since_last_checked_ms),
            Regulators::ThroughputSeeker(the_seeker) => the_seeker.regulate(reading, since_last_checked_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The one where the enum dispatches to the right regulator.
    /// Pattern matching: the least dramatic form of decision-making in Rust. 🎭
    #[test]
    fn the_one_where_enum_dispatch_actually_dispatches() {
        // 📏 Static variant — should return fixed value regardless of what you feed it
        let mut the_static = Regulators::Static(ByteValue::new(42.0));
        assert_eq!(the_static.regulate(GaugeReading::DrainResult { payload_bytes: 0, latency_ms: 999 }, Duration::from_millis(1000)), 42.0, "🎯 Static should return 42 regardless");

        // 🎛️ PID variant — should return something different from initial after regulation
        let mut the_pid = Regulators::Pid(PidController::new(75.0, 100.0, 1_000_000.0, 500_000.0));
        let the_first_output = the_pid.regulate(GaugeReading::DrainResult { payload_bytes: 0, latency_ms: 50 }, Duration::from_millis(3000));
        assert!(the_first_output > 0.0, "🎯 PID should return a positive value — got {}", the_first_output);
    }

    /// 🧪 The one where from_latency_config creates a PID that responds to latency.
    /// Same PID, different vibes. Like a thermostat that measures response time instead of heat. 🦆
    #[test]
    fn the_one_where_from_latency_config_creates_pid() {
        let the_config = LatencyRegulatorConfig {
            set_point_latency_ms: 200,
            min_request_size_bytes: 131_072,
            initial_output_bytes: 4_194_304,
        };

        let mut the_regulator = Regulators::from_latency_config(&the_config, 67_108_864);

        // 📡 Low latency (50ms vs 200ms setpoint) → headroom → PID should increase flow
        let the_output = the_regulator.regulate(GaugeReading::DrainResult { payload_bytes: 0, latency_ms: 50 }, Duration::from_millis(3000));
        assert!(the_output > 0.0, "🎯 from_latency_config regulator should produce positive output");
    }

    /// 🧪 The one where LatencyRegulatorConfig deserializes with defaults.
    /// Empty TOML = 200ms setpoint, 128 KiB min, 4 MiB initial. The sensible defaults club. 🏛️
    #[test]
    fn the_one_where_latency_config_defaults_are_sane() {
        let the_config: LatencyRegulatorConfig = toml::from_str("")
            .expect("💀 Empty TOML should produce sane latency defaults");

        assert_eq!(the_config.set_point_latency_ms, 200, "🎯 Default setpoint is 200ms");
        assert_eq!(the_config.min_request_size_bytes, 128 * 1024, "🎯 Default min is 128 KiB");
        assert_eq!(the_config.initial_output_bytes, 4 * 1024 * 1024, "🎯 Default initial is 4 MiB");
    }

    /// 🧪 The one where LatencyRegulatorConfig TOML overrides work.
    /// Partial overrides: the TOML says "200ms setpoint but bigger floor" and serde obliges. 🎛️
    #[test]
    fn the_one_where_latency_config_toml_overrides_work() {
        let the_toml = r#"
            set_point_latency_ms = 150
            min_request_size_bytes = 262144
        "#;

        let the_config: LatencyRegulatorConfig = toml::from_str(the_toml)
            .expect("💀 Partial latency TOML should deserialize");

        assert_eq!(the_config.set_point_latency_ms, 150, "🎯 Setpoint overridden to 150ms");
        assert_eq!(the_config.min_request_size_bytes, 262_144, "🎯 Min overridden to 256 KiB");
        assert_eq!(the_config.initial_output_bytes, 4 * 1024 * 1024, "🎯 Initial kept default 4 MiB");
    }
}
