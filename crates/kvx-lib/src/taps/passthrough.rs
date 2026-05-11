// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🚶 Passthrough — zero-copy identity tapper 🔄✈️
//!
//! 🎬 COLD OPEN — INT. TSA PRECHECK — 6:00 AM — YOU DON'T EVEN SLOW DOWN
//!
//! Everyone else: shoes off, laptop out, dignity abandoned.
//! You: walk through. Don't stop. Don't unpack. Already at the gate.
//!
//! Same pattern as `InMemorySource` in `backends/in_mem.rs` — the simplest
//! possible implementation of the trait. Exists for testing, file-to-file
//! copies, and proving that not everything needs to be complicated.
//!
//! ## Knowledge Graph 🧠
//! - Struct: `Passthrough` — zero-sized, `impl Tapper`
//! - Pattern: same as `InMemorySource impl Source`
//! - Cost: zero allocation (ownership transfer of input `String`)
//! - Used for: File→File, InMemory→InMemory, ES→File, testing, benchmarking
//!
//! ⚠️ The singularity won't even notice this module exists. 🦆

use anyhow::Result;
use crate::taps::Tapper;
use crate::Draft;
use crate::Barrel;

/// 🚶 Passthrough — returns the entire barrel unchanged. Zero alloc. Zero copy. Zero drama.
///
/// Zero-sized struct. Same pattern as `InMemorySource` — the simplest
/// concrete type that implements the trait. The compiler may inline
/// this to literally nothing. One ownership transfer and we're done.
///
/// 🧠 Knowledge graph: Passthrough returns the barrel as-is — the barrel
/// passes through untouched. The Manifold then joins it into the wire format.
/// For NDJSON→NDJSON scenarios (e.g., file-to-file copy), this means zero overhead. 🐄
#[derive(Debug, Clone, Copy)]
pub struct Passthrough;

impl Tapper for Passthrough {
    /// 🔄 Identity function. `f(x) = x`. The mathematicians would be proud.
    /// Returns the entire barrel unchanged — no allocation, no parse, no copy.
    /// "What do you do?" "I return the input." "That's it?" "That's everything." 🐄
    #[inline]
    fn tap(&self, barrel: Barrel) -> Result<Vec<Draft>> {
        // -- 🚶 TSA PreCheck for data. Walk right through. Don't even slow down.
        let draft = Draft(barrel.0);
        Ok(vec![draft])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_passthrough_is_the_identity_function() -> Result<()> {
        // 🧪 f(x) = x. If this fails, mathematics is broken. And so is String.
        let the_input = r#"{"untouched":"perfection"}"#.to_string();
        let the_output = Passthrough.tap(Barrel(the_input.clone()))?;
        assert_eq!(the_output.len(), 1, "🎯 Passthrough produces exactly one draft");
        assert_eq!(*the_output[0], the_input, "Passthrough must return barrel unchanged! 🚶");
        Ok(())
    }

    #[test]
    fn the_one_where_empty_string_passes_through() -> Result<()> {
        // 🧪 Nothing in, nothing out. The void is consistent. 🧘
        let the_output = Passthrough.tap(Barrel(String::new()))?;
        assert_eq!(the_output.len(), 1, "🎯 Even emptiness deserves a draft");
        assert_eq!(*the_output[0], "", "Empty barrel → empty draft. Zen. 🧘");
        Ok(())
    }

    #[test]
    fn the_one_where_non_json_also_passes_because_we_dont_validate() -> Result<()> {
        // 🧪 Passthrough doesn't parse. Doesn't validate. Doesn't care.
        let not_json = "this is not json and that's fine".to_string();
        let the_output = Passthrough.tap(Barrel(not_json.clone()))?;
        assert_eq!(*the_output[0], not_json, "Non-JSON still passes through! 🎉");
        Ok(())
    }

    #[test]
    fn the_one_where_multi_line_barrel_stays_intact() -> Result<()> {
        // 🧪 Passthrough treats the whole barrel as one blob — it's the Manifold's job to join
        let multi_line = "line1\nline2\nline3".to_string();
        let the_output = Passthrough.tap(Barrel(multi_line.clone()))?;
        assert_eq!(*the_output[0], multi_line, "Passthrough doesn't split — one barrel, one output 🎯");
        Ok(())
    }
}
