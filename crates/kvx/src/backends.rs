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
use crate::common::HitBatch;

// ===== Source Trait and Backend Enum =====

/// 🚰 A source that produces hits.
///
/// Implement this trait and you too can be the origin of someone else's data problems.
/// Guaranteed to dispense only the finest organic, free-range, artisanal JSON.
///
/// # Contract
/// - `next_batch` returns hits until the well runs dry, at which point it returns an empty batch.
/// - What counts as "empty" is a philosophical question we've deferred to the implementor.
/// - The borrow checker demands `&mut self` because sources have state. And feelings. Mostly state.
#[async_trait]
pub(crate) trait Source: std::fmt::Debug {
    /// 📦 Fetch the next batch of hits from wherever the data lives.
    ///
    /// Returns `Ok(HitBatch)` while data flows. Returns an empty batch when the tap runs dry.
    /// Returns `Err(...)` when something has gone sideways, sidelong, or fully upside-down.
    async fn next_batch(&mut self) -> Result<HitBatch>;
}

pub(crate) mod in_mem_source;
pub(crate) mod in_mem_sink;
pub(crate) mod file_source;
pub(crate) mod file_sink;
pub(crate) mod elasticsearch_source;
pub(crate) mod elasticsearch_sink;

// 🎯 Re-export backend-specific configs so callers can do `backends::FileSourceConfig`
// instead of spelunking into `backends::file::FileSourceConfig`.
// Convenience is a feature. So is not typing "backends::file::" fourteen times per file.
pub(crate) use file_source::FileSourceConfig;
pub(crate) use file_sink::FileSinkConfig;
pub(crate) use elasticsearch_source::ElasticsearchSourceConfig;
pub(crate) use elasticsearch_sink::ElasticsearchSinkConfig;

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
    InMemory(in_mem_source::InMemorySource),
    File(file_source::FileSource),
    Elasticsearch(elasticsearch_source::ElasticsearchSource),
}

#[async_trait]
impl Source for SourceBackend {
    async fn next_batch(&mut self) -> Result<HitBatch> {
        match self {
            SourceBackend::InMemory(i) => i.next_batch().await,
            SourceBackend::File(f) => f.next_batch().await,
            SourceBackend::Elasticsearch(es) => es.next_batch().await,
        }
    }
}

// ===== Sink Trait and Backend Enum =====

/// 🕳️ A sink that consumes hits.
///
/// The yin to the source's yang. The drain at the bottom of the pipeline tub.
/// We promise not to drop (too many) of them — `close()` exists precisely
/// because some sinks buffer internally and need a moment to compose themselves
/// before the session ends. Like a contractor who needs five minutes to clean up
/// after a job. Except this one actually shows up.
///
/// # Contract
/// - `receive` accepts a batch and does something useful with it. Hopefully.
/// - `close` flushes, finalizes, and bids the data a fond farewell. MUST be called.
///   Skipping `close` is a bug. It is also considered rude.
#[async_trait]
pub(crate) trait Sink: std::fmt::Debug {
    /// 📥 Accept a batch of hits and write/forward/stash them somewhere meaningful.
    async fn receive(&mut self, batch: HitBatch) -> Result<()>;
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
    InMemory(in_mem_sink::InMemorySink),
    File(file_sink::FileSink),
    Elasticsearch(elasticsearch_sink::ElasticsearchSink),
}

#[async_trait]
impl Sink for SinkBackend {
    async fn receive(&mut self, batch: HitBatch) -> Result<()> {
        match self {
            SinkBackend::InMemory(sink) => sink.receive(batch).await,
            SinkBackend::File(sink) => sink.receive(batch).await,
            SinkBackend::Elasticsearch(sink) => sink.receive(batch).await,
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
