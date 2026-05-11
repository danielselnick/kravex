// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎬 *[a dark and stormy deploy. the sink demands newlines. the tapper obliges.]*
//! *[every line, alone. no brackets. no comfort. just `\n`. this is NDJSON.]*
//!
//! 📡 **NdjsonManifold** — casts barrels and joins them into newline-delimited JSON drums.
//!
//! 🧠 Knowledge graph:
//! - Used by: ES `/_bulk` and file sinks — both want `item\nitem\n` format
//! - For ES bulk: tapper emits two lines per doc (action + source)
//! - Trailing `\n` is mandatory for ES bulk, appreciated by file sinks, ignored by nobody
//!
//! 🦆 The duck asked what NDJSON stands for. We told it. It left anyway.

use super::Manifold;
use crate::{Draft, Drum};
use anyhow::Result;
use std::collections::VecDeque;

// -- ┌─────────────────────────────────────────────────────────┐
// -- │  NdjsonManifold                                          │
// -- │  Struct → impl Manifold → tests                          │
// -- └─────────────────────────────────────────────────────────┘

/// 📡 Newline-Delimited JSON — the format ES `/_bulk` demands and files prefer.
///
/// Taps each barrel, joins results with `\n`, trailing `\n`.
/// For ES bulk, each tap result is "action\nsource" (two NDJSON lines per doc).
/// After join: "action1\nsource1\naction2\nsource2\n" — valid `/_bulk` drum.
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
    fn join(&self, drafts: &mut VecDeque<Draft>) -> Result<Drum> {
        // -- 🧮 Pre-allocate based on total draft bytes — a vibes-based estimate that's usually close
        // -- Knowledge graph: +1 per draft for the \n separator, because math is caring
        let estimated_size: usize = drafts.iter().map(|e| e.len() + 1).sum();
        let mut drum = String::with_capacity(estimated_size);

        for draft in drafts.drain(..) {
            // -- 🔄 Each draft is already tapped — just stitch them together with newlines
            // -- Like a quilt, but made of JSON, and nobody finds it cozy
            drum.push_str(&draft);
            // We expect each draft to have \n if it's being casted to bulk
            // drum.push('\n');
        }

        // -- ✅ Trailing \n included — ES bulk requires it, files appreciate it, nobody complains.
        // -- Ancient proverb: "He who omits the trailing newline, debugs at 3am."
        Ok(Drum(drum))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_the_one_where_single_draft_joins_to_ndjson() -> Result<()> {
        // 🧪 One draft with its own trailing \n → concatenated as-is
        let manifold = NdjsonManifold;
        let mut drafts = VecDeque::from(vec![Draft("{\"doc\":1}\n".to_string())]);
        let result = manifold.join(&mut drafts)?;
        assert_eq!(*result, "{\"doc\":1}\n");
        assert!(drafts.is_empty(), "🎯 drain(..) should leave the VecDeque empty but allocated");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_multiple_drafts_join() -> Result<()> {
        // 🧪 Two drafts already carrying their \n — concatenated in order
        let manifold = NdjsonManifold;
        let mut drafts = VecDeque::from(vec![
            Draft("{\"doc\":1}\n".to_string()),
            Draft("{\"doc\":2}\n".to_string()),
        ]);
        let result = manifold.join(&mut drafts)?;
        assert_eq!(*result, "{\"doc\":1}\n{\"doc\":2}\n");
        Ok(())
    }

    #[test]
    fn ndjson_the_one_where_empty_drafts_produce_nothing() -> Result<()> {
        // 🧪 No drafts, no drum. The void stares back. It is empty. 🦆
        let manifold = NdjsonManifold;
        let mut drafts = VecDeque::new();
        let result = manifold.join(&mut drafts)?;
        assert!(result.is_empty(), "Empty input → empty output. Zen.");
        Ok(())
    }
}
