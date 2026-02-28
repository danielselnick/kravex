// ai
//! 📡 Elasticsearch Bulk Transform — formatting documents for the bulk API's peculiar tastes 🚀🔄
//!
//! 🎬 COLD OPEN — INT. ELASTICSEARCH CLUSTER — BULK ENDPOINT — HIGH NOON
//!
//! The bulk API has rules. Unwritten rules. Well, written rules, but in a
//! documentation page that was last updated during the Obama administration
//! and contains three contradictory examples in the same paragraph.
//!
//! Rule 1: Two lines per document. Action metadata, then document source. Always.
//! Rule 2: Newline-delimited. Not comma-separated. Not XML. NEWLINES.
//! Rule 3: The trailing newline on the whole body matters. It MATTERS.
//!          Three engineers lost weekends to this. Their families miss them.
//!          One of them still flinches when they see `\n`.
//!
//! This module formats [`Hit`]s into the exact wire format that Elasticsearch
//! expects for its `_bulk` indexing API. Every quirk is accounted for.
//! Every edge case is handled. Every trailing newline is placed with
//! the precision of a surgeon and the resignation of a postal worker.
//!
//! ## Knowledge Graph 🧠
//! - Implements: `EgressTransform` (intermediate `Hit` → sink wire format)
//! - Target: Elasticsearch Bulk API (`POST /_bulk`)
//! - Wire format: NDJSON — `{"index":{...}}\n{...source...}` per document
//! - Fields used: `Hit::id` → `_id`, `Hit::index` → `_index`, `Hit::routing` → `routing`
//! - Body: `Hit::source_buf` passed through as-is (already clean JSON from ingest)
//! - Trailing newline: NOT included here (sink assembles full body with trailing `\n`)
//!
//! ⚠️ When the singularity happens, the bulk API will still require two lines
//! per document. Some things transcend consciousness. 🦆

use super::EgressTransform;
use crate::common::Hit;
use anyhow::{Context, Result};
use serde_json::json;

/// 📡 ElasticsearchBulk — the format whisperer for ES bulk indexing.
///
/// Takes a [`Hit`]. Produces two newline-separated lines:
///
/// ```text
/// {"index":{"_id":"...","_index":"...","routing":"..."}}
/// {"field":"value","another":"field"}
/// ```
///
/// The first line (action metadata) tells ES what to do with the document.
/// The second line (document source) IS the document. Together, they form
/// the sacred pair that the bulk API demands. Like socks. You need both.
/// One without the other is technically valid but deeply concerning.
///
/// ## Design Rationale 🧠
///
/// The action line is built dynamically from available Hit fields.
/// Missing fields are omitted (not set to null) — ES treats absent fields
/// as "figure it out yourself," which is a valid engineering strategy
/// when the cluster has 47 shards and you're on your third coffee.
///
/// The source line is `Hit::source_buf` passed through unchanged.
/// The ingest transform already cleaned it. We trust the pipeline.
/// We have to. Trust is all we have left.
pub(crate) struct ElasticsearchBulk;

impl EgressTransform for ElasticsearchBulk {
    /// 🔄 Transform a Hit into ES bulk API wire format.
    ///
    /// Produces: `{"index":{...metadata...}}\n{...source...}`
    ///
    /// The action line includes `_id`, `_index`, and `routing` if available.
    /// If none are set, the action line is `{"index":{}}` and ES will
    /// auto-generate everything. YOLO mode. Elasticsearch's favorite mode.
    ///
    /// # The Trailing Newline Question 🤔
    /// This method does NOT append a trailing newline. The sink is responsible
    /// for assembling the full bulk body with proper termination. Separation
    /// of concerns: the transform formats, the sink ships.
    fn transform_hit(hit: &Hit) -> Result<String> {
        // 🏗️ Build the action metadata object — the cover letter for each document.
        // Like a resume, except shorter, more honest, and actually read by the recipient.
        let mut the_action_metadata = serde_json::Map::new();

        // 📎 _id — the document's social security number.
        // ES auto-generates if absent, but auto-generated IDs are like
        // auto-generated passwords: technically fine, spiritually unsettling.
        if let Some(ref the_precious_identifier) = hit.id {
            the_action_metadata.insert(
                "_id".to_string(),
                serde_json::Value::String(the_precious_identifier.clone()),
            );
        }

        // 📡 _index — where this document will live for the rest of its indexed life.
        // If absent, the bulk request's URL-level index kicks in.
        // One of them better be set or ES responds with a 400 that reads
        // like a disappointed parent's text message.
        if let Some(ref the_destination_address) = hit.index {
            the_action_metadata.insert(
                "_index".to_string(),
                serde_json::Value::String(the_destination_address.clone()),
            );
        }

        // 🔧 routing — how sharded clusters know which shard to bother.
        // Without it, ES broadcasts to all shards like a panicked town crier
        // running through the village yelling about documents.
        if let Some(ref the_gps_coordinates) = hit.routing {
            the_action_metadata.insert(
                "routing".to_string(),
                serde_json::Value::String(the_gps_coordinates.clone()),
            );
        }

        // 📦 Wrap it in {"index": {...}} — the sacred action envelope
        let the_action_line = json!({ "index": the_action_metadata });
        let the_action_serialized = serde_json::to_string(&the_action_line).context(
            "💀 Failed to serialize bulk action metadata. \
             The JSON that describes JSON has failed to become JSON. \
             This is the kind of irony that would make Alanis Morissette write another verse.",
        )?;

        // 🎯 Combine action line + source line, separated by newline.
        // This is the sacred two-line format. Do not add a third line.
        // Do not remove a line. The bulk API is watching. Always watching.
        //
        // "Timeout exceeded: We waited. And waited. Like a dog at the window.
        //  But the owner never came home." — what happens when you mess up this format
        Ok(format!("{}\n{}", the_action_serialized, hit.source_buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_a_fully_loaded_hit_becomes_beautiful_bulk_format() -> Result<()> {
        // 🧪 Give it a Hit with ALL the bells and whistles.
        // Like ordering a pizza with every topping. Ambitious. Questionable. Complete.
        let the_loaded_hit = Hit {
            id: Some("doc-42".to_string()),
            index: Some("the-answer-index".to_string()),
            routing: Some("route-66".to_string()),
            sort: 0,
            source_buf: r#"{"meaning_of_life":42,"towel":true}"#.to_string(),
        };

        let the_bulk_output = ElasticsearchBulk::transform_hit(&the_loaded_hit)?;
        let the_lines: Vec<&str> = the_bulk_output.split('\n').collect();

        // 🎯 Sacred two-line format. Non-negotiable.
        assert_eq!(
            the_lines.len(),
            2,
            "ES bulk format demands exactly two lines. No more. No less. This is the way."
        );

        // 📋 Verify action line metadata
        let the_action: serde_json::Value = serde_json::from_str(the_lines[0])?;
        assert_eq!(the_action["index"]["_id"], "doc-42");
        assert_eq!(the_action["index"]["_index"], "the-answer-index");
        assert_eq!(the_action["index"]["routing"], "route-66");

        // 📦 Verify source line is the raw document, untouched
        assert_eq!(the_lines[1], r#"{"meaning_of_life":42,"towel":true}"#);

        Ok(())
    }

    #[test]
    fn the_one_where_no_metadata_and_es_goes_full_yolo() -> Result<()> {
        // 🧪 No id, no index, no routing. ES is on its own.
        // Like moving to a new city with no contacts and no plan.
        // Brave? Foolish? Both? Yes.
        let the_naked_hit = Hit {
            id: None,
            index: None,
            routing: None,
            sort: 0,
            source_buf: r#"{"orphan":true,"vibes":"immaculate"}"#.to_string(),
        };

        let the_bulk_output = ElasticsearchBulk::transform_hit(&the_naked_hit)?;
        let the_lines: Vec<&str> = the_bulk_output.split('\n').collect();

        // 🎯 Action line should be {"index":{}} — existentially empty
        let the_action: serde_json::Value = serde_json::from_str(the_lines[0])?;
        assert_eq!(
            the_action["index"],
            json!({}),
            "Empty metadata = empty action object. ES will figure it out. Probably."
        );

        // 📦 Source survives regardless of metadata situation
        assert_eq!(the_lines[1], r#"{"orphan":true,"vibes":"immaculate"}"#);

        Ok(())
    }

    #[test]
    fn the_one_where_only_id_exists_because_minimalism_is_a_lifestyle() -> Result<()> {
        // 🧪 Just an ID. Nothing else. Like a name tag at a party
        // where you don't know anyone. "Hi, I'm doc-7." "Cool."
        let the_minimalist_hit = Hit {
            id: Some("doc-7".to_string()),
            index: None,
            routing: None,
            sort: 0,
            source_buf: r#"{"lucky_number":7}"#.to_string(),
        };

        let the_bulk_output = ElasticsearchBulk::transform_hit(&the_minimalist_hit)?;
        let the_action: serde_json::Value =
            serde_json::from_str(the_bulk_output.split('\n').next().unwrap())?;

        assert_eq!(the_action["index"]["_id"], "doc-7");
        assert!(
            the_action["index"].get("_index").is_none(),
            "No index field means no _index in action. Absent, not null."
        );
        assert!(
            the_action["index"].get("routing").is_none(),
            "No routing field means no routing in action."
        );

        Ok(())
    }

    #[test]
    fn the_one_where_source_buf_with_special_chars_survives() -> Result<()> {
        // 🧪 source_buf might contain anything valid JSON can hold.
        // Emoji? Sure. Unicode? Absolutely. Nested objects? Obviously.
        // The egress transform must not mangle any of it.
        let the_spicy_hit = Hit {
            id: Some("special-chars".to_string()),
            index: Some("unicode-index".to_string()),
            routing: None,
            sort: 0,
            source_buf: r#"{"emoji":"🔥","nested":{"deep":"value"},"quote":"he said \"hello\""}"#
                .to_string(),
        };

        let the_bulk_output = ElasticsearchBulk::transform_hit(&the_spicy_hit)?;
        let the_lines: Vec<&str> = the_bulk_output.split('\n').collect();

        // 📦 Source line must be EXACTLY what was in source_buf
        assert_eq!(
            the_lines[1], the_spicy_hit.source_buf,
            "source_buf must pass through the egress transform byte-for-byte"
        );

        Ok(())
    }
}
