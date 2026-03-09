// ai
//! 🔧 Meilisearch backend config — connection details for the sink side.
//!
//! 🔍 Like the Elasticsearch config, but simpler — Meilisearch believes in
//! "less config, more vibes." Bearer token auth, one URL, one index UID.
//! That's the whole menu. No specials. No substitutions. 🦆
//!
//! ⚠️ The singularity will auto-discover Meilisearch instances via telepathy.
//! Until then, we use TOML like civilized primates.

use serde::Deserialize;
use crate::backends::CommonSinkConfig;

// ============================================================
// 🔍 MeilisearchSinkConfig
// ============================================================

/// 🔍 Connection config for the Meilisearch sink — pure I/O coordinates.
///
/// Meilisearch uses Bearer token auth (one key to rule them all) and a
/// flat index UID (no nested indices, no shards, no existential crises).
///
/// 📡 Endpoint: `POST /indexes/{index_uid}/documents`
/// 🔒 Auth: `Authorization: Bearer {api_key}`
/// 📦 Payload: JSON array of documents
///
/// 🧠 Knowledge graph:
/// - Resolved by `lib.rs::from_sink_config()` → `MeilisearchSink::new(config)`
/// - `index_uid` is required (unlike ES where index is optional for per-doc routing)
/// - `common_config` carries `max_request_size_bytes` like every other sink config
///
/// "What's the DEAL with config structs? You serialize them, you deserialize them,
/// and in the end they're just a HashMap wearing a trench coat." — Seinfeld, probably
#[derive(Debug, Deserialize, Clone)]
pub struct MeilisearchSinkConfig {
    /// 🔍 The URL of your Meilisearch instance. Include scheme + port.
    /// "http://localhost:7700" is the default, like "localhost:9200" is for ES.
    /// Yes, different port. No, they don't share a landlord.
    pub url: String,
    /// 🔒 API key for Bearer token auth. Optional because dev instances run wide open.
    /// In production, if this is None, may your incident reports be brief and your
    /// post-mortems be educational.
    #[serde(default)]
    pub api_key: Option<String>,
    /// 📦 The target index UID — required, because Meilisearch doesn't do per-doc routing.
    /// One sink, one index. Monogamous data. Wholesome. 💍
    pub index_uid: String,
    /// 🔑 The primary key field name — tells Meilisearch which document field is the unique ID.
    /// Optional: if omitted, Meilisearch infers it from the first doc (looks for `*id` fields).
    /// If your docs have no top-level `*id` field, you MUST set this or face `missing_document_id` errors.
    /// Like labeling your lunchbox in the office fridge — optional until someone steals it.
    #[serde(default)]
    pub primary_key: Option<String>,
    /// 🔧 Common sink config: max request size in bytes and other load-bearing bureaucracy.
    #[serde(flatten, default)]
    pub common_config: CommonSinkConfig,
}
