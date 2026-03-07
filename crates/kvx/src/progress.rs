// AI
//! 📊 progress.rs — "Are we there yet?" — every pipeline, every time, forever.
//!
//! 🚀 This module answers the age-old question: "how fast is our data draining?"
//! With cold hard numbers, a progress bar, and a table so comfy it has lumbar support.
//!
//! ⚠️  Warning: Watching this progress bar will not make it go faster.
//! Neither will refreshing it. We've tried. Science says no.
//!
//! 🦆 The duck has nothing to do with this module. It's just vibing.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets::NOTHING};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::task::JoinHandle;

// -- 📏 one mebibyte — not a megabyte, pedants. there's a difference and I will die on this hill.
const MIB: u64 = 1024 * 1024;

/// 📦 Converts raw bytes into a human-readable string with adaptive unit scaling.
/// Because "1073741824 bytes" is a war crime in a UI.
fn format_bytes_adaptive(bytes: u64) -> String {
    if bytes >= 512 * MIB {
        // -- 🚀 we're in MiB territory, congratulations on your large migration
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= MIB {
        // -- 📦 KiB zone — still respectable
        format!("{:.2} KiB", bytes as f64 / 1024.0)
    } else {
        // -- 🐛 raw bytes mode. we believe in you though. small payloads need love too.
        format!("{} bytes", bytes)
    }
}

/// 🔢 Formats a number with commas for the 3 people in the audience who like readability.
/// "1000000 docs" → "1,000,000 docs" — you're welcome, eyes.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    // -- 🧵 pre-allocate like we know what we're doing (we do, we read the book)
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// ⏱️ Formats a Duration into MM:SS or HH:MM:SS.
/// If it shows HH:MM:SS, you should probably call your mom. It's been a while.
fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        // -- 🔄 long haul migration. order pizza. plural.
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        // -- ✅ quick run. you have time for coffee. maybe.
        format!("{:02}:{:02}", minutes, seconds)
    }
}

/// 📡 Shared atomic counters for drain-side metrics.
///
/// N drainers atomically increment these counters. One reporter task reads them periodically.
/// Same pattern as FlowKnob = Arc<AtomicUsize> — lock-free, no Mutex, no channel overhead.
///
/// "He who shares mutable state without atomics, panics in production." — Ancient proverb 📜
///
/// 🧠 Knowledge graph: DrainMetrics is the bridge between Drainers and the progress reporter.
/// Drainers write (fetch_add/store) after each successful drain. Reporter reads (load) every 500ms.
/// No synchronization beyond the atomics themselves. Relaxed ordering is fine because the
/// reporter only needs a "close enough" snapshot — we're not building a stock exchange. 📈
// 🎭 Manual Debug because AtomicU64 debug output is noisy — we just show current values
impl std::fmt::Debug for DrainMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // -- 🐛 Snapshot the atomics for a clean debug view — no raw AtomicU64 noise
        f.debug_struct("DrainMetrics")
            .field("bytes_drained", &self.bytes_drained.load(Ordering::Relaxed))
            .field("requests_completed", &self.requests_completed.load(Ordering::Relaxed))
            .field("latency_sum_ms", &self.latency_sum_ms.load(Ordering::Relaxed))
            .field("latency_max_ms", &self.latency_max_ms.load(Ordering::Relaxed))
            .finish()
    }
}

pub struct DrainMetrics {
    /// 📦 total bytes drained — payload sizes accumulate like a 401k, except this one actually grows
    pub bytes_drained: AtomicU64,
    /// ✅ total drain requests completed — one per successful drain_with_retry
    pub requests_completed: AtomicU64,
    /// ⏱️ sum of all drain latencies in ms — divide by requests_completed for average
    pub latency_sum_ms: AtomicU64,
    /// ⏱️ max latency observed across all drains — the worst-case scenario metric
    pub latency_max_ms: AtomicU64,
    /// 📡 most recent request size in bytes — store (not add), always the latest
    pub last_request_size_bytes: AtomicU64,
    /// 📡 most recent drain latency in ms — store (not add), always the latest
    pub last_latency_ms: AtomicU64,
}

impl DrainMetrics {
    /// 🏗️ Birth of a DrainMetrics. All zeros. Like my bank account after paying the mortgage.
    pub fn new() -> Self {
        Self {
            bytes_drained: AtomicU64::new(0),
            requests_completed: AtomicU64::new(0),
            latency_sum_ms: AtomicU64::new(0),
            latency_max_ms: AtomicU64::new(0),
            last_request_size_bytes: AtomicU64::new(0),
            last_latency_ms: AtomicU64::new(0),
        }
    }

    /// 📡 Record a completed drain — called by Drainer after each successful drain_with_retry.
    /// fetch_add for accumulators, fetch_max for high-water mark, store for "latest" fields.
    /// All Relaxed ordering — the reporter just needs a ballpark, not a courtroom transcript. ⚖️
    pub fn record_drain(&self, payload_bytes: u64, latency_ms: u64) {
        // -- 📦 accumulate the evidence of hard work
        self.bytes_drained.fetch_add(payload_bytes, Ordering::Relaxed);
        self.requests_completed.fetch_add(1, Ordering::Relaxed);
        self.latency_sum_ms.fetch_add(latency_ms, Ordering::Relaxed);
        self.latency_max_ms.fetch_max(latency_ms, Ordering::Relaxed);
        // -- 📡 latest values — overwrites are fine, we only care about the most recent
        self.last_request_size_bytes.store(payload_bytes, Ordering::Relaxed);
        self.last_latency_ms.store(latency_ms, Ordering::Relaxed);
    }
}

/// 📡 A snapshot of throughput rates at any given moment.
/// Like a speedometer, but for bytes and documents. And less likely to get you a ticket.
struct Rates {
    /// 🚀 how many documents we're draining per second (the vanity metric)
    docs_per_sec: f64,
    /// 📦 how many MiB per second are flowing through the pipeline (the real metric)
    mib_per_sec: f64,
}

/// 📊 The brains behind the progress display. Reads from DrainMetrics atomics,
/// calculates rates via a sliding window, and renders a comfy-table to the terminal.
///
/// Uses a sliding 5-second window for rate calculations so spikes don't scare you.
/// (Your heart rate is not our responsibility.)
///
/// # Ancient Proverb
/// "He who runs a migration without a progress bar, migrates alone and in darkness."
struct ProgressReporter {
    /// 🏷️ pipeline name — what are we even migrating? displayed in the UI
    pipeline_name: String,
    /// 📡 shared atomic counters from drainers — the source of truth
    drain_metrics: Arc<DrainMetrics>,
    /// 🎨 the actual terminal progress bar (indicatif does the heavy lifting here)
    progress_bar: ProgressBar,
    /// 🔄 sliding window of (timestamp, bytes, docs) for rate calculation
    /// VecDeque because we pop from the front — linked list but make it cache-friendly-ish
    rate_samples: VecDeque<(Instant, u64, u64)>,
    /// ⏱️ when did this whole adventure start? hopefully not too long ago.
    start_time: Instant,
    /// 📏 total expected bytes — 0 if unknown (classic elasticsearch)
    total_expected_bytes: u64,
}

impl ProgressReporter {
    /// 🚀 Spin up a new ProgressReporter.
    ///
    /// # No cap
    /// This function slaps. fr fr. The progress bar will look sick in your terminal.
    fn new(pipeline_name: String, drain_metrics: Arc<DrainMetrics>, total_expected_bytes: u64) -> Self {
        // -- 🎨 build the progress bar — cyan because it's classy, blue because it's calm
        let progress_bar = if total_expected_bytes > 0 {
            ProgressBar::new(total_expected_bytes)
        } else {
            // -- ⚠️ unknown total — spinner mode, no ETA, just vibes
            ProgressBar::new_spinner()
        };
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n| [{bar:40.cyan/blue}]")
                .unwrap() // -- 🐛 safe unwrap: template string is hardcoded and valid, I checked, twice
                .progress_chars("=>-"),
        );

        let start_time = Instant::now();

        // -- 🔄 seed the rate window with t=0 so we don't divide by zero like animals
        let mut rate_samples = VecDeque::new();
        rate_samples.push_back((start_time, 0u64, 0u64));

        Self {
            pipeline_name,
            drain_metrics,
            progress_bar,
            rate_samples,
            start_time,
            total_expected_bytes,
        }
    }

    /// 🔄 Tick the reporter: snapshot atomics, calculate rates, render the display.
    /// Called every 500ms by the spawned reporter task.
    /// Like a heartbeat monitor, but for data. Beep. Beep. Beep. 💓
    fn tick(&mut self) {
        let the_bytes_drained = self.drain_metrics.bytes_drained.load(Ordering::Relaxed);
        let the_requests_completed = self.drain_metrics.requests_completed.load(Ordering::Relaxed);
        let the_latency_sum_ms = self.drain_metrics.latency_sum_ms.load(Ordering::Relaxed);
        let the_latency_max_ms = self.drain_metrics.latency_max_ms.load(Ordering::Relaxed);
        let the_last_request_size = self.drain_metrics.last_request_size_bytes.load(Ordering::Relaxed);
        let the_last_latency_ms = self.drain_metrics.last_latency_ms.load(Ordering::Relaxed);

        // 📊 estimate doc count from bytes — heuristic: count \n in bulk payload ÷ 2
        // (bulk format has action line + doc line per document, separated by \n)
        // For non-bulk formats this is a rough approximation. Good enough for a progress bar.
        // -- "Close enough for government work" — every engineer, ever
        let the_estimated_docs = the_bytes_drained / 512;

        // -- 📊 crunch the numbers, render the glory
        let rates = self.calculate_rates(the_bytes_drained, the_estimated_docs);
        self.render(
            rates,
            the_bytes_drained,
            the_estimated_docs,
            the_requests_completed,
            the_latency_sum_ms,
            the_latency_max_ms,
            the_last_request_size,
            the_last_latency_ms,
        );
        if self.total_expected_bytes > 0 {
            self.progress_bar.set_position(the_bytes_drained);
        }
    }

    /// 📈 Calculate current throughput rates using a 5-second sliding window.
    ///
    /// Sliding window keeps the displayed rate from looking like a seismograph
    /// during normal operations. Short bursts won't spike you into existential terror.
    ///
    /// TODO: win the lottery, retire, replace this with a proper time-series database
    fn calculate_rates(&mut self, current_bytes: u64, current_docs: u64) -> Rates {
        let now = Instant::now();
        // 🔄 evict samples older than 5 seconds from the front of the queue
        // -- like a bouncer at a club, but for data points
        let window = Duration::from_secs(5);
        while let Some(&(timestamp, _, _)) = self.rate_samples.front() {
            if now.duration_since(timestamp) > window {
                self.rate_samples.pop_front();
            } else {
                // ✅ this sample is fresh enough, and so are all the ones behind it
                break;
            }
        }

        // -- 📦 push the current moment into the window — the present is always relevant
        self.rate_samples
            .push_back((now, current_bytes, current_docs));

        // 📊 compare now vs oldest sample in window to get deltas
        if let Some(&(oldest_time, oldest_bytes, oldest_docs)) = self.rate_samples.front() {
            let elapsed = now.duration_since(oldest_time).as_secs_f64();
            if elapsed > 0.0 {
                // -- 🚀 we have a meaningful window — do the math
                let bytes_delta = current_bytes.saturating_sub(oldest_bytes);
                let docs_delta = current_docs.saturating_sub(oldest_docs);
                return Rates {
                    docs_per_sec: docs_delta as f64 / elapsed,
                    mib_per_sec: (bytes_delta as f64 / elapsed) / MIB as f64,
                };
            }
        }

        // -- 💤 not enough elapsed time yet — return zeros and maintain composure
        Rates {
            docs_per_sec: 0.0,
            mib_per_sec: 0.0,
        }
    }

    /// 🎨 Render the full progress display as a comfy-table message on the progress bar.
    ///
    /// Layout (6 rows x 2 cols):
    /// ```text
    /// | sink: <name>
    /// | [=====>----------]
    ///   <docs/min>       <total docs>
    ///   <MiB/s>          <bytes drained>
    ///   <avg latency>    <last latency>
    ///   <avg req size>   <last req size>
    ///   <elapsed>        <remaining>
    /// ```
    ///
    /// If you're reading this comment at 3am during an incident, I'm so sorry.
    /// At least the table looks nice.
    #[allow(clippy::too_many_arguments)]
    fn render(
        &self,
        rates: Rates,
        the_bytes_drained: u64,
        the_estimated_docs: u64,
        the_requests_completed: u64,
        the_latency_sum_ms: u64,
        the_latency_max_ms: u64,
        the_last_request_size: u64,
        the_last_latency_ms: u64,
    ) {
        let docs_per_min = rates.docs_per_sec * 60.0;
        // -- 🔢 human-friendly numbers because we are, ostensibly, human
        let docs_rate = format_number(docs_per_min as u64);
        let docs_total = format_number(the_estimated_docs);

        // ⏱️ average latency — avoid divide-by-zero like a responsible adult
        let the_avg_latency_ms = if the_requests_completed > 0 {
            the_latency_sum_ms / the_requests_completed
        } else {
            0
        };

        // 📏 average request size — again, no dividing by zero
        let the_avg_request_size = if the_requests_completed > 0 {
            the_bytes_drained / the_requests_completed
        } else {
            0
        };

        // ⏱️ time stats
        let elapsed = self.start_time.elapsed();
        let elapsed_fmt = format_duration(elapsed);

        // 📊 ETA calculation — only meaningful when we know the total
        let remaining = if self.total_expected_bytes > 0 && the_bytes_drained > 0 {
            let percent = the_bytes_drained as f64 / self.total_expected_bytes as f64;
            if percent > 0.0 {
                // 🔮 linear extrapolation — assumes the future looks like the past
                // -- (historically a bad assumption, but fine for data migration)
                let total_estimated = elapsed.as_secs_f64() / percent;
                let remaining_secs = total_estimated - elapsed.as_secs_f64();
                if remaining_secs > 0.0 {
                    format_duration(Duration::from_secs_f64(remaining_secs))
                } else {
                    // ✅ done or basically done — show a friendly placeholder
                    "--:--".to_string()
                }
            } else {
                "--:--".to_string()
            }
        } else {
            // -- ⚠️  no total known means no ETA — we're flying blind, captain
            "--:--".to_string()
        };

        // 🍽️ build the comfy table — two columns, right-aligned, no borders (preset: NOTHING)
        // -- NOTHING preset because we're minimalists. and also the borders looked bad.
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_content_arrangement(ContentArrangement::Dynamic);

        // 🚀 row 1: throughput rates
        table.add_row(vec![
            Cell::new(format!("{} Docs/min", docs_rate)).set_alignment(CellAlignment::Right),
            Cell::new(format!("~{} Docs", docs_total)).set_alignment(CellAlignment::Right),
        ]);
        // 📦 row 2: byte throughput and cumulative bytes
        table.add_row(vec![
            Cell::new(format!("{:.2} MiB/s", rates.mib_per_sec))
                .set_alignment(CellAlignment::Right),
            Cell::new(format_bytes_adaptive(the_bytes_drained)).set_alignment(CellAlignment::Right),
        ]);
        // ⏱️ row 3: latency — avg and last
        table.add_row(vec![
            Cell::new(format!("avg {}ms", the_avg_latency_ms)).set_alignment(CellAlignment::Right),
            Cell::new(format!("last {}ms", the_last_latency_ms)).set_alignment(CellAlignment::Right),
        ]);
        // 📏 row 4: request size — avg and last
        table.add_row(vec![
            Cell::new(format!("avg {}", format_bytes_adaptive(the_avg_request_size)))
                .set_alignment(CellAlignment::Right),
            Cell::new(format!("last {}", format_bytes_adaptive(the_last_request_size)))
                .set_alignment(CellAlignment::Right),
        ]);
        // ⏱️ row 5: time elapsed and estimated time remaining
        table.add_row(vec![
            Cell::new(format!("{} elapsed", elapsed_fmt)).set_alignment(CellAlignment::Right),
            Cell::new(format!("{} remaining", remaining)).set_alignment(CellAlignment::Right),
        ]);

        // -- 🎨 slam it all into the progress bar message
        // indicatif will handle the terminal magic (cursor positioning, redraw, etc.)
        self.progress_bar
            .set_message(format!("sink: {}\n{}", self.pipeline_name, table));
    }
}

/// 🚀 Spawns a tokio task that ticks the progress reporter every 500ms.
///
/// Returns a JoinHandle — the Foreman should .abort() this after all real workers complete.
/// The reporter is a leaf display task: it reads atomics, renders to terminal, and sleeps.
/// Aborting it is safe and expected. Like pulling the plug on a screensaver. 🖥️
///
/// "In the beginning there was no progress bar. And the developer stared into the void.
///  And the void did not stare back, because there was no render loop." — Genesis 0:0 🦆
pub fn spawn_progress_reporter(
    pipeline_name: String,
    drain_metrics: Arc<DrainMetrics>,
    total_expected_bytes: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut the_reporter = ProgressReporter::new(pipeline_name, drain_metrics, total_expected_bytes);
        loop {
            // -- 💤 sleep 500ms — fast enough to feel responsive, slow enough to not burn CPU
            tokio::time::sleep(Duration::from_millis(500)).await;
            the_reporter.tick();
        }
        // -- 🏁 unreachable: this loop runs until aborted by the Foreman.
        // -- Like a hamster wheel, it doesn't stop on its own. 🐹
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The one where DrainMetrics atomically tracks bytes and latency.
    /// Like a fitness tracker, but for data instead of steps. 🏃🦆
    #[test]
    fn the_one_where_drain_metrics_count_everything() {
        let metrics = DrainMetrics::new();

        metrics.record_drain(1024, 50);
        metrics.record_drain(2048, 100);
        metrics.record_drain(512, 25);

        // -- 📦 accumulators should sum up
        assert_eq!(metrics.bytes_drained.load(Ordering::Relaxed), 1024 + 2048 + 512);
        assert_eq!(metrics.requests_completed.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.latency_sum_ms.load(Ordering::Relaxed), 50 + 100 + 25);
        // -- ⏱️ max should be the highest latency seen
        assert_eq!(metrics.latency_max_ms.load(Ordering::Relaxed), 100);
        // -- 📡 last values should be from the most recent call
        assert_eq!(metrics.last_request_size_bytes.load(Ordering::Relaxed), 512);
        assert_eq!(metrics.last_latency_ms.load(Ordering::Relaxed), 25);
    }

    /// 🧪 The one where DrainMetrics starts at zero because hope springs eternal.
    /// Fresh out of the factory. No drains. No trauma. Yet. 🧘🦆
    #[test]
    fn the_one_where_fresh_metrics_are_all_zeros() {
        let metrics = DrainMetrics::new();

        assert_eq!(metrics.bytes_drained.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.requests_completed.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.latency_sum_ms.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.latency_max_ms.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.last_request_size_bytes.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.last_latency_ms.load(Ordering::Relaxed), 0);
    }

    /// 🧪 The one where format_number adds commas like a civilized human being.
    /// "1000000" → "1,000,000" — the Oxford comma of numbers. 📝🦆
    #[test]
    fn the_one_where_format_number_adds_commas() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
        assert_eq!(format_number(1_234_567_890), "1,234,567,890");
    }

    /// 🧪 The one where format_duration handles hours like a champ.
    /// If your migration takes hours, the progress bar is the least of your problems. ⏰🦆
    #[test]
    fn the_one_where_format_duration_handles_hours() {
        assert_eq!(format_duration(Duration::from_secs(0)), "00:00");
        assert_eq!(format_duration(Duration::from_secs(65)), "01:05");
        assert_eq!(format_duration(Duration::from_secs(3661)), "01:01:01");
    }

    /// 🧪 The one where spawn_progress_reporter can be created and aborted.
    /// Like hiring an intern and immediately firing them. Cruel but necessary. 🏢🦆
    #[tokio::test]
    async fn the_one_where_reporter_starts_and_aborts_cleanly() {
        let metrics = Arc::new(DrainMetrics::new());
        metrics.record_drain(4096, 42);

        let handle = spawn_progress_reporter(
            "test-pipeline".to_string(),
            metrics.clone(),
            0,
        );

        // -- 💤 let it tick once
        tokio::time::sleep(Duration::from_millis(600)).await;

        // -- 🗑️ abort — the foreman does this after workers complete
        handle.abort();
        let _ = handle.await;
        // -- ✅ if we got here without panicking, the reporter handled abort gracefully
    }

    /// 🧪 The one where concurrent drainers don't lose data.
    /// Multiple threads hammering the same counters — like Black Friday at Costco. 🛒🦆
    #[tokio::test]
    async fn the_one_where_concurrent_record_drains_are_accurate() {
        let metrics = Arc::new(DrainMetrics::new());
        let mut handles = Vec::new();

        for _ in 0..10 {
            let m = metrics.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..100 {
                    m.record_drain(1000, 10);
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // -- 📦 10 tasks × 100 drains × 1000 bytes = 1,000,000 bytes
        assert_eq!(metrics.bytes_drained.load(Ordering::Relaxed), 1_000_000);
        // -- ✅ 10 tasks × 100 drains = 1000 requests
        assert_eq!(metrics.requests_completed.load(Ordering::Relaxed), 1_000);
        // -- ⏱️ 10 tasks × 100 drains × 10ms = 10,000ms total latency
        assert_eq!(metrics.latency_sum_ms.load(Ordering::Relaxed), 10_000);
    }
}
