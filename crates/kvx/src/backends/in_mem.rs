//! # Previously, on Kravex...
//!
//! 🎬 The data was trapped. Stranded between two search engines like a traveler
//! stuck in a connecting airport with no WiFi and a dead phone. Someone had to
//! move it. Someone had to be brave. Someone had to write a backend so simple
//! it lives entirely in RAM, gone the moment you blink.
//!
//! That someone was this module.
//!
//! `in_mem` provides an in-memory [`Source`] and [`Sink`] for testing and local
//! development. The [`InMemorySource`] emits exactly one batch of hardcoded docs
//! and then, like my motivation on a Friday afternoon, yields nothing further.
//! The [`InMemorySink`] collects received batches behind an `Arc<Mutex<...>>`
//! so callers can inspect what arrived — great for assertions, great for trust
//! issues, great for both.
//!
//! 🦆
//!
//! ⚠️ This is NOT for production. This is for tests. If you're deploying this
//! to prod, please also deploy a therapist.
//!
//! ✅ No network calls. No disk I/O. No heartbeat. No mortgage on the line.
//! Just vibes and heap memory.

use anyhow::Result;
use async_trait::async_trait;

use crate::backends::{Sink, Source};
use crate::common::HitBatch;

/// 📦 The world's most optimistic data source.
///
/// `InMemorySource` is the "Hello, World!" of [`Source`] implementations.
/// It knows exactly four documents. They are `{"doc":1}` through `{"doc":4}`.
/// It has no opinions about them. It will give them to you once, and only once,
/// because it is a one-trick pony who has already done its trick.
///
/// 🎯 Designed entirely for testing. Not for feelings. Feelings are unindexed.
#[derive(Debug, Default)]
pub(crate) struct InMemorySource {
    /// 🔒 The virginity of this source — once yielded, forever yielded.
    /// Like watching a movie spoiler. Can't un-yield it.
    /// The borrow checker wished it could reject this concept. It could not.
    has_yielded: bool, // true = "I already gave you everything I had, please stop asking"
}

impl InMemorySource {
    /// 🚀 Constructs a new `InMemorySource` ready to disappoint exactly once.
    ///
    /// No I/O. No config. No environment variables lurking in the shadows.
    /// You call `new()`, you get a fresh source, hat tips are exchanged.
    /// It's async because we respect the trait contract, not because we need it.
    /// Ancient proverb: "He who makes everything async learns nothing, but ships faster."
    pub(crate) async fn new() -> Result<Self> {
        // ✅ No config to load, no server to ping, no prayers to send.
        // This is the most peaceful constructor in the entire codebase.
        // Cherish this moment.
        Ok(Self { has_yielded: false })
    }
}

#[async_trait]
impl Source for InMemorySource {
    /// 🎯 Returns the one and only batch this source will ever produce.
    ///
    /// Call it once: you get the goods. Call it again: empty batch, go home.
    /// It's like going to the snack cabinet after midnight — first time, jackpot.
    /// Second time, you're staring into an abyss that stares back.
    ///
    /// ⚠️ What's the DEAL with `has_yielded`? It's a boolean. A single boolean.
    /// This is the entire state machine. One field. One decision. One life.
    /// Seinfeld would have a bit about this and honestly he'd be right.
    ///
    /// The singularity will happen before we replace these hardcoded docs with
    /// real fixture loading logic. And that's fine. The singularity can deal with it.
    async fn next_batch(&mut self) -> Result<HitBatch> {
        // 🔒 Guard against second helpings. One batch per customer. This is Costco,
        // not a buffet. (Actually it's neither. It's RAM. But you get the idea.)
        if self.has_yielded {
            // 💀 Nothing left. The well is dry. The larder is bare.
            // The CI pipeline will at least get a clean empty batch, which is
            // more than I can say for my emotional availability on Mondays.
            return HitBatch::new(vec![]);
        }

        // ✅ First call — we commit. No cap, we are actually doing this.
        self.has_yielded = true;

        // 📦 Behold: the sacred test corpus. Four documents. FOUR.
        // {"doc":1}, {"doc":2}, {"doc":3}, {"doc":4}.
        // No title. No body. No `_source`. No dignity.
        // Whoever hardcoded these was either:
        //   a) in a hurry (relatable),
        //   b) writing a test and planning to come back (lol),
        //   c) a time traveler who knew tests don't care about real data.
        // The docs don't know they're fake. Please don't tell them.
        let hit_items = vec![
            String::from(r#"{"doc":1}"#), // Document 1: a classic. timeless. no metadata.
            String::from(r#"{"doc":2}"#), // Document 2: the sophomore slump. still no metadata.
            String::from(r#"{"doc":3}"#), // Document 3: the deep cut. fans only.
            String::from(r#"{"doc":4}"#), // Document 4: the finale. no arc. no resolution. just {"doc":4}.
        ];

        // 🚀 And we're off! All the velocity of a thumbtack rolling downhill.
        HitBatch::new(hit_items)
    }
}

/// 📦 A sink that never forgets. Unlike my dad, who forgot my soccer game in 1998.
///
/// `InMemorySink` receives [`HitBatch`]es and hoards them in a shared Vec
/// wrapped in a Mutex wrapped in an Arc. It's structs all the way down.
///
/// 🔒 The `Arc<Mutex<Vec<HitBatch>>>` is an existential nesting doll:
/// "I need to share ownership of a thing that must be accessed one thread at a
/// time and that thing is a list of other things." You ever just stare at a type
/// signature and feel the weight of every decision that led to this moment?
/// That's this field. Every. Single. Time.
///
/// Clone-able because tests need to peek inside after handing `self` off to the
/// pipeline. The `Arc` means everyone shares the same Vec. Communist data, but
/// in a good way. The borrow checker approved. Barely. It had notes.
#[derive(Debug, Default, Clone)]
pub(crate) struct InMemorySink {
    /// 🔒 The vault. The evidence locker. The "I told you I received that batch" proof.
    /// Arc so multiple owners can hold a reference. Mutex so only one panics at a time.
    /// Vec so we can have MORE than one batch, theoretically, if we're being ambitious.
    pub(crate) received: std::sync::Arc<tokio::sync::Mutex<Vec<HitBatch>>>,
}

impl InMemorySink {
    /// 🚀 Spins up a brand new sink, ready to absorb batches like a paper towel
    /// in a infomercial — except this one actually works and isn't $19.99 plus S&H.
    ///
    /// Conspiracy theory: `tokio::sync::Mutex` is just `std::sync::Mutex` wearing
    /// a trench coat to look taller in async contexts. I have no proof. I have
    /// strong feelings.
    pub(crate) async fn new() -> Result<Self> {
        // ✅ Birth of the sink. An empty Vec, full of potential, unmarred by batches.
        // This is the most hopeful a Vec will ever be. Downhill from here.
        Ok(Self {
            received: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
        })
    }
}

#[async_trait]
impl Sink for InMemorySink {
    /// 🚰 Receives a batch and lovingly shoves it into the Vec.
    ///
    /// No cap this function slaps fr fr. It locks, it pushes, it returns Ok(()).
    /// Three lines. Three acts. The hero's journey in O(1) amortized time.
    /// The borrow checker didn't even complain. We don't talk about why.
    ///
    /// 🎯 This is the whole job: take batch, store batch, go home.
    /// Some of us wish our jobs were this clear.
    async fn receive(&mut self, batch: HitBatch) -> Result<()> {
        // 🔒 Acquire the lock. Await the Mutex. Respect the concurrency gods.
        // This is the ONE place multiple async tasks might collide.
        // The Mutex is load-bearing. Do not remove. I know it looks optional. It isn't.
        self.received.lock().await.push(batch);

        // ✅ Did it. Pushed it. No drama.
        // "It works on my machine" — inscribed on the tombstone of many a developer.
        // But this? This actually works. On all machines. Probably.
        Ok(())
    }

    /// 🗑️ Closes the sink with all the ceremony of closing a browser tab.
    ///
    /// There is nothing to clean up. We live in RAM. When this drops, the OS
    /// reclaims everything faster than HR reclaims your badge on your last day.
    /// We don't hold file handles. We don't hold sockets. We hold batches and
    /// vibes, and the vibes are ref-counted.
    ///
    /// Dad joke mandatory by AGENTS.md section 4, paragraph "comedy density":
    /// Why did the in-memory sink go to therapy? It had trouble letting go.
    /// (The Arc kept bumping the ref count. It never actually dropped.)
    async fn close(&mut self) -> Result<()> {
        // 🗑️ Cleanup routine: [REDACTED — there is nothing here]
        // The singularity will have already occurred by the time we need real
        // teardown logic in an in-memory backend. We'll deal with it then.
        // The singularity can file a PR.
        Ok(())
    }
}
