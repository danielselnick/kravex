// ai
//! 🎬 *[NDJSON: the format ES demands. The format logs whisper about at night.]*
//! *[one newline per doc. no commas. no brackets. just vibes and vertical whitespace.]*
//!
//! 📡 `NdjsonCollector` — newline-delimited JSON assembly, artisan grade.
//!
//! 🧠 Knowledge graph:
//! - **Used by**: ES `/_bulk` (action+source per doc), file sinks (passthrough)
//! - **Format**: each item gets exactly one trailing `\n`, nothing else
//! - **Wire contract**: for ES bulk, each "item" is already `"action\nsource"` from the transform layer
//! - **No framing**: no `[`, no `]`, no `,` — just sequential lines. NDJSON is antisocial JSON.
//!
//! What's the DEAL with NDJSON? It's JSON but every document is in solitary confinement.
//! No brackets to hold them together. No commas. Just newlines and existential loneliness.
//!
//! 🦆 (the duck asks: if NDJSON falls in a forest with no parser, does it stream?)

use super::PayloadCollector;

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
/// Rust borrow checker trauma: the borrow checker approved this function on the first try.
/// We are still suspicious. Something must be wrong.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NdjsonCollector;

impl PayloadCollector for NdjsonCollector {
    #[inline]
    fn collect(&self, items: &[String]) -> String {
        // 🧮 Pre-allocate: sum of all strings + 1 newline per string. No reallocs. No drama.
        // Knowledge graph: `with_capacity` prevents the Vec-equivalent of moving every 2 moves.
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

#[cfg(test)]
mod tests {
    use super::*;

    // 🧪 NDJSON tests: where newlines are the hero and brackets are the villain

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
        // 🧪 Solo doc still earns its newline. Participation trophy: granted.
        let collector = NdjsonCollector;
        let items = vec![String::from(r#"{"solo":true}"#)];
        let result = collector.collect(&items);
        assert_eq!(result, "{\"solo\":true}\n");
    }
}
