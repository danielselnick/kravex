// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
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
//!   backend-level concern — how big a barrel do we pull?
//! - `CommonSinkConfig`: embedded in every backend sink config (`ElasticsearchSinkConfig`,
//!   `FileSinkConfig`). Controls max request size in bytes. How big a drum do we push?
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
/// Controls how large a "barrel" the source emits per `pump()` call.
/// Sources are ignorant of downstream concerns — they just pour raw barrels
/// at whatever batch size the config allows. 🚰
///
/// 🧠 Knowledge graph:
/// - Embedded in `ElasticsearchSourceConfig`, `FileSourceConfig` (and future source configs)
/// - `max_barrel_size_docs`: doc-count ceiling per barrel (ES scroll size, etc.)
/// - `max_barrel_size_bytes`: byte-size ceiling per barrel (avoid sending 1GB barrels)
/// - Both serde defaults and the `Default` trait use 1000 docs / 1MB — conservative and safe.
///
/// No cap: these defaults were chosen empirically by staring at them until they felt right.
#[derive(Debug, Deserialize, Clone)]
pub struct CommonSourceConfig {
    /// 📦 Max docs per batch barrel — the doc-count speed limiter
    #[serde(default = "default_max_barrel_size_docs")]
    pub max_barrel_size_docs: usize,
    /// 📦 Max bytes per batch barrel — the byte-size speed limiter
    #[serde(default = "default_max_barrel_size_bytes" )]
    pub max_barrel_size_bytes: usize,
}

// 📦 1,000 docs per batch — conservative and safe.
fn default_max_barrel_size_docs() -> usize {
    1000
}

// 📦 1 MB — conservative and safe.
fn default_max_barrel_size_bytes() -> usize {
    1024 * 1024
}

impl Default for CommonSourceConfig {
    fn default() -> Self {
        Self {
            max_barrel_size_docs: default_max_barrel_size_docs(),
            max_barrel_size_bytes: default_max_barrel_size_bytes(),
        }
    }
}

// ============================================================
// 🚰 CommonSinkConfig — shared sink-side knobs
// ============================================================

/// 🚰 Shared configuration embedded by every sink backend config.
///
/// Controls the maximum request drum size when sending data to the sink.
/// The `Refiner` uses this to decide when to flush the accumulator —
/// accumulate Drafts until approaching this limit, then join → send. 💡
///
/// 🧠 Knowledge graph:
/// - Embedded in `ElasticsearchSinkConfig`, `FileSinkConfig` (and future sink configs)
/// - `max_drum_size_bytes`: flush threshold for the Refiner accumulator
/// - Default is 64MB — generous but safe for ES `_bulk` APIs.
///
/// Knock knock. Who's there? 64 meg. 64 meg who? 64 megabytes per batch.
#[derive(Debug, Deserialize, Clone)]
pub struct CommonSinkConfig {
    /// 🚰 Max drum bytes per sink request — the flush trigger
    #[serde(default = "default_max_drum_size_bytes")]
    pub max_drum_size_bytes: usize,
}

// 🚰 64MB sink request size — generous but safe for ES `_bulk` API limits.
fn default_max_drum_size_bytes() -> usize {
    64 * 1024 * 1024
}

impl Default for CommonSinkConfig {
    fn default() -> Self {
        CommonSinkConfig {
            max_drum_size_bytes: default_max_drum_size_bytes(),
        }
    }
}

// ============================================================
// 🎭 SourceConfig / SinkConfig — the velvet rope at the backend club
// ============================================================

use crate::backends::elasticsearch::ElasticsearchSinkConfig;
use crate::backends::elasticsearch::ElasticsearchSourceConfig;
use crate::backends::file::{FileSinkConfig, FileSourceConfig};

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
/// reads `max_drum_size_bytes()` to know when to flush its accumulator. 🚰
#[derive(Debug, Deserialize, Clone)]
pub enum SinkConfig {
    /// 📡 Write to an Elasticsearch index via bulk API
    Elasticsearch(ElasticsearchSinkConfig),
    /// 📂 Write to a local file (NDJSON)
    File(FileSinkConfig),
    /// 🧪 In-memory test sink — captures drums for assertion, no I/O
    InMemory(()),
}

impl SinkConfig {
    /// 📏 Extract `max_drum_size_bytes` from whichever sink config variant we are.
    ///
    /// Each backend sink config embeds a `CommonSinkConfig` with this field.
    /// InMemory has no config struct, so it gets the `CommonSinkConfig::default()` value.
    /// "He who queries the config, avoids the match in the hot path." — Ancient proverb 📜
    ///
    /// 🧠 Knowledge graph: Refiner uses this to know when to flush its drafts accumulator.
    /// The accumulator accumulates Drafts until their total byte size approaches this limit,
    /// then the Manifold casts+joins them into a single drum for the sink.
    pub fn max_drum_size_bytes(&self) -> usize {
        match self {
            SinkConfig::Elasticsearch(es) => es.common_config.max_drum_size_bytes,
            SinkConfig::File(f) => f.common_config.max_drum_size_bytes,
            // 🧠 InMemory gets the default — it's testing, we don't limit 🦆
            SinkConfig::InMemory(_) => CommonSinkConfig::default().max_drum_size_bytes,
        }
    }
}
