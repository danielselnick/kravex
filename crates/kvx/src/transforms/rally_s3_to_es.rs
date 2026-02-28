// ai
//! 🏎️📡 Rally S3 JSON → Elasticsearch Bulk — the direct flight, no layover 🔄✈️
//!
//! 🎬 COLD OPEN — INT. AIRPORT TERMINAL — GATE CHANGE ANNOUNCEMENT
//!
//! "Attention passengers on Flight RALLY-TO-ES: your intermediate layover through
//! Hit International Airport has been cancelled. You will now fly direct.
//! Please proceed directly to the Elasticsearch gate. Do not collect a Hit struct.
//! Do not pass through an intermediate format. Do not allocate $200."
//!
//! The crowd cheers. The data cheers louder. Somewhere, a Hit struct weeps softly,
//! packing its `source_buf` into a box labeled "memories." It had a good run.
//! Four fields. Zero purpose. A life well-lived? Debatable.
//!
//! This module combines what used to be TWO separate transforms (ingest + egress)
//! into ONE direct conversion function. Rally JSON goes in. ES bulk NDJSON comes out.
//! No intermediate representation. No Hit struct. No existential middle ground.
//!
//! ## What This Does (All In One Function) 🔧
//!
//! 1. **Parse** Rally JSON blob
//! 2. **Extract** `ObjectID` → ES `_id` (stringified)
//! 3. **Strip** Rally API metadata cruft (6 fields, see `THE_METADATA_FIELDS_WE_DONT_NEED`)
//! 4. **Format** directly as ES bulk NDJSON: `{"index":{"_id":"..."}}\n{...cleaned body...}`
//!
//! Previously this was: parse → create Hit → read Hit → format → output.
//! Now it's: parse → format → output. Two fewer allocations. One fewer existential crisis.
//!
//! ## Knowledge Graph 🧠
//! - Pair: `InputFormatType::RallyS3Json` → `OutputFormatType::ElasticsearchBulk`
//! - Resolved via: `DocumentTransformer::RallyS3ToElasticsearch`
//! - Rally fields extracted: `ObjectID` → `_id`
//! - Rally fields stripped: `_rallyAPIMajor/Minor`, `_ref`, `_refObjectUUID`, `_objectVersion`, `_CreatedAt`
//! - Output: two-line NDJSON (`{"index":{...}}\n{...source...}`)
//! - Index: NOT embedded in action line (sink sets via URL or config)
//! - Routing: NOT set (Rally doesn't do routing)
//! - Trailing newline: NOT appended (sink assembles the bulk body)
//!
//! ⚠️ Rally was acquired by CA was acquired by Broadcom. The JSON outlived them all.
//! When the singularity arrives, it will find this function still parsing ObjectIDs. 🦆

use anyhow::{Context, Result};
use serde_json::Value;

/// 🗑️ Rally metadata fields that get stripped during transform.
///
/// Tribal knowledge from the Rally REST API: every object is wrapped with these
/// API-level metadata fields. They're useful for the Rally client SDK. They're
/// useless for a search index. They're like those "do not remove" tags on
/// mattresses: technically there for a reason, absolutely removed by everyone.
///
/// We strip at the top level only — nested objects (Project._ref, Iteration._ref)
/// survive because they're part of the document's relational structure.
const THE_METADATA_FIELDS_WE_DONT_NEED: &[&str] = &[
    "_rallyAPIMajor",
    "_rallyAPIMinor",
    "_ref",
    "_refObjectUUID",
    "_objectVersion",
    "_CreatedAt",
];

/// 🏎️📡 Transform a Rally S3 JSON blob directly into ES bulk NDJSON.
///
/// This is the direct flight. Rally JSON enters the function. ES bulk format
/// exits the function. Nothing in between. No structs harmed. No allocations
/// wasted. Just parse, extract, strip, format, done.
///
/// # Returns
/// Two newline-separated lines:
/// ```text
/// {"index":{"_id":"<ObjectID>"}}
/// {"Name":"...","Description":"...",...}
/// ```
///
/// If no `ObjectID` is found, the action line omits `_id` and ES auto-generates.
/// If the JSON is invalid, we return an error. If the JSON is valid but weird,
/// we strip what we can and carry on. This is a migration tool, not a therapist.
///
/// # Errors
/// 💀 Returns error if `raw` is not valid JSON. Everything else is handled gracefully,
/// because at 3am during a migration, "gracefully" means "doesn't crash."
#[inline]
pub(crate) fn transform(raw: String) -> Result<String> {
    // 🔬 Phase 1: Parse the Rally JSON blob.
    // If this fails, the S3 object was a lie. Not the first time S3 has betrayed us.
    let mut doc: Value = serde_json::from_str(&raw).context(
        "💀 Rally JSON parse failed — the blob from S3 is not valid JSON. \
         Either the export is corrupted, someone uploaded a JPEG to the JSON bucket, \
         or Mercury is in retrograde. Check the source data and try again.",
    )?;

    // 🎯 Phase 2: Extract ObjectID → _id for the ES action line.
    // Rally can't decide if ObjectID is a number or a string.
    // We handle both because we're professionals. (Barely.)
    let the_document_identity = doc.get("ObjectID").map(|oid| match oid {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        // 🦆 Bool? Array? Null? The heat death of the universe?
        // Stringify it and move on. We've seen worse.
        honestly_who_knows => honestly_who_knows.to_string(),
    });

    // 🗑️ Phase 3: Strip Rally API metadata from the document body.
    // These are the metadata barnacles that Rally attaches to every object.
    // Useful for API pagination. Useless for search. Gone in O(6) map removes.
    if let Value::Object(ref mut map) = doc {
        for doomed_field in THE_METADATA_FIELDS_WE_DONT_NEED {
            map.remove(*doomed_field);
        }
    }

    // 📦 Phase 4: Re-serialize the cleaned document body.
    let the_cleaned_body = serde_json::to_string(&doc).context(
        "💀 Failed to re-serialize cleaned Rally JSON. The JSON went in valid \
         and came out... not. This is thermodynamically unlikely. File a bug.",
    )?;

    // 📡 Phase 5: Build the ES bulk action line.
    // {"index":{"_id":"..."}} — or just {"index":{}} if no ObjectID
    let the_action_line = build_es_action_line(the_document_identity.as_deref());

    // 🎯 Phase 6: Combine action + source into the sacred two-line format.
    // No trailing newline. The sink handles bulk body assembly.
    // "Two lines walk into a bar. The bartender says, 'Is this a bulk request?'"
    Ok(format!("{}\n{}", the_action_line, the_cleaned_body))
}

/// 🏗️ Build an ES bulk action line from optional metadata.
///
/// Produces `{"index":{"_id":"..."}}` if id is Some,
/// or `{"index":{}}` if id is None.
///
/// We don't include `_index` here — that's the sink's responsibility
/// (set via bulk URL path or per-document in the action line by a
/// future config layer). We also don't include `routing` because
/// Rally doesn't do routing. Rally does what Rally wants.
///
/// # Why a separate function? 🤔
/// Because ES bulk action line formatting will be reused by other
/// `*_to_es.rs` pair transforms. DRY, but only when it makes sense.
/// This is one of those times. "Ancient proverb: extract a helper
/// only when you've copied it twice. We extracted it preemptively
/// because we can see the future. The future has more ES transforms."
#[inline]
fn build_es_action_line(the_document_id: Option<&str>) -> String {
    // -- 📋 Build the action metadata map. Start empty. Add what we have.
    // -- Like packing for a trip: start with an empty suitcase, add what you need.
    // -- Unlike packing, we don't bring 7 shirts for a 3-day trip.
    match the_document_id {
        Some(id) => {
            // -- 📎 We have an ID. Include it. ES will use it.
            // -- The document has a name. It belongs somewhere. It matters.
            format!(r#"{{"index":{{"_id":"{}"}}}}"#, escape_json_string(id))
        }
        None => {
            // -- 💀 No ID. ES will auto-generate one. Like a hospital bracelet
            // -- for a patient who came in unconscious. Not ideal. Functional.
            r#"{"index":{}}"#.to_string()
        }
    }
}

/// 🔧 Escape a string for safe JSON embedding.
///
/// Handles the usual suspects: backslash, double quotes, control characters.
/// This exists so we can build the action line without a full serde round-trip
/// for the trivial `{"index":{"_id":"..."}}` case. Micro-optimization?
/// Maybe. But in a hot loop processing millions of documents, every
/// serde_json::to_string we avoid is a tiny victory.
///
/// "Knock knock." "Who's there?" "Backslash-n." "Backslash-n who?"
/// "Backslash-n-ewline. Get it? ...I'll see myself out."
#[inline]
fn escape_json_string(s: &str) -> String {
    // -- 🔍 Fast path: if no special chars, return as-is.
    // -- Most document IDs are alphanumeric. This check avoids allocation.
    if s.bytes()
        .all(|b| b != b'"' && b != b'\\' && b >= 0x20)
    {
        return s.to_string();
    }

    // -- 🐢 Slow path: escape special characters one by one.
    // -- This is the penalty box for IDs that contain weird characters.
    let mut escaped = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // -- 🎭 Control characters get the \uXXXX treatment.
                // -- If your document ID contains control characters, we need to talk.
                escaped.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_rally_json_becomes_es_bulk_in_one_shot() -> Result<()> {
        // 🧪 The headline act. Rally JSON → ES Bulk. Direct. No intermediate.
        // Previously: Rally → Hit → ES Bulk (two hops). Now: one function call.
        // The data never touches a Hit struct. It flies direct.
        let rally_blob = serde_json::json!({
            "ObjectID": 12345,
            "FormattedID": "US789",
            "Name": "As a user, I want to migrate data without crying",
            "Description": "Acceptance criteria: fewer tears than last sprint",
            "_type": "HierarchicalRequirement",
            "_rallyAPIMajor": "2",
            "_rallyAPIMinor": "0",
            "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/hierarchicalrequirement/12345",
            "_refObjectUUID": "abc-123-def-456",
            "_objectVersion": "7",
            "_CreatedAt": "2023-01-15T10:00:00.000Z",
            "ScheduleState": "Accepted"
        });

        let the_output = transform(rally_blob.to_string())?;
        let the_lines: Vec<&str> = the_output.split('\n').collect();

        // 🎯 Two sacred lines. Always.
        assert_eq!(the_lines.len(), 2, "ES bulk = two lines, no exceptions");

        // 📋 Action line: {"index":{"_id":"12345"}}
        let the_action: serde_json::Value = serde_json::from_str(the_lines[0])?;
        assert_eq!(the_action["index"]["_id"], "12345");

        // 📦 Source line: cleaned Rally JSON, no metadata cruft
        let the_source: serde_json::Value = serde_json::from_str(the_lines[1])?;
        assert!(the_source.get("_rallyAPIMajor").is_none(), "_rallyAPIMajor stripped");
        assert!(the_source.get("_rallyAPIMinor").is_none(), "_rallyAPIMinor stripped");
        assert!(the_source.get("_ref").is_none(), "_ref stripped");
        assert!(the_source.get("_refObjectUUID").is_none(), "_refObjectUUID stripped");
        assert!(the_source.get("_objectVersion").is_none(), "_objectVersion stripped");
        assert!(the_source.get("_CreatedAt").is_none(), "_CreatedAt stripped");

        // ✅ Real fields survive
        assert_eq!(the_source["Name"], "As a user, I want to migrate data without crying");
        assert_eq!(the_source["FormattedID"], "US789");
        assert_eq!(the_source["ScheduleState"], "Accepted");
        assert_eq!(the_source["_type"], "HierarchicalRequirement");

        Ok(())
    }

    #[test]
    fn the_one_where_string_object_id_works_because_rally_is_inconsistent() -> Result<()> {
        // 🧪 Rally: "ObjectID is a number." Also Rally: "...unless it's a string."
        // Us: "Fine. We handle both. Like responsible adults."
        let rally_blob = serde_json::json!({
            "ObjectID": "67890",
            "Name": "String ObjectID test"
        });

        let the_output = transform(rally_blob.to_string())?;
        let the_action: serde_json::Value =
            serde_json::from_str(the_output.split('\n').next().unwrap())?;
        assert_eq!(the_action["index"]["_id"], "67890");
        Ok(())
    }

    #[test]
    fn the_one_where_no_object_id_produces_empty_action_metadata() -> Result<()> {
        // 🧪 No ObjectID? No _id in the action line. ES auto-generates.
        // The document is born without a name. Like a stray cat. ES will name it.
        let rally_blob = serde_json::json!({
            "Name": "Nameless wanderer",
            "Description": "Found in the back of the S3 bucket"
        });

        let the_output = transform(rally_blob.to_string())?;
        let the_action: serde_json::Value =
            serde_json::from_str(the_output.split('\n').next().unwrap())?;
        assert_eq!(the_action["index"], serde_json::json!({}));
        Ok(())
    }

    #[test]
    fn the_one_where_invalid_json_returns_error_not_miracle() {
        // 🧪 Not JSON? Error. Not "try harder." Not "maybe if we squint."
        // Error. The function has standards.
        let not_json = "this is a cry for help, not JSON".to_string();
        assert!(transform(not_json).is_err());
    }

    #[test]
    fn the_one_where_nested_rally_refs_survive_the_top_level_purge() -> Result<()> {
        // 🧪 Top-level metadata gets stripped. Nested refs survive.
        // Because Project._ref is relational data, not API cruft.
        let nested_blob = serde_json::json!({
            "ObjectID": 11111,
            "_rallyAPIMajor": "2",
            "Project": {
                "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/project/222",
                "Name": "Project Alpha"
            }
        });

        let the_output = transform(nested_blob.to_string())?;
        let the_source: serde_json::Value =
            serde_json::from_str(the_output.split('\n').nth(1).unwrap())?;

        assert!(the_source.get("_rallyAPIMajor").is_none(), "Top-level stripped");
        assert!(the_source["Project"]["_ref"].is_string(), "Nested _ref survives");
        Ok(())
    }

    #[test]
    fn the_one_where_metadata_stripping_is_a_noop_on_clean_docs() -> Result<()> {
        // 🧪 Already clean? Cool. We strip nothing. No-op. Fast.
        let clean_blob = serde_json::json!({
            "ObjectID": 99999,
            "Name": "Born clean, die clean",
            "CustomField_c": "custom fields are the cockroaches of JSON: they survive everything"
        });

        let the_output = transform(clean_blob.to_string())?;
        let the_source: serde_json::Value =
            serde_json::from_str(the_output.split('\n').nth(1).unwrap())?;
        assert_eq!(the_source["Name"], "Born clean, die clean");
        assert!(the_source.get("CustomField_c").is_some());
        Ok(())
    }

    #[test]
    fn the_one_where_special_chars_in_object_id_get_escaped() -> Result<()> {
        // 🧪 What if ObjectID contains quotes? Backslashes? Chaos?
        // We escape. We always escape. (The characters, not the situation.)
        let the_output = transform(
            serde_json::json!({"ObjectID": "doc\"with\\quotes"}).to_string(),
        )?;
        let the_action_line = the_output.split('\n').next().unwrap();
        // 🔍 Verify the action line is valid JSON despite the special chars
        let parsed: serde_json::Value = serde_json::from_str(the_action_line)?;
        assert_eq!(parsed["index"]["_id"], "doc\"with\\quotes");
        Ok(())
    }

    #[test]
    fn the_one_where_escape_json_string_handles_control_chars() {
        // 🧪 Control characters in IDs? In THIS economy?
        assert_eq!(escape_json_string("normal"), "normal");
        assert_eq!(escape_json_string(r#"has"quotes"#), r#"has\"quotes"#);
        assert_eq!(escape_json_string("has\\backslash"), "has\\\\backslash");
        assert_eq!(escape_json_string("has\nnewline"), "has\\nnewline");
        assert_eq!(escape_json_string("has\ttab"), "has\\ttab");
    }
}
