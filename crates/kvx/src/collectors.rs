// ai
//! 🎬 *[the transforms have been applied. the sink awaits. but someone... must assemble the payload.]*
//! *[dramatic zoom on a trait nobody knew they needed until the newlines got political]*
//!
//! 📦 The Collectors module — payload assembly, extracted and dignified.
//!
//! Takes a slice of transformed document strings and assembles them into
//! a single payload string in the wire format the sink expects. This is the
//! formatting step between "I transformed each doc" and "I sent the payload."
//!
//! 🧠 Knowledge graph:
//! - **NDJSON** (`NdjsonCollector`): `\n`-delimited. Used by ES `/_bulk` and file sinks.
//! - **JSON Array** (`JsonArrayCollector`): `[doc,doc,doc]`. Used by in-memory sinks for testing.
//! - Resolution: from `SinkConfig`, same pattern as backends and transforms.
//! - Zero serde for JSON array — just brackets and commas, artisan-grade string assembly.
//!
//! ```text
//! SinkWorker pipeline:
//!   channel → transform each → collector.collect(&[String]) → sink.send(payload)
//! ```
//!
//! The collector is the bouncer between transforms and sinks. It decides whether
//! your documents get newlines, commas, or brackets. It does not negotiate.
//!
//! 🦆 (the duck collects... ducks? rubber ones? unclear. the duck has no comment.)

use crate::supervisors::config::SinkConfig;

// ===== Trait =====

/// 📦 Assembles transformed doc strings into a final payload format.
///
/// Each sink type has its own wire format. NDJSON for Elasticsearch and files.
/// JSON array for in-memory testing. The collector handles this concern
/// so the SinkWorker, Sink, and Transform don't have to know about delimiters.
///
/// 🧠 Knowledge graph: this trait mirrors the `Transform` and `Source`/`Sink` pattern —
/// trait → concrete impls → enum dispatcher → from_config resolver.
///
/// Ancient proverb: "He who hardcodes '\n' in the worker, reformats in production."
pub(crate) trait PayloadCollector: std::fmt::Debug {
    /// 📦 Assemble a slice of transformed strings into a single payload.
    /// The input strings are already in their final per-document format.
    /// The collector only adds inter-document delimiters and framing.
    fn collect(&self, items: &[String]) -> String;
}

// ===== NDJSON Collector =====

/// 📡 Newline-Delimited JSON — the format ES `/_bulk` demands and files prefer.
///
/// Each string gets a trailing `\n`. That's it. That's the whole format.
/// NDJSON: because JSON arrays were too organized for some people.
/// Also because streaming parsers love it. Also because ES said so. We don't argue with ES.
///
/// For ES bulk, each transformed string is "action\nsource" (two NDJSON lines per doc).
/// After collect: "action1\nsource1\naction2\nsource2\n" — valid `/_bulk` payload.
///
/// For file passthrough: "doc1\ndoc2\n" — valid newline-delimited file content.
///
/// What's the DEAL with NDJSON? It's JSON but unfriendly. Every line is lonely.
/// No brackets to hold them. No commas to connect them. Just newlines. And silence.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NdjsonCollector;

impl PayloadCollector for NdjsonCollector {
    #[inline]
    fn collect(&self, items: &[String]) -> String {
        // 🧮 Pre-allocate: sum of all strings + 1 newline per string. No reallocs. No drama.
        let estimated_size: usize = items.iter().map(|s| s.len() + 1).sum();
        let mut payload = String::with_capacity(estimated_size);
        for item in items {
            payload.push_str(item);
            payload.push('\n');
        }
        // ✅ Trailing \n included — ES bulk requires it, files appreciate it, nobody complains.
        payload
    }
}

// ===== JSON Array Collector =====

/// 📦 JSON Array format — `[doc1,doc2,doc3]` — for when you want valid JSON output.
///
/// Zero serde. Zero parsing. Zero copy of the inner doc strings.
/// Just brackets and commas. Assembled by hand in a `String::with_capacity`,
/// like artisans at a craft fair who specialize in very fast string concatenation.
///
/// 🧠 Used for in-memory sinks where tests want valid JSON arrays to assert against.
/// The docs inside are already valid JSON strings from the transforms — we just
/// frame them as an array without re-parsing. Trust the transforms. They did their job.
///
/// Conspiracy theory: the borrow checker is sentient, and it WANTS you to use serde.
/// We resist. We concatenate manually. We are free. 🦆
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsonArrayCollector;

impl PayloadCollector for JsonArrayCollector {
    #[inline]
    fn collect(&self, items: &[String]) -> String {
        // 🧮 Pre-allocate: brackets(2) + sum of strings + commas(max n-1).
        // This is vibes-based capacity estimation but with actual math backing it up.
        let commas = if items.is_empty() { 0 } else { items.len() - 1 };
        let estimated_size: usize =
            2 + items.iter().map(|s| s.len()).sum::<usize>() + commas;
        let mut payload = String::with_capacity(estimated_size);
        payload.push('[');
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                // 🔗 The comma: JSON's way of saying "and there's more where that came from."
                payload.push(',');
            }
            payload.push_str(item);
        }
        payload.push(']');
        // ✅ Valid JSON array. No serde was harmed in the making of this string.
        payload
    }
}

// ===== Dispatcher Enum =====

/// 🎭 The polymorphic collector — wraps concrete collectors, dispatches via match.
///
/// Same pattern as `DocumentTransformer`, `SourceBackend`, `SinkBackend`.
/// The compiler monomorphizes each arm. Branch prediction eliminates the match
/// after a couple iterations. The enum is a formality. The dispatch is basically free.
///
/// 🧠 Knowledge graph: resolved from `SinkConfig` because the payload format
/// is determined by where the data is going, not where it came from.
/// ES needs NDJSON. Files need NDJSON. InMemory wants JSON arrays. Simple.
#[derive(Debug, Clone)]
pub(crate) enum CollectorBackend {
    /// 📡 Newline-delimited JSON — one `\n` per transformed string
    Ndjson(NdjsonCollector),
    /// 📦 JSON array — `[`, commas, `]`, zero serde
    JsonArray(JsonArrayCollector),
}

impl CollectorBackend {
    /// 🔧 Resolve the collector from the sink config.
    ///
    /// | SinkConfig | Collector | Format |
    /// |---|---|---|
    /// | Elasticsearch | NdjsonCollector | `doc\ndoc\n` |
    /// | File | NdjsonCollector | `doc\ndoc\n` |
    /// | InMemory | JsonArrayCollector | `[doc,doc]` |
    pub(crate) fn from_sink_config(sink: &SinkConfig) -> Self {
        match sink {
            SinkConfig::Elasticsearch(_) => Self::Ndjson(NdjsonCollector),
            SinkConfig::File(_) => Self::Ndjson(NdjsonCollector),
            SinkConfig::InMemory(_) => Self::JsonArray(JsonArrayCollector),
        }
    }
}

impl PayloadCollector for CollectorBackend {
    #[inline]
    fn collect(&self, items: &[String]) -> String {
        match self {
            Self::Ndjson(c) => c.collect(items),
            Self::JsonArray(c) => c.collect(items),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== NDJSON tests =====

    #[test]
    fn ndjson_the_one_where_multiple_docs_get_newlines() {
        // 🧪 Three docs in, three lines out, each with trailing \n
        let collector = NdjsonCollector;
        let items = vec![
            String::from(r#"{"action":"index"}\n{"doc":1}"#),
            String::from(r#"{"action":"index"}\n{"doc":2}"#),
        ];
        let result = collector.collect(&items);
        assert!(result.ends_with('\n'), "NDJSON must end with trailing newline");
        assert_eq!(result.matches('\n').count(), 2, "One trailing \\n per item");
    }

    #[test]
    fn ndjson_the_one_where_empty_vec_produces_nothing() {
        // 🧪 No docs, no payload. The void stares back. It is empty. 🦆
        let collector = NdjsonCollector;
        let result = collector.collect(&[]);
        assert!(result.is_empty(), "Empty input → empty output. Zen.");
    }

    #[test]
    fn ndjson_the_one_where_single_doc_still_gets_newline() {
        let collector = NdjsonCollector;
        let items = vec![String::from(r#"{"solo":true}"#)];
        let result = collector.collect(&items);
        assert_eq!(result, "{\"solo\":true}\n");
    }

    // ===== JSON Array tests =====

    #[test]
    fn json_array_the_one_where_docs_become_an_array() {
        // 🧪 The JSON array: brackets, commas, and zero serde. As nature intended.
        let collector = JsonArrayCollector;
        let items = vec![
            String::from(r#"{"doc":1}"#),
            String::from(r#"{"doc":2}"#),
            String::from(r#"{"doc":3}"#),
        ];
        let result = collector.collect(&items);
        assert_eq!(result, r#"[{"doc":1},{"doc":2},{"doc":3}]"#);
    }

    #[test]
    fn json_array_the_one_where_empty_vec_is_empty_array() {
        // 🧪 No docs → []. Still valid JSON. Still technically correct. The best kind of correct.
        let collector = JsonArrayCollector;
        let result = collector.collect(&[]);
        assert_eq!(result, "[]");
    }

    #[test]
    fn json_array_the_one_where_single_doc_has_no_commas() {
        // 🧪 One doc, no commas. Like a party with one guest. Awkward but valid.
        let collector = JsonArrayCollector;
        let items = vec![String::from(r#"{"lonely":true}"#)];
        let result = collector.collect(&items);
        assert_eq!(result, r#"[{"lonely":true}]"#);
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
        let collector = CollectorBackend::from_sink_config(&config);
        assert!(matches!(collector, CollectorBackend::Ndjson(_)));
    }

    #[test]
    fn backend_the_one_where_inmemory_resolves_to_json_array() {
        let config = SinkConfig::InMemory(());
        let collector = CollectorBackend::from_sink_config(&config);
        assert!(matches!(collector, CollectorBackend::JsonArray(_)));
    }

    #[test]
    fn backend_the_one_where_file_resolves_to_ndjson() {
        use crate::backends::file::FileSinkConfig;
        let config = SinkConfig::File(FileSinkConfig {
            file_name: "output.json".into(),
            common_config: Default::default(),
        });
        let collector = CollectorBackend::from_sink_config(&config);
        assert!(matches!(collector, CollectorBackend::Ndjson(_)));
    }
}
