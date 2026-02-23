//! 🔧 App Configuration — the sacred TOML-to-struct pipeline.
//!
//! 📡 "Config not found: We looked everywhere. Under the couch. Behind the fridge.
//! In the junk drawer. Nothing." — every developer at 3am 🦆
//!
//! 🏗️ Powered by Figment, because manually parsing env vars is a form of
//! self-harm that even the borrow checker wouldn't approve of.

use anyhow::Context;
use serde::Deserialize;
// 🔧 To load the configuration, so I don't have to manually parse
// environment variables or files. Bleh. Like doing taxes but for bytes.
use figment::{Figment, providers::{Env, Format, Toml}};
use crate::supervisors::config::{SourceWorkerConfig, SinkWorkerConfig};
use std::path::Path;

/// 📦 The AppConfig: one struct to rule them all, one struct to find them,
/// one struct to bring them all, and in the Figment bind them.
///
/// 🎯 Contains everything the app needs to know about itself,
/// which is more self-awareness than most apps achieve in their lifetime.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// 📡 How shall the source workers behave? Configurable, unlike my children.
    pub source_worker_config: SourceWorkerConfig,
    /// 🚰 Sink worker settings — because even plumbers need instructions
    pub sink_worker_config: SinkWorkerConfig,
    /// 🧵 Supervisor config — currently just a usize, living its best minimalist life
    pub supervisor_config: usize,
}

/// 🚀 Load the config from a file, or from the default path if you're feeling lucky.
///
/// 🔧 Merges environment variables (prefixed KVX_) with TOML config,
/// because why have one source of truth when you can have two that disagree?
///
/// 💀 Returns an error if the config is garbage. Which, statistically speaking,
/// it probably is on the first try. And the second. Third time's the charm? 🔄
pub fn load_config(file_path : Option<&Path>) -> anyhow::Result<AppConfig> {
    // 🎯 If no path provided, default to config.toml
    // like defaulting to pizza when you can't decide on dinner
    let file = file_path.map_or_else(|| Path::new("config.toml"), |path| path);
    let config: AppConfig = Figment::new()
        .merge(Env::prefixed("KVX_").only(&["SOURCE", "SINK", "CHANNEL"]))
        .merge(Toml::file(file).nested())
        .extract()?; // ✅ or 💀, there is no try — actually there is, it's called `?`

    // 🏗️ Reconstruct the config because... well... we already have it...
    // but this is Rust and we like being explicit about things.
    // Like how I'm explicit about my 2 kids, 1 wife, 1 mortgage, and 2 cars.
    Ok(AppConfig {
        source_worker_config: config.source_worker_config,
        sink_worker_config: config.sink_worker_config,
        supervisor_config: config.supervisor_config,
    })
}
