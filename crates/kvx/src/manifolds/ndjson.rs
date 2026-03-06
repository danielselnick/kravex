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
/// Casts each feed, splits cast output into doc-units of `lines_per_doc()` lines,
/// then greedily packs doc-units into size-bounded chunks (`max_bytes`).
///
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
    fn join(&self, feeds: &[String], caster: &DocumentCaster, max_bytes: usize) -> Result<(Vec<String>, usize)> {
        
    }
}
