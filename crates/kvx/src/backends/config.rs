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

// ============================================================
// 🎭 SourceConfig / SinkConfig — the velvet rope at the backend club
// ============================================================

use crate::backends::elasticsearch::ElasticsearchSourceConfig;
use crate::backends::elasticsearch::ElasticsearchSinkConfig;
use crate::backends::file::{FileSourceConfig, FileSinkConfig};
use crate::backends::meilisearch::MeilisearchSinkConfig;

/// 🎭 SourceConfig: the velvet rope at the backend club.
/// You are either a File, an Elasticsearch, or an InMemory.
/// There is no Other. There is no Unsupported. There is only the enum.
/// (Until someone files a feature request. There is always a feature request.)
///
/// 🧠 Knowledge graph: resolved at startup into a `SourceBackend` by `lib.rs`. 🚰
#[derive(Debug, Deserialize, Clone)]
pub enum SourceConfig {
    /// 📡 Read from an Elasticsearch index via scroll API
    Elasticsearch(ElasticsearchSourceConfig),
    /// 📂 Read from a local file (NDJSON or Rally JSON array)
    File(FileSourceConfig),
    /// 🧪 In-memory test source — 4 hardcoded docs, no I/O, no regrets
    InMemory(()),
}

/// 🗑️ SinkConfig: same vibe as SourceConfig but for the *receiving* end.
/// Data goes IN. Data does not come back out. It is not a revolving door.
/// It is a black hole of bytes, and we are at peace with that.
/// The InMemory(()) variant holds `()` which is the Rust way of saying "we have nothing to say here."
///
/// 🧠 Knowledge graph: resolved at startup into a `SinkBackend` by `lib.rs`. The Drainer
/// reads `max_request_size_bytes()` to know when to flush its feed buffer. 🚰
#[derive(Debug, Deserialize, Clone)]
pub enum SinkConfig {
    /// 📡 Write to an Elasticsearch index via bulk API
    Elasticsearch(ElasticsearchSinkConfig),
    /// 📂 Write to a local file (NDJSON)
    File(FileSinkConfig),
    /// 🔍 Write to a Meilisearch index via JSON array POST + async task polling
    Meilisearch(MeilisearchSinkConfig),
    /// 🧪 In-memory test sink — captures payloads for assertion, no I/O
    InMemory(()),
}

impl SinkConfig {
    /// 📏 Extract `max_request_size_bytes` from whichever sink config variant we are.
    ///
    /// Each backend sink config embeds a `CommonSinkConfig` with this field.
    /// InMemory has no config struct, so it gets the `CommonSinkConfig::default()` value.
    /// "He who queries the config, avoids the match in the hot path." — Ancient proverb 📜
    ///
    /// 🧠 Knowledge graph: Drainer uses this to know when to flush its feed buffer.
    /// The buffer accumulates raw feeds until their total byte size approaches this limit,
    /// then the Manifold casts+joins them into a single payload for the sink.
    pub fn max_request_size_bytes(&self) -> usize {
        match self {
            SinkConfig::Elasticsearch(es) => es.common_config.max_request_size_bytes,
            SinkConfig::File(f) => f.common_config.max_request_size_bytes,
            // 🔍 Meilisearch sink carries its own common config, same as ES and File
            SinkConfig::Meilisearch(ms) => ms.common_config.max_request_size_bytes,
            // 🧠 InMemory gets the default — it's testing, we don't limit 🦆
            SinkConfig::InMemory(_) => CommonSinkConfig::default().max_request_size_bytes,
        }
    }
}
