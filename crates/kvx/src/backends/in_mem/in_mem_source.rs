use std::collections::VecDeque;

use anyhow::Result;
use async_trait::async_trait;

use crate::Page;
use crate::backends::Source;

/// 📦 The world's most versatile test data source — feed-aware and configurable! 📄🚀
///
/// `InMemorySource` is the Swiss Army knife of [`Source`] implementations.
/// By default it knows exactly four documents (`{"doc":1}` through `{"doc":4}`),
/// but feed it custom pages via [`with_pages`] and it'll replay whatever you want —
/// ES PIT responses, NDJSON feeds, your diary entries, anything.
///
/// Pages are stored in a `VecDeque` and popped front on each `pump()` call.
/// When the queue is empty, it returns `None`. Like a vending machine that's been
/// cleaned out at a developer conference. Nothing left. Not even the weird flavors. 🍿
///
/// 🎯 Designed entirely for testing. Not for feelings. Feelings are unindexed.
///
/// 🧠 Knowledge graph: Source returns `Option<Page>` (raw feed), not `Vec<String>` (parsed docs).
/// The Manifold downstream handles splitting + casting via the Caster.
/// `with_pages()` enables injection of arbitrary format data (ES PIT responses, etc.)
/// for integration tests that exercise specific caster paths (PitToBulk, NdJsonToBulk). 🦆
#[derive(Debug)]
pub struct InMemorySource {
    // 📬 The mailbox — pages waiting to be delivered, one per pump() call.
    // VecDeque because pop_front() is O(1) and we're not savages.
    pages: VecDeque<Page>,
}

impl InMemorySource {
    /// 🚀 Constructs a new `InMemorySource` with the classic 4-doc sacred corpus.
    ///
    /// No I/O. No config. No environment variables lurking in the shadows.
    /// You call `new()`, you get the original 4-doc page, hat tips are exchanged.
    /// It's async because we respect the trait contract, not because we need it.
    /// Ancient proverb: "He who makes everything async learns nothing, but ships faster."
    pub async fn new() -> Result<Self> {
        // 📦 The sacred test corpus. Four docs, joined with newlines into one raw feed.
        // "I don't always return data, but when I do, it's newline-delimited." — This source, probably.
        let the_sacred_page = [
            r#"{"doc":1}"#,
            r#"{"doc":2}"#,
            r#"{"doc":3}"#,
            r#"{"doc":4}"#,
        ]
        .join("\n");

        Ok(Self {
            pages: VecDeque::from(vec![Page(the_sacred_page)]),
        })
    }

    /// 🏗️ Constructs an `InMemorySource` with custom pages — the choose-your-own-adventure constructor.
    ///
    /// Feed it ES PIT search responses, NDJSON feeds, base64-encoded cat photos — whatever.
    /// Each page is yielded once per `pump()` call, in order, then it's gone forever.
    /// Like Snapchat but for data pipelines. And less regrettable. Probably.
    ///
    /// 🧠 Knowledge graph: enables integration tests that exercise specific caster paths
    /// (PitToBulk for ES→ES, NdJsonToBulk for File→ES) without needing real backends.
    /// The test controls the input format; the pipeline resolves the caster from config enums.
    pub fn with_pages(pages: Vec<Page>) -> Self {
        Self {
            pages: VecDeque::from(pages),
        }
    }
}

#[async_trait]
impl Source for InMemorySource {
    /// 📄 Pops and returns the next page from the queue.
    ///
    /// Each call drains one page. When the queue is empty: `None`. Go home.
    /// The snack cabinet is empty. The vending machine is dark. The source has spoken. 🍪
    ///
    /// 🧠 Knowledge graph: pages are popped front (FIFO order preserved).
    /// The Manifold+Caster downstream will split and process them.
    /// Source is ignorant. Source is bliss. Source is a faucet. 🚰
    async fn pump(&mut self) -> Result<Option<Page>> {
        // 🎰 Pop front — O(1), preserves insertion order, returns None when empty.
        // No booleans. No state machines. Just a queue doing queue things.
        Ok(self.pages.pop_front())
    }
}
