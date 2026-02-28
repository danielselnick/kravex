// ai
//! 🏎️ Rally S3 JSON Transform — because Rally decided JSON needed extra steps 📦🔄
//!
//! 🎬 COLD OPEN — INT. S3 BUCKET — us-east-1 — TIME IS MEANINGLESS HERE
//!
//! Somewhere in us-east-1, a JSON blob sleeps. It has an `ObjectID`. It has a
//! `FormattedID`. It has seventeen fields nobody reads and three fields everyone
//! fights about in sprint review. It is a Rally artifact. It is waiting.
//!
//! It has been waiting since Sprint 47. Nobody remembers Sprint 47.
//! The Product Owner from Sprint 47 works in real estate now. The Scrum Master
//! became a yoga instructor. The tech lead? The tech lead is here. Writing this
//! module. At 2am. The tech lead never left.
//!
//! This module knows how to wake that blob up, extract what matters, strip the
//! metadata cruft that Rally bolts onto every response like unwanted browser
//! toolbars from 2004, and send the cleaned document on its way to a search
//! index where it can finally be useful. Or at least findable.
//!
//! ## Knowledge Graph 🧠
//! - Implements: `IngestTransform` (source format → intermediate `Hit`)
//! - Source format: Rally/Broadcom Agile Central JSON export blobs (S3 or local filesystem)
//! - Key fields extracted: `ObjectID` → `Hit::id`
//! - Metadata stripped: `_rallyAPIMajor`, `_rallyAPIMinor`, `_ref`, `_refObjectUUID`,
//!   `_objectVersion`, `_CreatedAt` (the six horsemen of unnecessary metadata)
//! - Output: cleaned JSON in `Hit::source_buf`, `ObjectID` in `Hit::id`
//! - Index/routing: NOT set here (that's the sink's job or config-driven)
//!
//! ⚠️ Rally was acquired by CA was acquired by Broadcom was acquired by
//! the heat death of the universe. The JSON remains. The JSON is eternal. 🦆

use super::IngestTransform;
use crate::common::Hit;
use anyhow::{Context, Result};
use serde_json::Value;

/// 🏎️ RallyS3Json — the translator for Rally's particular brand of JSON drama.
///
/// Rally S3 exports have their own... personality. They're like that friend who
/// always over-explains everything. "Here's the document," Rally says, "and also
/// here's my API version, and a reference URL, and a UUID, and the creation date
/// in a format nobody asked for." Thanks, Rally. Very helpful.
///
/// ## What this transform does 🔧
///
/// 1. **Parses** the Rally JSON blob (panics internally if it's not valid JSON —
///    but returns an error externally because we are civilized)
/// 2. **Extracts** `ObjectID` → `Hit::id` (stringified, because ES ids are strings
///    and Rally can't decide if ObjectID is a number or a string)
/// 3. **Strips** Rally API metadata fields (the six fields listed in the module docs)
/// 4. **Stores** the cleaned document body in `Hit::source_buf`
///
/// ## What this transform does NOT do 🚫
///
/// - Assign an index (that's the sink's job or config-driven)
/// - Set routing (Rally doesn't do routing — Rally does what Rally wants)
/// - Validate business logic (is this a valid User Story? we don't know. we don't care.)
/// - Make coffee (yet — see roadmap item #47, "Add coffee to all transforms")
///
/// All hardcoded. All monomorphized. No vtables were harmed.
/// The compiler generates straight-line code for this exact format.
pub(crate) struct RallyS3Json;

/// 🗑️ Rally metadata fields that get stripped during ingest.
///
/// These are the API-level metadata fields that Rally attaches to every object.
/// They're useful for the Rally API client. They're useless for a search index.
/// They're like the plastic wrap on a new laptop: technically there for a reason,
/// but you rip it off immediately.
///
/// Tribal knowledge: Rally's REST API wraps every object with these fields.
/// S3 exports sometimes include them, sometimes don't, depending on how
/// the export was configured (and the phase of the moon, probably).
/// We strip them unconditionally because it's cheaper than checking.
const THE_METADATA_FIELDS_WE_DONT_NEED: &[&str] = &[
    "_rallyAPIMajor",
    "_rallyAPIMinor",
    "_ref",
    "_refObjectUUID",
    "_objectVersion",
    "_CreatedAt",
];

impl IngestTransform for RallyS3Json {
    /// 🔄 Parse a Rally S3 JSON blob into a properly-hydrated Hit.
    ///
    /// Extracts the good parts. Discards the metadata cruft. Stores the rest.
    /// It's like panning for gold, except the gold is `ObjectID` and the river
    /// is a 47KB JSON blob with nested custom fields that someone added during
    /// a "quick hackathon" in 2019 and nobody has touched since.
    ///
    /// # Errors
    /// 💀 Returns error if the input is not valid JSON (which means someone
    /// put something in S3 that shouldn't have been there — looking at you, Kevin)
    fn transform_hit(raw: String) -> Result<Hit> {
        // 🔬 Parse the JSON first — if this fails, the blob was never valid
        // and we should have questioned our life choices earlier in the pipeline.
        // "Config not found: We looked everywhere. Under the couch. Behind the fridge.
        //  In the junk drawer. Nothing." — except replace "config" with "valid JSON"
        let mut doc: Value = serde_json::from_str(&raw).context(
            "💀 Rally JSON parse failed — the blob has gone rogue. \
             This is either not JSON or it's JSON that has seen things. \
             Check the S3 object. Check your assumptions. Check your will to live.",
        )?;

        // 🎯 Extract ObjectID — the one true identifier, the North Star,
        // the thing Rally uses to know what is what.
        // Rally uses numeric ObjectIDs in the API but sometimes string in exports.
        // Because consistency is for people who don't ship fast enough. 🏎️
        let the_identity_crisis_resolved = doc.get("ObjectID").map(|oid| match oid {
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            // 🦆 For anything else (bool? array? null? cosmic ray?), just stringify it
            honestly_who_knows => honestly_who_knows.to_string(),
        });

        // 🗑️ Strip Rally API metadata — the JSON equivalent of removing
        // all those "REMOVE BEFORE FLIGHT" tags from an airplane.
        // Except these tags don't prevent crashes, they just waste bytes.
        if let Value::Object(ref mut map) = doc {
            for doomed_field in THE_METADATA_FIELDS_WE_DONT_NEED {
                map.remove(*doomed_field);
            }
        }

        // 📦 Re-serialize the cleaned document — leaner, meaner, metadata-free.
        // The document has been through customs. Its bags are lighter.
        // It is ready for its new life in the search index.
        let the_cleaned_up_aftermath = serde_json::to_string(&doc).context(
            "💀 Failed to re-serialize cleaned Rally JSON. \
             The JSON went in valid and came out... not. \
             This shouldn't happen. If it does, the laws of physics need a patch.",
        )?;

        // ✅ Birth a fully-hydrated Hit. It has a name (maybe). It has purpose (debatable).
        // It is no longer the nameless void-entity from Hit::new().
        // It has been through the Rally transform and emerged... different. Better?
        // That's not for us to say. We just build the pipeline.
        Ok(Hit {
            id: the_identity_crisis_resolved,
            routing: None, // 🔧 Rally doesn't do routing. Rally does what Rally wants.
            index: None, // 📡 Index assignment is the sink's problem. Separation of concerns. Beautiful.
            sort: 0,     // 🔢 Default sort. Sprint ordering is someone else's existential crisis.
            source_buf: the_cleaned_up_aftermath,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_rally_json_gets_a_proper_haircut() -> Result<()> {
        // 🧪 A Rally JSON blob walks into a bar. The bartender says,
        // "We don't serve _rallyAPIMajor here." The blob orders a refactor.
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

        let the_hit = RallyS3Json::transform_hit(rally_blob.to_string())?;

        // 🎯 ObjectID extracted as string id
        assert_eq!(
            the_hit.id,
            Some("12345".to_string()),
            "ObjectID should become the Hit's id"
        );

        // 🗑️ Rally metadata should be nuked from orbit (it's the only way to be sure)
        let the_aftermath: Value = serde_json::from_str(&the_hit.source_buf)?;
        assert!(
            the_aftermath.get("_rallyAPIMajor").is_none(),
            "_rallyAPIMajor should be stripped"
        );
        assert!(
            the_aftermath.get("_rallyAPIMinor").is_none(),
            "_rallyAPIMinor should be stripped"
        );
        assert!(
            the_aftermath.get("_ref").is_none(),
            "_ref should be stripped"
        );
        assert!(
            the_aftermath.get("_refObjectUUID").is_none(),
            "_refObjectUUID should be stripped"
        );
        assert!(
            the_aftermath.get("_objectVersion").is_none(),
            "_objectVersion should be stripped"
        );
        assert!(
            the_aftermath.get("_CreatedAt").is_none(),
            "_CreatedAt should be stripped"
        );

        // ✅ Actual document fields survive the cleansing
        assert_eq!(
            the_aftermath.get("Name").and_then(Value::as_str),
            Some("As a user, I want to migrate data without crying")
        );
        assert_eq!(
            the_aftermath.get("FormattedID").and_then(Value::as_str),
            Some("US789")
        );
        assert_eq!(
            the_aftermath.get("ScheduleState").and_then(Value::as_str),
            Some("Accepted")
        );
        // _type is NOT in the strip list — it's potentially useful for index routing
        assert_eq!(
            the_aftermath.get("_type").and_then(Value::as_str),
            Some("HierarchicalRequirement")
        );

        Ok(())
    }

    #[test]
    fn the_one_where_object_id_is_a_string_because_consistency_is_dead() -> Result<()> {
        // 🧪 Rally S3 exports: where ObjectID is a number in the API
        // but sometimes a string in the export. Because why be predictable
        // when you can be ✨ exciting ✨
        let rally_blob = serde_json::json!({
            "ObjectID": "67890",
            "Name": "String ObjectID test"
        });

        let the_hit = RallyS3Json::transform_hit(rally_blob.to_string())?;
        assert_eq!(the_hit.id, Some("67890".to_string()));
        Ok(())
    }

    #[test]
    fn the_one_where_no_object_id_and_the_hit_copes() -> Result<()> {
        // 🧪 What if there's no ObjectID? We don't panic.
        // We have trust issues, sure, but we don't panic.
        // The Hit gets None for an id. It'll figure it out. Or it won't.
        // Not everything needs an identity. Ask any philosopher.
        let rally_blob = serde_json::json!({
            "Name": "Mystery document with no identity",
            "Description": "Found in the back of the S3 bucket behind the Christmas decorations"
        });

        let the_hit = RallyS3Json::transform_hit(rally_blob.to_string())?;
        assert_eq!(the_hit.id, None, "No ObjectID means no id. Zen.");

        Ok(())
    }

    #[test]
    fn the_one_where_invalid_json_meets_its_maker() {
        // 🧪 Garbage in, error out. This is a feature, not a bug.
        // If you put non-JSON in an S3 bucket labeled "Rally JSON,"
        // that's between you and your incident postmortem.
        let definitely_not_json = "this is definitely not json, kevin".to_string();
        let the_result = RallyS3Json::transform_hit(definitely_not_json);
        assert!(
            the_result.is_err(),
            "Invalid JSON should produce an error, not a miracle"
        );
    }

    #[test]
    fn the_one_where_metadata_stripping_is_idempotent() -> Result<()> {
        // 🧪 If the metadata fields aren't there, stripping should be a no-op.
        // Like trying to fire someone who already quit. Nothing happens.
        let clean_blob = serde_json::json!({
            "ObjectID": 99999,
            "Name": "Already clean document",
            "CustomField_c": "custom fields survive everything, like cockroaches"
        });

        let the_hit = RallyS3Json::transform_hit(clean_blob.to_string())?;
        let the_body: Value = serde_json::from_str(&the_hit.source_buf)?;
        assert_eq!(the_body.get("Name").and_then(Value::as_str), Some("Already clean document"));
        assert!(the_body.get("CustomField_c").is_some(), "Custom fields must survive");

        Ok(())
    }

    #[test]
    fn the_one_where_nested_rally_objects_survive_the_purge() -> Result<()> {
        // 🧪 Rally JSON can have nested objects (Project, Iteration, etc.)
        // These should pass through untouched, even if they contain
        // metadata-looking fields at deeper levels.
        let nested_blob = serde_json::json!({
            "ObjectID": 11111,
            "Name": "Story with nested refs",
            "_rallyAPIMajor": "2",
            "Project": {
                "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/project/222",
                "Name": "Project Alpha"
            },
            "Iteration": {
                "_ref": "https://rally1.rallydev.com/slm/webservice/v2.0/iteration/333",
                "Name": "Sprint 42"
            }
        });

        let the_hit = RallyS3Json::transform_hit(nested_blob.to_string())?;
        let the_body: Value = serde_json::from_str(&the_hit.source_buf)?;

        // Top-level _rallyAPIMajor should be stripped
        assert!(the_body.get("_rallyAPIMajor").is_none());

        // But nested _ref inside Project and Iteration should survive
        // (we only strip top-level metadata, not nested refs)
        assert!(the_body["Project"]["_ref"].is_string(), "Nested _ref should survive");
        assert!(the_body["Iteration"]["_ref"].is_string(), "Nested _ref should survive");

        Ok(())
    }
}
