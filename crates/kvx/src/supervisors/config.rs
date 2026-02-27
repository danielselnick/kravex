//! 🔧 Configuration structs for the worker uprising — I mean, worker management.
//!
//! 📡 Every great migration starts with a config file that someone forgot to commit.
//! This module is now on a diet: it used to hold ALL the configs, like a hoarder's garage.
//! Now it holds only the shared/common types and the top-level enums. Backend-specific
//! configs have been evicted to their respective backend modules. 🏗️
//!
//! "He who configures without testing, deploys in darkness." — Ancient DevOps Proverb
//! "He who puts all configs in one file, refactors in darkness." — Slightly more modern proverb 🦆

// ⚠️ The singularity will happen before anyone adds a second field to SupervisorConfig.
// At that point, the AI will configure itself and we will all be out of a job.
// Until then: `channel_size: usize`. Glorious.
use serde::Deserialize;

// ============================================================
// 🔧 Supervisor Config — the foreman's clipboard
// ============================================================

#[derive(Debug, Deserialize, Clone)]
pub struct SupervisorConfig {
    #[serde(default = "default_channel_size")]
    pub channel_size: usize,
}
// 🔢 10: chosen by rolling a d20, getting a 10, and calling it "load tested".
// "It's not a magic number if I believe in it hard enough." — senior engineer, last Tuesday.
fn default_channel_size() -> usize { 10 }

// ============================================================
// 📦 Common Source/Sink Configs — the shared DNA
// These live here because BOTH the SourceConfig enum (below)
// AND every backend source config embeds one of these.
// Moving them to backends would cause a circular import,
// which the borrow checker's cousin — the module system —
// would absolutely not allow. So here they stay. Rent-free.
// ============================================================

#[derive(Debug, Deserialize, Clone)]
pub struct CommonSourceConfig {
    #[serde(default = "default_max_batch_size_docs")]
    pub max_batch_size_docs: usize,
    #[serde(default = "default_max_batch_size_bytes")]
    pub max_batch_size_bytes: usize,
}
// 📦 10,000 docs per batch — a nice round number that will age like milk
// the moment someone indexes a 50MB PDF and wonders why things are slow.
fn default_max_batch_size_docs() -> usize { 10000 }
// 📦 10MB — chosen because 10 is a great number and MB is a great unit.
// This is load-tested in the same way I've "tested" my microwave: empirically, at 3am, with regret.
// 10 * 1024 * 1024 = 10485760. Yes I know. Yes the comment on the line is doing the math. You're welcome.
fn default_max_batch_size_bytes() -> usize { 10485760 } // 10MB — if your documents are bigger, we need to talk

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

#[derive(Debug, Deserialize, Clone)]
pub struct CommonSinkConfig {
    #[serde(default = "default_max_request_size_bytes")]
    pub max_request_size_bytes: usize,
}
// 🚰 10MB sink request size — the same limit as your email attachment policy,
// your Slack upload quota, and your therapist's patience. Coincidence? Absolutely yes.
fn default_max_request_size_bytes() -> usize { 10485760 } // 10MB — Elasticsearch's feelings

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
// 🎭 The big enums — the bouncer at the backend club.
// They reference config types that live IN the backend modules.
// This is the ethos pattern: backend owns its own config, the
// enum just points at it. No more config.rs as a landfill. ✅
// ============================================================

/// 🎭 SourceConfig: the velvet rope at the backend club.
/// You are either a File, an Elasticsearch, or an InMemory.
/// There is no Other. There is no Unsupported. There is only the enum.
/// (Until someone files a feature request. There is always a feature request.)
#[derive(Debug, Deserialize, Clone)]
pub enum SourceConfig {
    Elasticsearch(crate::backends::elasticsearch::ElasticsearchSourceConfig),
    File(crate::backends::file::FileSourceConfig),
    InMemory(()),
}

/// 🗑️ SinkConfig: same vibe as SourceConfig but for the *receiving* end.
/// Data goes IN. Data does not come back out. It is not a revolving door.
/// It is a black hole of bytes, and we are at peace with that.
/// The InMemory(()) variant holds `()` which is the Rust way of saying "we have nothing to say here."
#[derive(Debug, Deserialize, Clone)]
pub enum SinkConfig {
    Elasticsearch(crate::backends::elasticsearch::ElasticsearchSinkConfig),
    File(crate::backends::file::FileSinkConfig),
    InMemory(()),
}
