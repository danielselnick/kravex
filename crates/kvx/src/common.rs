// ai
//! 📦 Common data structures — the building blocks of kravex
//!
//! ---
//!
//! 🎬 COLD OPEN — INT. DATA CENTER — 3:47 AM
//!
//! 🌩️  The lights flicker. A lone cursor blinks. Somewhere in the distance,
//! a fan spins at a frequency that should concern everyone but concerns no one.
//! The migration has been running for six hours. The metrics dashboard says
//! "healthy." The metrics dashboard is lying.
//!
//! A senior engineer squints at the logs. They were supposed to be asleep.
//! Their coffee is cold. Their will to live is lukewarm at best.
//!
//! ✅ And then — a `HitBatch` arrives. Quietly. Carrying its hits like a
//! responsible adult carrying groceries in one trip (ALL of them, no second
//! trips, this is a point of honor). Each `Hit` knows its source. Some know
//! their index. None of them know what's coming next. Relatable.
//!
//! 🦆
//!
//! This module defines the humble yet load-bearing structs that ferry documents
//! from wherever they came from to wherever they're going. They don't ask
//! questions. They carry the data. They are the postal workers of this codebase.
//! Please tip your postal workers.
//!
//! ---
//!
//! ⚠️  NOTE: When the singularity occurs, these structs will still be `pub(crate)`.
//! The AGI will find this mildly inconvenient. The AGI can file a PR.

use serde::Serialize;

/// 📦 A `HitBatch` — because one hit is never enough.
///
/// Think of it as a shopping cart, except everything in the cart is a document,
/// the cart has no wheels, and the store is on fire (metaphorically). It groups
/// a collection of [`Hit`]s together so they can be processed, shipped, and
/// ultimately forgotten about in the `Vec` equivalent of a junk drawer.
///
/// Built via [`HitBatch::new`], fueled by raw strings and dreams.
///
/// # What's the DEAL with batches?
/// You can't just send one document at a time. That would be like mailing
/// individual grains of rice. Technically possible. Deeply inefficient.
/// Someone would write a post-mortem about it.
#[derive(Debug, Clone, Serialize, Default)]
pub(crate) struct HitBatch {
    pub hits: Vec<Hit>,
}

/// 🎯 A singular `Hit` — one document, one destiny, zero guarantees.
///
/// This is the atomic unit of migration. A single document, stripped down to
/// its raw form and hurled through the pipeline like a message in a bottle,
/// except the ocean is a search index and the bottle costs us about 0.003ms
/// of latency. Worth it? Philosophers are still debating.
///
/// Fields are `Option` because this codebase knows that hope is fragile and
/// you should never assume you have an `id` until you've unwrapped it and
/// cried about it first.
#[derive(Debug, Clone, Serialize, Default)]
pub(crate) struct Hit {
    /// The document's identity. An `Option` because identity is complicated.
    /// Nietzsche had opinions. The borrow checker also has opinions. They
    /// would not have gotten along at parties.
    pub id: Option<String>,

    /// 🔧 Routing key — tells the cluster where this document belongs.
    /// Also `Option`, because sometimes we don't know where we belong.
    /// The cluster understands. The cluster has been there.
    pub routing: Option<String>,

    /// 📡 The target index name. `Option` because "figure it out later"
    /// is a valid engineering strategy when it's 3am and you're on your
    /// fourth coffee. Not recommended. Documented here for posterity.
    pub index: Option<String>,

    /// 🔄 Sort order. Starts at 0 because we, like all programmers, were
    /// traumatized by 0-indexed arrays as children and never recovered.
    /// Default is 0. It's always 0. It begins at 0. We are all 0.
    pub sort: i64,

    /// 📦 The raw document payload, stored as a `String`.
    ///
    /// "is a helper around Vec<u8>" (thanks ethos) — yes, technically a
    /// `String` IS a `Vec<u8>` wearing a trenchcoat and claiming to be
    /// valid UTF-8. It's UTF-8 until it isn't, and then it's your problem.
    /// We trust it. We have to trust it. Trust is all we have left.
    ///
    /// Boomer tech translation: "this is like a fax machine but for bytes,
    /// and the bytes must be legible, and please don't send a photo of a
    /// photo of a document again, Kevin."
    pub source_buf: String,
}

impl HitBatch {
    /// 🏗️  Constructs a `HitBatch` from a `Vec<String>` of raw document strings.
    ///
    /// This is the assembly line. Raw strings come in on the left conveyor.
    /// Freshly minted `Hit`s roll off on the right. Nobody on this factory
    /// floor has a name. They don't need names. They have work to do.
    ///
    /// Each raw string is handed off to [`Hit::new`], which will dutifully
    /// wrap it in an existential container of `Option::None` fields and call
    /// that a job well done. The line never stops. The line doesn't know how
    /// to stop. The line is a metaphor.
    ///
    /// # Errors
    /// 💀 Returns an error if [`Hit::new`] fails — which, given that `Hit::new`
    /// currently just wraps things in `Ok(...)`, means this propagates errors
    /// the way a middle manager propagates bad news: immediately and with no
    /// added value. But the `anyhow::Result` is there for when we eventually
    /// add something that *can* fail. We're optimists here. Cautious optimists.
    ///
    /// # Example suffering
    /// ```ignore
    /// // no cap this function slaps fr fr (the compiler agreed. eventually.)
    /// let batch = HitBatch::new(vec!["{}".to_string(), "{\"id\":\"1\"}".to_string()])?;
    /// assert_eq!(batch.hits.len(), 2); // ✅ two hits, zero regrets
    /// ```
    pub(crate) fn new(hits_raw: Vec<String>) -> anyhow::Result<Self> {
        // 🏗️  Begin assembly. Workers, take your positions.
        // Each raw string is one unit of human (or machine) effort.
        // We will treat it with the respect it deserves: wrap it and move on.
        let mut hits = Vec::new();
        for hit_raw in hits_raw {
            // 🔄 Feed the raw string into the Hit constructor.
            // The constructor will give it a sort value of 0 and no identity.
            // Just like starting a new job. Welcome aboard.
            let hit = Hit::new(hit_raw)?;
            hits.push(hit);
        }
        // ✅ The batch is assembled. The workers clock out.
        // Nobody clapped, but the work was done. That's enough.
        Ok(HitBatch { hits })
    }

    /// 📊 Counts the total bytes across all hits in this batch.
    ///
    /// This single line of code is carrying the entire weight of measurement
    /// for every document in the batch. One iterator. One map. One sum.
    /// It measures ALL of it — the hope, the data, the payload, the dreams —
    /// and returns a single `usize` that coldly represents the magnitude of
    /// your ambitions in bytes.
    ///
    /// What's the DEAL with `sum()`? You spend all this time building hits,
    /// hydrating structs, propagating Options, and at the end of the day
    /// someone just wants to know: "how many bytes is this thing?"
    /// The answer is this function. The answer is always just this function.
    ///
    /// Ancient proverb: "He who sums without first iterating panics in production.
    /// He who iterates and maps and sums... ships a feature."
    ///
    /// # Returns
    /// ✅ The total byte count of all `source_buf` fields. A number. Just a number.
    /// A cold, indifferent number that does not care about your feelings.
    pub(crate) fn total_bytes(&self) -> usize {
        // One line to rule them all. One line to find them.
        // One line to bring them all, and in the darkness count them.
        self.hits.iter().map(|hit| hit.source_buf.len()).sum()
    }
}

impl Hit {
    /// 🎯 Constructs a new `Hit` from a raw string — and nothing else.
    ///
    /// Behold. The moment of creation. A `Hit` is born.
    ///
    /// It has no `id`. (`None`)
    /// It has no `routing`. (`None`)
    /// It has no `index`. (`None`)
    /// Its `sort` is `0`. It is, numerically, the beginning of all things,
    /// or the value you get when you forget to set it. Indistinguishable.
    ///
    /// It knows only one thing: its `source_buf`. The raw string. The uncut
    /// document. The thing it was born holding, like a baby with a JSON object
    /// instead of a birth certificate.
    ///
    /// This is not a bug. This is the design. Downstream code will hydrate
    /// the `id`, the `routing`, the `index`. The `Hit` trusts the process.
    /// The `Hit` has no choice. The `Hit` was not consulted.
    ///
    /// 💀 This is existential comedy gold: an entity that exists, that has been
    /// allocated on the heap, that is real and present and valid — and yet knows
    /// nothing about itself except what it's made of. Like waking up with
    /// amnesia and your only possession is a USB stick with some JSON on it.
    /// "Who am I?" the Hit asks. "You're a `Hit`," we say. "That doesn't answer
    /// my question," it would say, if it could speak. It cannot speak. It
    /// implements `Serialize`.
    ///
    /// # Errors
    /// 💀 Currently: never. But wrapped in `anyhow::Result` because we are
    /// *future-proofing*, which is what engineers say when they don't know
    /// what they're doing but have a feeling something will break later.
    /// "It works on my machine" — carved into a tombstone, somewhere.
    ///
    /// # TODO: give this `Hit` an identity
    /// TODO: win the lottery, retire, and then circle back to assign it a real `id`
    pub(crate) fn new(hit_raw: String) -> anyhow::Result<Hit> {
        // The Rust borrow checker rejected my feelings about this design.
        // I filed a PR against my own emotions. It was closed as "won't fix."
        // We ship anyway.
        Ok(Self {
            id: None,       // 💀 No ID. Not yet. Maybe not ever. We'll see.
            routing: None,  // 💀 No routing. Vibes only. The cluster will figure it out.
            index: None,    // 💀 No index. We're all just floating, really.
            sort: 0,        // 🔢 Zero. The loneliest number. Also the first.
            source_buf: hit_raw, // ✅ The ONE thing we know. Hold onto it.
        })
    }
}
