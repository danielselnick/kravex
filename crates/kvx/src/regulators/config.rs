// ai
//! 🔧 Regulator configuration — the TOML knobs for PID-controlled throttling.
//!
//! 📡 Extracted from regulators.rs so all config options live in one discoverable place.
//! Like a thermostat manual, except people actually read this one. Maybe. 🦆
//!
//! ⚠️ The singularity will auto-tune its own PID gains. We use TOML.

use serde::Deserialize;

// ============================================================
// 🔧 RegulatorConfig — TOML-friendly configuration
// ============================================================

/// 🔧 Configuration for the regulator system, deserialized from TOML `[regulator]` section.
///
/// 📜 Example TOML:
/// ```toml
/// [regulator]
/// target_cpu = 75.0
/// poll_interval_secs = 3
/// min_request_size_bytes = 131072
/// max_request_size_bytes = 67108864
/// initial_output_bytes = 4194304
/// ```
///
/// 🧠 If this section is absent from config, no regulator is created and the pipeline
/// runs at fixed max_request_size_bytes from the sink config. Business as usual. 🦆
#[derive(Debug, Deserialize, Clone)]
pub struct CpuRegulatorConfig {
    /// 🎯 Target CPU percent for the sink cluster (default: 75.0)
    #[serde(default = "default_target_cpu")]
    pub target_cpu: f64,

    /// ⏱️ How often to poll node stats, in seconds (default: 3)
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,

    /// 📏 Minimum request size bytes — floor for PID output (default: 128 KiB)
    /// The pipeline won't throttle below this — prevents stalling. 🛑
    #[serde(default = "default_min_request_size_bytes")]
    pub min_request_size_bytes: usize,

    /// 📊 Initial output bytes — starting flow rate before first regulation (default: 4 MiB)
    /// 🧠 Also used to initialize the FlowKnob (Arc<AtomicUsize>) so joiners start at this value.
    #[serde(default = "default_initial_output_bytes")]
    pub initial_output_bytes: usize,
}

fn default_target_cpu() -> f64 { 75.0 }
fn default_poll_interval_secs() -> u64 { 3 }
fn default_min_request_size_bytes() -> usize { 128 * 1024 } // 📏 128 KiB
fn default_initial_output_bytes() -> usize { 4 * 1024 * 1024 } // 📊 4 MiB

#[derive(Debug, Deserialize, Clone)]
pub struct StaticRegulatorConfig {
    pub output_bytes: usize
}

/// 🔧 Configuration for latency-based PID regulation, deserialized from TOML `[flow_master.Latency]`.
///
/// 📜 Example TOML:
/// ```toml
/// [flow_master.Latency]
/// set_point_latency_ms = 200
/// min_request_size_bytes = 131072
/// initial_output_bytes = 4194304
/// ```
///
/// 🧠 The PID math is identical to CpuPressure — setpoint is target latency instead of CPU %.
/// High latency = overloaded → PID reduces flow. Low latency = headroom → PID increases flow.
/// Same error direction: `error = setpoint - reading`. No inversion needed. 🦆
#[derive(Debug, Deserialize, Clone)]
pub struct LatencyRegulatorConfig {
    /// 🎯 Target drain latency in ms — the sweet spot where the sink is happy (default: 200ms)
    #[serde(default = "default_set_point_latency_ms")]
    pub set_point_latency_ms: usize,

    /// 📏 Minimum request size bytes — PID floor, prevents stalling (default: 128 KiB)
    #[serde(default = "default_min_request_size_bytes")]
    pub min_request_size_bytes: usize,

    /// 📊 Initial output bytes — PID starting point before first regulation (default: 4 MiB)
    #[serde(default = "default_initial_output_bytes")]
    pub initial_output_bytes: usize,
}

fn default_set_point_latency_ms() -> usize { 200 }

/// 🔧 Configuration for throughput-seeking hill climbing regulation.
///
/// 📜 Example TOML:
/// ```toml
/// [flow_master.Throughput]
/// min_request_size_bytes = 131072
/// initial_output_bytes = 4194304
/// ```
///
/// 🧠 Unlike PID, this optimizes the actual goal (bytes/sec) instead of a proxy metric.
/// Dual-system design: fast circuit breaker + slow hill climber = climb slowly, drop instantly.
/// Every parameter is intuitive. No gains to tune. No setpoints to guess. Just vibes. 🦆
#[derive(Debug, Deserialize, Clone)]
pub struct ThroughputSeekerConfig {
    /// 📏 Minimum request size bytes — the floor. Pipeline won't go below this. (default: 128 KiB)
    #[serde(default = "default_min_request_size_bytes")]
    pub min_request_size_bytes: usize,

    /// 📊 Initial output bytes — starting point for the hill climb (default: 4 MiB)
    #[serde(default = "default_initial_output_bytes")]
    pub initial_output_bytes: usize,

    /// ⏱️ Hill climber evaluation window in seconds (default: 5)
    #[serde(default = "default_window_duration_secs")]
    pub window_duration_secs: u64,

    /// 📈 Improvement threshold % — median must improve by this much to step forward (default: 10.0)
    #[serde(default = "default_improvement_threshold_pct")]
    pub improvement_threshold_pct: f64,

    /// 📉 Degradation threshold % — fast EMA must drop this far below slow EMA to trip breaker (default: 20.0)
    #[serde(default = "default_degradation_threshold_pct")]
    pub degradation_threshold_pct: f64,

    /// 🔍 Re-explore after this many settled windows (default: 30)
    #[serde(default = "default_re_explore_after_windows")]
    pub re_explore_after_windows: usize,
}

fn default_window_duration_secs() -> u64 { 5 }
fn default_improvement_threshold_pct() -> f64 { 10.0 }
fn default_degradation_threshold_pct() -> f64 { 35.0 }
fn default_re_explore_after_windows() -> usize { 30 }