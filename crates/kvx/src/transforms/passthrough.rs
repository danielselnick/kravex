// ai
//! 🚶 Passthrough Transform — zero-copy, zero-effort, zero apologies 🔄✈️
//!
//! 🎬 COLD OPEN — INT. AIRPORT — TSA PRECHECK LINE — 6:00 AM
//!
//! Everyone else is in line. Shoes off. Laptops out. Dignity abandoned.
//! But not you. You have PreCheck. You walk through. You don't stop.
//! You don't unpack. You don't even slow down. The scanner beeps.
//! Nobody cares. You're already at the gate.
//!
//! That's this function. Data comes in. Data goes out. Nothing changes.
//! No parsing. No reformatting. No allocation. The `String` ownership
//! transfers directly. `Ok(raw)` — three characters of pure efficiency.
//!
//! ## Knowledge Graph 🧠
//! - Pair: `RawJson → RawJson`, `RawJson → JsonLines`
//! - Resolved via: `DocumentTransformer::Passthrough`
//! - Allocation: zero (ownership transfer of the input `String`)
//! - CPU cost: one function call, one `Ok` wrapper, one return
//! - Used for: file-to-file copies, testing, benchmarking raw throughput
//!
//! ⚠️ The singularity will look at this function and say "I could have
//! written that." Yes. Yes it could have. That's the point. 🦆

use anyhow::Result;

/// 🚶 Pass data through unchanged. Zero-copy. Zero-effort.
///
/// Takes ownership of the input `String` and returns it as-is.
/// The compiler is encouraged to inline this into nothingness.
/// No parsing. No serde. No allocation. Just vibes.
///
/// "I used to have an intermediate format. Then I took an `Ok(raw)` to the knee."
///
/// # When to use 🤔
/// - File → File copies where format conversion isn't needed
/// - Testing the pipeline without transform overhead
/// - Benchmarking to prove the transform ISN'T the bottleneck (it never is)
/// - When you don't care what the JSON looks like, you just want it THERE
#[inline]
pub(crate) fn transform(raw: String) -> Result<String> {
    // -- 🚶 And just like that... it's done. Three characters of implementation.
    // -- The function is the code equivalent of a glass of water:
    // -- transparent, essential, and wildly underappreciated.
    // -- "What do you do?" "I return the input." "That's it?" "That's everything."
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_where_passthrough_is_literally_the_identity_function() -> Result<()> {
        // 🧪 f(x) = x. That's it. That's the test.
        // If this fails, mathematics is broken and we have bigger problems.
        let the_sacred_input = r#"{"untouched":"perfection","vibes":"immaculate"}"#.to_string();
        let the_output = transform(the_sacred_input.clone())?;
        assert_eq!(
            the_output, the_sacred_input,
            "Passthrough must return input unchanged. This is not negotiable."
        );
        Ok(())
    }

    #[test]
    fn the_one_where_empty_string_passes_through() -> Result<()> {
        // 🧪 Empty string? Still valid. Still passes through.
        // Nature abhors a vacuum, but passthrough doesn't judge.
        let the_void = String::new();
        let the_output = transform(the_void.clone())?;
        assert_eq!(the_output, the_void);
        Ok(())
    }

    #[test]
    fn the_one_where_complex_json_survives_untouched() -> Result<()> {
        // 🧪 Nested objects, arrays, emoji, unicode — all must survive.
        let the_complex_beast = r#"{"nested":{"deep":{"deeper":true}},"array":[1,2,3],"emoji":"🦆","unicode":"日本語"}"#.to_string();
        let the_output = transform(the_complex_beast.clone())?;
        assert_eq!(the_output, the_complex_beast, "Complex JSON must pass through byte-identical");
        Ok(())
    }

    #[test]
    fn the_one_where_non_json_also_passes_through_because_we_dont_validate() -> Result<()> {
        // 🧪 Passthrough doesn't parse. It doesn't validate. It doesn't care.
        // You could send it your diary entry and it would return it unchanged.
        // This is a feature, not a bug.
        let not_json = "this is definitely not json and we're fine with that".to_string();
        let the_output = transform(not_json.clone())?;
        assert_eq!(the_output, not_json);
        Ok(())
    }
}
