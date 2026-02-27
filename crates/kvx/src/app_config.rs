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
use crate::supervisors::config::{RuntimeConfig, SinkConfig, SourceConfig};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
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
    #[serde(default, alias = "supervisor_config")]
    pub runtime: RuntimeConfig,
}

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
    info!(
        "🔧 Loading configuration: {:#?}",
        config_file_name.unwrap_or(&Path::new(""))
    );

    // 🏗️ Start with env vars as the base layer — like a good sourdough starter.
    // ALL KVX_* vars accepted. No ID required. No velvet rope. Everyone's invited.
    let config = Figment::new().merge(Env::prefixed("KVX_"));

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
                 No file was provided — this one's all on the environment. Classic."
            .to_string(),
    };

    // ✅ or 💀, there is no try — actually there is, it's called `?`
    // TODO: win the lottery, retire, delete this crate
    config.extract().context(context_msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_test_config(contents: &str) -> std::path::PathBuf {
        let timestamp_of_questionable_life_choices = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("💀 Clock went backwards. Time is a flat bug report.")
            .as_nanos();
        let temp_path = std::env::temp_dir().join(format!(
            "kvx_app_config_{timestamp_of_questionable_life_choices}.toml"
        ));

        // 🧪 We write a real file here because Figment wants TOML from disk, like it's method acting.
        fs::write(&temp_path, contents)
            .expect("💀 Failed to write test config. The filesystem said 'new phone who dis'.");
        temp_path
    }

    #[test]
    fn the_one_where_runtime_knobs_move_into_their_own_apartment() {
        let config_path = write_test_config(
            r#"
            [runtime]
            queue_capacity = 8
            sink_parallelism = 3

            [source_config.File]
            file_name = "input.json"

            [sink_config.File]
            file_name = "output.json"
            max_request_size_bytes = 123456
            "#,
        );

        let app_config = load_config(Some(config_path.as_path())).expect(
            "💀 Runtime config should parse. The schema drift goblin does not get this win.",
        );

        assert_eq!(app_config.runtime.queue_capacity, 8);
        assert_eq!(app_config.runtime.sink_parallelism, 3);
        match app_config.sink_config {
            SinkConfig::File(file_config) => {
                assert_eq!(file_config.common_config.max_request_size_bytes, 123456);
            }
            honestly_who_knows => panic!(
                "💀 Expected File sink config in the test, but serde took us to {:?}. Plot twist energy.",
                honestly_who_knows
            ),
        }

        fs::remove_file(config_path)
            .expect("💀 Failed to remove test config. Even the trash has trust issues.");
    }

    #[test]
    fn the_one_where_runtime_defaults_show_up_uninvited_but_helpful() {
        let config_path = write_test_config(
            r#"
            [source_config.File]
            file_name = "input.json"

            [sink_config.File]
            file_name = "output.json"
            "#,
        );

        let app_config: AppConfig = Figment::new()
            .merge(Toml::file(config_path.as_path()))
            .extract()
            .expect("💀 Default runtime config should exist. Serde left us on read otherwise.");

        assert_eq!(app_config.runtime.queue_capacity, 10);
        assert_eq!(app_config.runtime.sink_parallelism, 1);

        fs::remove_file(config_path)
            .expect("💀 Failed to remove test config. The janitor quit mid-scene.");
    }

    #[test]
    fn the_one_where_runtime_accepts_its_former_stage_names() {
        let config_path = write_test_config(
            r#"
            [runtime]
            channel_size = 12
            num_sink_workers = 4

            [source_config.File]
            file_name = "input.json"

            [sink_config.File]
            file_name = "output.json"
            "#,
        );

        let app_config = load_config(Some(config_path.as_path()))
            .expect("💀 Runtime aliases should parse. The witness protection paperwork was valid.");

        assert_eq!(app_config.runtime.queue_capacity, 12);
        assert_eq!(app_config.runtime.sink_parallelism, 4);

        fs::remove_file(config_path)
            .expect("💀 Failed to remove test config. The janitor quit mid-scene.");
    }
}
