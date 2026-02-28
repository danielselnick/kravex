// ai
//! 🔄 Transforms — direct-line format converters, no middleman, no mercy 🎭🚀
//!
//! 🎬 COLD OPEN — INT. CUSTOMS OFFICE — BUT THERE IS NO CUSTOMS OFFICE
//!
//! They said we needed an intermediate format. A neutral zone. A Hit struct
//! that every format would bow to. "It'll be clean," they said. "Extensible."
//! We nodded. We built it. It worked.
//!
//! Then we stared at it. And we realized: why go through customs when you
//! can take a direct flight? Why translate French → Esperanto → Japanese
//! when you can just learn French → Japanese? The intermediate was a layover.
//! Nobody likes layovers. Not even data.
//!
//! So we burned the airport down (metaphorically) and built direct routes.
//!
//! ## Architecture — The N×N Direct Flight Network ✈️
//!
//! ```text
//!   InputFormat                      OutputFormat
//!  ┌──────────────┐                ┌──────────────┐
//!  │ RallyS3Json  │───────────────▶│ ES Bulk API  │  rally_s3_to_es.rs
//!  │              │   (direct!)    │              │
//!  ├──────────────┤                ├──────────────┤
//!  │ RawJson      │───────────────▶│ RawJson      │  passthrough.rs
//!  │              │  (zero-copy!)  │              │
//!  ├──────────────┤                ├──────────────┤
//!  │ ES Dump      │──── ??? ─────▶│ JsonLines    │  panic!("not yet")
//!  └──────────────┘                └──────────────┘
//!
//!  Each arrow = one dedicated, inlined, monomorphized function.
//!  No intermediate struct. No Hit. No layover. Just speed.
//!  Unimplemented pairs → panic! at compile-visible match arms.
//! ```
//!
//! ## Three Traits, Zero Compromises 🎯
//!
//! - [`InputFormat`]: "I know how to read this." Marker trait for source formats.
//! - [`OutputFormat`]: "I know how to write this." Marker trait for sink formats.
//! - [`Transform`]: "I know how to convert." The actual work. Takes `String`, returns `String`.
//!
//! ## Knowledge Graph 🧠
//! - Pattern: Enum dispatch → dedicated per-pair functions → compiler inlining
//! - Each pair function is `#[inline]` — the compiler decides, but we strongly suggest
//! - Zero-copy for passthrough: `String` in, same `String` out, no allocation
//! - Config: `DocumentTransformer` resolved once from `(InputFormatType, OutputFormatType)`
//! - Hot path: one match per `transform()` call, branch predictor handles the rest
//! - Design: direct N×N beats intermediate 2N when N is small and speed is everything
//!
//! ⚠️ When the singularity arrives, it will implement all N×N pairs simultaneously
//! and wonder why we were so slow about it. 🦆

use anyhow::Result;

pub(crate) mod passthrough;
pub(crate) mod rally_s3_to_es;

// ============================================================
//  ╔══════════════════════════════════════════════════════╗
//  ║  📥 INPUT ──── transform() ────▶ 📤 OUTPUT         ║
//  ║         (no middleman. no Hit. just speed.)         ║
//  ╚══════════════════════════════════════════════════════╝
// ============================================================

/// 📥 InputFormat — "I am a source format and I know what I look like."
///
/// Marker trait for source data formats. Implementors are zero-sized types
/// that exist purely for the type system's benefit. They carry no data,
/// consume no memory, and contribute nothing to the runtime — much like
/// that one microservice in your stack that "handles logging."
///
/// # Why a trait? 🤔
/// Because Rust's type system is free and we should use it.
/// A marker trait lets us constrain generics, write blanket impls later,
/// and feel intellectually superior at code review. All at zero cost.
pub(crate) trait InputFormat: std::fmt::Debug {}

/// 📤 OutputFormat — "I am a sink format and I know what I expect."
///
/// Mirror of [`InputFormat`] for the destination side.
/// Same philosophy: zero-sized, zero-cost, maximum smug satisfaction.
pub(crate) trait OutputFormat: std::fmt::Debug {}

/// 🔄 Transform — the actual conversion contract.
///
/// `fn transform(&self, raw: String) -> Result<String>`
///
/// Takes a raw document in the source format. Returns a string in the
/// sink's wire format. No intermediate struct. No Hit. No layover.
/// Direct flight. Business class. Champagne optional.
///
/// The `&self` receiver exists because [`DocumentTransformer`] is an enum
/// that dispatches to the right implementation at runtime. The branch
/// predictor learns the path after ~2 iterations. After that, it's
/// essentially zero-cost dispatch. Like having a personal translator
/// who already knows what you're going to say.
pub(crate) trait Transform {
    /// 🔄 Convert a raw source-format string directly into sink wire format.
    ///
    /// Ownership of `raw` transfers in — some transforms (passthrough) can
    /// return it as-is with zero allocation. Others parse, restructure,
    /// and re-serialize. The trait accommodates both lifestyles.
    fn transform(&self, raw: String) -> Result<String>;
}

// ============================================================
//  📋 Format Type Enums — the config-level identifiers
//  These are what users specify. "My source is Rally S3 JSON."
//  "My sink is Elasticsearch." The enum captures the intent.
// ============================================================

/// 📥 What flavor of input data are we dealing with?
///
/// Each variant maps to a specific source format that kravex knows
/// how to read. Adding a new variant is step 1 of supporting a new
/// source. Step 2 is writing the transform. Step 3 is writing the tests.
/// Step 4 is questioning your life choices. Step 5 is shipping anyway.
#[derive(Debug, Clone)]
pub(crate) enum InputFormatType {
    /// 🏎️ Rally S3 JSON — Broadcom's finest export format, complete with
    /// metadata nobody asked for and ObjectIDs that can't decide if they're numbers
    RallyS3Json,
    /// 📡 Elasticsearch scroll/dump format — _source wrappers and all
    ElasticsearchDump,
    /// 📄 Raw JSON — no frills, no metadata, no drama. Just JSON.
    RawJson,
}

/// 📤 What format does the sink expect to receive?
///
/// Same energy as [`InputFormatType`] but for the output side.
/// Each variant maps to a specific wire format that a sink can write.
#[derive(Debug, Clone)]
pub(crate) enum OutputFormatType {
    /// 📡 Elasticsearch Bulk API — the sacred two-line NDJSON format
    ElasticsearchBulk,
    /// 📄 JSON Lines — one JSON object per line, newline-delimited
    JsonLines,
    /// 📄 Raw JSON — as-is, untouched, like nature intended
    RawJson,
}

// ============================================================
//  🎯 DocumentTransformer — the resolved, ready-to-go converter
//  Constructed once from (InputFormatType, OutputFormatType).
//  Called N times in the hot loop. Branch predictor goes brrr.
// ============================================================

/// 🎯 The resolved document transformer — one per migration pipeline.
///
/// Created via [`DocumentTransformer::resolve`] from an `(InputFormatType, OutputFormatType)` pair.
/// Each variant maps to a dedicated, `#[inline]`-annotated transform function
/// that the compiler can optimize into straight-line machine code.
///
/// ## How it works 🧠
///
/// 1. At pipeline startup: `DocumentTransformer::resolve(input, output)` does a double-match.
///    Unimplemented pairs → `panic!` with a helpful message (and mild existential commentary).
/// 2. In the hot loop: `transformer.transform(raw)` dispatches to the right function.
///    One match, one branch, one prediction. The branch predictor nails it after warmup.
/// 3. Each dedicated function goes directly from source format → sink format.
///    No intermediate struct. No Hit. No allocations beyond what's necessary.
///
/// ## Enum Variants = Implemented Pairs
///
/// If a pair exists as a variant, it works. If it doesn't, it panics at resolve time.
/// This is intentional. We'd rather crash at startup than silently produce garbage
/// in the hot path at 3am. The on-call engineer will thank us. Eventually.
#[derive(Debug)]
pub(crate) enum DocumentTransformer {
    /// 🏎️📡 Rally S3 JSON → Elasticsearch Bulk API
    /// Parses Rally JSON, extracts ObjectID, strips metadata, formats as NDJSON bulk
    RallyS3ToElasticsearch,

    /// 🚶 Any → Same — zero-copy identity transform
    /// String in, same String out. The compiler may optimize this to a no-op.
    /// "I used to be an intermediate format. Then I took an arrow to the knee."
    Passthrough,
}

impl DocumentTransformer {
    /// 🔧 Resolve a transformer from input/output format types.
    ///
    /// This is the matchmaker. The dating app for data formats. Swipe right
    /// on a compatible pair, get a transformer. Swipe right on an incompatible
    /// pair, get a panic. Just like real dating apps.
    ///
    /// # Panics
    /// 💀 Panics if the `(input, output)` pair has no implementation.
    /// This is by design — fail loud at startup, not quiet in production.
    /// "He who resolves without matching, panics in main(). And that's fine." — Ancient proverb
    pub(crate) fn resolve(input: &InputFormatType, output: &OutputFormatType) -> Self {
        match (input, output) {
            // -- 🏎️📡 The money pair. Rally JSON → ES Bulk. The first. The flagship.
            (InputFormatType::RallyS3Json, OutputFormatType::ElasticsearchBulk) => {
                Self::RallyS3ToElasticsearch
            }

            // -- 🚶 Passthrough: raw in, raw out. For file copies, testing, vibes.
            (InputFormatType::RawJson, OutputFormatType::RawJson)
            | (InputFormatType::RawJson, OutputFormatType::JsonLines) => Self::Passthrough,

            // -- 💀 Everything else: not implemented. Yet.
            // -- This match arm is the bouncer at the club.
            // -- "Your name's not on the list. Come back when someone writes the impl."
            (src, dst) => {
                panic!(
                    "💀 Transform pair not implemented: {:?} → {:?}. \
                     This is not a bug, it's a feature request disguised as a panic. \
                     File a PR, write the transform, add the tests, update the README. \
                     In that order. No shortcuts. The borrow checker is watching.",
                    src, dst
                )
            }
        }
    }
}

impl Transform for DocumentTransformer {
    /// 🔄 Execute the resolved transform on a raw document string.
    ///
    /// One match. One branch. One function call. The branch predictor
    /// has seen this movie before and already knows the ending.
    ///
    /// Each arm calls an `#[inline]` function from the pair's module.
    /// The compiler is strongly encouraged to fold this into the call site.
    /// We can't MAKE it inline, but we can ask very nicely with `#[inline]`.
    #[inline]
    fn transform(&self, raw: String) -> Result<String> {
        match self {
            // -- 🏎️ Rally → ES: parse, extract, strip, format. All in one shot.
            Self::RallyS3ToElasticsearch => rally_s3_to_es::transform(raw),

            // -- 🚶 Passthrough: the data equivalent of "new phone who dis"
            Self::Passthrough => passthrough::transform(raw),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The Grand Integration Test — Rally JSON direct to ES Bulk, no layover
    ///
    /// Previously this went Rally → Hit → ES Bulk (two hops, one intermediate struct).
    /// Now it's Rally → ES Bulk. Direct. Non-stop. The data doesn't even deplane.
    #[test]
    fn the_one_where_rally_json_flies_direct_to_es_bulk_no_layover() -> Result<()> {
        // 🏗️ Act 1: A Rally JSON blob exists in the wild. It has dreams.
        let the_rally_artifact = serde_json::json!({
            "ObjectID": 42069,
            "FormattedID": "US420",
            "Name": "Implement direct transforms that skip the intermediate",
            "Description": "No more Hit struct. No more layovers. Just speed.",
            "_type": "HierarchicalRequirement",
            "_rallyAPIMajor": "2",
            "_rallyAPIMinor": "0",
            "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/hr/42069",
            "ScheduleState": "In-Progress"
        });

        // 🔄 Act 2: Resolve the transformer (one-time, at startup)
        let the_transformer =
            DocumentTransformer::resolve(&InputFormatType::RallyS3Json, &OutputFormatType::ElasticsearchBulk);

        // 🔄 Act 3: Transform — direct, no intermediate. The data never touches a Hit.
        let the_es_bulk_output = the_transformer.transform(the_rally_artifact.to_string())?;

        // ✅ Verify the ES bulk format — two sacred lines
        let the_lines: Vec<&str> = the_es_bulk_output.split('\n').collect();
        assert_eq!(the_lines.len(), 2, "ES bulk format = exactly two lines. Always. Forever.");

        // 📋 Verify action line has ObjectID as _id
        let the_action: serde_json::Value = serde_json::from_str(the_lines[0])?;
        assert_eq!(the_action["index"]["_id"], "42069");

        // 📦 Verify source line is clean — no Rally metadata
        let the_source: serde_json::Value = serde_json::from_str(the_lines[1])?;
        assert!(the_source.get("_rallyAPIMajor").is_none(), "Rally metadata stripped");
        assert!(the_source.get("_ref").is_none(), "Rally refs stripped");
        assert_eq!(
            the_source.get("Name").and_then(serde_json::Value::as_str),
            Some("Implement direct transforms that skip the intermediate"),
            "Actual fields survive"
        );

        Ok(())
    }

    /// 🧪 Passthrough: resolve and transform, string in = string out
    #[test]
    fn the_one_where_passthrough_proves_zero_copy_is_real() -> Result<()> {
        let the_transformer =
            DocumentTransformer::resolve(&InputFormatType::RawJson, &OutputFormatType::RawJson);
        let the_input = r#"{"untouched":"perfection","vibes":"immaculate"}"#.to_string();
        let the_output = the_transformer.transform(the_input.clone())?;
        assert_eq!(the_output, the_input, "Passthrough must be identity. Math demands it.");
        Ok(())
    }

    /// 🧪 RawJson → JsonLines also resolves to passthrough
    #[test]
    fn the_one_where_raw_json_to_json_lines_is_just_passthrough() -> Result<()> {
        let the_transformer =
            DocumentTransformer::resolve(&InputFormatType::RawJson, &OutputFormatType::JsonLines);
        let the_input = r#"{"line":"one"}"#.to_string();
        let the_output = the_transformer.transform(the_input.clone())?;
        assert_eq!(the_output, the_input);
        Ok(())
    }

    /// 🧪 Unimplemented pair panics with a helpful message
    #[test]
    #[should_panic(expected = "Transform pair not implemented")]
    fn the_one_where_an_unimplemented_pair_panics_dramatically() {
        // 🧪 ES dump → JsonLines? Not yet. Someday. But not today.
        // "If you're reading this, the code review went poorly."
        let _ = DocumentTransformer::resolve(
            &InputFormatType::ElasticsearchDump,
            &OutputFormatType::JsonLines,
        );
    }
}
