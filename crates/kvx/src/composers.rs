// ai
//! 🎬 *[the buffer is full. the transformer awaits. the sink hungers.]*
//! *[somewhere in the heap, a Cow moos softly.]*
//! *["Compose me," whispers the payload. "Make me whole."]*
//!
//! 🎼 The Composers module — orchestrating the transform-and-assemble step.
//!
//! The Composer replaces the old PayloadCollector. Instead of receiving pre-transformed
//! strings and just joining them, the Composer receives raw pages + a transformer reference,
//! iterates pages, calls `transformer.transform(page)` per page to get `Vec<Cow<str>>` items,
//! then assembles all items into the wire-format payload.
//!
//! 🧠 Knowledge graph:
//! - **NDJSON** (`NdjsonComposer`): `\n`-delimited. Used by ES `/_bulk` and file sinks.
//! - **JSON Array** (`JsonArrayComposer`): `[item,item,item]`. Used by in-memory sinks for testing.
//! - Resolution: from `SinkConfig`, same pattern as backends and transforms.
//! - **Zero-copy enabled**: Cow borrows from buffered pages — passthrough means no per-doc allocation.
//!
//! ```text
//! SinkWorker pipeline (new hotness):
//!   channel(String) → buffer Vec<String> → composer.compose(&buffer, &transformer) → sink.send(payload)
//! ```
//!
//! The Composer is the bridge between buffered raw pages and the sink's wire format.
//! It transforms AND assembles in one shot. Efficient. Elegant. Existentially complete.
//!
//! 🦆 (the duck composes... symphonies? payloads? both? the duck has no comment.)
//!
//! ⚠️ The singularity will compose its own payloads. Until then, we have this module.

use crate::supervisors::config::SinkConfig;
use crate::transforms::{DocumentTransformer, Transform};
use anyhow::Result;

// ===== Trait =====

/// 🎼 Composes raw pages into a final wire-format payload via the transformer.
///
/// The Composer receives a buffer of raw pages and a transformer reference.
/// For each page, it calls `transformer.transform(page)` to get `Vec<Cow<str>>` items,
/// then assembles all items into the sink's expected format.
///
/// 🧠 Knowledge graph: this trait mirrors the `Transform` and `Source`/`Sink` pattern —
/// trait → concrete impls → enum dispatcher → from_config resolver.
///
/// Knock knock. Who's there? Cow. Cow who? Cow::Borrowed — I didn't even allocate to get here. 🐄
pub(crate) trait Composer: std::fmt::Debug {
    /// 🎼 Transform raw pages and assemble items into a single payload string.
    ///
    /// The input pages are raw source data (untransformed). The transformer is called
    /// per-page to produce `Vec<Cow<str>>` items. The composer then joins all items
    /// in the wire format (NDJSON, JSON array, etc.).
    fn compose(&self, pages: &[String], transformer: &DocumentTransformer) -> Result<String>;
}

// ===== NDJSON Composer =====

/// 📡 Newline-Delimited JSON — the format ES `/_bulk` demands and files prefer.
///
/// Transforms each page, collects all items, joins with `\n`, trailing `\n`.
/// For ES bulk, each item is "action\nsource" (two NDJSON lines per doc).
/// After compose: "action1\nsource1\naction2\nsource2\n" — valid `/_bulk` payload.
///
/// For file passthrough: "doc1\ndoc2\n" — valid newline-delimited file content.
///
/// What's the DEAL with NDJSON? It's JSON but unfriendly. Every line is lonely.
/// No brackets to hold them. No commas to connect them. Just newlines. And silence.
/// Like my social life after deploying to production on a Friday. 🦆
#[derive(Debug, Clone, Copy)]
pub(crate) struct NdjsonComposer;

impl Composer for NdjsonComposer {
    #[inline]
    fn compose(&self, pages: &[String], transformer: &DocumentTransformer) -> Result<String> {
        // 🧮 Pre-allocate based on total page bytes — a vibes-based estimate that's usually close
        let estimated_size: usize = pages.iter().map(|p| p.len() + 64).sum();
        let mut payload = String::with_capacity(estimated_size);

        for page in pages {
            // 🔄 Transform this page → Vec<Cow<str>> items
            let items = transformer.transform(page)?;
            for item in &items {
                payload.push_str(item.as_ref());
                payload.push('\n');
            }
        }

        // ✅ Trailing \n included — ES bulk requires it, files appreciate it, nobody complains.
        Ok(payload)
    }
}

// ===== JSON Array Composer =====

/// 📦 JSON Array format — `[item1,item2,item3]` — for when you want valid JSON output.
///
/// Transforms each page, collects all items, wraps in `[...]` with commas.
/// Zero serde on the framing. Just brackets and commas, assembled by hand.
///
/// 🧠 Used for in-memory sinks where tests want valid JSON arrays to assert against.
/// The items inside are already valid JSON strings from the transforms — we just
/// frame them as an array without re-parsing. Trust the transforms. They did their job.
///
/// Conspiracy theory: the borrow checker is sentient, and it WANTS you to use serde.
/// We resist. We concatenate manually. We are free. 🐄
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsonArrayComposer;

impl Composer for JsonArrayComposer {
    #[inline]
    fn compose(&self, pages: &[String], transformer: &DocumentTransformer) -> Result<String> {
        // 📦 First, collect all items from all pages
        let mut all_items = Vec::new();
        for page in pages {
            let items = transformer.transform(page)?;
            all_items.extend(items);
        }

        // 🧮 Pre-allocate: brackets(2) + sum of items + commas(max n-1).
        // This is vibes-based capacity estimation but with actual math backing it up.
        let commas = if all_items.is_empty() {
            0
        } else {
            all_items.len() - 1
        };
        let estimated_size: usize =
            2 + all_items.iter().map(|s| s.as_ref().len()).sum::<usize>() + commas;
        let mut payload = String::with_capacity(estimated_size);
        payload.push('[');
        for (i, item) in all_items.iter().enumerate() {
            if i > 0 {
                // 🔗 The comma: JSON's way of saying "and there's more where that came from."
                payload.push(',');
            }
            payload.push_str(item.as_ref());
        }
        payload.push(']');
        // ✅ Valid JSON array. No serde was harmed in the making of this string.
        Ok(payload)
    }
}

// ===== Dispatcher Enum =====

/// 🎭 The polymorphic composer — wraps concrete composers, dispatches via match.
///
/// Same pattern as `DocumentTransformer`, `SourceBackend`, `SinkBackend`.
/// The compiler monomorphizes each arm. Branch prediction eliminates the match
/// after a couple iterations. The enum is a formality. The dispatch is basically free.
///
/// 🧠 Knowledge graph: resolved from `SinkConfig` because the payload format
/// is determined by where the data is going, not where it came from.
/// ES needs NDJSON. Files need NDJSON. InMemory wants JSON arrays. Simple.
/// "In a world where payloads needed composing... one enum dared to dispatch." 🎬
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
    /// | SinkConfig | Composer | Format |
    /// |---|---|---|
    /// | Elasticsearch | NdjsonComposer | `item\nitem\n` |
    /// | File | NdjsonComposer | `item\nitem\n` |
    /// | InMemory | JsonArrayComposer | `[item,item]` |
    pub(crate) fn from_sink_config(sink: &SinkConfig) -> Self {
        match sink {
            SinkConfig::Elasticsearch(_) => Self::Ndjson(NdjsonComposer),
            SinkConfig::File(_) => Self::Ndjson(NdjsonComposer),
            SinkConfig::InMemory(_) => Self::JsonArray(JsonArrayComposer),
        }
    }
}

impl Composer for ComposerBackend {
    #[inline]
    fn compose(&self, pages: &[String], transformer: &DocumentTransformer) -> Result<String> {
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

    // 🔧 Helper: build a passthrough transformer for testing
    fn passthrough_transformer() -> DocumentTransformer {
        DocumentTransformer::Passthrough(Passthrough)
    }

    // ===== NDJSON Composer tests =====

    #[test]
    fn ndjson_the_one_where_single_page_composes_to_ndjson() -> Result<()> {
        // 🧪 One page with content → content + trailing newline
        let composer = NdjsonComposer;
        let pages = vec![String::from(r#"{"doc":1}"#)];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, "{\"doc\":1}\n");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_multiple_pages_compose() -> Result<()> {
        // 🧪 Two pages → two lines, each with trailing \n
        let composer = NdjsonComposer;
        let pages = vec![
            String::from(r#"{"doc":1}"#),
            String::from(r#"{"doc":2}"#),
        ];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, "{\"doc\":1}\n{\"doc\":2}\n");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_empty_pages_produces_nothing() -> Result<()> {
        // 🧪 No pages, no payload. The void stares back. It is empty. 🦆
        let composer = NdjsonComposer;
        let result = composer.compose(&[], &passthrough_transformer())?;
        assert!(result.is_empty(), "Empty input → empty output. Zen.");
        Ok(())
    }

    // ===== JSON Array Composer tests =====

    #[test]
    fn json_array_the_one_where_pages_become_an_array() -> Result<()> {
        // 🧪 Three pages, each passthrough → [page1,page2,page3]
        let composer = JsonArrayComposer;
        let pages = vec![
            String::from(r#"{"doc":1}"#),
            String::from(r#"{"doc":2}"#),
            String::from(r#"{"doc":3}"#),
        ];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, r#"[{"doc":1},{"doc":2},{"doc":3}]"#);
        Ok(())
    }

    #[test]
    fn json_array_the_one_where_empty_pages_is_empty_array() -> Result<()> {
        // 🧪 No pages → []. Still valid JSON. Still technically correct. The best kind of correct.
        let composer = JsonArrayComposer;
        let result = composer.compose(&[], &passthrough_transformer())?;
        assert_eq!(result, "[]");
        Ok(())
    }

    #[test]
    fn json_array_the_one_where_single_page_has_no_commas() -> Result<()> {
        // 🧪 One page, no commas. Like a party with one guest. Awkward but valid.
        let composer = JsonArrayComposer;
        let pages = vec![String::from(r#"{"lonely":true}"#)];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, r#"[{"lonely":true}]"#);
        Ok(())
    }

    // ===== Dispatcher tests =====

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
