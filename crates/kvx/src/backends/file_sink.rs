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
use tokio::{fs::File, io::{self, AsyncWriteExt}};
use tracing::trace;

use crate::backends::Sink;
use crate::common::HitBatch;
use crate::supervisors::config::CommonSinkConfig;

// 🚰 FileSinkConfig — cousin of FileSourceConfig, equally traumatized by disk full errors.
// Also lives here, cozy next to its FileSink bestie. No more long-distance config relationships.
// KNOWLEDGE GRAPH: same co-location principle as above. One backend = one config = one file. Clean.
#[derive(Debug, Deserialize, Clone)]
pub struct FileSinkConfig {
    pub file_name: String,
    #[serde(default = "default_file_common_sink_config")]
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
        let file_handle = File::create(&sink_config.file_name)
            .await
            .context(format!(
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
        trace!("📬 {} hits just walked into the sink like they own the place. write them all down.", batch.hits.len());
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
        trace!("🎬 final flush. the file sink takes its bow, the BufWriter empties its soul to disk, the orchestra swells");
        self.file_buf.flush().await.context(
            // 💀 poetic error for the poetic act of flushing.
            // The data was SO CLOSE. It was in the buffer. It could SEE the disk.
            // And then the flush failed. A tragedy in one line. Shakespeare would've used more lines.
            "💀 Error flushing file — the buffer held its data to the very end, \
            like a hoarder who finally agreed to let go, only for the storage unit to be locked. \
            The bytes are still in memory. The disk remains unwritten. The migration weeps."
        )
    }
}
