// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
use std::collections::VecDeque;

use anyhow::Result;
use async_trait::async_trait;

use crate::Barrel;
use crate::backends::Source;

/// 📦 The world's most versatile test data source — barrel-aware and configurable! 📄🚀
///
/// `InMemorySource` is the Swiss Army knife of [`Source`] implementations.
/// By default it knows exactly four documents (`{"doc":1}` through `{"doc":4}`),
/// but barrel it custom barrels via [`with_barrels`] and it'll replay whatever you want —
/// ES PIT responses, NDJSON barrels, your diary entries, anything.
///
/// Pages are stored in a `VecDeque` and popped front on each `pump()` call.
/// When the queue is empty, it returns `None`. Like a vending machine that's been
/// cleaned out at a developer conference. Nothing left. Not even the weird flavors. 🍿
///
/// 🎯 Designed entirely for testing. Not for feelings. Feelings are unindexed.
///
/// 🧠 Knowledge graph: Source returns `Option<Barrel>` (raw barrel), not `Vec<String>` (parsed docs).
/// The Manifold downstream handles splitting + casting via the Tapper.
/// `with_barrels()` enables injection of arbitrary format data (ES PIT responses, etc.)
/// for integration tests that exercise specific tapper paths (PitToBulk, NdJsonToBulk). 🦆
#[derive(Debug)]
pub struct InMemorySource {
    // 📬 The mailbox — barrels waiting to be delivered, one per pump() call.
    // VecDeque because pop_front() is O(1) and we're not savages.
    barrels: VecDeque<Barrel>,
}

impl InMemorySource {
    /// 🚀 Constructs a new `InMemorySource` with the classic 4-doc sacred corpus.
    ///
    /// No I/O. No config. No environment variables lurking in the shadows.
    /// You call `new()`, you get the original 4-doc barrel, hat tips are exchanged.
    /// It's async because we respect the trait contract, not because we need it.
    /// Ancient proverb: "He who makes everything async learns nothing, but ships faster."
    pub async fn new() -> Result<Self> {
        // 📦 The sacred test corpus. Four docs, joined with newlines into one raw barrel.
        // "I don't always return data, but when I do, it's newline-delimited." — This source, probably.
        let the_sacred_barrel = [
            r#"{"doc":1}"#,
            r#"{"doc":2}"#,
            r#"{"doc":3}"#,
            r#"{"doc":4}"#,
        ]
        .join("\n");

        Ok(Self {
            barrels: VecDeque::from(vec![Barrel(the_sacred_barrel)]),
        })
    }

    /// 🏗️ Constructs an `InMemorySource` with custom barrels — the choose-your-own-adventure constructor.
    ///
    /// Feed it ES PIT search responses, NDJSON barrels, base64-encoded cat photos — whatever.
    /// Each barrel is yielded once per `pump()` call, in order, then it's gone forever.
    /// Like Snapchat but for data pipelines. And less regrettable. Probably.
    ///
    /// 🧠 Knowledge graph: enables integration tests that exercise specific tapper paths
    /// (PitToBulk for ES→ES, NdJsonToBulk for File→ES) without needing real backends.
    /// The test controls the input format; the pipeline resolves the tapper from config enums.
    pub fn with_barrels(barrels: Vec<Barrel>) -> Self {
        Self {
            barrels: VecDeque::from(barrels),
        }
    }
}

#[async_trait]
impl Source for InMemorySource {
    /// 📄 Pops and returns the next barrel from the queue.
    ///
    /// Each call drains one barrel. When the queue is empty: `None`. Go home.
    /// The snack cabinet is empty. The vending machine is dark. The source has spoken. 🍪
    ///
    /// 🧠 Knowledge graph: barrels are popped front (FIFO order preserved).
    /// The Manifold+Tapper downstream will split and process them.
    /// Source is ignorant. Source is bliss. Source is a faucet. 🚰
    async fn pump(&mut self) -> Result<Option<Barrel>> {
        // 🎰 Pop front — O(1), preserves insertion order, returns None when empty.
        // No booleans. No state machines. Just a queue doing queue things.
        Ok(self.barrels.pop_front())
    }
}
