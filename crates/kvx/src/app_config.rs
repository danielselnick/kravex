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
use crate::supervisors::config::{SupervisorConfig, SourceConfig, SinkConfig};
use std::path::Path;
// 🚀 tracing::info — because println! in production is a cry for help.
// "I used to use println! for debugging... but then I got help." — anonymous dev, 2 kids, 1 wife, 1 mortgage
use tracing::info;

/// 📦 The AppConfig: one struct to rule them all, one struct to find them,
/// one struct to bring them all, and in the Figment bind them.
///
/// 🎯 Contains everything the app needs to know about itself,
/// which is more self-awareness than most apps achieve in their lifetime.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// 📡 How shall the source workers behave? Configurable, unlike my children.
    pub source_config: SourceConfig,
    pub sink_config: SinkConfig,
    pub supervisor_config: SupervisorConfig,
    #[serde(default = "default_num_sink_workers")]
    pub num_sink_workers: usize,
}

/// 🧵 Returns the default number of sink workers: `1`.
///
/// One. A single, solitary, lone-wolf worker. The One. The Chosen One.
/// Not because we couldn't afford more — but because one is all it takes
/// to carry the entire pipeline on its back while its coworkers are "in a meeting."
///
/// 🦺 Also: 1 is the loneliest number. Three Dog Night said so. Harry Nilsson said so.
/// The borrow checker has no opinion, but it's watching. It's always watching.
///
/// ⚠️ The singularity will occur before anyone remembers to bump this default to 2.
/// And when it does, the one worker will just keep going. Unbothered. Iconic.
fn default_num_sink_workers() -> usize { 1 }

/// 🚀 Load the config — from a file, from env vars, or from the sheer power of hoping.
///
/// 🔧 Merges environment variables (KVX_*) with an optional TOML file.
/// Notice: no `.only(...)` restriction — ALL KVX_ vars are fair game now.
/// We don't gatekeep env vars here. This is a safe space. 🦆
///
/// 📐 DESIGN NOTE (no cap, this is tribal knowledge):
///   - If `config_file_name` is None  → env vars only. No file. No assumptions. No pizza defaults.
///   - If `config_file_name` is Some  → env vars + TOML file, merged. TOML wins on conflicts.
///   Previously kravex always fell back to "config.toml" — like assuming everyone wants pineapple
///   on their pizza. We fixed that. ethos showed us the light.
///
/// 💀 Returns an error if config is unparseable. Which it will be. Check the error message though —
/// it's contextual, informative, and written with love. Or despair. Hard to tell at 3am.
pub fn load_config(config_file_name: Option<&Path>) -> anyhow::Result<AppConfig> {
    // 🚀 Log what we're loading — because silent failures are the villain origin story
    // of every 3am incident. "The config loaded fine." — famous last words.
    info!("🔧 Loading configuration: {:#?}", config_file_name.unwrap_or(&Path::new("")));

    // 🏗️ Start with env vars as the base layer — like a good sourdough starter.
    // ALL KVX_* vars accepted. No ID required. No velvet rope. Everyone's invited.
    let config = Figment::new()
        .merge(Env::prefixed("KVX_"));

    // 🎯 Conditionally layer in TOML only if a file was actually provided.
    // No file? No problem. We trust the env. Like a golden retriever trusts everyone.
    // Ancient proverb: "He who defaults to config.toml uninvited, deploys to production alone."
    let config = match config_file_name {
        Some(file_name) => config.merge(Toml::file(file_name)),
        None => config,
    };

    // 💬 Build a context message that will actually TELL you what went wrong.
    // None of that "error: error" energy. This isn't a Kafka novel. (The author, not the queue.)
    let context_msg = match config_file_name {
        Some(path) => format!(
            "💀 Failed to parse configuration from file '{}' and environment variables (KVX_*). \
             The file exists in our hearts, but apparently not on disk.",
            path.display()
        ),
        None => "💀 Failed to parse configuration from environment variables (KVX_*). \
                 No file was provided — this one's all on the environment. Classic.".to_string(),
    };

    // ✅ or 💀, there is no try — actually there is, it's called `?`
    // TODO: win the lottery, retire, delete this crate
    config.extract().context(context_msg)
}
