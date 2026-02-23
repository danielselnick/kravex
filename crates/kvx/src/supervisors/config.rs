//! 🔧 Configuration structs for the worker uprising — I mean, worker management.
//!
//! 📡 Every great migration starts with a config file that someone forgot to commit.
//! This module ensures that when things go wrong, at least we know WHO configured
//! the wrong thing. (It was you. It's always you.) 🦆
//!
//! "He who configures without testing, deploys in darkness." — Ancient DevOps Proverb

use serde::Deserialize;

/// 📦 Generic/common/core configuration values, applicable to any type of SOURCE:
/// Elasticsearch, OpenSearch, Mongo, your mom's recipe database.
///
/// 🏗️ Currently emptier than my soul on a Monday morning.
/// But like a fine wine, or my mortgage balance, it will grow with time.
#[derive(Debug, Deserialize, Clone)]
pub struct SourceWorkerConfig {
    // 💤 Nothing here yet. This struct is on vacation.
    // 🐛 TODO: add fields before the heat death of the universe
    // (or before Daniel's kids graduate college, whichever comes first)
}

/// 🚰 Configuration for sink workers — the plumbers of the data pipeline.
/// They take your data and put it somewhere. Like a moving company,
/// but less likely to break your grandmother's china.
///
/// ⚠️ "4 workers ought to be enough for anybody" — Bill Gates, probably
#[derive(Debug, Deserialize, Clone)]
pub struct SinkWorkerConfig {
    /// 🧵 How many sink workers to spawn. Default is 4 because
    /// someone once said "4 cores is standard" and we never questioned it.
    /// Much like how nobody questions why we have 2 cars but only 1 garage.
    #[serde(default = "default_num_sink_workers")]
    num_sink_workers: usize
}

/// 🎯 The sacred default. The chosen number. The one true sink worker count.
/// Why 4? Because 3 felt lonely and 5 felt like showing off.
fn default_num_sink_workers() -> usize { 4 }
// ^ if you're reading this during a code review, the answer is still 4.
// it will always be 4. 4 is eternal. 4 is life.