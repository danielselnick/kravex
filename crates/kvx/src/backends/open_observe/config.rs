// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🔧 OpenObserve backend config — connection details for the sink.
//!
//! 📡 One config struct, one dream: shoving NDJSON into OpenObserve's bulk API.
//! Like Elasticsearch's config, but simpler — no API key, no index existence check,
//! no existential crises about mappings. Just vibes and basic auth. 🦆
//!
//! ⚠️ The singularity will configure itself. Until then, we have serde.

use serde::Deserialize;
use crate::backends::CommonSinkConfig;

// 🏢 Default org name — "default" because creativity peaks at config time
fn default_org() -> String {
    "default".to_string()
}

// ============================================================
// 🚰 OpenObserveSinkConfig
// ============================================================

/// 🚰 Configuration for the OpenObserve sink backend.
///
/// OpenObserve exposes an ES-compatible `_bulk` API at `POST /api/{org}/_bulk`.
/// Streams auto-create on first write, so no existence check is needed at startup.
/// Auth is basic auth only — no API key, no OAuth, no blood sacrifice.
///
/// 🧠 Knowledge graph:
/// - `url`: base URL of the OpenObserve instance (e.g., `http://localhost:5080`)
/// - `org`: organization name, appears in the URL path. Defaults to `"default"`.
/// - `stream`: the target stream name, used in the `_index` field of bulk action lines.
///   OpenObserve auto-creates streams on first write — like a river that digs its own bed.
/// - `username`/`password`: basic auth. Optional in theory, required in practice
///   unless your OpenObserve instance is more trusting than a golden retriever. 🐕
/// - `common_config`: max request size and other shared sink knobs.
#[derive(Debug, Deserialize, Clone)]
pub struct OpenObserveSinkConfig {
    /// 📡 Base URL of the OpenObserve instance. Include scheme + port.
    /// "http://localhost:5080" — the address of your data's new home.
    pub url: String,
    /// 🏢 Organization name — appears in the API path as `/api/{org}/_bulk`.
    /// Defaults to "default" because naming things is one of the two hard problems.
    #[serde(default = "default_org")]
    pub org: String,
    /// 🚰 Target stream name — where the documents land.
    /// Used in the bulk action line: `{"index":{"_index":"<stream>"}}`.
    /// OpenObserve auto-creates streams, so typos just create new ones. You're welcome.
    pub stream: String,
    /// 🔒 Username for basic auth. Like a library card — technically optional, practically required.
    #[serde(default)]
    pub username: Option<String>,
    /// 🔒 Password. If this is "admin" I am judging you from inside the compiler.
    #[serde(default)]
    pub password: Option<String>,
    /// 🔧 Common sink config: max request size and other life decisions.
    #[serde(flatten, default)]
    pub common_config: CommonSinkConfig,
}
