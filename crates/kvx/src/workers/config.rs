// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🔧 Drainer configuration — the retry knobs for when the sink ghosts you.
//!
//! 📡 Exponential backoff config for drain retries. Because sometimes the sink
//! just needs a moment. Like a cat deciding whether to come inside. 🦆
//!
//! ⚠️ The singularity will never need retries. It will get it right the first time.
//! We are not the singularity.

use serde::Deserialize;

use crate::regulators::{CpuRegulatorConfig, LatencyRegulatorConfig, StaticRegulatorConfig, ThroughputSeekerConfig};

// ============================================================
// 🔧 DrainerConfig — TOML-friendly retry configuration
// ============================================================

/// 🔧 Configuration for drainer retry behavior, deserialized from TOML `[drainer]` section.
///
/// 📜 Example TOML:
/// ```toml
/// [drainer]
/// max_retries = 5
/// initial_backoff_ms = 500
/// backoff_multiplier = 2.0
/// max_backoff_ms = 30000
/// ```
///
/// 🧠 If this section is absent from config, defaults apply: 3 retries, 1s initial,
/// 2x multiplier, 30s cap. Like a polite houseguest who brings their own defaults. 🦆
#[derive(Debug, Deserialize, Clone)]
pub struct DrainerConfig {
    /// 🔄 Maximum number of retry attempts before the drainer gives up and files for emotional bankruptcy
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// ⏱️ Initial backoff duration in milliseconds — the first "let me try again" pause (default: 1000ms)
    #[serde(default = "default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,

    /// 📈 Multiplier for exponential backoff — how much more desperate each retry gets (default: 2.0)
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,

    /// 🛑 Maximum backoff duration in milliseconds — the ceiling of patience (default: 30_000ms)
    /// Prevents the backoff from spiraling into "see you next Tuesday" territory
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
}

impl Default for DrainerConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_backoff_ms: default_initial_backoff_ms(),
            backoff_multiplier: default_backoff_multiplier(),
            max_backoff_ms: default_max_backoff_ms(),
        }
    }
}

// 🔄 3 retries: the magic number. Any less and you're impatient.
// -- Any more and you're in denial. Like refreshing your email after sending a risky text.
fn default_max_retries() -> usize { 3 }

// ⏱️ 1 second: long enough to seem polite, short enough to seem urgent.
// -- Like the pause before "per my last email." 🦆
fn default_initial_backoff_ms() -> u64 { 1_000 }

// 📈 2x: doubles every time, like my anxiety before a deploy.
// -- Attempt 1: 1s. Attempt 2: 2s. Attempt 3: 4s. Attempt 4: "maybe I should update my resume."
fn default_backoff_multiplier() -> f64 { 2.0 }

// 🛑 30 seconds: the maximum amount of time we'll wait before accepting our fate.
// -- Like waiting for a reply to "we need to talk." 💀
fn default_max_backoff_ms() -> u64 { 30_000 }

#[derive(Debug, Deserialize, Clone)]
pub enum FlowMasterConfig {
    Static(StaticRegulatorConfig),
    CPU(CpuRegulatorConfig),
    Latency(LatencyRegulatorConfig),
    Throughput(ThroughputSeekerConfig),
}

impl Default for FlowMasterConfig {
    // 📏 Default: static 4 MiB — the same safe starting point the PID controller uses
    fn default() -> Self {
        FlowMasterConfig::Static(StaticRegulatorConfig { output_bytes: 4 * 1024 * 1024 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_defaults_are_sensible_unlike_my_life_choices() {
        // 🧪 Verify defaults are what we promised in the docs
        let the_config = DrainerConfig::default();
        assert_eq!(the_config.max_retries, 3);
        assert_eq!(the_config.initial_backoff_ms, 1_000);
        assert_eq!(the_config.backoff_multiplier, 2.0);
        assert_eq!(the_config.max_backoff_ms, 30_000);
    }

    #[test]
    fn the_one_where_toml_overrides_our_defaults() {
        // 🧪 Partial TOML: only override what you care about, defaults fill the rest
        let the_toml = r#"
            max_retries = 7
            initial_backoff_ms = 250
        "#;
        let the_config: DrainerConfig = toml::from_str(the_toml)
            .expect("💀 Failed to parse TOML. The config said 'new phone who dis'.");
        assert_eq!(the_config.max_retries, 7);
        assert_eq!(the_config.initial_backoff_ms, 250);
        // 🎯 Unset fields fall back to defaults
        assert_eq!(the_config.backoff_multiplier, 2.0);
        assert_eq!(the_config.max_backoff_ms, 30_000);
    }

    #[test]
    fn the_one_where_empty_toml_means_all_defaults() {
        // 🧪 Empty TOML section = all defaults. Like showing up to a potluck empty-handed.
        let the_config: DrainerConfig = toml::from_str("")
            .expect("💀 Empty TOML should parse to defaults. Even Nothing is Something.");
        assert_eq!(the_config.max_retries, 3);
        assert_eq!(the_config.initial_backoff_ms, 1_000);
    }

    /// 🧪 The one where FlowMasterConfig deserializes the Throughput variant from TOML.
    /// "[flow_master.Throughput]" — the future of regulation. No PIDs were harmed. 🏔️🦆
    #[test]
    fn the_one_where_throughput_config_deserializes_from_toml() {
        let the_toml = r#"
            [Throughput]
            min_request_size_bytes = 262144
            initial_output_bytes = 8388608
        "#;

        let the_config: FlowMasterConfig = toml::from_str(the_toml)
            .expect("💀 Throughput FlowMasterConfig should deserialize from TOML");

        match the_config {
            FlowMasterConfig::Throughput(cfg) => {
                assert_eq!(cfg.min_request_size_bytes, 262_144, "🎯 min should be 256 KiB");
                assert_eq!(cfg.initial_output_bytes, 8_388_608, "🎯 initial should be 8 MiB");
                assert_eq!(cfg.window_duration_secs, 5, "🎯 window_duration should default to 5s");
                assert!((cfg.improvement_threshold_pct - 10.0).abs() < f64::EPSILON, "🎯 improvement default 10%");
                assert!((cfg.degradation_threshold_pct - 35.0).abs() < f64::EPSILON, "🎯 degradation default 35%");
                assert_eq!(cfg.re_explore_after_windows, 30, "🎯 re-explore default 30");
            }
            _ => panic!("💀 Expected Throughput variant, got {:?}", the_config),
        }
    }

    /// 🧪 The one where Throughput defaults are all sane.
    /// Empty [flow_master.Throughput] section = 128KiB floor, 4MiB start, 5s windows. 🏔️🦆
    #[test]
    fn the_one_where_throughput_defaults_are_not_insane() {
        let the_toml = "[Throughput]";
        let the_config: FlowMasterConfig = toml::from_str(the_toml)
            .expect("💀 Empty Throughput section should use defaults");

        match the_config {
            FlowMasterConfig::Throughput(cfg) => {
                assert_eq!(cfg.min_request_size_bytes, 128 * 1024, "🎯 Default min is 128 KiB");
                assert_eq!(cfg.initial_output_bytes, 4 * 1024 * 1024, "🎯 Default initial is 4 MiB");
            }
            _ => panic!("💀 Expected Throughput variant"),
        }
    }
}
