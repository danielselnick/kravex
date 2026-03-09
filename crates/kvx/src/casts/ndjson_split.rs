// ai
//! 🔪 NdJsonSplit — slices NDJSON feeds into individual JSON entries 🚀📦🎭
//!
//! 🎬 COLD OPEN — INT. DATA PIPELINE — SOMEWHERE BETWEEN SOURCE AND SINK
//! *[An NDJSON feed stumbles in, exhausted from its journey.]*
//! *["I contain multitudes," it gasps. "Each line is a separate document."]*
//! *[NdJsonSplit nods. "I know. Hold still. This won't hurt."]*
//! *[SNIP. SNIP. SNIP. Individual JSON entries fall like autumn leaves.]*
//!
//! Like `NdJsonToBulk`, but without the ES bulk action header.
//! Pure split: newlines in → individual entries out. No metadata.
//! No `{"index":{}}`. Just the docs, ma'am.
//!
//! 🧠 Knowledge graph:
//! - Input: NDJSON page (one JSON object per line)
//! - Output: Vec<Entry>, one Entry per non-empty line
//! - Used for: File→Meilisearch (source has NDJSON, sink wants individual JSON docs in array)
//! - Sister caster: `NdJsonToBulk` (adds ES bulk headers), `PitToJson` (extracts from PIT envelope)
//!
//! ⚠️ The singularity will split atoms. We split newlines. Close enough. 🦆

use anyhow::Result;
use crate::Entry;
use crate::Page;
use crate::casts::Caster;

/// 🔪 Splits NDJSON pages into individual JSON entries — no bulk headers, no drama.
///
/// Zero-sized struct. Cloning costs nothing. The compiler inlines everything.
/// It's the Marie Kondo of casters — does it have a newline? Split it.
/// Does it spark joy? Irrelevant. We split on newlines, not feelings. 🧹
#[derive(Debug, Clone, Copy)]
pub struct NdJsonSplit;

impl Caster for NdJsonSplit {
    #[inline]
    fn cast(&self, page: Page) -> Result<Vec<Entry>> {
        // 🔪 Split by newlines, keep non-empty lines, wrap each as an Entry
        // no cap this function slaps fr fr — one line per doc, no overhead, no bulk headers 🦆
        let the_entries: Vec<Entry> = page
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(|line| Entry(line.to_string()))
            .collect();
        Ok(the_entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 One doc in, one entry out. The simplest possible split.
    #[test]
    fn the_one_where_a_single_doc_passes_through_unmolested() -> Result<()> {
        let the_caster = NdJsonSplit;
        let the_lone_doc = r#"{"id":1,"name":"The One"}"#;

        let the_entries = the_caster.cast(Page(the_lone_doc.to_string()))?;

        assert_eq!(the_entries.len(), 1, "💀 Expected 1 entry, got {}", the_entries.len());
        assert_eq!(*the_entries[0], the_lone_doc, "💀 Entry content should match input exactly");
        Ok(())
    }

    /// 🧪 Multiple docs — each line becomes its own Entry, order preserved.
    #[test]
    fn the_one_where_three_docs_split_like_an_amicable_divorce() -> Result<()> {
        let the_caster = NdJsonSplit;
        let doc_a = r#"{"id":1,"name":"Alpha"}"#;
        let doc_b = r#"{"id":2,"name":"Bravo"}"#;
        let doc_c = r#"{"id":3,"name":"Charlie"}"#;
        let the_feed = format!("{doc_a}\n{doc_b}\n{doc_c}");

        let the_entries = the_caster.cast(Page(the_feed))?;

        assert_eq!(the_entries.len(), 3, "💀 Expected 3 entries for 3 docs");
        assert_eq!(*the_entries[0], doc_a);
        assert_eq!(*the_entries[1], doc_b);
        assert_eq!(*the_entries[2], doc_c);
        Ok(())
    }

    /// 🧪 Empty input — the void returns void. No phantom entries.
    #[test]
    fn the_one_where_emptiness_begets_emptiness_like_my_fridge_on_sunday() -> Result<()> {
        let the_caster = NdJsonSplit;
        let the_entries = the_caster.cast(Page("".to_string()))?;
        assert!(the_entries.is_empty(), "💀 Empty input should produce empty output");
        Ok(())
    }

    /// 🧪 Trailing newline — no ghost entry at the end.
    #[test]
    fn the_one_where_trailing_newlines_dont_spawn_ghost_entries() -> Result<()> {
        let the_caster = NdJsonSplit;
        let the_feed = r#"{"id":1}
"#;
        let the_entries = the_caster.cast(Page(the_feed.to_string()))?;
        assert_eq!(the_entries.len(), 1, "💀 Trailing newline should not create extra entry");
        Ok(())
    }

    /// 🧪 Blank lines scattered through the feed — filtered out like bad Tinder profiles.
    #[test]
    fn the_one_where_blank_lines_are_filtered_like_spam_emails() -> Result<()> {
        let the_caster = NdJsonSplit;
        let the_chaotic_feed = "\n\n{\"id\":1}\n\n\n{\"id\":2}\n\n";

        let the_entries = the_caster.cast(Page(the_chaotic_feed.to_string()))?;
        assert_eq!(the_entries.len(), 2, "💀 Only non-empty lines should become entries, got {}", the_entries.len());
        assert_eq!(*the_entries[0], r#"{"id":1}"#);
        assert_eq!(*the_entries[1], r#"{"id":2}"#);
        Ok(())
    }

    /// 🧪 Each entry is valid JSON — no corruption from the split.
    #[test]
    fn the_one_where_every_entry_is_valid_json_because_we_have_standards() -> Result<()> {
        let the_caster = NdJsonSplit;
        let the_feed = r#"{"id":1,"nested":{"deep":true}}
{"id":2,"tags":["rust","meili"]}
{"id":3,"score":null}"#;

        let the_entries = the_caster.cast(Page(the_feed.to_string()))?;
        for (i, entry) in the_entries.iter().enumerate() {
            let _parsed: serde_json::Value = serde_json::from_str(&entry.0)
                .map_err(|e| anyhow::anyhow!("💀 Entry {} is not valid JSON: '{}' — error: {}", i, entry.0, e))?;
        }
        Ok(())
    }
}
