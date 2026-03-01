// ai
//! 🎬 *[the items arrive. they are many. they need brackets. they need commas.]*
//! *[serde was not invited. it doesn't even know this function exists.]*
//! *["wrap me," said the items. "wrap me in valid JSON." we obliged.]*
//!
//! 📦 **JsonArrayComposer** — assembles items into `[item1,item2,item3]` without serde.
//!
//! 🧠 Knowledge graph:
//! - Used by: InMemory sinks — tests want valid JSON arrays to assert against
//! - Zero serde on the framing: just `[`, commas, `]`, assembled by hand like artisans
//! - Items inside are already valid JSON strings from transforms — we trust them
//! - Capacity math: 2 (brackets) + sum(item lengths) + (n-1) commas — exact, no vibes needed
//!
//! 🦆 The duck asked why we don't use serde. We said "trust the process." It nodded.

use super::Composer;
use crate::transforms::{DocumentTransformer, Transform};
use anyhow::Result;

// -- ┌─────────────────────────────────────────────────────────┐
// -- │  JsonArrayComposer                                      │
// -- │  Struct → impl Composer → tests                         │
// -- └─────────────────────────────────────────────────────────┘

/// 📦 JSON Array format — `[item1,item2,item3]` — for when you want valid JSON output.
///
/// Transforms each page, collects all items, wraps in `[...]` with commas.
/// Zero serde on the framing. Just brackets and commas, assembled by hand.
///
/// 🧠 Used for in-memory sinks where tests want valid JSON arrays to assert against.
/// The items inside are already valid JSON strings from the transforms — we just
/// frame them as an array without re-parsing. Trust the transforms. They did their job.
///
/// Conspiracy theory: the borrow checker is sentient, and it WANTS you to use serde.
/// We resist. We concatenate manually. We are free. 🐄
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsonArrayComposer;

impl Composer for JsonArrayComposer {
    #[inline]
    fn compose(&self, pages: &[String], transformer: &DocumentTransformer) -> Result<String> {
        // -- 📦 First, collect all items from all pages into one flat list
        // -- Must collect before sizing because we need total count for comma math
        let mut all_items = Vec::new();
        for page in pages {
            let items = transformer.transform(page)?;
            all_items.extend(items);
        }

        // -- 🧮 Pre-allocate: brackets(2) + sum of items + commas(max n-1).
        // -- This is exact capacity — no growth, no realloc, no drama.
        // -- No cap this capacity math slaps fr fr 🎯
        let commas = if all_items.is_empty() {
            0
        } else {
            all_items.len() - 1
        };
        let estimated_size: usize =
            2 + all_items.iter().map(|s| s.as_ref().len()).sum::<usize>() + commas;
        let mut payload = String::with_capacity(estimated_size);
        payload.push('[');
        for (i, item) in all_items.iter().enumerate() {
            if i > 0 {
                // -- 🔗 The comma: JSON's way of saying "and there's more where that came from."
                // -- Without this comma, the JSON validator weeps. With it, it beams with pride.
                payload.push(',');
            }
            payload.push_str(item.as_ref());
        }
        payload.push(']');
        // -- ✅ Valid JSON array. No serde was harmed in the making of this string.
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transforms::passthrough::Passthrough;

    // -- 🔧 Helper: passthrough transformer — transforms by doing absolutely nothing. Inspirational.
    fn passthrough_transformer() -> DocumentTransformer {
        DocumentTransformer::Passthrough(Passthrough)
    }

    #[test]
    fn json_array_the_one_where_pages_become_an_array() -> Result<()> {
        // 🧪 Three pages, each passthrough → [page1,page2,page3]
        let composer = JsonArrayComposer;
        let pages = vec![
            String::from(r#"{"doc":1}"#),
            String::from(r#"{"doc":2}"#),
            String::from(r#"{"doc":3}"#),
        ];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, r#"[{"doc":1},{"doc":2},{"doc":3}]"#);
        Ok(())
    }

    #[test]
    fn json_array_the_one_where_empty_pages_is_empty_array() -> Result<()> {
        // 🧪 No pages → []. Still valid JSON. Still technically correct. The best kind of correct.
        let composer = JsonArrayComposer;
        let result = composer.compose(&[], &passthrough_transformer())?;
        assert_eq!(result, "[]");
        Ok(())
    }

    #[test]
    fn json_array_the_one_where_single_page_has_no_commas() -> Result<()> {
        // 🧪 One page, no commas. Like a party with one guest. Awkward but valid.
        let composer = JsonArrayComposer;
        let pages = vec![String::from(r#"{"lonely":true}"#)];
        let result = composer.compose(&pages, &passthrough_transformer())?;
        assert_eq!(result, r#"[{"lonely":true}]"#);
        Ok(())
    }
}
