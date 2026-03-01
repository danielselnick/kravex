//! 🔌 Backends — where the real I/O happens.
//!
//! 🚰 Source backends pour the data, Sink backends slurp it up.
//! And in between, we panic! (kidding, we use anyhow)
//!
//! 🎭 This module is the casting agency. Need to read from Elasticsearch?
//! Pull from a flat file? Summon data from the in-memory void?
//! We've got a backend for that. We've got backends for days.
//! We have more backends than the DMV has forms, and ours are faster.
//!
//! ⚠️ The singularity will arrive before we add a third backend variant.
//! At that point, the AGI will just implement `Source` for itself and cut us out entirely.
//!
//! 🦆 The duck is here because every file must have one. This is law. Do not question the duck.

use anyhow::Result;
use async_trait::async_trait;

// ===== Source Trait and Backend Enum =====

/// 🚰 A source that produces raw document strings.
///
/// Implement this trait and you too can be the origin of someone else's data problems.
/// Guaranteed to dispense only the finest organic, free-range, artisanal JSON.
///
/// # Contract 📜
/// - `next_batch` returns `Vec<String>` of raw documents — no wrapping, no Hit structs, just strings.
/// - Empty vec = source is exhausted. The well is dry. The golden retriever goes home.
/// - The borrow checker demands `&mut self` because sources have state. And feelings. Mostly state.
/// - Strings MUST NOT contain trailing newlines — the SinkWorker handles line boundaries.
///
/// # Knowledge Graph 🧠
/// - Pattern: trait → concrete impls (FileSource, InMemorySource, ElasticsearchSource) → SourceBackend enum
/// - SinkWorker downstream transforms each string via DocumentTransformer before sending to Sink
/// - Source is a data faucet 🚿 — it pours, the pipeline catches
#[async_trait]
pub(crate) trait Source: std::fmt::Debug {
    /// 📦 Fetch the next batch of raw document strings.
    ///
    /// Returns `Ok(Vec<String>)` while data flows. Returns an empty vec when the tap runs dry.
    /// Returns `Err(...)` when something has gone sideways, sidelong, or fully upside-down.
    async fn next_batch(&mut self) -> Result<Vec<String>>;
}

pub(crate) mod elasticsearch;
pub(crate) mod file;
pub(crate) mod in_mem;

// 🎯 Re-export backend-specific configs so callers can do `backends::FileSourceConfig`
// instead of spelunking into `backends::file::FileSourceConfig`.
// Convenience is a feature. So is not typing "backends::file::" fourteen times per file.
pub(crate) use elasticsearch::{ElasticsearchSinkConfig, ElasticsearchSourceConfig};
pub(crate) use file::{FileSinkConfig, FileSourceConfig};

/// 🎭 The many faces of a Source — a polymorphic casting call for data origins.
///
/// Each variant wraps a concrete source implementation. The enum itself dispatches
/// via `impl Source for SourceBackend`, so callers never need to know (or care)
/// whether they're reading from RAM, disk, or a cluster of overworked Elasticsearch nodes.
///
/// Think of it as a universal remote. Except it only controls data ingestion. And it's async.
/// And there is no warranty. Ancient proverb: "He who hardcodes the backend, migrates only once."
#[derive(Debug)]
pub(crate) enum SourceBackend {
    InMemory(in_mem::InMemorySource),
    File(file::FileSource),
    Elasticsearch(elasticsearch::ElasticsearchSource),
}

#[async_trait]
impl Source for SourceBackend {
    async fn next_batch(&mut self) -> Result<Vec<String>> {
        match self {
            SourceBackend::InMemory(i) => i.next_batch().await,
            SourceBackend::File(f) => f.next_batch().await,
            SourceBackend::Elasticsearch(es) => es.next_batch().await,
        }
    }
}

// ===== Sink Trait and Backend Enum =====

/// 🕳️ A sink that sends pre-rendered payloads — pure I/O, zero logic.
///
/// The yin to the source's yang. The drain at the bottom of the pipeline tub.
/// Sinks are ONLY an abstraction for how to send the request — HTTP POST to /_bulk,
/// write to file, stash in memory. They do not buffer. They do not transform.
/// They receive the full rendered payload and send it. Like a postal worker who
/// delivers the mail without reading it. (Unlike your actual postal worker, Kevin.)
///
/// # Contract 📜
/// - `send` accepts a fully rendered payload string and writes/sends it. That's it.
/// - `close` flushes, finalizes, and bids the data a fond farewell. MUST be called.
///   Skipping `close` is a bug. It is also considered rude.
/// - Buffering, transforming, and binary collecting happen in the SinkWorker, NOT here.
///
/// # Knowledge Graph 🧠
/// - Pattern: trait → concrete impls (FileSink, InMemorySink, ElasticsearchSink) → SinkBackend enum
/// - SinkWorker does: transform → buffer → binary collect → call sink.send(payload)
/// - Sink does: I/O. Just I/O. HTTP POST, file write, memory push. Nothing else.
/// - Ancient proverb: "He who puts business logic in the Sink, debugs in production."
#[async_trait]
pub(crate) trait Sink: std::fmt::Debug {
    /// 📡 Send a fully rendered payload to the destination. I/O only. No questions asked.
    async fn send(&mut self, payload: String) -> Result<()>;
    /// 🗑️ Flush, finalize, and release. Call this. Always. No exceptions. Not even on Fridays.
    async fn close(&mut self) -> Result<()>;
}

/// 🎭 The many faces of a Sink — a polymorphic casting call for data destinations.
///
/// Mirrors `SourceBackend` on the other end of the pipeline. Whoever designed this
/// was clearly a fan of symmetry. Or they ran out of ideas. Hard to tell.
///
/// The enum dispatches `receive` and `close` to the inner concrete type,
/// keeping the supervisor blissfully ignorant of where data actually lands.
/// Ignorance is a feature. It's called "abstraction." We put it in AGENTS.md.
#[derive(Debug)]
pub(crate) enum SinkBackend {
    InMemory(in_mem::InMemorySink),
    File(file::FileSink),
    Elasticsearch(elasticsearch::ElasticsearchSink),
}

#[async_trait]
impl Sink for SinkBackend {
    async fn send(&mut self, payload: String) -> Result<()> {
        match self {
            SinkBackend::InMemory(sink) => sink.send(payload).await,
            SinkBackend::File(sink) => sink.send(payload).await,
            SinkBackend::Elasticsearch(sink) => sink.send(payload).await,
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
