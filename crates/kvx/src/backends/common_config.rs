// ai
//! 🎬 *[two configs walk into a struct. one limits batch size. one limits request size.]*
//! *["We belong together," they said. "In the backends module." The module system nodded.]*
//! *["Finally," said the borrow checker. "A config that knows its place."]*
//!
//! 📦 **Common Backend Configs** — the shared DNA of source and sink configuration.
//!
//! 🧠 Knowledge graph:
//! - `CommonSourceConfig`: embedded in every backend source config (`ElasticsearchSourceConfig`,
//!   `FileSourceConfig`). Controls batch size in docs and bytes. Lives here because it's a
//!   backend-level concern — how big a page do we pull?
//! - `CommonSinkConfig`: embedded in every backend sink config (`ElasticsearchSinkConfig`,
//!   `FileSinkConfig`). Controls max request size in bytes. How big a payload do we push?
//! - Both are re-exported from `backends.rs` so callers can `use crate::backends::CommonSinkConfig`
//! - **Former home**: `supervisors/config.rs` — evicted in the Great Config Migration of 2026.
//!   The landlord was `app_config.rs`. The neighbors were happy to see them go.
//!
//! ⚠️ These live in `backends` (not `app_config`) to avoid a circular import:
//!   `app_config` → `backends/es` → `app_config`. The module system has opinions.
//!   Strong ones. And no patience for circular deps. Like the borrow checker's angrier sibling. 🦆
//!
//! "He who puts common config in app_config, creates circular imports in production."
//!   — Ancient Rust module proverb, written in tears at 3am 💀

use serde::Deserialize;

// ============================================================
// 📦 CommonSourceConfig — shared source-side knobs
// ============================================================

/// 📦 Shared configuration embedded by every source backend config.
///
/// Controls how large a "page" the source emits per `next_page()` call.
/// Sources are ignorant of downstream concerns — they just pour raw pages
/// at whatever batch size the config allows. 🚰
///
/// 🧠 Knowledge graph:
/// - Embedded in `ElasticsearchSourceConfig`, `FileSourceConfig` (and future source configs)
/// - `max_batch_size_docs`: doc-count ceiling per page (ES scroll size, etc.)
/// - `max_batch_size_bytes`: byte-size ceiling per page (avoid sending 1GB pages)
/// - The DEFAULT impl gives conservative values (1000 docs / 1MB)
///   while the serde defaults give more generous values (10k docs / 10MB)
///   because apparently we have two opinions and we're committed to both 🦆
///
/// No cap: these defaults were chosen empirically by staring at them until they felt right.
#[derive(Debug, Deserialize, Clone)]
pub struct CommonSourceConfig {
    /// 📦 Max docs per batch page — the doc-count speed limiter
    #[serde(default = "default_max_batch_size_docs")]
    pub max_batch_size_docs: usize,
    /// 📦 Max bytes per batch page — the byte-size speed limiter
    #[serde(default = "default_max_batch_size_bytes")]
    pub max_batch_size_bytes: usize,
}

// 📦 10,000 docs per batch — a nice round number that will age like milk
// the moment someone indexes a 50MB PDF and wonders why things are slow.
fn default_max_batch_size_docs() -> usize {
    10000
}

// 📦 10MB — chosen because 10 is a great number and MB is a great unit.
// This is load-tested in the same way I've "tested" my microwave: empirically, at 3am, with regret.
// 10 * 1024 * 1024 = 10485760. Yes I know. Yes the comment on the line is doing the math. You're welcome.
fn default_max_batch_size_bytes() -> usize {
    10485760
} // -- 10MB — if your documents are bigger, we need to talk

impl Default for CommonSourceConfig {
    fn default() -> Self {
        Self {
            // 🎯 1000 docs / 1MB per batch — sensible defaults chosen by someone who definitely
            // did NOT just pick round numbers and call it "empirically validated"
            max_batch_size_docs: 1000,
            max_batch_size_bytes: 1024 * 1024,
        }
    }
}

// ============================================================
// 🚰 CommonSinkConfig — shared sink-side knobs
// ============================================================

/// 🚰 Shared configuration embedded by every sink backend config.
///
/// Controls the maximum request payload size when sending data to the sink.
/// The `SinkWorker` uses this to decide when to flush its page buffer —
/// accumulate until approaching this limit, then compose + send. 💡
///
/// 🧠 Knowledge graph:
/// - Embedded in `ElasticsearchSinkConfig`, `FileSinkConfig` (and future sink configs)
/// - `max_request_size_bytes`: flush threshold for the SinkWorker buffer
/// - Default is 64MB — generous, because we trust the sink to handle it
///   (and because the Elasticsearch docs said "up to 100MB" and we wanted buffer room) 🔧
/// - Serde default fn gives 10MB (the "I'm being careful" default)
/// - The `Default` impl gives 64MB (the "I'm feeling confident today" default)
/// - These being different is a known quirk. It's not a bug. It's a vibe. 🦆
///
/// Knock knock. Who's there? Race condition. Race condition wh— Who's there?
#[derive(Debug, Deserialize, Clone)]
pub struct CommonSinkConfig {
    /// 🚰 Max payload bytes per sink request — the flush trigger
    #[serde(default = "default_max_request_size_bytes")]
    pub max_request_size_bytes: usize,
    /// 🧠 Throttle strategy — Static (fixed bytes) or Pid (adaptive). Default: Static.
    /// When Static, uses `max_request_size_bytes` as the fixed output.
    /// When Pid, `max_request_size_bytes` is the initial guess if `initial_output_bytes` isn't set.
    #[serde(default)]
    pub throttle: ThrottleConfig,
}

// 🚰 10MB sink request size — the same limit as your email attachment policy,
// your Slack upload quota, and your therapist's patience. Coincidence? Absolutely yes.
fn default_max_request_size_bytes() -> usize {
    10485760
} // -- 10MB — Elasticsearch's feelings

impl Default for CommonSinkConfig {
    fn default() -> Self {
        CommonSinkConfig {
            // 🚰 64MB default request size because we dream big
            // (and because the Elasticsearch docs said "up to 100MB" and we wanted buffer)
            max_request_size_bytes: 64 * 1024 * 1024,
            // 🧊 Static throttle by default — backwards compatible, no feedback loop
            throttle: ThrottleConfig::default(),
        }
    }
}

// ============================================================
// 🧠 ThrottleConfig — adaptive throttling strategy selection
// ============================================================

/// 🧠 Throttle configuration — choose your fighter.
///
/// Controls how the SinkWorker decides the max request size for each bulk request.
/// Either a fixed static value (the OG), or a PID controller that adapts based on
/// measured request latency (the secret sauce, licensed under LICENSE-EE). 🔒
///
/// 🧠 Knowledge graph:
/// - Lives in `common_config.rs` alongside `CommonSinkConfig` because it's a sink-side concern.
/// - Resolved in `lib.rs` into a `ThrottleControllerBackend` which is passed to SinkWorker.
/// - When `Static`: uses `CommonSinkConfig.max_request_size_bytes` as the fixed output.
/// - When `Pid`: builds a `PidControllerBytesToMs` with the configured gains and bounds.
/// - Default: `Static` (backwards compatible — existing configs don't break). 🦆
///
/// ```toml
/// [sink_config.Elasticsearch.throttle]
/// mode = "Pid"
/// set_point_ms = 8000
/// min_bytes = 1048576
/// max_bytes = 104857600
/// initial_output_bytes = 10485760
/// ```
///
/// "He who chooses Static, sleeps peacefully. He who chooses Pid, sleeps never." — Ancient proverb
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "mode", deny_unknown_fields)]
pub enum ThrottleConfig {
    /// 🧊 Fixed byte size. No feedback. Like a rock. Reliable, but not dynamic.
    Static,
    /// 🧠 PID control: adapts byte output based on measured latency.
    /// Licensed under LICENSE-EE (BSL 1.1). The Krabby Patty formula. 🔒
    Pid {
        /// 🎯 Target request duration in milliseconds — the thermostat setting.
        /// For Elasticsearch bulk, 8000ms is a reasonable starting point.
        #[serde(default = "default_pid_set_point_ms")]
        set_point_ms: f64,
        /// 📏 Minimum output in bytes — the floor. Even during a 429 storm,
        /// we won't shrink below this. "A controller needs standards." 🎯
        #[serde(default = "default_pid_min_bytes")]
        min_bytes: usize,
        /// 📏 Maximum output in bytes — the ceiling. Even with a blazing fast server,
        /// we won't go bigger than this. "Hubris has limits." ⚠️
        #[serde(default = "default_pid_max_bytes")]
        max_bytes: usize,
        /// 📦 Initial byte size guess before any measurements arrive.
        /// The PID loop adjusts from here. Your first order at the restaurant. 🍔
        #[serde(default = "default_pid_initial_output_bytes")]
        initial_output_bytes: usize,
    },
}

impl Default for ThrottleConfig {
    /// 🧊 Default is Static — backwards compatible, no surprises. Like comfort food. 🍕
    fn default() -> Self {
        ThrottleConfig::Static
    }
}

impl ThrottleConfig {
    /// 🧊 A static reference to the Static variant — for InMemory sinks that can't produce
    /// a reference from a temporary. The eternal constant. The immovable object. 🪨
    pub const STATIC_DEFAULT: ThrottleConfig = ThrottleConfig::Static;
}

// 🎯 8 seconds — the Goldilocks duration for Elasticsearch bulk requests.
// Not too hot (overloading the cluster), not too cold (underutilizing bandwidth).
// Derived from empirical observation and staring at latency graphs until they "felt right."
fn default_pid_set_point_ms() -> f64 {
    8000.0
}

// 📏 1MB floor — even if the server is choking, we send at least this much.
// Sending less than 1MB per bulk request is like ordering a single french fry. Why bother.
fn default_pid_min_bytes() -> usize {
    1_048_576
}

// 📏 100MB ceiling — the theoretical max. In practice, Elasticsearch says "up to 100MB"
// in the docs, and we choose to believe them. Hope is a strategy. Sometimes.
fn default_pid_max_bytes() -> usize {
    104_857_600
}

// 📦 10MB starting guess — same as the default max_request_size_bytes serde default.
// A sensible initial guess that PID will adjust up or down based on actual latency.
fn default_pid_initial_output_bytes() -> usize {
    10_485_760
}
