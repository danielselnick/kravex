// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎭 Casters — the alchemists of the pipeline 🚀📦🔮
//!
//! 🎬 COLD OPEN — INT. DATA FORGE — MIDNIGHT
//! *[raw feeds arrive, unformatted, confused, smelling faintly of source API]*
//! *["Cast me," they whisper. "Make me worthy of the sink."]*
//! *[a Caster steps forward. It has no fear. Only `match` arms.]*
//!
//! Each Caster takes a raw feed String and casts it into the format
//! the sink expects. Passthrough? Identity. NdJsonToBulk? ES bulk action lines.
//!
//! 🧠 Knowledge graph:
//! - **Caster** trait: `fn cast(&self, feed: String) -> Result<String>`
//! - **DocumentCaster** enum: dispatches to concrete casters (same pattern as ManifoldBackend)
//! - Resolution: `DocumentCaster::from_configs(source, sink)` matches the pair
//!
//! 🦆 The duck casts no shadow. Only feeds.
//!
//! ⚠️ The singularity will cast its own feeds. Until then, we have enums.

pub mod passthrough;
pub mod ndjson_to_bulk;
pub mod ndjson_split;
pub mod pit_to_bulk;
pub mod pit_to_json;
use ndjson_to_bulk::NdJsonToBulk;
use ndjson_split::NdJsonSplit;
use pit_to_bulk::PitToBulk;
use pit_to_json::PitToJson;

use crate::config::{SourceConfig, SinkConfig};
use anyhow::Result;
use crate::Page;
use crate::Entry;

// ===== Trait =====

/// 🎭 A Caster transforms a raw feed into the sink's expected format.
///
pub trait Caster: std::fmt::Debug {
    /// 🔄 Cast a raw source feed into sink-format output entries.
    /// The feed goes in raw. It comes out ready. Like a pottery kiln, but for JSON. 🏺
    fn cast(&self, page: Page) -> Result<Vec<Entry>>;
}

// ===== Enum Dispatcher =====

/// 🎭 The polymorphic caster — dispatches to the right concrete caster at runtime.
///
/// 📦 Same pattern as `ManifoldBackend`, `SourceBackend`, `SinkBackend`:
/// enum wraps concrete types, match dispatches, compiler monomorphizes, branch prediction
/// eliminates the overhead after warmup. The enum is a formality. The cast is free. 🐄
#[derive(Debug, Clone)]
pub enum PageToEntriesCaster {
    // -- 📡 NDJSON raw docs → ES bulk action+source pairs
    NdJsonToBulk(ndjson_to_bulk::NdJsonToBulk),
    // -- 🔪 NDJSON raw docs → individual JSON entries (no bulk headers, for Meilisearch)
    NdJsonSplit(ndjson_split::NdJsonSplit),
    // -- 🚶 Identity cast — feed passes through unchanged, like TSA PreCheck for data
    Passthrough(passthrough::Passthrough),
    // -- 📡🎭 ES _search PIT response → _bulk NDJSON (extracts hits from envelope)
    PitToBulk(pit_to_bulk::PitToBulk),
    // -- 🔍🎭 ES _search PIT response → raw JSON entries (for Meilisearch, no bulk headers)
    PitToJson(pit_to_json::PitToJson),
}

impl Caster for PageToEntriesCaster {
    #[inline]
    fn cast(&self, page: Page) -> Result<Vec<Entry>> {
        // -- 🎭 Dispatch to the concrete caster — "choose your fighter" but for data formats
        match self {
            Self::NdJsonToBulk(t) => t.cast(page),
            Self::NdJsonSplit(t) => t.cast(page),
            Self::Passthrough(t) => t.cast(page),
            Self::PitToBulk(t) => t.cast(page),
            Self::PitToJson(t) => t.cast(page),
        }
    }
}


// ===== Factory =====

impl PageToEntriesCaster {
    /// 🔧 Resolve a caster from source/sink config enums.
    ///
    /// Same approach as `from_source_config()` / `from_sink_config()` in `lib.rs`:
    /// match on the config enum, construct the right concrete type, wrap in the
    /// dispatching enum.
    ///
    /// The (SourceConfig, SinkConfig) pair determines which caster to use:
    /// - File → Elasticsearch = NdJsonToBulk (the flagship pair)
    /// - File → Meilisearch = NdJsonSplit (split NDJSON lines, no bulk headers)
    /// - File → File = Passthrough
    /// - InMemory → InMemory = Passthrough (testing)
    /// - InMemory → Meilisearch = Passthrough (testing)
    /// - Elasticsearch → File = Passthrough (ES dump to file)
    /// - Elasticsearch → Meilisearch = PitToJson (extract _source, no bulk headers)
    ///
    /// # Panics
    /// 💀 Panics if the `(source, sink)` pair has no caster implementation.
    /// Fail loud at startup, not silent in the hot path.
    pub fn from_configs(source: &SourceConfig, sink: &SinkConfig) -> Self {
        match (source, sink) {
            // -- 🏎️📡 File source → Elasticsearch sink:
            // -- The first and flagship pair. Raw NDJSON to ES bulk.
            // -- "In a world where JSON had too many fields... one caster dared to strip them."
            (SourceConfig::File(_), SinkConfig::Elasticsearch(_)) => {
                Self::NdJsonToBulk(NdJsonToBulk {})
            }

            // -- 🔍🔪 File source → Meilisearch sink: split NDJSON lines into individual entries.
            // -- No bulk headers. Just the raw docs. Meilisearch likes its JSON naked.
            (SourceConfig::File(_), SinkConfig::Meilisearch(_)) => {
                Self::NdJsonSplit(NdJsonSplit)
            }

            // -- 🚶 Passthrough pairs: same format, no conversion needed.
            // -- File→File, InMemory→InMemory, InMemory→Meilisearch, ES→File — just move the bytes.
            (SourceConfig::File(_), SinkConfig::File(_))
            | (SourceConfig::InMemory(_), SinkConfig::InMemory(_))
            | (SourceConfig::InMemory(_), SinkConfig::Meilisearch(_))
            | (SourceConfig::Elasticsearch(_), SinkConfig::File(_)) => {
                Self::Passthrough(passthrough::Passthrough)
            }

            // -- 📡🎭 ES source → ES sink: PIT response envelope → _bulk NDJSON
            // -- "One does not simply walk into Elasticsearch without a bulk action line." — Boromir, probably
            (SourceConfig::Elasticsearch(_), SinkConfig::Elasticsearch(_)) => {
                Self::PitToBulk(PitToBulk)
            }

            // -- 🔍🎭 ES source → Meilisearch sink: PIT response → raw JSON entries (no bulk headers)
            // -- "Do you ever feel like you're just extracting _source into the void?" — ES hit, in therapy
            (SourceConfig::Elasticsearch(_), SinkConfig::Meilisearch(_)) => {
                Self::PitToJson(PitToJson)
            }

            // -- 📡 OpenObserve sink: ES-compatible bulk format, same casters apply.
            // -- "In a world where APIs were compatible... one sink reused all the casters." 🎬
            (SourceConfig::File(_), SinkConfig::OpenObserve(_)) => {
                Self::NdJsonToBulk(NdJsonToBulk {})
            }
            // -- 📡🎭 ES source → OpenObserve sink: same PIT-to-bulk dance, different venue
            (SourceConfig::Elasticsearch(_), SinkConfig::OpenObserve(_)) => {
                Self::PitToBulk(PitToBulk)
            }
            // -- 🧪 InMemory → OpenObserve: testing path, passthrough all the way
            (SourceConfig::InMemory(_), SinkConfig::OpenObserve(_)) => {
                Self::Passthrough(passthrough::Passthrough)
            }

            // -- 💀 Unimplemented pairs: panic with context.
            // -- "Config not found: We looked everywhere. Under the couch. Behind the fridge.
            // -- In the junk drawer. Nothing."
            #[allow(unreachable_patterns)]
            (src, dst) => {
                panic!(
                    "💀 No caster implemented for source {:?} → sink {:?}. \
                     This is the resolve() equivalent of 'new phone who dis.' \
                     Add a variant to DocumentCaster, write the impl, add tests.",
                    src, dst
                )
            }
        }
    }
}

/// 🧠 `DocumentCaster` dispatches to the concrete caster inside each variant.
/// Same pattern as `impl Source for SourceBackend` in `backends.rs`.
/// The borrow checker approves. The compiler inlines. Life is good. 🐄


#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::file::{FileSinkConfig, FileSourceConfig};
    use crate::backends::{ElasticsearchSinkConfig, ElasticsearchSourceConfig};
    use crate::backends::{CommonSinkConfig, CommonSourceConfig};

    /// 🧪 Resolve File→ES to NdJsonToBulk caster.
    #[test]
    fn the_one_where_config_enums_resolve_to_the_right_caster() -> Result<()> {
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
        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(
            matches!(the_caster, PageToEntriesCaster::NdJsonToBulk(_)),
            "File → ES should resolve to NdJsonToBulk 🏎️"
        );

        // 🔄 Cast a feed through it
        let rally_feed = serde_json::json!({
            "ObjectID": 42069,
            "Name": "Test story",
            "_rallyAPIMajor": "2"
        })
        .to_string();
        let the_output = the_caster.cast(Page(rally_feed))?;

        // ✅ Output should be non-empty (NdJsonToBulk produces action+source lines)
        assert!(!the_output.is_empty(), "Cast output should not be empty 🎯");

        Ok(())
    }

    /// 🧪 Resolve File→File to Passthrough — feed passes through unchanged.
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

        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(matches!(the_caster, PageToEntriesCaster::Passthrough(_)));

        // 🔄 Passthrough returns the feed unchanged — zero drama
        let the_input = r#"{"whatever":"goes"}"#.to_string();
        let the_output = the_caster.cast(Page(the_input.clone()))?;
        assert_eq!(*the_output[0], the_input, "Passthrough must return feed unchanged! 🚶");

        Ok(())
    }

    /// 🧪 Resolve InMemory→InMemory to Passthrough (testing config).
    #[test]
    fn the_one_where_in_memory_resolves_to_passthrough_for_testing() {
        let source = SourceConfig::InMemory(());
        let sink = SinkConfig::InMemory(());
        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(matches!(the_caster, PageToEntriesCaster::Passthrough(_)));
    }

    /// 🧪 Full pipeline integration: resolve + cast multi-doc feed through NdJsonToBulk.
    #[test]
    fn the_one_where_ndjson_feeds_get_cast_via_config_resolution() -> Result<()> {
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

        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);

        // 📄 Build a two-doc feed (newline-separated Rally blobs)
        let rally_feed = format!(
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

        let the_output = the_caster.cast(Page(rally_feed))?;
        // ✅ NdJsonToBulk should produce non-empty output for a multi-doc feed
        assert!(!the_output.is_empty(), "Cast output should not be empty for multi-doc feed 🎯");

        Ok(())
    }

    /// 🧪 File→OpenObserve resolves to NdJsonToBulk — same bulk format as ES, reuse all the things.
    #[test]
    fn the_one_where_file_to_openobserve_resolves_to_ndjson_to_bulk() -> Result<()> {
        use crate::backends::open_observe::OpenObserveSinkConfig;
        let source = SourceConfig::File(FileSourceConfig {
            file_name: "rally_export.json".to_string(),
            common_config: CommonSourceConfig::default(),
        });
        let sink = SinkConfig::OpenObserve(OpenObserveSinkConfig {
            url: "http://localhost:5080".to_string(),
            org: "default".to_string(),
            stream: "rally".to_string(),
            username: None,
            password: None,
            common_config: CommonSinkConfig::default(),
        });

        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(
            matches!(the_caster, PageToEntriesCaster::NdJsonToBulk(_)),
            "File → OpenObserve should resolve to NdJsonToBulk — same wire format as ES 🏎️"
        );

        Ok(())
    }

    /// 🧪 ES→OpenObserve resolves to PitToBulk — PIT response to bulk, different destination same dance.
    #[test]
    fn the_one_where_es_to_openobserve_resolves_to_pit_to_bulk() -> Result<()> {
        use crate::backends::open_observe::OpenObserveSinkConfig;
        let source = SourceConfig::Elasticsearch(ElasticsearchSourceConfig {
            url: "http://source-cluster:9200".to_string(),
            username: None,
            password: None,
            api_key: None,
            common_config: CommonSourceConfig::default(),
        });
        let sink = SinkConfig::OpenObserve(OpenObserveSinkConfig {
            url: "http://localhost:5080".to_string(),
            org: "default".to_string(),
            stream: "migrated".to_string(),
            username: None,
            password: None,
            common_config: CommonSinkConfig::default(),
        });

        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(
            matches!(the_caster, PageToEntriesCaster::PitToBulk(_)),
            "ES → OpenObserve should resolve to PitToBulk 🎭"
        );

        Ok(())
    }

    /// 🧪 InMemory→OpenObserve resolves to Passthrough — testing path, no conversion needed.
    #[test]
    fn the_one_where_inmemory_to_openobserve_resolves_to_passthrough() {
        use crate::backends::open_observe::OpenObserveSinkConfig;
        let source = SourceConfig::InMemory(());
        let sink = SinkConfig::OpenObserve(OpenObserveSinkConfig {
            url: "http://localhost:5080".to_string(),
            org: "default".to_string(),
            stream: "test".to_string(),
            username: None,
            password: None,
            common_config: CommonSinkConfig::default(),
        });
        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(matches!(the_caster, PageToEntriesCaster::Passthrough(_)));
    }

    /// 🧪 ES→ES resolves to PitToBulk — the PIT response caster for cross-cluster migration.
    #[test]
    fn the_one_where_es_to_es_resolves_to_pit_to_bulk() -> Result<()> {
        let source = SourceConfig::Elasticsearch(ElasticsearchSourceConfig {
            url: "http://source-cluster:9200".to_string(),
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

        let the_caster = PageToEntriesCaster::from_configs(&source, &sink);
        assert!(
            matches!(the_caster, PageToEntriesCaster::PitToBulk(_)),
            "💀 ES → ES should resolve to PitToBulk, not {:?}", the_caster
        );

        // 🔄 Verify it actually casts a search response into bulk format
        let the_search_response = r#"{"hits":{"hits":[{"_index":"src","_id":"1","_source":{"ok":true}}]}}"#.to_string();
        let the_output = the_caster.cast(Page(the_search_response))?;
        assert!(!the_output.is_empty(), "💀 PitToBulk should produce output for a valid search response");

        Ok(())
    }
}
