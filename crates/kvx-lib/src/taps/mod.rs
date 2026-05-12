// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎭 Tappers — the alchemists of the pipeline 🚀📦🔮
//!
//! 🎬 COLD OPEN — INT. DATA FORGE — MIDNIGHT
//! *[raw barrels arrive, unformatted, confused, smelling faintly of source API]*
//! *["Tap me," they whisper. "Make me worthy of the sink."]*
//! *[a Tapper steps forward. It has no fear. Only `match` arms.]*
//!
//! Each Tapper takes a raw barrel String and casts it into the format
//! the sink expects. Passthrough? Identity. NdJsonToBulk? ES bulk action lines.
//!
//! 🧠 Knowledge graph:
//! - **Tapper** trait: `fn tap(&self, barrel: Barrel) -> Result<Vec<Draft>>`
//! - **BarrelToDraftsTapper** enum: dispatches to concrete tappers (same pattern as ManifoldBackend)
//! - Resolution: `BarrelToDraftsTapper::from_configs(source, sink)` matches the pair
//!
//! 🦆 The duck taps no shadow. Only barrels.
//!
//! ⚠️ The singularity will tap its own barrels. Until then, we have enums.

pub mod passthrough;
pub mod ndjson_to_bulk;
pub mod pit_to_bulk;
use ndjson_to_bulk::NdJsonToBulk;
use pit_to_bulk::PitToBulk;

use crate::config::{SourceConfig, SinkConfig};
use anyhow::Result;
use crate::Barrel;
use crate::Draft;

// ===== Trait =====

/// 🎭 A Tapper transforms a raw barrel into the sink's expected format.
///
pub trait Tapper: std::fmt::Debug {
    /// 🔄 Tap a raw source barrel into sink-format output drafts.
        /// The barrel goes in raw. It comes out ready. Like a pottery kiln, but for JSON. 🏺
        fn tap(&self, barrel: Barrel) -> Result<Vec<Draft>>;
}

// ===== Enum Dispatcher =====

/// 🎭 The polymorphic tapper — dispatches to the right concrete tapper at runtime.
///
/// 📦 Same pattern as `ManifoldBackend`, `SourceBackend`, `SinkBackend`:
/// enum wraps concrete types, match dispatches, compiler monomorphizes, branch prediction
/// eliminates the overhead after warmup. The enum is a formality. The tap is free. 🐄
#[derive(Debug, Clone)]
pub enum BarrelToDraftsTapper {
    // -- 📡 NDJSON raw docs → ES bulk action+source pairs
    NdJsonToBulk(ndjson_to_bulk::NdJsonToBulk),
    // -- 🚶 Identity tap — barrel passes through unchanged, like TSA PreCheck for data
    Passthrough(passthrough::Passthrough),
    // -- 📡🎭 ES _search PIT response → _bulk NDJSON (extracts hits from envelope)
    PitToBulk(pit_to_bulk::PitToBulk),
}

impl Tapper for BarrelToDraftsTapper {
    // Tap the barrel, return drafts of beer
    #[inline]
    fn tap(&self, barrel: Barrel) -> Result<Vec<Draft>> {
        // -- 🎭 Dispatch to the concrete tapper — "choose your fighter" but for data formats
        match self {
            Self::NdJsonToBulk(t) => t.tap(barrel),
            Self::Passthrough(t) => t.tap(barrel),
            Self::PitToBulk(t) => t.tap(barrel),
        }
    }
}


// ===== Factory =====

impl BarrelToDraftsTapper {
    /// 🔧 Resolve a tapper from source/sink config enums.
    ///
    /// Same approach as `from_source_config()` / `from_sink_config()` in `lib.rs`:
    /// match on the config enum, construct the right concrete type, wrap in the
    /// dispatching enum.
    ///
    /// The (SourceConfig, SinkConfig) pair determines which tapper to use:
    /// - File → Elasticsearch = NdJsonToBulk (the flagship pair)
    /// - File → File = Passthrough
    /// - InMemory → InMemory = Passthrough (testing)
    /// - Elasticsearch → File = Passthrough (ES dump to file)
    /// - Elasticsearch → Elasticsearch = PitToBulk (cross-cluster migration)
    ///
    /// # Panics
    /// 💀 Panics if the `(source, sink)` pair has no tapper implementation.
    /// Fail loud at startup, not silent in the hot path.
    pub fn from_configs(source: &SourceConfig, sink: &SinkConfig) -> Self {
        match (source, sink) {
            // -- 🏎️📡 File source → Elasticsearch sink:
            // -- The first and flagship pair. Raw NDJSON to ES bulk.
            // -- "In a world where JSON had too many fields... one tapper dared to strip them."
            (SourceConfig::File(_), SinkConfig::Elasticsearch(_)) => {
                Self::NdJsonToBulk(NdJsonToBulk {})
            }

            // -- 🚶 Passthrough pairs: same format, no conversion needed.
            // -- File→File, InMemory→InMemory, ES→File — just move the bytes.
            (SourceConfig::File(_), SinkConfig::File(_))
            | (SourceConfig::InMemory(_), SinkConfig::InMemory(_))
            | (SourceConfig::Elasticsearch(_), SinkConfig::File(_)) => {
                Self::Passthrough(passthrough::Passthrough)
            }

            // -- 📡🎭 ES source → ES sink: PIT response envelope → _bulk NDJSON
            // -- "One does not simply walk into Elasticsearch without a bulk action line." — Boromir, probably
            (SourceConfig::Elasticsearch(_), SinkConfig::Elasticsearch(_)) => {
                Self::PitToBulk(PitToBulk)
            }

            // -- 💀 Unimplemented pairs: panic with context.
            // -- "Config not found: We looked everywhere. Under the couch. Behind the fridge.
            // -- In the junk drawer. Nothing."
            #[allow(unreachable_patterns)]
            (src, dst) => {
                panic!(
                    "💀 No tapper implemented for source {:?} → sink {:?}. \
                     This is the resolve() equivalent of 'new phone who dis.' \
                     Add a variant to BarrelToDraftsTapper, write the impl, add tests.",
                    src, dst
                )
            }
        }
    }
}

// -- 🧠 `BarrelToDraftsTapper` dispatches to the concrete tapper inside each variant. 🦆
// -- Same pattern as `impl Source for SourceBackend` in `backends.rs`. 🚀
// -- The borrow checker approves. The compiler inlines. Life is good. 🧵
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::file::{FileSinkConfig, FileSourceConfig};
    use crate::backends::{ElasticsearchSinkConfig, ElasticsearchSourceConfig};
    use crate::backends::{CommonSinkConfig, CommonSourceConfig};

    /// 🧪 Resolve File→ES to NdJsonToBulk tapper.
    #[test]
    fn the_one_where_config_enums_resolve_to_the_right_tapper() -> Result<()> {
        // 🔧 Build source/sink configs like the real pipeline does
        let source = SourceConfig::File(FileSourceConfig {
            file_name: "rally_export.json".to_string(),
            common_config: CommonSourceConfig::default(),
        });
        let sink = SinkConfig::Elasticsearch(ElasticsearchSinkConfig {
            url: "http://localhost:9200".to_string(),
            username: None,
            password: None,
            api_key: None,
            index: Some("rally".to_string()),
            common_config: CommonSinkConfig::default(),
        });

        // 🎯 Resolve — should give us NdJsonToBulk
        let the_tapper = BarrelToDraftsTapper::from_configs(&source, &sink);
        assert!(
            matches!(the_tapper, BarrelToDraftsTapper::NdJsonToBulk(_)),
            "File → ES should resolve to NdJsonToBulk 🏎️"
        );

        // 🔄 Tap a barrel through it
        let rally_barrel = serde_json::json!({
            "ObjectID": 42069,
            "Name": "Test story",
            "_rallyAPIMajor": "2"
        })
        .to_string();
        let the_output = the_tapper.tap(Barrel(rally_barrel))?;

        // ✅ Output should be non-empty (NdJsonToBulk produces action+source lines)
        assert!(!the_output.is_empty(), "Tap output should not be empty 🎯");

        Ok(())
    }

    /// 🧪 Resolve File→File to Passthrough — barrel passes through unchanged.
    #[test]
    fn the_one_where_file_to_file_resolves_to_passthrough() -> Result<()> {
        let source = SourceConfig::File(FileSourceConfig {
            file_name: "input.json".to_string(),
            common_config: CommonSourceConfig::default(),
        });
        let sink = SinkConfig::File(FileSinkConfig {
            file_name: "output.json".to_string(),
            common_config: CommonSinkConfig::default(),
        });

        let the_tapper = BarrelToDraftsTapper::from_configs(&source, &sink);
        assert!(matches!(the_tapper, BarrelToDraftsTapper::Passthrough(_)));

        // 🔄 Passthrough returns the barrel unchanged — zero drama
        let the_input = r#"{"whatever":"goes"}"#.to_string();
        let the_output = the_tapper.tap(Barrel(the_input.clone()))?;
        assert_eq!(*the_output[0], the_input, "Passthrough must return barrel unchanged! 🚶");

        Ok(())
    }

    /// 🧪 Resolve InMemory→InMemory to Passthrough (testing config).
    #[test]
    fn the_one_where_in_memory_resolves_to_passthrough_for_testing() {
        let source = SourceConfig::InMemory(());
        let sink = SinkConfig::InMemory(());
        let the_tapper = BarrelToDraftsTapper::from_configs(&source, &sink);
        assert!(matches!(the_tapper, BarrelToDraftsTapper::Passthrough(_)));
    }

    /// 🧪 Full pipeline integration: resolve + tap multi-doc barrel through NdJsonToBulk.
    #[test]
    fn the_one_where_ndjson_barrels_get_tapped_via_config_resolution() -> Result<()> {
        let source = SourceConfig::File(FileSourceConfig {
            file_name: "data.json".to_string(),
            common_config: CommonSourceConfig::default(),
        });
        let sink = SinkConfig::Elasticsearch(ElasticsearchSinkConfig {
            url: "http://localhost:9200".to_string(),
            username: None,
            password: None,
            api_key: None,
            index: Some("rally-artifacts".to_string()),
            common_config: CommonSinkConfig::default(),
        });

        let the_tapper = BarrelToDraftsTapper::from_configs(&source, &sink);

        // 📄 Build a two-doc barrel (newline-separated Rally blobs)
        let rally_barrel = format!(
            "{}\n{}",
            serde_json::json!({
                "ObjectID": 99999,
                "FormattedID": "US001",
                "Name": "The one that made it through the whole pipeline",
                "_rallyAPIMajor": "2",
                "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/hr/99999",
                "_CreatedAt": "2024-01-01T00:00:00.000Z"
            }),
            serde_json::json!({
                "ObjectID": 88888,
                "Name": "The sequel nobody asked for"
            })
        );

        let the_output = the_tapper.tap(Barrel(rally_barrel))?;
        // ✅ NdJsonToBulk should produce non-empty output for a multi-doc barrel
        assert!(!the_output.is_empty(), "Tap output should not be empty for multi-doc barrel 🎯");

        Ok(())
    }

    /// 🧪 ES→ES resolves to PitToBulk — the PIT response tapper for cross-cluster migration.
    #[test]
    fn the_one_where_es_to_es_resolves_to_pit_to_bulk() -> Result<()> {
        let source = SourceConfig::Elasticsearch(ElasticsearchSourceConfig {
            url: "http://source-cluster:9200".to_string(),
            index: "test-index".to_string(),
            username: None,
            password: None,
            api_key: None,
            common_config: CommonSourceConfig::default(),
        });
        let sink = SinkConfig::Elasticsearch(ElasticsearchSinkConfig {
            url: "http://dest-cluster:9200".to_string(),
            username: None,
            password: None,
            api_key: None,
            index: Some("dest-index".to_string()),
            common_config: CommonSinkConfig::default(),
        });

        let the_tapper = BarrelToDraftsTapper::from_configs(&source, &sink);
        assert!(
            matches!(the_tapper, BarrelToDraftsTapper::PitToBulk(_)),
            "💀 ES → ES should resolve to PitToBulk, not {:?}", the_tapper
        );

        // 🔄 Verify it actually casts a search response into bulk format
        let the_search_response = r#"{"hits":{"hits":[{"_index":"src","_id":"1","_source":{"ok":true}}]}}"#.to_string();
        let the_output = the_tapper.tap(Barrel(the_search_response))?;
        assert!(!the_output.is_empty(), "💀 PitToBulk should produce output for a valid search response");

        Ok(())
    }
}
