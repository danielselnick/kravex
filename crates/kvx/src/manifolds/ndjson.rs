// ai
//! 🎬 *[a dark and stormy deploy. the sink demands newlines. the caster obliges.]*
//! *[every line, alone. no brackets. no comfort. just `\n`. this is NDJSON.]*
//!
//! 📡 **NdjsonManifold** — casts feeds and joins them into newline-delimited JSON payloads.
//!
//! 🧠 Knowledge graph:
//! - Used by: ES `/_bulk` and file sinks — both want `item\nitem\n` format
//! - For ES bulk: caster emits two lines per doc (action + source)
//! - Trailing `\n` is mandatory for ES bulk, appreciated by file sinks, ignored by nobody
//!
//! 🦆 The duck asked what NDJSON stands for. We told it. It left anyway.

use super::Manifold;
use crate::{Entry, Payload};
use anyhow::Result;
use std::collections::VecDeque;

// -- ┌─────────────────────────────────────────────────────────┐
// -- │  NdjsonManifold                                          │
// -- │  Struct → impl Manifold → tests                          │
// -- └─────────────────────────────────────────────────────────┘

/// 📡 Newline-Delimited JSON — the format ES `/_bulk` demands and files prefer.
///
/// Casts each feed, joins results with `\n`, trailing `\n`.
/// For ES bulk, each cast result is "action\nsource" (two NDJSON lines per doc).
/// After join: "action1\nsource1\naction2\nsource2\n" — valid `/_bulk` payload.
///
/// For file passthrough: "doc1\ndoc2\n" — valid newline-delimited file content.
///
/// What's the DEAL with NDJSON? It's JSON but unfriendly. Every line is lonely.
/// No brackets to hold them. No commas to connect them. Just newlines. And silence.
/// Like my social life after deploying to production on a Friday. 🦆
#[derive(Debug, Clone, Copy)]
pub struct NdjsonManifold;

impl Manifold for NdjsonManifold {
    #[inline]
    fn join(&self, entries: &mut VecDeque<Entry>) -> Result<Payload> {
        // -- 🧮 Pre-allocate based on total entry bytes — a vibes-based estimate that's usually close
        // -- Knowledge graph: +1 per entry for the \n separator, because math is caring
        let estimated_size: usize = entries.iter().map(|e| e.len() + 1).sum();
        let mut payload = String::with_capacity(estimated_size);

        for entry in entries.drain(..) {
            // -- 🔄 Each entry is already cast — just stitch them together with newlines
            // -- Like a quilt, but made of JSON, and nobody finds it cozy
            payload.push_str(&entry);
            // We expect each entry to have \n if it's being casted to bulk
            // payload.push('\n');
        }

        // -- ✅ Trailing \n included — ES bulk requires it, files appreciate it, nobody complains.
        // -- Ancient proverb: "He who omits the trailing newline, debugs at 3am."
        Ok(Payload(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_the_one_where_single_entry_joins_to_ndjson() -> Result<()> {
        // 🧪 One entry with its own trailing \n → concatenated as-is
        let manifold = NdjsonManifold;
        let mut entries = VecDeque::from(vec![Entry("{\"doc\":1}\n".to_string())]);
        let result = manifold.join(&mut entries)?;
        assert_eq!(*result, "{\"doc\":1}\n");
        assert!(entries.is_empty(), "🎯 drain(..) should leave the VecDeque empty but allocated");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_multiple_entries_join() -> Result<()> {
        // 🧪 Two entries already carrying their \n — concatenated in order
        let manifold = NdjsonManifold;
        let mut entries = VecDeque::from(vec![
            Entry("{\"doc\":1}\n".to_string()),
            Entry("{\"doc\":2}\n".to_string()),
        ]);
        let result = manifold.join(&mut entries)?;
        assert_eq!(*result, "{\"doc\":1}\n{\"doc\":2}\n");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_empty_entries_produces_nothing() -> Result<()> {
        // 🧪 No entries, no payload. The void stares back. It is empty. 🦆
        let manifold = NdjsonManifold;
        let mut entries = VecDeque::new();
        let result = manifold.join(&mut entries)?;
        assert!(result.is_empty(), "Empty input → empty output. Zen.");
        Ok(())
    }
}
