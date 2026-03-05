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
//!   backend-level concern — how big a feed do we pull?
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
/// Controls how large a "feed" the source emits per `next_page()` call.
/// Sources are ignorant of downstream concerns — they just pour raw feeds
/// at whatever batch size the config allows. 🚰
///
/// 🧠 Knowledge graph:
/// - Embedded in `ElasticsearchSourceConfig`, `FileSourceConfig` (and future source configs)
/// - `max_batch_size_docs`: doc-count ceiling per feed (ES scroll size, etc.)
/// - `max_batch_size_bytes`: byte-size ceiling per feed (avoid sending 1GB feeds)
/// - The DEFAULT impl gives conservative values (1000 docs / 1MB)
///   while the serde defaults give more generous values (10k docs / 10MB)
///   because apparently we have two opinions and we're committed to both 🦆
///
/// No cap: these defaults were chosen empirically by staring at them until they felt right.
#[derive(Debug, Deserialize, Clone)]
pub struct CommonSourceConfig {
    /// 📦 Max docs per batch feed — the doc-count speed limiter
    #[serde(default = "default_max_batch_size_docs")]
    pub max_batch_size_docs: usize,
    /// 📦 Max bytes per batch feed — the byte-size speed limiter
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
/// The `Drainer` uses this to decide when to flush its feed buffer —
/// accumulate until approaching this limit, then join + send. 💡
///
/// 🧠 Knowledge graph:
/// - Embedded in `ElasticsearchSinkConfig`, `FileSinkConfig` (and future sink configs)
/// - `max_request_size_bytes`: flush threshold for the Drainer buffer
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
        }
    }
}
