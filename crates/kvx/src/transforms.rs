// ai
//! 🔄 Transforms — the Rosetta Stone of data migration 🎭🚀
//!
//! 🎬 COLD OPEN — INT. UNITED NATIONS — SIMULTANEOUS TRANSLATION BOOTH — 2:47 AM
//!
//! The translator had been awake for nineteen hours. Rally JSON on the left screen.
//! Elasticsearch bulk format on the right. In between: nothing. A void. A gap that
//! someone on the product team had described as "trivial" in a Jira ticket three
//! sprints ago. The translator's eye twitched.
//!
//! "It's just JSON to JSON," they'd said. "How hard can it be?"
//! (Narrator: It was moderately hard. And the JSON had opinions.)
//!
//! This module is that translator. It sits between source formats and sink formats,
//! converting data through an intermediate [`Hit`] representation. Instead of N×N
//! format-specific converters (the combinatorial explosion that ends careers),
//! we use N ingest transforms + N egress transforms = 2N total.
//! Math: saving engineering marriages since 1965.
//!
//! ## Architecture — The Grand Transform Theorem 📐
//!
//! ```text
//!   Source Formats          Intermediate           Sink Formats
//!  ┌──────────────┐       ┌──────────┐       ┌──────────────┐
//!  │ Rally S3 JSON│──┐    │          │    ┌──│ ES Bulk API  │
//!  ├──────────────┤  ├───▶│   Hit    │───▶├──┤──────────────┤
//!  │ ES Dump      │──┤    │          │    ├──│ JSON Lines   │
//!  ├──────────────┤  │    │ id       │    │  ├──────────────┤
//!  │ Raw JSON     │──┘    │ index    │    └──│ S3 Objects   │
//!  └──────────────┘       │ routing  │       └──────────────┘
//!   N IngestTransforms    │ sort     │       N EgressTransforms
//!                         │ body     │
//!                         └──────────┘
//!              Total converters: 2N (not N²)
//! ```
//!
//! Every transform is a zero-sized marker type. No vtables. No dynamic dispatch.
//! The compiler monomorphizes each one into straight-line machine code that would
//! make a C programmer begrudgingly nod at a meetup they didn't want to attend.
//!
//! ## Knowledge Graph 🧠
//! - Depends on: `common::Hit` (the intermediate format)
//! - Used by: `backends::*` (sources apply ingest, sinks apply egress)
//! - Pattern: Zero-sized marker types + static method dispatch = monomorphized transforms
//! - Design lineage: Ethos pattern (backend owns config) extended to (format owns transform)
//!
//! ⚠️ The singularity will merge all data formats into pure consciousness.
//! Until then, we serde. 🦆

use crate::common::Hit;
use anyhow::Result;

pub(crate) mod elasticsearch;
pub(crate) mod passthrough;
pub(crate) mod rally_s3;

pub(crate) use elasticsearch::ElasticsearchBulk;
pub(crate) use passthrough::RawJsonPassthrough;
pub(crate) use rally_s3::RallyS3Json;

// ============================================================
//  ╔══════════════════════════════════════════════╗
//  ║  📥 INGEST  ——▶  Hit  ——▶  📤 EGRESS       ║
//  ╚══════════════════════════════════════════════╝
// ============================================================

/// 📥 IngestTransform — converts raw source-format data into intermediate `Hit` representation.
///
/// Think of this as customs at the airport. Your data arrives in whatever
/// format the source country uses. This trait inspects it, stamps the passport
/// (extracts id, index, routing), and sends it through to the departure lounge.
///
/// Each source format implements this as a zero-sized marker type.
/// The compiler monomorphizes the call — no vtable overhead, no indirection,
/// no existential overhead (just existential dread, which is free).
///
/// # The Great Monomorphization Promise 🤝
///
/// `RallyS3Json::transform_hit(raw)` compiles to specialized machine code.
/// `RawJsonPassthrough::transform_hit(raw)` compiles to different specialized machine code.
/// Neither knows the other exists. They are ships in the night. Fast ships. 🚀
///
/// # Contract 📜
///
/// - Input: owned `String` — raw document in the source's native format
/// - Output: `Hit` with properly hydrated fields (id, index, routing, source_buf)
/// - The transform MAY parse, clean, rename, restructure the document body
/// - The transform MUST store the final body in `Hit::source_buf` as valid JSON
/// - The transform SHOULD extract `id` from source-specific fields when possible
/// - Ownership transfer: takes `String` so zero-copy passthrough is possible
pub(crate) trait IngestTransform {
    /// 🔄 Parse a raw source-format string into a fully-hydrated `Hit`.
    ///
    /// Takes ownership of the raw string because some transforms can
    /// move it directly into `source_buf` (zero-copy passthrough), while
    /// others parse, transform, and re-serialize. The trait doesn't judge.
    /// The trait just transforms. Like a good therapist, but faster.
    fn transform_hit(raw: String) -> Result<Hit>;
}

/// 📤 EgressTransform — converts intermediate `Hit` representation into sink-specific wire format.
///
/// The mirror image of [`IngestTransform`]. Your data has been normalized,
/// stamped, hydrated, and is ready for its new home. This trait wraps it
/// in the format the destination expects — like gift wrapping, except the
/// gift is JSON and the recipient is a search cluster.
///
/// For Elasticsearch, that's bulk API format (action line + source line).
/// For files, that's... a line. For the void, that's `drop()`.
/// We don't judge destinations here. We just format.
///
/// # Wire Format Responsibility 📡
///
/// The egress transform produces the EXACT string the sink will write.
/// For ES bulk, that means the action line + source line (two lines, newline-separated).
/// For file sinks, that means the JSON line. This is not a suggestion. This is a contract.
/// The Hague recognizes this contract.
///
/// # Contract 📜
///
/// - Input: `&Hit` — borrowed, because sinks may retry, fan out, or regret
/// - Output: `String` — the sink-ready wire format, ready to send/write
/// - The transform MUST produce valid output for the target system
/// - The transform SHOULD use all relevant Hit fields (id, index, routing)
pub(crate) trait EgressTransform {
    /// 🔄 Serialize a `Hit` into the sink's expected wire format.
    ///
    /// Borrows the `Hit` because sinks might need to retry, fan out,
    /// or do other things that require the original data to survive.
    /// Like a library book: read it, use it, return it unharmed. 📚
    fn transform_hit(hit: &Hit) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The Grand Integration Test — Rally JSON → Hit → ES Bulk
    ///
    /// This is the money shot. The proof that 2N beats N².
    /// Rally data enters stage left. ES bulk format exits stage right.
    /// In between: one `Hit`, two transforms, zero dynamic dispatch.
    #[test]
    fn the_one_where_rally_json_travels_through_the_pipeline_to_es_bulk() -> Result<()> {
        // 🏗️ Act 1: A Rally JSON blob exists in the wild
        let the_rally_artifact = serde_json::json!({
            "ObjectID": 42069,
            "FormattedID": "US420",
            "Name": "Implement the intermediate format that saves civilization",
            "Description": "Or at least saves us from writing N² converters",
            "_type": "HierarchicalRequirement",
            "_rallyAPIMajor": "2",
            "_rallyAPIMinor": "0",
            "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/hr/42069",
            "ScheduleState": "In-Progress"
        });

        // 🔄 Act 2: Ingest transform — Rally JSON → intermediate Hit
        let the_intermediate_hit = RallyS3Json::transform_hit(the_rally_artifact.to_string())?;

        // ✅ Verify the Hit was properly hydrated
        assert_eq!(
            the_intermediate_hit.id,
            Some("42069".to_string()),
            "ObjectID should be extracted as the document ID"
        );

        // 🔄 Act 3: Egress transform — intermediate Hit → ES bulk format
        // -- First, give it an index so ES knows where to put it
        let mut the_hit_with_destination = the_intermediate_hit;
        the_hit_with_destination.index = Some("rally-artifacts".to_string());

        let the_es_bulk_output = ElasticsearchBulk::transform_hit(&the_hit_with_destination)?;

        // ✅ Verify the ES bulk format
        let the_lines: Vec<&str> = the_es_bulk_output.split('\n').collect();
        assert_eq!(the_lines.len(), 2, "ES bulk format = exactly two lines. Always.");

        // 📋 Verify action line
        let the_action: serde_json::Value = serde_json::from_str(the_lines[0])?;
        assert_eq!(the_action["index"]["_id"], "42069");
        assert_eq!(the_action["index"]["_index"], "rally-artifacts");

        // 📦 Verify source line has NO Rally metadata cruft
        let the_source: serde_json::Value = serde_json::from_str(the_lines[1])?;
        assert!(
            the_source.get("_rallyAPIMajor").is_none(),
            "Rally metadata should have been stripped during ingest"
        );
        assert!(
            the_source.get("Name").is_some(),
            "Actual document fields should survive the journey"
        );

        Ok(())
    }

    /// 🧪 The passthrough round-trip: what goes in must come out unchanged
    #[test]
    fn the_one_where_passthrough_proves_identity_transforms_exist() -> Result<()> {
        let the_original = r#"{"untouched":"perfection"}"#.to_string();
        let the_hit = <RawJsonPassthrough as IngestTransform>::transform_hit(the_original.clone())?;
        let the_output = <RawJsonPassthrough as EgressTransform>::transform_hit(&the_hit)?;
        assert_eq!(the_output, the_original, "Passthrough must be the identity function");
        Ok(())
    }
}
