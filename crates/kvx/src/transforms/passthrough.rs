// ai
//! 🚶 Passthrough Transform — the "I changed nothing and took credit" of data transforms 🎭🔄
//!
//! 🎬 COLD OPEN — INT. OFFICE — STANDUP MEETING — 9:03 AM
//!
//! "What did you do yesterday?"
//! "I passed data through unchanged."
//! "And what are you doing today?"
//! "Passing data through unchanged."
//! "Any blockers?"
//! "No. I am the blocker. I am become passthrough, destroyer of nothing."
//!
//! This module implements the identity transform — data goes in, same data comes out.
//! It exists for testing, for "I just want to move files without transforming them,"
//! and for that one developer who runs benchmarks without transforms to prove the
//! transforms aren't the bottleneck (they aren't, Kevin, it's always the network).
//!
//! ## Knowledge Graph 🧠
//! - Implements: `IngestTransform`, `EgressTransform`
//! - Used for: testing, benchmarking, raw file-to-file copies
//! - Cost: zero. Literally zero. The compiler may even optimize this to nothing.
//! - Side effects: existential satisfaction that identity functions exist in production
//!
//! Zero-cost abstraction? Try zero-effort abstraction.
//! ⚠️ The singularity won't even notice this module exists. 🦆

use super::{EgressTransform, IngestTransform};
use crate::common::Hit;
use anyhow::Result;

/// 🚶 RawJsonPassthrough — when you want your data moved, not understood.
///
/// Implements both [`IngestTransform`] and [`EgressTransform`] as identity operations.
/// The data goes in raw. The data comes out raw. Nobody is transformed.
/// Nobody is changed. It's like a hallway: architecturally necessary,
/// emotionally neutral, and surprisingly load-bearing.
///
/// # When to use this 🤔
/// - File → File copies (just move the bytes, don't ask questions)
/// - Testing the pipeline without format-specific logic
/// - Benchmarking raw throughput (is it the transform? no. it's never the transform.)
/// - When you genuinely don't care what the JSON looks like inside
pub(crate) struct RawJsonPassthrough;

impl IngestTransform for RawJsonPassthrough {
    /// 🔄 "Transform" is a strong word for what happens here.
    ///
    /// Takes the raw string. Puts it in a Hit. That's it.
    /// No parsing. No extraction. No id, no routing, no index.
    /// The Hit is born nameless and indexless — like a protagonist
    /// at the start of an RPG who hasn't chosen a class yet.
    /// Bureaucracy and identity come later. Or never. We don't judge.
    fn transform_hit(raw: String) -> Result<Hit> {
        // -- 📋 The UN translator shrugs. "It's already in the target language."
        // -- The borrow checker nods approvingly. Ownership transferred cleanly.
        // -- Everyone goes home early. This is the dream.
        Ok(Hit {
            id: None,       // 💀 No ID. Identity is a social construct anyway.
            routing: None,  // 💀 No routing. Go wherever you want. Be free.
            index: None,    // 💀 No index. The void awaits. Or the default. Same thing.
            sort: 0,        // 🔢 Zero. The existential default.
            source_buf: raw, // ✅ The ONE thing we keep. Zero-copy. Zero-effort. Zero regrets.
        })
    }
}

impl EgressTransform for RawJsonPassthrough {
    /// 🔄 Returns `source_buf` as-is. The data equivalent of forwarding an email
    /// without reading it. We've all done it. Don't lie. HR knows.
    fn transform_hit(hit: &Hit) -> Result<String> {
        // -- 📬 Return to sender. No modification. No judgment. Just vibes.
        // -- Clone because the trait borrows. The clone is the cost of politeness.
        Ok(hit.source_buf.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_passthrough_ingest_preserves_everything() -> Result<()> {
        // 🧪 What goes in must come out. Conservation of data.
        // Newton would have approved, if he'd had JSON.
        let the_original_masterpiece = r#"{"sacred":"data","do_not":"touch"}"#.to_string();
        let the_hit = <RawJsonPassthrough as IngestTransform>::transform_hit(the_original_masterpiece.clone())?;

        assert_eq!(the_hit.source_buf, the_original_masterpiece, "source_buf must be identical");
        assert_eq!(the_hit.id, None, "No id extraction for passthrough");
        assert_eq!(the_hit.index, None, "No index assignment for passthrough");
        assert_eq!(the_hit.routing, None, "No routing for passthrough");
        assert_eq!(the_hit.sort, 0, "Sort is always 0 for passthrough");

        Ok(())
    }

    #[test]
    fn the_one_where_passthrough_egress_is_literally_a_clone() -> Result<()> {
        // 🧪 Egress should return source_buf unchanged.
        // If this test fails, physics is broken. Call CERN.
        let the_hit = Hit {
            id: Some("ignored-anyway".to_string()),
            index: Some("also-ignored".to_string()),
            routing: Some("yep-ignored-too".to_string()),
            sort: 42,
            source_buf: r#"{"the":"payload"}"#.to_string(),
        };

        let the_output = <RawJsonPassthrough as EgressTransform>::transform_hit(&the_hit)?;
        assert_eq!(the_output, r#"{"the":"payload"}"#, "Egress must return source_buf as-is");

        Ok(())
    }

    #[test]
    fn the_one_where_passthrough_roundtrip_is_identity() -> Result<()> {
        // 🧪 Ingest → Egress should be the identity function.
        // If f(g(x)) != x, we have invented a new kind of math. Patent pending.
        let the_sacred_input = r#"{"round":"trip","verified":true}"#.to_string();

        let the_hit = <RawJsonPassthrough as IngestTransform>::transform_hit(the_sacred_input.clone())?;
        let the_output = <RawJsonPassthrough as EgressTransform>::transform_hit(&the_hit)?;

        assert_eq!(
            the_output, the_sacred_input,
            "Round-trip must be identity. Math demands it."
        );

        Ok(())
    }
}
