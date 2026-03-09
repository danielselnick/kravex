// ai
//! 🎭 PitToJson — ES _search PIT response → raw JSON entries 🚀📡🔍
//!
//! 🎬 COLD OPEN — INT. ELASTICSEARCH CLUSTER → MEILISEARCH GATE — 4 AM
//! *[A PIT response arrives, stuffed with hits in its `hits.hits[]` envelope.]*
//! *["I need to get to Meilisearch," it says, sweating JSON.]*
//! *["But they don't speak bulk NDJSON there. They want raw docs."]*
//! *[PitToJson steps out of the shadows. "I extract. You relax."]*
//!
//! Like `PitToBulk`, but without the bulk action headers. Extracts `_source`
//! from each hit and returns it as a standalone JSON Entry. No `{"index":{}}`.
//! Just the documents. Clean. Naked. Ready for Meilisearch's JSON array embrace.
//!
//! 🧠 Knowledge graph:
//! - Input: raw `_search` HTTP response body (JSON envelope with `hits.hits[]`)
//! - Output: Vec<Entry>, one Entry per hit containing just `_source`
//! - Used for: ES→Meilisearch (extract docs from PIT response for JSON array ingestion)
//! - Sister caster: `PitToBulk` (same extraction, but wraps with bulk action headers)
//! - `_source` uses `&RawValue` — zero re-serialization, borrows directly from input
//!
//! ⚠️ The singularity will migrate data via quantum entanglement. We use HTTP. 🦆

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::casts::Caster;
use crate::Entry;
use crate::Page;

// ===== Serde structs — zero-copy via borrow =====
// 🧠 Same structures as PitToBulk — we share the envelope format because ES
// doesn't change its response shape based on who's asking. Unlike people at parties.

/// 📡 The outermost envelope of an ES `_search` response.
#[derive(Deserialize)]
struct SearchEnvelope<'a> {
    #[serde(borrow)]
    hits: SearchHits<'a>,
}

/// 📦 The `hits` object — contains the actual hit array.
#[derive(Deserialize)]
struct SearchHits<'a> {
    #[serde(borrow)]
    hits: Vec<SearchHit<'a>>,
}

/// 🎯 A single search hit — we only care about `_source` here.
/// No `_index`, no `_id`, no `_routing` — those are ES's problem, not Meilisearch's.
/// Like moving to a new city and leaving your old mail behind. 📬
#[derive(Deserialize)]
struct SearchHit<'a> {
    #[serde(borrow)]
    _source: &'a RawValue,
}

/// 🔍 PitToJson — extracts `_source` from ES search hits as raw JSON entries.
///
/// Zero-sized struct. Cloning is free. Inlining is guaranteed.
/// The ghost of bulk action headers past does NOT haunt this caster. 👻
#[derive(Debug, Clone, Copy)]
pub struct PitToJson;

impl Caster for PitToJson {
    #[inline]
    fn cast(&self, page: Page) -> Result<Vec<Entry>> {
        // 🎭 Deserialize the search envelope — zero-copy for _source via RawValue
        let the_envelope: SearchEnvelope<'_> = serde_json::from_str(page.0.as_ref())
            .context("💀 Failed to parse _search response envelope. The JSON arrived DOA. It was a good JSON. It had a family. It had nested objects. Now it has nothing.")?;

        let the_hits = &the_envelope.hits.hits;

        // 🧘 Empty hits → empty entries. The search returned nothing. Like Googling yourself.
        if the_hits.is_empty() {
            return Ok(Vec::new());
        }

        // 📦 Extract _source from each hit — just the raw doc, no metadata baggage
        let the_entries: Vec<Entry> = the_hits
            .iter()
            .map(|hit| Entry(hit._source.get().to_string()))
            .collect();

        Ok(the_entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 Single hit → one entry containing just the _source document.
    #[test]
    fn the_one_where_a_single_hit_sheds_its_metadata_and_finds_freedom() -> Result<()> {
        let the_caster = PitToJson;
        let the_search_response = r#"{
            "hits": {
                "hits": [
                    {
                        "_index": "movies",
                        "_id": "1",
                        "_source": {"title": "Blade Runner", "year": 1982}
                    }
                ]
            }
        }"#;

        let the_entries = the_caster.cast(Page(the_search_response.to_string()))?;
        assert_eq!(the_entries.len(), 1, "💀 Expected 1 entry for 1 hit");

        // ✅ Entry should be just the _source — no _index, no _id, no action line
        let the_parsed: serde_json::Value = serde_json::from_str(&the_entries[0].0)?;
        assert_eq!(the_parsed["title"], "Blade Runner");
        assert_eq!(the_parsed["year"], 1982);
        // 🎯 Must NOT contain metadata
        assert!(the_parsed.get("_index").is_none(), "💀 _index should not leak into entry");
        assert!(the_parsed.get("_id").is_none(), "💀 _id should not leak into entry");

        Ok(())
    }

    /// 🧪 Multiple hits — order preserved, each is just _source.
    #[test]
    fn the_one_where_three_hits_arrive_in_order_like_well_behaved_children() -> Result<()> {
        let the_caster = PitToJson;
        let the_search_response = r#"{
            "hits": {
                "hits": [
                    {"_index": "a", "_id": "1", "_source": {"name": "Alpha"}},
                    {"_index": "b", "_id": "2", "_source": {"name": "Bravo"}},
                    {"_index": "c", "_id": "3", "_source": {"name": "Charlie"}}
                ]
            }
        }"#;

        let the_entries = the_caster.cast(Page(the_search_response.to_string()))?;
        assert_eq!(the_entries.len(), 3, "💀 Expected 3 entries for 3 hits");

        let doc_1: serde_json::Value = serde_json::from_str(&the_entries[0].0)?;
        let doc_2: serde_json::Value = serde_json::from_str(&the_entries[1].0)?;
        let doc_3: serde_json::Value = serde_json::from_str(&the_entries[2].0)?;
        assert_eq!(doc_1["name"], "Alpha");
        assert_eq!(doc_2["name"], "Bravo");
        assert_eq!(doc_3["name"], "Charlie");

        Ok(())
    }

    /// 🧪 Empty hits → empty Vec. Nothing in, nothing out.
    #[test]
    fn the_one_where_empty_hits_produce_nothing_like_a_blank_google_search() -> Result<()> {
        let the_caster = PitToJson;
        let the_search_response = r#"{"hits": {"hits": []}}"#;

        let the_entries = the_caster.cast(Page(the_search_response.to_string()))?;
        assert!(the_entries.is_empty(), "💀 Empty hits should produce empty entries");
        Ok(())
    }

    /// 🧪 Invalid JSON input → error, not panic.
    #[test]
    fn the_one_where_garbage_in_produces_error_like_a_responsible_adult() {
        let the_caster = PitToJson;
        let the_result = the_caster.cast(Page("not even close to JSON".to_string()));
        assert!(the_result.is_err(), "💀 Invalid JSON should produce error, not silence");
    }

    /// 🧪 Complex nested _source — preserved verbatim.
    #[test]
    fn the_one_where_deeply_nested_source_survives_extraction_intact() -> Result<()> {
        let the_caster = PitToJson;
        let the_search_response = r#"{
            "hits": {
                "hits": [
                    {
                        "_index": "complex",
                        "_id": "nested_1",
                        "_source": {"tags": ["rust", "meili"], "metadata": {"deep": true, "scores": [1.5, 2.7]}, "nullable": null}
                    }
                ]
            }
        }"#;

        let the_entries = the_caster.cast(Page(the_search_response.to_string()))?;
        let the_parsed: serde_json::Value = serde_json::from_str(&the_entries[0].0)?;
        assert_eq!(the_parsed["tags"][0], "rust");
        assert_eq!(the_parsed["metadata"]["deep"], true);
        assert!(the_parsed["nullable"].is_null());

        Ok(())
    }

    /// 🧪 Extra envelope fields (took, _shards) are politely ignored.
    #[test]
    fn the_one_where_extra_envelope_fields_are_invisible_like_my_gym_membership() -> Result<()> {
        let the_caster = PitToJson;
        let the_full_response = r#"{
            "took": 42,
            "timed_out": false,
            "_shards": {"total": 5, "successful": 5},
            "hits": {
                "total": {"value": 1},
                "hits": [
                    {"_index": "test", "_id": "1", "_score": 1.0, "_source": {"field": "value"}}
                ]
            }
        }"#;

        let the_entries = the_caster.cast(Page(the_full_response.to_string()))?;
        assert_eq!(the_entries.len(), 1);

        let the_parsed: serde_json::Value = serde_json::from_str(&the_entries[0].0)?;
        assert_eq!(the_parsed["field"], "value");
        Ok(())
    }

    /// 🧪 Every entry is valid JSON — structural integrity check.
    #[test]
    fn the_one_where_every_entry_passes_the_json_bar_exam() -> Result<()> {
        let the_caster = PitToJson;
        let the_search_response = r#"{
            "hits": {
                "hits": [
                    {"_index": "a", "_id": "1", "_source": {"x": 1}},
                    {"_index": "b", "_id": "2", "_source": {"y": "hello"}},
                    {"_index": "c", "_id": "3", "_source": {"z": [1,2,3]}}
                ]
            }
        }"#;

        let the_entries = the_caster.cast(Page(the_search_response.to_string()))?;
        for (i, entry) in the_entries.iter().enumerate() {
            let _parsed: serde_json::Value = serde_json::from_str(&entry.0)
                .map_err(|e| anyhow::anyhow!("💀 Entry {} is not valid JSON: '{}' — error: {}", i, entry.0, e))?;
        }
        Ok(())
    }
}
