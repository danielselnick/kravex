use anyhow::{Context, Result};
use async_trait::async_trait;
use memchr::memchr;
use serde::Deserialize;
use tokio::{
    fs::File,
    io::AsyncReadExt,
};
use tracing::trace;

use crate::backends::{Sink, Source};
use crate::progress::ProgressMetrics;
use crate::backends::{CommonSinkConfig, CommonSourceConfig};

// -- 📂 FileSourceConfig — "It's just a file", said no sysadmin ever before the disk filled up.
// -- Lives here now, close to the FileSource that actually uses it. Ethos pattern, baby. 🎯
// KNOWLEDGE GRAPH: config lives co-located with the backend that uses it. This is intentional.
// It avoids the "where the heck is that config defined" scavenger hunt at 2am during an incident.
// -- No cap, this pattern slaps fr fr.
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
    // -- ✅ "It just works" — the three most dangerous words in software engineering
    CommonSourceConfig::default()
}
// 📏 128 KiB per OS read — the Goldilocks zone between "too many syscalls" and "too much RAM".
// BufReader's default is 8 KiB. We're 16x that. Fewer context switches, happier kernel.
// KNOWLEDGE GRAPH: this constant controls the I/O batch size for raw file reads.
// Increasing it reduces syscall overhead at the cost of memory. 128 KiB is the sweet spot
// where amortized syscall cost plateaus on modern Linux (readahead does the rest).
const CHUNK_SIZE: usize = 128 * 1024;

/// 📂 FileSource — reads a file in fat 128 KiB chunks, scans for newlines with SIMD via memchr,
/// and batches docs into feeds without the overhead of per-line syscalls. 🚀
///
/// Think of it like a very diligent intern who reads a massive CSV, never complains,
/// and only stops when (a) the file ends, (b) the batch is full by doc count,
/// or (c) the batch is full by byte count — whichever comes first.
///
/// The borrow checker approved this struct. It did not approve of my feelings about the borrow
/// checker. We are at an impasse.
///
/// 🧵 Async, non-blocking. Raw tokio `File` — we ARE the buffer now. No middleman. No BufReader.
/// 📊 Tracks progress via `ProgressMetrics` — bytes read, docs read, reported to a progress table.
/// ⚠️  If the file is being written to while we read it, the size estimate will be wrong.
///     This is fine. We are fine. Everything is fine. 🐛
///
/// 🦆 The singularity will arrive before this struct learns to read backwards.
pub struct FileSource {
    // 📁 raw async file handle — no BufReader wrapper, we roll our own buffering
    // KNOWLEDGE GRAPH: we dropped BufReader because its 8 KiB default buffer caused too many
    // small reads. Our CHUNK_SIZE (128 KiB) batches I/O better and lets us scan for newlines
    // in bulk using memchr's SIMD magic instead of one-char-at-a-time read_line.
    file: File,
    // 🧱 reusable read buffer — pre-allocated to CHUNK_SIZE, never reallocated.
    // Each loop iteration fills this from the OS and appends to working_buf.
    read_buf: Vec<u8>,
    // 🧩 leftover bytes from the previous next_page() call — the tail end of a chunk
    // that didn't end on a newline. Gets prepended to working_buf on the next call.
    // KNOWLEDGE GRAPH: this is the key to correctness across page boundaries.
    // Without it, lines that span two chunks would get split into two incomplete docs.
    remainder: Vec<u8>,
    source_config: FileSourceConfig,
    // -- 📊 progress tracker — because "it's running" is not a status update.
    // -- This feeds the fancy progress table in the supervisor. Without it, you'd be flying blind
    // -- in a storm with no instruments. With it, you're flying blind in a storm, but at least
    // -- you have a very attractive progress bar.
    progress: ProgressMetrics,
}

// 🐛 NOTE: progress is intentionally excluded from this Debug impl.
// ProgressMetrics contains internal counters and spinners that don't format cleanly,
// and more importantly, nobody debugging a FileSource wants to read a wall of atomic integers.
// -- "Perfection is achieved not when there is nothing more to add, but when there is nothing
// -- left to add" — Antoine de Saint-Exupéry, who never had to impl Debug for a progress bar.
impl std::fmt::Debug for FileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSource")
            .field("source_config", &self.source_config)
            .finish()
    }
}

impl FileSource {
    /// 🚀 Opens the source file, grabs its size for the progress bar, allocates our chunk buffers,
    /// and returns a fully initialized `FileSource` ready to vend feeds at ludicrous speed.
    ///
    /// If the file doesn't exist: 💀 anyhow will tell you with *theatrical flair*.
    /// If metadata fails: we assume 0 bytes, progress bar shows unknown. Shrug emoji as a service.
    ///
    /// No cap: `File::open` is async here because we're in tokio-land. This is not your
    /// grandfather's `std::fs::File::open`. This is `std::fs::File::open`'s cooler younger sibling
    /// who got into the async runtime scene and never looked back.
    pub async fn new(source_config: FileSourceConfig) -> Result<Self> {
        // -- 💀 The door. It's locked. Or it doesn't exist. Or the filesystem lied to you.
        // -- In any case, the source file refused to open — like a very stubborn bouncer
        // -- at an exclusive club where the club is just a text file and we are very small data.
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
        // --    This is fine. We will not panic. We are calm. The borrow checker, however, is not calm.
        // --    The borrow checker is never calm. The borrow checker has seen things.
        let file_size = file_handle.metadata().await.map(|m| m.len()).unwrap_or(0);

        // 🚀 spin up the progress metrics — source name is the file path, honest and boring.
        // KNOWLEDGE GRAPH: file_name is used as the "source name" label in the progress table.
        // It's the human-readable handle. Keep it meaningful — it shows up in the TUI.
        let progress = ProgressMetrics::new(source_config.file_name.clone(), file_size);

        Ok(Self {
            file: file_handle,
            read_buf: vec![0u8; CHUNK_SIZE],
            remainder: Vec::new(),
            source_config,
            progress,
        })
    }
}

#[async_trait]
impl Source for FileSource {
    /// 📄 Read the next feed of lines from the file. Returns `None` when EOF.
    ///
    /// 🧠 Knowledge graph: sources return `Option<String>` — one raw feed of newline-delimited
    /// content, uninterpreted. The source accumulates lines up to byte/doc caps and returns
    /// the whole thing as a single String. The Manifold downstream splits and casts.
    ///
    /// KNOWLEDGE GRAPH: two exit conditions exist beyond EOF —
    ///   1. `max_batch_size_docs`: line count cap. Don't build a feed the size of Texas.
    ///   2. `max_batch_size_bytes`: byte cap. Protects against memory-busting accumulation.
    /// Both are checked on every iteration. Whichever fires first wins.
    ///
    /// 🚀 IMPLEMENTATION: reads 128 KiB chunks from the OS, scans for newlines using memchr
    /// (SIMD-accelerated), and splits on `\n` boundaries. Leftover bytes after the last newline
    /// are stashed in `self.remainder` for the next call. This batches I/O at a higher level
    /// than BufReader's 8 KiB default, slashing syscall overhead for large NDJSON files.
    ///
    /// KNOWLEDGE GRAPH: `\n` is always byte `0x0A` in UTF-8 and can never appear as a
    /// continuation byte in a multi-byte sequence. Scanning raw bytes for `0x0A` is therefore
    /// safe for any valid UTF-8 input. The final `String::from_utf8` validates the output.
    ///
    /// "He who reads the entire file into one String, OOMs in production." — Ancient proverb 📜
    async fn next_page(&mut self) -> Result<Option<String>> {
        let max_docs = self.source_config.common_config.max_batch_size_docs;
        let max_bytes = self.source_config.common_config.max_batch_size_bytes;

        // 🧱 feed accumulator — raw bytes, converted to String at the end.
        // We work in bytes to avoid repeated UTF-8 validation on every append.
        let mut feed: Vec<u8> = Vec::with_capacity(max_bytes);
        let mut doc_count = 0usize;
        let mut total_bytes_from_file = 0usize;

        // 🧩 drain the remainder from the previous call — these are bytes that were
        // left over after the last newline in the previous chunk. They form the
        // prefix of the first line in this page.
        let mut working_buf: Vec<u8> = std::mem::take(&mut self.remainder);

        // -- 🔄 the main loop: read chunks, scan for newlines, accumulate docs
        // -- like a combine harvester but for JSON lines
        loop {
            // 🔍 scan working_buf for newlines using memchr (SIMD go brrrr)
            // KNOWLEDGE GRAPH: memchr uses platform-specific SIMD (SSE2/AVX2 on x86,
            // NEON on ARM) to scan ~32 bytes per cycle. Way faster than byte-by-byte.
            let mut cursor = 0;
            let mut batch_limit_reached = false;
            while let Some(newline_offset) = memchr(b'\n', &working_buf[cursor..]) {
                let line_end = cursor + newline_offset;
                // 🧹 strip \r if this is a \r\n line ending (Windows refugees welcome)
                let line_content_end = if line_end > cursor
                    && working_buf[line_end - 1] == b'\r'
                {
                    line_end - 1
                } else {
                    line_end
                };

                let line = &working_buf[cursor..line_content_end];

                // ⏭️ skip empty lines — they're not docs, they're just vibes
                if !line.is_empty() {
                    // 🔗 separate docs with \n in the feed, but no trailing newline
                    if !feed.is_empty() {
                        feed.push(b'\n');
                    }
                    feed.extend_from_slice(line);
                    doc_count += 1;
                }

                // ⏩ advance cursor past the \n
                cursor = line_end + 1;

                // 🎯 check batch limits — whichever fires first wins
                if doc_count >= max_docs || feed.len() >= max_bytes {
                    batch_limit_reached = true;
                    break;
                }
            }

            if batch_limit_reached {
                // 🧩 stash everything after our cursor as remainder for next call
                self.remainder = working_buf[cursor..].to_vec();
                break;
            }

            // 🧩 everything after the last newline is a trailing fragment (incomplete line).
            // Keep it as the start of working_buf for the next read.
            let trailing_fragment = working_buf[cursor..].to_vec();

            // 📡 read the next chunk from the OS
            let bytes_read = self.file.read(&mut self.read_buf).await?;
            if bytes_read == 0 {
                // 🏁 EOF — if there's a trailing fragment, it's the final doc (no trailing \n)
                let fragment = trailing_fragment;
                // 🧹 strip trailing \r from the final fragment too
                let content_end = if fragment.last() == Some(&b'\r') {
                    fragment.len() - 1
                } else {
                    fragment.len()
                };
                if content_end > 0 {
                    if !feed.is_empty() {
                        feed.push(b'\n');
                    }
                    feed.extend_from_slice(&fragment[..content_end]);
                    doc_count += 1;
                }
                break;
            }

            total_bytes_from_file += bytes_read;

            // 🔧 build the new working_buf: trailing fragment + freshly read bytes
            // KNOWLEDGE GRAPH: the trailing fragment is typically small (< one line),
            // so this concat is cheap. The bulk of working_buf is the fresh chunk.
            working_buf = Vec::with_capacity(trailing_fragment.len() + bytes_read);
            working_buf.extend_from_slice(&trailing_fragment);
            working_buf.extend_from_slice(&self.read_buf[..bytes_read]);
        }

        // -- 📖 "I caught a fish THIS big" — every angler and every source, every time
        trace!(
            "📖 hauled {} bytes out of the file like a digital fishing trip — catch of the day",
            total_bytes_from_file
        );
        self.progress
            .update(total_bytes_from_file as u64, doc_count as u64);

        // 📄 Empty feed = EOF. The well is dry. Return None. 🏁
        if feed.is_empty() {
            // -- 🏁 "That's all folks!" — Porky Pig, and also this file source
            Ok(None)
        } else {
            // ✅ convert bytes to String — this validates UTF-8 in one pass at the end
            // rather than on every line. Efficiency AND correctness. Chef's kiss. 🤌
            let feed_string = String::from_utf8(feed).context(
                "💀 The file contained bytes that aren't valid UTF-8. \
                We tried to make a String. The String said no. \
                Like trying to fit a square peg in a round hole, \
                except the peg is binary garbage and the hole is Unicode.",
            )?;
            Ok(Some(feed_string))
        }
    }
}
