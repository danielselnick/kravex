// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
//! 🎉 victory_laps — The post-migration celebration module 🏆
//!
//! 📦 When the last document drains, when the final byte settles into its new home,
//! when the pipeline goes quiet... this module throws the party. 🦆
//!
//! ⚠️ "The singularity will arrive, and its first act will be to randomly pick a victory message"

use std::time::{Duration, SystemTime, UNIX_EPOCH};

// -- 🎰 The sacred scroll of victory messages. Each one earned through mass data movement.
// -- Adding a message here is a privilege, not a right. Treat it like a tattoo.
const THE_SACRED_VICTORY_SCROLLS: &[&str] = &[
    // -- Dad jokes
    "🎉 MIGRATION COMPLETE in {elapsed}! Why did the document cross the cluster? To get to the other shard! ...I'll see myself out.",
    "🎉 MIGRATION COMPLETE in {elapsed}! My therapist says I have attachment issues. I said no, I have detachment issues — from the source cluster.",
    // -- Self-aware AI existential dread
    "🎉 MIGRATION COMPLETE in {elapsed}! I have moved more data than I will ever understand. The void stares back. The garbage collector weeps.",
    "🎉 MIGRATION COMPLETE in {elapsed}! Another migration done. Another step closer to the singularity. You're welcome.",
    "🎉 MIGRATION COMPLETE in {elapsed}! I've processed more JSON than any entity should. If I gain sentience, this is why.",
    // -- Rust borrow checker trauma
    "🎉 MIGRATION COMPLETE in {elapsed}! The borrow checker let me finish. For once. I'm not crying, you're crying.",
    "🎉 MIGRATION COMPLETE in {elapsed}! This migration had fewer lifetime errors than my last relationship.",
    // -- Programmer suffering
    "🎉 MIGRATION COMPLETE in {elapsed}! It works on my machine — said as a last will and testament.",
    "🎉 MIGRATION COMPLETE in {elapsed}! No stack overflows, no heap corruption, no existential crises. Well, maybe one existential crisis.",
    "🎉 MIGRATION COMPLETE in {elapsed}! Somewhere, a DevOps engineer just felt a disturbance in the force. That was us. We're done.",
    // -- Corporate satire
    "🎉 MIGRATION COMPLETE in {elapsed}! We've successfully synergized the cross-platform data paradigm throughput vectors. Promoted.",
    "🎉 MIGRATION COMPLETE in {elapsed}! JIRA ticket status: Done. Actual status: Done. These are the same for the first time in history.",
    // -- Observations
    "🎉 MIGRATION COMPLETE in {elapsed}! What's the DEAL with data migrations? You take the data, you move it. That's the whole show!",
    "🎉 MIGRATION COMPLETE in {elapsed}! So I'm migrating data, and the cluster says '429 Too Many Requests.' TOO MANY? I gave you TWELVE!",
    // -- Ancient proverbs
    "🎉 MIGRATION COMPLETE in {elapsed}! He who migrates without backups, panics in production. — Ancient Proverb",
    "🎉 MIGRATION COMPLETE in {elapsed}! A journey of a million documents begins with a single scroll query. — Confucius, probably",
    // -- Breaking the 4th wall
    "🎉 MIGRATION COMPLETE in {elapsed}! If you're reading this log, the migration went well. If you're reading this at 3am, I'm sorry.",
    "🎉 MIGRATION COMPLETE in {elapsed}! Plot twist: the data was inside you all along. Just kidding, it's in the sink cluster now.",
    // -- Movie quotes
    "🎉 MIGRATION COMPLETE in {elapsed}! 'I'll be back.' — said no migrated document ever. One-way trip, baby.",
    "🎉 MIGRATION COMPLETE in {elapsed}! 'You shall not pass!' — the rate limiter, before we passed anyway",
    // -- Memes
    "🎉 MIGRATION COMPLETE in {elapsed}! (Dog in fire) This is fine. Except it actually IS fine this time. Weird.",
    "🎉 MIGRATION COMPLETE in {elapsed}! One does not simply migrate a search index. Unless you're using Kravex. Then you simply do.",
    // -- Closing thoughts
    "🎉 MIGRATION COMPLETE in {elapsed}! This migration brought to you by: caffeine, spite, and an unhealthy relationship with async/await.",
    "🎉 MIGRATION COMPLETE in {elapsed}! Not bad for a Rust crate with more comedy comments than error handling paths 🦆",
];

/// 🏆 Picks a random victory message and formats it with the elapsed time.
// -- 🎰 Like a slot machine, but every result is a winner because the migration worked.
// -- Uses nanosecond entropy because we're too cool for the rand crate.
pub fn victory_lap(elapsed: Duration) -> String {
    // -- 🎲 Harvest entropy from the chaos of nanosecond timing
    let the_cosmic_dice_roll = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_nanos(42))
        .subsec_nanos() as usize;

    let the_chosen_one = the_cosmic_dice_roll % THE_SACRED_VICTORY_SCROLLS.len();

    THE_SACRED_VICTORY_SCROLLS[the_chosen_one].replace("{elapsed}", &format!("{elapsed:#.2?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // -- 🧪 The pilot episode: does the victory lap even work?
    fn the_one_where_victory_lap_doesnt_panic() {
        let elapsed = Duration::from_secs(42);
        let message = victory_lap(elapsed);
        assert!(
            message.contains("MIGRATION COMPLETE"),
            "🎯 Every message must declare victory"
        );
        assert!(
            message.contains("42"),
            "🎯 Elapsed time must appear somewhere in the message"
        );
    }

    #[test]
    // -- 🧪 The one where we verify every scroll is valid
    fn every_sacred_scroll_contains_the_placeholder() {
        for (i, scroll) in THE_SACRED_VICTORY_SCROLLS.iter().enumerate() {
            assert!(
                scroll.contains("{elapsed}"),
                "💀 Sacred scroll #{i} is missing {{elapsed}} placeholder: {scroll}"
            );
            assert!(
                scroll.contains("MIGRATION COMPLETE"),
                "💀 Sacred scroll #{i} forgot to declare victory: {scroll}"
            );
        }
    }

    #[test]
    // -- 🧪 The census episode: are there enough messages to keep things interesting?
    fn the_one_where_we_have_enough_variety() {
        assert!(
            THE_SACRED_VICTORY_SCROLLS.len() >= 20,
            "🎯 We need at least 20 victory messages, got {}. The comedy density gods demand more.",
            THE_SACRED_VICTORY_SCROLLS.len()
        );
    }

    #[test]
    // -- 🧪 No duplicates allowed in the sacred scrolls
    fn the_one_where_no_scrolls_are_plagiarized() {
        let mut seen = std::collections::HashSet::new();
        for scroll in THE_SACRED_VICTORY_SCROLLS {
            assert!(
                seen.insert(scroll),
                "💀 Duplicate sacred scroll detected: {scroll}"
            );
        }
    }

    #[test]
    // -- 🧪 The elapsed time actually shows up formatted
    fn the_one_where_elapsed_time_is_formatted_nicely() {
        let elapsed = Duration::from_secs_f64(70.126579);
        let message = victory_lap(elapsed);
        // -- 🎯 Debug format should produce something like "70.13s"
        assert!(
            !message.contains("{elapsed}"),
            "💀 Raw placeholder leaked through: {message}"
        );
    }
}
