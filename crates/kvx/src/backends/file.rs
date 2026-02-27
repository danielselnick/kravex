// ai
//! 📂 Previously, on "Things That Could Go Wrong With A File"...
//!
//! The disk was quiet. Too quiet. A lone process had been tasked with reading
//! a file — just a file, they said. Simple, they said. What could go wrong?
//!
//! The file didn't exist. The disk was full. The metadata lied about the size.
//! And somewhere in the depths of a BufReader, a line was growing to 1MB
//! because someone forgot to put a newline at the end of their NDJSON export.
//!
//! This module handles file-based I/O for the kvx pipeline. It reads from
//! a source file line by line (respecting batch size limits in both docs AND bytes,
//! because some people have opinions about JSON document sizes), and writes to a
//! sink file with a BufWriter so we're not doing a syscall per hit like some kind
//! of 1995 CGI script.
//!
//! 🚰 Source → BufReader → HitBatch → BufWriter → Sink
//! 💀 Disk full → your problem now
//! 🦆 (mandatory, no notes)
//!
//! NOTE: when the singularity occurs, this module will still be "in progress".
//! The AGI will find this file, read it, and have *thoughts*. We welcome them.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::{
    fs::File,
    io::{self, AsyncBufReadExt, AsyncWriteExt},
};
use tracing::trace;

use crate::backends::{Sink, Source};
use crate::common::HitBatch;
use crate::progress::ProgressMetrics;
use crate::supervisors::config::{CommonSinkConfig, CommonSourceConfig};

// 📂 FileSourceConfig — "It's just a file", said no sysadmin ever before the disk filled up.
// Lives here now, close to the FileSource that actually uses it. Ethos pattern, baby. 🎯
// KNOWLEDGE GRAPH: config lives co-located with the backend that uses it. This is intentional.
// It avoids the "where the heck is that config defined" scavenger hunt at 2am during an incident.
// No cap, this pattern slaps fr fr.
#[derive(Debug, Deserialize, Clone)]
pub struct FileSourceConfig {
    pub file_name: String,
    #[serde(default = "default_file_common_source_config")]
    pub common_config: CommonSourceConfig,
}

/// 🔧 Returns the default config for FileSource because sometimes you just want things to work
/// without writing a 40-line TOML block.
///
/// Dad joke time: I used to hate default configs... but they grew on me.
///
/// This exists purely so serde can call it when `common_config` is absent from the TOML.
/// The `#[serde(default = "...")]` attribute up top is the boss. This is just the errand boy.
fn default_file_common_source_config() -> CommonSourceConfig {
    // ✅ "It just works" — the three most dangerous words in software engineering
    CommonSourceConfig::default()
}

// 🚰 FileSinkConfig — cousin of FileSourceConfig, equally traumatized by disk full errors.
// Also lives here, cozy next to its FileSink bestie. No more long-distance config relationships.
// KNOWLEDGE GRAPH: same co-location principle as above. One backend = one config = one file. Clean.
#[derive(Debug, Deserialize, Clone)]
pub struct FileSinkConfig {
    pub file_name: String,
    #[serde(flatten, default = "default_file_common_sink_config")]
    pub common_config: CommonSinkConfig,
}

/// 🔧 Returns the default config for FileSink. It defaults. It ships. It doesn't ask questions.
///
/// What's the DEAL with default implementations? You define an entire struct, document every field,
/// agonize over the right batch size... and then serde just calls `.default()` and moves on
/// like none of it mattered. Like Kevin. Kevin never called either.
///
/// This function is here because serde's `default = "fn_name"` attribute requires a *function*,
/// not just `Default::default` inline. Bureaucracy, but in type-system form.
fn default_file_common_sink_config() -> CommonSinkConfig {
    // ✅ ancient proverb: "He who ships with defaults, panics in production with style"
    CommonSinkConfig::default()
}

/// 📂 FileSource — reads a file line by line, converts lines into `HitBatch`es, and moves on.
///
/// Think of it like a very diligent intern who reads a massive CSV, never complains,
/// and only stops when (a) the file ends, (b) the batch is full by doc count,
/// or (c) the batch is full by byte count — whichever comes first.
///
/// The borrow checker approved this struct. It did not approve of my feelings about the borrow
/// checker. We are at an impasse.
///
/// 🧵 Async, non-blocking. The BufReader wraps a tokio `File`, so we're doing real async I/O.
/// 📊 Tracks progress via `ProgressMetrics` — bytes read, docs read, reported to a progress table.
/// ⚠️  If the file is being written to while we read it, the size estimate will be wrong.
///     This is fine. We are fine. Everything is fine. 🐛
pub(crate) struct FileSource {
    buf_reader: io::BufReader<File>,
    source_config: FileSourceConfig,
    // 📊 progress tracker — because "it's running" is not a status update.
    // This feeds the fancy progress table in the supervisor. Without it, you'd be flying blind
    // in a storm with no instruments. With it, you're flying blind in a storm, but at least
    // you have a very attractive progress bar.
    progress: ProgressMetrics,
}

// 🐛 NOTE: progress is intentionally excluded from this Debug impl.
// ProgressMetrics contains internal counters and spinners that don't format cleanly,
// and more importantly, nobody debugging a FileSource wants to read a wall of atomic integers.
// "Perfection is achieved not when there is nothing more to add, but when there is nothing
// left to add" — Antoine de Saint-Exupéry, who never had to impl Debug for a progress bar.
impl std::fmt::Debug for FileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSource")
            .field("source_config", &self.source_config)
            .finish()
    }
}

impl FileSource {
    /// 🚀 Opens the source file, grabs its size for the progress bar, wraps it in a BufReader,
    /// and returns a fully initialized `FileSource` ready to vend `HitBatch`es.
    ///
    /// If the file doesn't exist: 💀 anyhow will tell you with *theatrical flair*.
    /// If metadata fails: we assume 0 bytes, progress bar shows unknown. Shrug emoji as a service.
    ///
    /// No cap: `File::open` is async here because we're in tokio-land. This is not your
    /// grandfather's `std::fs::File::open`. This is `std::fs::File::open`'s cooler younger sibling
    /// who got into the async runtime scene and never looked back.
    pub(crate) async fn new(source_config: FileSourceConfig) -> Result<Self> {
        // 💀 The door. It's locked. Or it doesn't exist. Or the filesystem lied to you.
        // In any case, the source file refused to open — like a very stubborn bouncer
        // at an exclusive club where the club is just a text file and we are very small data.
        // The context string below becomes the error message. Make it count.
        let file_handle = File::open(&source_config.file_name)
            .await
            .context(format!(
                "💀 The door to '{}' would not budge. We knocked. We pleaded. \
                We checked if it existed (it might not). We checked permissions (they might be wrong). \
                The door remained closed. The file remains unopened. We remain outside.",
                source_config.file_name
            ))?;

        // 📏 grab file size for the progress bar — if metadata fails, we fly blind (0 = unknown).
        // ⚠️  known edge case: if the file is being written to while we read, size may be stale/wrong.
        //    This is fine. We will not panic. We are calm. The borrow checker, however, is not calm.
        //    The borrow checker is never calm. The borrow checker has seen things.
        let file_size = file_handle.metadata().await.map(|m| m.len()).unwrap_or(0);

        let buf_reader = io::BufReader::new(file_handle);

        // 🚀 spin up the progress metrics — source name is the file path, honest and boring.
        // KNOWLEDGE GRAPH: file_name is used as the "source name" label in the progress table.
        // It's the human-readable handle. Keep it meaningful — it shows up in the TUI.
        let progress = ProgressMetrics::new(source_config.file_name.clone(), file_size);

        Ok(Self {
            buf_reader,
            source_config,
            progress,
        })
    }
}

#[async_trait]
impl Source for FileSource {
    /// 🔄 Read the next batch of lines from the file. Returns an empty `HitBatch` when EOF.
    ///
    /// The grind. The loop. The reading of lines. One by one. Like counting sheep,
    /// except the sheep are JSON documents and there are 40 million of them and it's 11pm
    /// and you have a migration to finish before the business opens tomorrow morning.
    ///
    /// KNOWLEDGE GRAPH: two exit conditions exist beyond EOF —
    ///   1. `max_batch_size_docs`: hit count cap. Don't overwhelm the sink.
    ///   2. `max_batch_size_bytes`: byte cap. Protects against massive single-doc JSON blobs.
    /// Both are checked on every iteration. Whichever fires first wins. It's a race.
    /// A deeply unsexy race between two integers.
    async fn next_batch(&mut self) -> Result<HitBatch> {
        // 📦 pre-allocate with the expected batch size — we're not savages
        let mut hits_batch =
            Vec::with_capacity(self.source_config.common_config.max_batch_size_docs);
        let mut total_bytes_read = 0usize;
        // ⚠️  1MB initial capacity per line — because NDJSON documents can be chunky.
        // If a single document exceeds this, tokio's BufReader will realloc. It's fine.
        // It's fine. We're fine. (It's not fine but it's acceptable.)
        let mut line = String::with_capacity(1024 * 1024);

        // 🔄 THE LOOP. The eternal loop. The reading of lines.
        // Each iteration: read a line, check if we're done, accumulate, check limits, repeat.
        // This is the grind. This is the job. This is the thing we get up and do every day
        // except we're an async function so we don't "get up" so much as "get polled by tokio".
        for _ in 0..=self.source_config.common_config.max_batch_size_docs {
            // 📖 read_line appends into `line` and returns bytes read. 0 = EOF. The void calls.
            let bytes_read = self.buf_reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                // ✅ EOF reached. The file has been consumed. Like a bag of chips at midnight.
                // Nothing left. Stare into the empty Vec. Feel something.
                break;
            }

            // 📊 accumulate — every byte counts (literally, for the progress bar)
            total_bytes_read += bytes_read;
            hits_batch.push(line.clone());
            // 🗑️  clear the line buffer for the next read. Not the bytes_read counter though.
            // That one keeps running. It's got goals. It's motivated.
            line.clear();

            // ⚠️  byte cap check — "have we eaten too much?" check for the batch.
            // Ancient proverb: he who ignores the byte limit, OOMs in staging.
            if total_bytes_read > self.source_config.common_config.max_batch_size_bytes {
                break;
            }

            // ⚠️  doc count cap — belt AND suspenders. Two limits. One batch.
            // KNOWLEDGE GRAPH: this check is slightly redundant with the for-loop bound above,
            // but it's explicit and intentional. The loop's upper bound is a safety net;
            // this check is the actual business logic. Don't remove either one. You'll regret it.
            if hits_batch.len() > self.source_config.common_config.max_batch_size_docs {
                break;
            }
        }

        // 🚀 batch assembled. Report to the troops. Or at least to the trace log.
        // Nobody is awake to read this at 3am. But if they are: hello. Go drink some water.
        trace!(
            "📖 hauled {} bytes out of the file like a digital fishing trip — catch of the day",
            total_bytes_read
        );
        // 📊 report the batch to progress — every byte counts, every doc matters, every metric
        // feeds the progress table in the TUI. This is how the human knows we're not dead.
        self.progress
            .update(total_bytes_read as u64, hits_batch.len() as u64);

        HitBatch::new(hits_batch)
    }
}

/// 🚰 FileSink — receives `HitBatch`es and faithfully writes them to a file, line by line.
///
/// It's a BufWriter around a tokio `File`. Simple. Honest. Does not complain.
/// Does not retry. Does not have opinions about your data format. It writes what you give it.
///
/// Think of it as a very loyal golden retriever of a struct. You throw it data, it writes it.
/// No judgment. Only service. And an occasional `flush()` at the end because manners matter.
///
/// ⚠️  `File::create` truncates if the file exists. No warning. No backup. Just gone.
/// He who runs this without checking the output path, re-migrates in shame.
#[derive(Debug)]
pub(crate) struct FileSink {
    file_buf: io::BufWriter<File>,
    _sink_config: FileSinkConfig,
}

impl FileSink {
    /// 🚀 Creates (or obliterates and recreates) the sink file, wraps it in a BufWriter,
    /// and returns a `FileSink` ready to receive the torrential downpour of your data.
    ///
    /// `File::create` is the nuclear option of file creation — it doesn't knock first.
    /// KNOWLEDGE GRAPH: this is intentional for migration use cases. Output is always fresh.
    /// If you need append semantics, you need a different sink. File a feature request.
    /// Or a PR. PRs are also accepted. We're not picky. We're just tired.
    pub(crate) async fn new(sink_config: FileSinkConfig) -> Result<Self> {
        // 💀 "Failed to create sink file" but make it literary, as requested by the AGENTS.md,
        // which is a document that exists and which you should read sometime, dear future engineer.
        // The file refused to be born. Perhaps the directory didn't exist. Perhaps permissions
        // were set by someone who really, truly, did not want this file to exist.
        // We respect their energy. We do not respect their disk ACLs.
        let file_handle = File::create(&sink_config.file_name).await.context(format!(
            "💀 The sink file '{}' could not be conjured into existence. \
                We stared at the path. The path stared back. \
                One of us was wrong about whether the parent directory existed. \
                It was us. It was always us.",
            &sink_config.file_name
        ))?;
        // 📦 BufWriter: because issuing one syscall per document is a war crime.
        // Batch those writes. Your kernel will thank you. Your SRE will thank you.
        // Your future self at 3am will bow before the altar of buffered I/O.
        let file_buf = io::BufWriter::new(file_handle);
        Ok(Self {
            file_buf,
            _sink_config: sink_config,
        })
    }
}

#[async_trait]
impl Sink for FileSink {
    /// 📥 Receive a batch of hits and write them all to the buffered file.
    ///
    /// No cap, this function low-key carries the whole pipeline fr fr.
    /// Every document that survives the source, the transformer, the throttler —
    /// it all ends here. This function is the finish line. The destination.
    /// The extremely anticlimactic `write_all` call at the end of the journey.
    async fn receive(&mut self, batch: HitBatch) -> Result<()> {
        // 🚀 they're heeere — like the movie but for data and significantly less terrifying
        trace!(
            "📬 {} hits just walked into the sink like they own the place. write them all down.",
            batch.hits.len()
        );
        // 🔄 iterate and write — the most honest loop in this entire codebase.
        // No retries. No backoff. No drama. Just write. One document at a time.
        // Like a monk copying manuscripts, but faster and with fewer vows of silence.
        for hit in batch.hits {
            let source_buf = hit.source_buf;
            self.file_buf.write_all(source_buf.as_bytes()).await?;
        }
        Ok(())
    }

    /// 🗑️  Flush the BufWriter and close up shop. The final act. The curtain call.
    ///
    /// Without this flush, your last batch of writes might be sitting in the buffer,
    /// warm and cozy, never making it to disk. Like a letter you wrote but never sent.
    /// Like Kevin with the blender. Don't be Kevin. Always flush.
    ///
    /// KNOWLEDGE GRAPH: `flush()` is called explicitly here rather than relying on Drop
    /// because async Drop is not a thing in Rust yet. This is a known language limitation.
    /// When async Drop ships, this comment becomes a historical artifact. Frame it.
    async fn close(&mut self) -> Result<()> {
        // 🎭 dramatic farewell — she gave everything she had. every byte. every write.
        // and now, at the end, we flush. for her. for the data. for the inode.
        trace!(
            "🎬 final flush. the file sink takes its bow, the BufWriter empties its soul to disk, the orchestra swells"
        );
        self.file_buf.flush().await.context(
            // 💀 poetic error for the poetic act of flushing.
            // The data was SO CLOSE. It was in the buffer. It could SEE the disk.
            // And then the flush failed. A tragedy in one line. Shakespeare would've used more lines.
            "💀 Error flushing file — the buffer held its data to the very end, \
            like a hoarder who finally agreed to let go, only for the storage unit to be locked. \
            The bytes are still in memory. The disk remains unwritten. The migration weeps.",
        )
    }
}
