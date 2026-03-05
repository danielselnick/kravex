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
use crate::casts::{DocumentCaster, Caster};
use anyhow::Result;

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
    fn join(&self, feeds: &[String], caster: &DocumentCaster) -> Result<String> {
        // -- 🧮 Pre-allocate based on total feed bytes — a vibes-based estimate that's usually close
        // -- Knowledge graph: +64 per feed accounts for cast overhead (action lines in ES bulk)
        let estimated_size: usize = feeds.iter().map(|f| f.len() + 64).sum();
        let mut payload = String::with_capacity(estimated_size);

        for feed in feeds {
            // -- 🔄 Cast this feed → transformed String
            // -- Passthrough: identity (zero overhead). NdJsonToBulk: action+source pairs.
            let cast_result = caster.cast(feed)?;
            payload.push_str(&cast_result);
            payload.push('\n');
        }

        // -- ✅ Trailing \n included — ES bulk requires it, files appreciate it, nobody complains.
        // -- Ancient proverb: "He who omits the trailing newline, debugs at 3am."
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::casts::passthrough::Passthrough;

    // -- 🔧 Helper: build a passthrough caster — the laziest caster that ever lived
    fn passthrough_caster() -> DocumentCaster {
        DocumentCaster::Passthrough(Passthrough)
    }

    #[test]
    fn ndjson_the_one_where_single_feed_joins_to_ndjson() -> Result<()> {
        // 🧪 One feed with content → content + trailing newline
        let manifold = NdjsonManifold;
        let feeds = vec![String::from(r#"{"doc":1}"#)];
        let result = manifold.join(&feeds, &passthrough_caster())?;
        assert_eq!(result, "{\"doc\":1}\n");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_multiple_feeds_join() -> Result<()> {
        // 🧪 Two feeds → two lines, each with trailing \n
        let manifold = NdjsonManifold;
        let feeds = vec![
            String::from(r#"{"doc":1}"#),
            String::from(r#"{"doc":2}"#),
        ];
        let result = manifold.join(&feeds, &passthrough_caster())?;
        assert_eq!(result, "{\"doc\":1}\n{\"doc\":2}\n");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_empty_feeds_produces_nothing() -> Result<()> {
        // 🧪 No feeds, no payload. The void stares back. It is empty. 🦆
        let manifold = NdjsonManifold;
        let result = manifold.join(&[], &passthrough_caster())?;
        assert!(result.is_empty(), "Empty input → empty output. Zen.");
        Ok(())
    }
}
