// ai
//! 🎬 *[two composers walk into a bar. the dispatcher buys both a drink.]*
//! *[one wants newlines. one wants brackets. the enum holds them both.]*
//! *["In a world where payloads needed composing... one enum dared to dispatch."]*
//!
//! 🎭 **ComposerBackend** — polymorphic dispatcher resolved from `SinkConfig`.
//!
//! 🧠 Knowledge graph:
//! - Same pattern as `DocumentTransformer`, `SourceBackend`, `SinkBackend`
//! - Resolution: SinkConfig → ComposerBackend::from_sink_config() → concrete composer
//! - ES/File → NdjsonComposer | InMemory → JsonArrayComposer
//! - The compiler monomorphizes each arm; branch prediction eliminates the match
//!   after a couple iterations. The enum is a formality. The dispatch is basically free.
//! - Cloning ComposerBackend is free — NdjsonComposer and JsonArrayComposer are zero-sized.
//!
//! 🦆 The duck asked why we need a backend enum when we have trait objects.
//!    We said "monomorphization." The duck left. It didn't want a lecture.

use super::{Composer, JsonArrayComposer, NdjsonComposer};
use crate::supervisors::config::SinkConfig;
use crate::transforms::DocumentTransformer;
use anyhow::Result;

// -- ┌─────────────────────────────────────────────────────────┐
// -- │  ComposerBackend                                        │
// -- │  Enum → impl ComposerBackend → impl Composer → tests   │
// -- └─────────────────────────────────────────────────────────┘

/// 🎭 The polymorphic composer — wraps concrete composers, dispatches via match.
///
/// Same pattern as `DocumentTransformer`, `SourceBackend`, `SinkBackend`.
/// The compiler monomorphizes each arm. Branch prediction eliminates the match
/// after a couple iterations. The enum is a formality. The dispatch is basically free.
///
/// 🧠 Knowledge graph: resolved from `SinkConfig` because the payload format
/// is determined by where the data is going, not where it came from.
/// ES needs NDJSON. Files need NDJSON. InMemory wants JSON arrays. Simple.
#[derive(Debug, Clone)]
pub(crate) enum ComposerBackend {
    /// 📡 Newline-delimited JSON — transform + join with `\n`
    Ndjson(NdjsonComposer),
    /// 📦 JSON array — transform + wrap in `[`, commas, `]`
    JsonArray(JsonArrayComposer),
}

impl ComposerBackend {
    /// 🔧 Resolve the composer from the sink config.
    ///
    /// | SinkConfig      | Composer          | Format          |
    /// |-----------------|-------------------|-----------------|
    /// | Elasticsearch   | NdjsonComposer    | `item\nitem\n`  |
    /// | File            | NdjsonComposer    | `item\nitem\n`  |
    /// | InMemory        | JsonArrayComposer | `[item,item]`   |
    ///
    /// 🧠 Format follows the sink, not the source. The sink decides the wire format.
    /// This is the one true law. Do not question it. The borrow checker already has enough opinions.
    pub(crate) fn from_sink_config(sink: &SinkConfig) -> Self {
        match sink {
            // -- 📡 ES bulk requires NDJSON — action+source pairs, trailing \n
            SinkConfig::Elasticsearch(_) => Self::Ndjson(NdjsonComposer),
            // -- 📡 File sinks: NDJSON — one doc per line, trailing \n, everyone's happy
            SinkConfig::File(_) => Self::Ndjson(NdjsonComposer),
            // -- 📦 InMemory: JSON array — test assertions want `[doc1,doc2]` not `doc1\ndoc2\n`
            SinkConfig::InMemory(_) => Self::JsonArray(JsonArrayComposer),
        }
    }
}

impl Composer for ComposerBackend {
    #[inline]
    fn compose(&self, pages: &[String], transformer: &DocumentTransformer) -> Result<String> {
        // -- 🎭 Dispatch to the concrete composer — the match arm that wins is the one that deserves to
        // -- TODO: win the lottery, retire, replace this with a lookup table. Just kidding. This is fine.
        match self {
            Self::Ndjson(c) => c.compose(pages, transformer),
            Self::JsonArray(c) => c.compose(pages, transformer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transforms::passthrough::Passthrough;

    // -- 🔧 Passthrough transformer helper — the identity function of the transform world
    fn passthrough_transformer() -> DocumentTransformer {
        DocumentTransformer::Passthrough(Passthrough)
    }

    #[test]
    fn backend_the_one_where_es_config_resolves_to_ndjson() {
        use crate::backends::elasticsearch::ElasticsearchSinkConfig;
        let config = SinkConfig::Elasticsearch(ElasticsearchSinkConfig {
            url: "http://localhost:9200".into(),
            username: None,
            password: None,
            api_key: None,
            index: None,
            common_config: Default::default(),
        });
        let composer = ComposerBackend::from_sink_config(&config);
        assert!(matches!(composer, ComposerBackend::Ndjson(_)));
    }

    #[test]
    fn backend_the_one_where_inmemory_resolves_to_json_array() {
        let config = SinkConfig::InMemory(());
        let composer = ComposerBackend::from_sink_config(&config);
        assert!(matches!(composer, ComposerBackend::JsonArray(_)));
    }

    #[test]
    fn backend_the_one_where_file_resolves_to_ndjson() {
        use crate::backends::file::FileSinkConfig;
        let config = SinkConfig::File(FileSinkConfig {
            file_name: "output.json".into(),
            common_config: Default::default(),
        });
        let composer = ComposerBackend::from_sink_config(&config);
        assert!(matches!(composer, ComposerBackend::Ndjson(_)));
    }

    #[test]
    fn backend_the_one_where_compose_dispatches_correctly() -> Result<()> {
        // 🧪 ComposerBackend dispatches to the right concrete composer
        let composer = ComposerBackend::from_sink_config(&SinkConfig::InMemory(()));
        let pages = vec![
            String::from(r#"{"a":1}"#),
            String::from(r#"{"b":2}"#),
        ];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, r#"[{"a":1},{"b":2}]"#);
        Ok(())
    }
}
