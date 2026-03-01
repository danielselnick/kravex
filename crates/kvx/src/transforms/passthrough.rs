// ai
//! 🚶 Passthrough — zero-copy identity transform 🔄✈️
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
//! - Struct: `Passthrough` — zero-sized, `impl Transform`
//! - Pattern: same as `InMemorySource impl Source`
//! - Cost: zero allocation (ownership transfer of input `String`)
//! - Used for: File→File, InMemory→InMemory, ES→File, testing, benchmarking
//!
//! ⚠️ The singularity won't even notice this module exists. 🦆

use super::Transform;
use anyhow::Result;

/// 🚶 Passthrough — returns input unchanged. `Ok(raw)`. That's the whole impl.
///
/// Zero-sized struct. Same pattern as `InMemorySource` — the simplest
/// concrete type that implements the trait. The compiler may inline
/// this to literally nothing. Three characters of implementation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Passthrough;

impl Transform for Passthrough {
    /// 🔄 Identity function. `f(x) = x`. The mathematicians would be proud.
    /// The ownership transfers directly — no allocation, no parse, no copy.
    #[inline]
    fn transform(&self, raw: String) -> Result<String> {
        // -- 🚶 And just like that... it's done.
        // -- "What do you do?" "I return the input." "That's it?" "That's everything."
        Ok(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_passthrough_is_the_identity_function() -> Result<()> {
        // 🧪 f(x) = x. If this fails, mathematics is broken.
        let the_input = r#"{"untouched":"perfection"}"#.to_string();
        let the_output = Passthrough.transform(the_input.clone())?;
        assert_eq!(the_output, the_input);
        Ok(())
    }

    #[test]
    fn the_one_where_empty_string_passes_through() -> Result<()> {
        assert_eq!(Passthrough.transform(String::new())?, "");
        Ok(())
    }

    #[test]
    fn the_one_where_non_json_also_passes_because_we_dont_validate() -> Result<()> {
        // 🧪 Passthrough doesn't parse. Doesn't validate. Doesn't care.
        let not_json = "this is not json and that's fine".to_string();
        assert_eq!(Passthrough.transform(not_json.clone())?, not_json);
        Ok(())
    }
}
