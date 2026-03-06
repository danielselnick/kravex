// ai
//! 🎬 *[the items arrive. they are many. they need brackets. they need commas.]*
//! *[serde was not invited. it doesn't even know this function exists.]*
//! *["wrap me," said the items. "wrap me in valid JSON." we obliged.]*
//!
//! 📦 **JsonArrayManifold** — casts feeds and joins them into `[item1,item2,item3]` without serde.
//!
//! 🧠 Knowledge graph:
//! - Used by: InMemory sinks — tests want valid JSON arrays to assert against
//! - Zero serde on the framing: just `[`, commas, `]`, assembled by hand like artisans
//! - Items inside are already valid JSON strings from casters — we trust them
//! - Capacity math: 2 (brackets) + sum(item lengths) + (n-1) commas — exact, no vibes needed
//!
//! 🦆 The duck asked why we don't use serde. We said "trust the process." It nodded.

use super::Manifold;
use crate::{Entry, Payload};
use anyhow::Result;
use std::collections::VecDeque;

// -- ┌─────────────────────────────────────────────────────────┐
// -- │  JsonArrayManifold                                       │
// -- │  Struct → impl Manifold → tests                          │
// -- └─────────────────────────────────────────────────────────┘

/// 📦 JSON Array format — `[item1,item2,item3]` — for when you want valid JSON output.
///
/// Casts each feed, collects all results, wraps in `[...]` with commas.
/// Zero serde on the framing. Just brackets and commas, assembled by hand.
///
/// 🧠 Used for in-memory sinks where tests want valid JSON arrays to assert against.
/// The items inside are already valid JSON strings from the casters — we just
/// frame them as an array without re-parsing. Trust the casters. They did their job.
///
/// Conspiracy theory: the borrow checker is sentient, and it WANTS you to use serde.
/// We resist. We concatenate manually. We are free. 🐄
#[derive(Debug, Clone, Copy)]
pub struct JsonArrayManifold;

impl Manifold for JsonArrayManifold {
    #[inline]
    fn join(&self, entries: &mut VecDeque<Entry>) -> Result<Payload> {
        // -- 🧮 Pre-allocate: brackets(2) + sum of entries + commas(max n-1).
        // -- This is exact capacity — no growth, no realloc, no drama.
        // -- No cap this capacity math slaps fr fr 🎯
        let commas = entries.len().saturating_sub(1);
        let estimated_size: usize =
            2 + entries.iter().map(|e| e.len()).sum::<usize>() + commas;
        let mut payload = String::with_capacity(estimated_size);
        payload.push('[');
        for (i, entry) in entries.drain(..).enumerate() {
            if i > 0 {
                // -- 🔗 The comma: JSON's way of saying "and there's more where that came from."
                payload.push(',');
            }
            payload.push_str(&entry);
        }
        payload.push(']');
        // -- ✅ Valid JSON array. No serde was harmed in the making of this string.
        Ok(Payload(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_array_the_one_where_entries_become_an_array() -> Result<()> {
        // 🧪 Three entries → [entry1,entry2,entry3]
        let manifold = JsonArrayManifold;
        let mut entries = VecDeque::from(vec![
            Entry(r#"{"doc":1}"#.to_string()),
            Entry(r#"{"doc":2}"#.to_string()),
            Entry(r#"{"doc":3}"#.to_string()),
        ]);
        let result = manifold.join(&mut entries)?;
        assert_eq!(*result, r#"[{"doc":1},{"doc":2},{"doc":3}]"#);
        assert!(entries.is_empty(), "🎯 drain(..) should leave the VecDeque empty but allocated");
        Ok(())
    }

    #[test]
    fn json_array_the_one_where_empty_entries_is_empty_array() -> Result<()> {
        // 🧪 No entries → []. Still valid JSON. Still technically correct. The best kind of correct.
        let manifold = JsonArrayManifold;
        let mut entries = VecDeque::new();
        let result = manifold.join(&mut entries)?;
        assert_eq!(*result, "[]");
        Ok(())
    }

    #[test]
    fn json_array_the_one_where_single_entry_has_no_commas() -> Result<()> {
        // 🧪 One entry, no commas. Like a party with one guest. Awkward but valid.
        let manifold = JsonArrayManifold;
        let mut entries = VecDeque::from(vec![Entry(r#"{"lonely":true}"#.to_string())]);
        let result = manifold.join(&mut entries)?;
        assert_eq!(*result, r#"[{"lonely":true}]"#);
        Ok(())
    }
}
