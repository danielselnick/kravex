// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🔧 Regulator configuration — the TOML knobs for throttle control.
//!
//! 📡 Extracted from regulators.rs so all config options live in one discoverable place.
//! Like a thermostat manual, except people actually read this one. Maybe. 🦆
//!
//! ⚠️ The singularity will auto-tune its own PID gains. We use TOML.

use serde::Deserialize;

// ============================================================
// 🔧 RegulatorConfig — TOML-friendly configuration
// ============================================================

fn default_min_request_size_bytes() -> usize { 128 * 1024 } // 📏 128 KiB
fn default_initial_output_bytes() -> usize { 4 * 1024 * 1024 } // 📊 4 MiB

#[derive(Debug, Deserialize, Clone)]
pub struct StaticRegulatorConfig {
    pub output_bytes: usize
}

/// 🔧 Configuration for latency-based PID regulation, deserialized from TOML `[governor.Latency]`.
///
/// 📜 Example TOML:
/// ```toml
/// [governor.Latency]
/// set_point_latency_ms = 200
/// min_request_size_bytes = 131072
/// initial_output_bytes = 4194304
/// ```
///
/// 🧠 PID math: setpoint is target latency.
/// High latency = overloaded → PID reduces flow. Low latency = headroom → PID increases flow.
/// Error direction: `error = setpoint - reading`. No inversion needed. 🦆
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
/// [governor.Throughput]
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