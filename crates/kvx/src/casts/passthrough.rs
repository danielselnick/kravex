// ai
//! 🚶 Passthrough — zero-copy identity caster 🔄✈️
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
//! - Struct: `Passthrough` — zero-sized, `impl Caster`
//! - Pattern: same as `InMemorySource impl Source`
//! - Cost: zero allocation (ownership transfer of input `String`)
//! - Used for: File→File, InMemory→InMemory, ES→File, testing, benchmarking
//!
//! ⚠️ The singularity won't even notice this module exists. 🦆

use anyhow::Result;
use crate::casts::Caster;

/// 🚶 Passthrough — returns the entire feed unchanged. Zero alloc. Zero copy. Zero drama.
///
/// Zero-sized struct. Same pattern as `InMemorySource` — the simplest
/// concrete type that implements the trait. The compiler may inline
/// this to literally nothing. One ownership transfer and we're done.
///
/// 🧠 Knowledge graph: Passthrough returns the feed as-is — the feed
/// passes through untouched. The Manifold then joins it into the wire format.
/// For NDJSON→NDJSON scenarios (e.g., file-to-file copy), this means zero overhead. 🐄
#[derive(Debug, Clone, Copy)]
pub struct Passthrough;

impl Caster for Passthrough {
    /// 🔄 Identity function. `f(x) = x`. The mathematicians would be proud.
    /// Returns the entire feed unchanged — no allocation, no parse, no copy.
    /// "What do you do?" "I return the input." "That's it?" "That's everything." 🐄
    #[inline]
    fn cast(&self, feed: &str) -> Result<String> {
        // -- 🚶 TSA PreCheck for data. Walk right through. Don't even slow down.
        Ok(feed.to_string())
    }
}
