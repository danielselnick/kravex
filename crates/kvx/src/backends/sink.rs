use anyhow::Result;
use async_trait::async_trait;

use crate::backends::{elasticsearch, file, in_mem};

/// 🕳️ A sink that sends pre-rendered payloads — pure I/O, zero logic.
///
/// The yin to the source's yang. The drain at the bottom of the pipeline tub.
/// Sinks are ONLY an abstraction for how to send the request — HTTP POST to /_bulk,
/// write to file, stash in memory. They do not buffer. They do not cast.
/// They receive the full rendered payload and send it. Like a postal worker who
/// delivers the mail without reading it. (Unlike your actual postal worker, Kevin.)
///
/// # Contract 📜
/// - `send` accepts a fully rendered payload string and writes/sends it. That's it.
/// - `close` flushes, finalizes, and bids the data a fond farewell. MUST be called.
///   Skipping `close` is a bug. It is also considered rude.
/// - Buffering, casting, and binary collecting happen in the Drainer, NOT here.
///
/// # Knowledge Graph 🧠
/// - Pattern: trait → concrete impls (FileSink, InMemorySink, ElasticsearchSink) → SinkBackend enum
/// - Drainer does: cast → buffer → binary collect → call sink.send(payload)
/// - Sink does: I/O. Just I/O. HTTP POST, file write, memory push. Nothing else.
/// - Ancient proverb: "He who puts business logic in the Sink, debugs in production."
#[async_trait]
pub trait Sink: std::fmt::Debug {
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
pub enum SinkBackend {
    InMemory(in_mem::InMemorySink),
    File(file::FileSink),
    Elasticsearch(elasticsearch::ElasticsearchSink),
    /// 🧪 A sink with trust issues — fails N times before grudgingly succeeding.
    /// Test-only. If this shows up in production, someone has made a series of
    /// regrettable life choices. 🦆
    #[cfg(test)]
    FlakyMock(flaky_mock::FlakySink),
}

/// 🧪 Test-only module: a sink that fails on purpose. Like a coworker who "didn't
/// get the email" but definitely got the email. 📧
#[cfg(test)]
pub mod flaky_mock {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// 🎭 A sink that fails `failures_remaining` times, then succeeds.
    /// Tracks how many payloads it successfully received, for assertions.
    /// Named after every microservice dependency you've ever had.
    #[derive(Debug, Clone)]
    pub struct FlakySink {
        /// 🔢 Countdown to cooperation — each send() decrements until 0, then it plays nice
        pub failures_remaining: Arc<AtomicU32>,
        /// 📊 How many payloads actually made it through — proof of life
        pub successful_sends: Arc<AtomicU32>,
    }

    impl FlakySink {
        /// 🏗️ Create a new flaky sink that will fail `fail_count` times before succeeding.
        /// Like training a puppy — it takes patience and repeated attempts.
        pub fn new(fail_count: u32) -> Self {
            Self {
                failures_remaining: Arc::new(AtomicU32::new(fail_count)),
                successful_sends: Arc::new(AtomicU32::new(0)),
            }
        }
    }

    #[async_trait]
    impl Sink for FlakySink {
        async fn send(&mut self, _payload: String) -> Result<()> {
            let remaining = self.failures_remaining.load(Ordering::Relaxed);
            if remaining > 0 {
                self.failures_remaining.fetch_sub(1, Ordering::Relaxed);
                anyhow::bail!(
                    "💀 FlakySink says 'nah' ({} failures left). Like a vending machine \
                     that eats your dollar — try again, sucker.",
                    remaining - 1
                );
            }
            self.successful_sends.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }
}

#[async_trait]
impl Sink for SinkBackend {
    async fn send(&mut self, payload: String) -> Result<()> {
        match self {
            SinkBackend::InMemory(sink) => sink.send(payload).await,
            SinkBackend::File(sink) => sink.send(payload).await,
            SinkBackend::Elasticsearch(sink) => sink.send(payload).await,
            #[cfg(test)]
            SinkBackend::FlakyMock(sink) => sink.send(payload).await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            SinkBackend::InMemory(sink) => sink.close().await,
            SinkBackend::File(sink) => sink.close().await,
            SinkBackend::Elasticsearch(sink) => sink.close().await,
            #[cfg(test)]
            SinkBackend::FlakyMock(sink) => sink.close().await,
        }
    }
}
