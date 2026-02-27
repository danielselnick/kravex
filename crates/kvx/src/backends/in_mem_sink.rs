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

use crate::backends::Sink;
use crate::common::HitBatch;

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
