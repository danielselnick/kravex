// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
use anyhow::Result;
use async_trait::async_trait;

use crate::Drum;
use crate::backends::{elasticsearch, file, in_mem};

/// 🕳️ A sink that sends pre-rendered drums — pure I/O, zero logic.
///
/// The yin to the source's yang. The drain at the bottom of the pipeline tub.
/// Sinks are ONLY an abstraction for how to send the request — HTTP POST to /_bulk,
/// write to file, stash in memory. They do not buffer. They do not cast.
/// They receive the full rendered drum and send it. Like a postal worker who
/// delivers the mail without reading it. (Unlike your actual postal worker, Kevin.)
///
/// # Contract 📜
/// - `drain` accepts a fully rendered drum string and writes/sends it. That's it.
/// - `close` flushes, finalizes, and bids the data a fond farewell. MUST be called.
///   Skipping `close` is a bug. It is also considered rude.
/// - Buffering, casting, and binary collecting happen in the Drainer, NOT here.
///
/// # Knowledge Graph 🧠
/// - Pattern: trait → concrete impls (FileSink, InMemorySink, ElasticsearchSink) → SinkBackend enum
/// - Drainer does: cast → buffer → binary collect → call sink.drain(drum)
/// - Sink does: I/O. Just I/O. HTTP POST, file write, memory push. Nothing else.
/// - Ancient proverb: "He who puts business logic in the Sink, debugs in production."
#[async_trait]
pub trait Sink: std::fmt::Debug {
    /// 📡 Drain a fully rendered drum to the destination. I/O only. No questions asked.
    async fn drain(&mut self, drum: Drum) -> Result<()>;
    /// 🗑️ Flush, finalize, and release. Call this. Always. No exceptions. Not even on Fridays.
    async fn close(&mut self) -> Result<()>;
}

/// 🎭 The many faces of a Sink — a polymorphic casting call for data destinations.
///
/// Mirrors `SourceBackend` on the other end of the pipeline. Whoever designed this
/// was clearly a fan of symmetry. Or they ran out of ideas. Hard to tell.
///
/// The enum dispatches `drain` and `close` to the inner concrete type,
/// keeping the supervisor blissfully ignorant of where data actually lands.
/// Ignorance is a feature. It's called "abstraction." We put it in AGENTS.md.
#[derive(Debug)]
pub enum SinkBackend {
    InMemory(in_mem::InMemorySink),
    File(file::FileSink),
    Elasticsearch(elasticsearch::ElasticsearchSink),
}

#[async_trait]
impl Sink for SinkBackend {
    async fn drain(&mut self, drum: Drum) -> Result<()> {
        match self {
            SinkBackend::InMemory(sink) => sink.drain(drum).await,
            SinkBackend::File(sink) => sink.drain(drum).await,
            SinkBackend::Elasticsearch(sink) => sink.drain(drum).await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            SinkBackend::InMemory(sink) => sink.close().await,
            SinkBackend::File(sink) => sink.close().await,
            SinkBackend::Elasticsearch(sink) => sink.close().await,
        }
    }
}
