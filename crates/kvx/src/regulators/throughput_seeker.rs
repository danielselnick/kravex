// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎬 *[INT. MOUNTAIN SUMMIT — DAWN]*
//! *[A lone figure stands at the peak, surveying the landscape below.]*
//! *["I didn't climb here by asking for directions," it whispers.]*
//! *["I climbed here by trying. And falling. And trying again."]*  🏔️📈🦆
//!
//! 📦 ThroughputSeeker — hill climbing regulator that directly optimizes bytes/sec.
//!
//! 🧠 Knowledge graph:
//! ```text
//! Drainer completes → DrainResult { payload_bytes, latency_ms }
//!   → System 1: Circuit Breaker (dual EMA crossover, every reading)
//!     → fast EMA drops 20% below slow EMA? TRIP → immediate halve
//!   → System 2: Hill Climber (5s windowed median)
//!     → improved >10%? step forward
//!     → worsened >10%? reverse + shrink step (×0.618)
//!     → noise band? hold steady, re-explore after 30 windows
//! ```
//!
//! ⚠️ The PID is dead. Long live the hill climber. The singularity approves.

use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::GaugeReading;
use crate::regulators::Regulate;
use crate::regulators::config::ThroughputSeekerConfig;

// -- 🎛️ EMA smoothing constants — fast reacts in ~3-5 readings, slow in ~20
const THE_NERVOUS_ALPHA: f64 = 0.3;
// -- 🎛️ slow EMA α — like a glacier, but with opinions
const THE_CHILL_ALPHA: f64 = 0.05;

// -- 📐 Golden ratio's shy cousin — step shrink factor on reversal
const THE_GOLDEN_SHRINK: f64 = 0.618;

// 📏 Below this step size, the seeker has found its happy place
const THE_MINIMUM_STEP_WORTH_TAKING: f64 = 65_536.0;

// 🧘 Slow EMA needs ~1/α = 20 readings to converge after a reset.
// Until then, the crossover signal is noise — arming the breaker early causes re-trip loops.
const THE_EMA_WARMUP_READINGS: usize = 20;

// -- ⏱️ Circuit breaker needs a moment to process its feelings
const THE_CIRCUIT_BREAKER_COOLDOWN: Duration = Duration::from_secs(3);
// -- ⏱️ Errors get a longer timeout because rejection hurts more
const THE_ERROR_COOLDOWN: Duration = Duration::from_secs(5);

/// 🏔️ ThroughputSeeker — a dual-system regulator that climbs toward peak throughput.
///
/// System 1 (Circuit Breaker): Dual EMA crossover detects sudden degradation.
/// Fast EMA drops 20% below slow EMA? TRIP — immediate halve. Like a fuse box
/// for your data pipeline. Except you can't just flip it back and pretend nothing happened. ⚡
///
/// System 2 (Hill Climber): 5-second windowed median comparison.
/// Improved? Step forward. Worsened? Reverse + shrink. Settled? Wait. Re-explore eventually.
/// Like house hunting but for request sizes. "This one has good throughput,
/// but have you seen the one next door?" 🏠🦆
#[derive(Debug, Clone)]
pub struct ThroughputSeeker {
    // 🏃 System 1: Circuit Breaker — the panic button with math backing it up
    the_nervous_average: f64,
    the_chill_average: f64,

    // 🧗 System 2: Hill Climber — the slow methodical search for the peak
    the_current_request_size: f64,
    the_step_size: f64,
    the_heading: f64,
    the_throughput_samples: Vec<f64>,
    the_window_start: Instant,
    the_previous_window_vibe: Option<f64>,
    the_boredom_counter: usize,
    the_seeker_found_peace: bool,

    // 🛡️ Safety — mandatory rest after a bad day
    the_mandatory_chill_until: Instant,

    // 📏 Bounds — thou shalt not go below or above
    the_floor: f64,
    the_ceiling: f64,

    // 🔧 Config — the knobs that make this thing tick
    the_window_duration: Duration,
    the_improvement_bar: f64,
    the_degradation_bar: f64,
    the_boredom_threshold: usize,

    // 🎬 First contact — EMAs need initialization before they can have opinions
    the_first_contact_happened: bool,

    // 🧘 Readings since last EMA reset — breaker stays disarmed until EMAs converge
    the_ema_warmup_counter: usize,
}

impl ThroughputSeeker {
    /// 🏗️ Birth of a seeker — armed with config and a ceiling to respect.
    ///
    /// "Every mountain climber starts at base camp.
    ///  Ours starts at 4 MiB and dreams of bigger payloads." — Edmund Hillary (probably) 🏔️🦆
    pub fn new(config: &ThroughputSeekerConfig, the_ceiling: f64) -> Self {
        let the_starting_point = config.initial_output_bytes as f64;
        let the_right_now = Instant::now();
        Self {
            the_nervous_average: 0.0,
            the_chill_average: 0.0,
            the_current_request_size: the_starting_point,
            the_step_size: the_starting_point / 4.0,
            the_heading: 1.0,
            the_throughput_samples: Vec::with_capacity(128),
            the_window_start: the_right_now,
            the_previous_window_vibe: None,
            the_boredom_counter: 0,
            the_seeker_found_peace: false,
            the_mandatory_chill_until: the_right_now,
            the_floor: config.min_request_size_bytes as f64,
            the_ceiling,
            the_window_duration: Duration::from_secs(config.window_duration_secs),
            the_improvement_bar: config.improvement_threshold_pct / 100.0,
            the_degradation_bar: config.degradation_threshold_pct / 100.0,
            the_boredom_threshold: config.re_explore_after_windows,
            the_first_contact_happened: false,
            the_ema_warmup_counter: 0,
        }
    }

    /// 📡 Process a successful drain — compute throughput, run both systems.
    ///
    /// "Two systems. One goal. Zero chill." — The ThroughputSeeker's LinkedIn bio 📈
    fn on_drain_complete(&mut self, the_throughput: f64) -> f64 {
        // 🎬 First contact — initialize EMAs to avoid cold-start nonsense
        if !self.the_first_contact_happened {
            self.the_nervous_average = the_throughput;
            self.the_chill_average = the_throughput;
            self.the_first_contact_happened = true;
            return self.the_current_request_size;
        }

        // ═══════════════════════════════════════════════════════════════════
        // 🏃 SYSTEM 1: CIRCUIT BREAKER (every reading, instant response)
        //
        // Dual EMA crossover — when the fast average sinks below the
        // degradation threshold of the slow average, something went very
        // wrong very fast. TRIP. HALVE. NOW. Like pulling the rip cord
        // on a parachute — you don't wait for consensus.
        // ═══════════════════════════════════════════════════════════════════
        self.the_nervous_average = THE_NERVOUS_ALPHA * the_throughput
            + (1.0 - THE_NERVOUS_ALPHA) * self.the_nervous_average;
        self.the_chill_average = THE_CHILL_ALPHA * the_throughput
            + (1.0 - THE_CHILL_ALPHA) * self.the_chill_average;
        self.the_ema_warmup_counter += 1;

        // Gate the trip on: not in cooldown, not at floor, and EMAs have had
        // enough readings to converge after a reset. Without the warmup gate,
        // the trip→reset→immediate re-trip cycle never breaks.
        let the_not_cooling_down = Instant::now() >= self.the_mandatory_chill_until;
        let the_not_pinned_to_floor = self.the_current_request_size > self.the_floor;
        let the_emas_have_settled = self.the_ema_warmup_counter >= THE_EMA_WARMUP_READINGS;

        if the_not_cooling_down
            && the_not_pinned_to_floor
            && the_emas_have_settled
            && self.the_chill_average > 0.0
            && self.the_nervous_average
                < self.the_chill_average * (1.0 - self.the_degradation_bar)
        {
            // ⚡ CIRCUIT BREAKER TRIPPED
            let the_old_size = self.the_current_request_size;
            self.the_current_request_size =
                (self.the_current_request_size * 0.5).max(self.the_floor);
            // Accept the new reality — slow EMA snaps to fast EMA
            self.the_chill_average = self.the_nervous_average;
            // Reset warmup — EMAs need to reconverge before the breaker can fire again
            self.the_ema_warmup_counter = 0;
            self.reset_the_hill_climber();
            self.the_mandatory_chill_until = Instant::now() + THE_CIRCUIT_BREAKER_COOLDOWN;

            warn!(
                "⚡ Circuit breaker TRIPPED — fast EMA ({:.0}) dropped below {:.0}% of slow EMA. \
                 Halved: {} → {} bytes. Like pulling the emergency brake on a freight train. 🚂",
                self.the_nervous_average,
                (1.0 - self.the_degradation_bar) * 100.0,
                the_old_size as usize,
                self.the_current_request_size as usize
            );

            return self.the_current_request_size;
        }

        // ═══════════════════════════════════════════════════════════════════
        // 🧗 SYSTEM 2: HILL CLIMBER (windowed, patient, methodical)
        //
        // Accumulate throughput readings. Every N seconds, take the median.
        // Compare to last window. Step toward improvement. Shrink on reversal.
        // The tortoise beats the hare, especially when the hare is a PID
        // controller that can't decide which direction to go.
        // ═══════════════════════════════════════════════════════════════════
        if Instant::now() < self.the_mandatory_chill_until {
            return self.the_current_request_size;
        }

        self.the_throughput_samples.push(the_throughput);

        if self.the_window_start.elapsed() < self.the_window_duration {
            return self.the_current_request_size;
        }

        // 🏁 Window complete — time to judge
        let the_median = the_wisdom_of_the_middle(&mut self.the_throughput_samples);
        self.the_throughput_samples.clear();
        self.the_window_start = Instant::now();

        match self.the_previous_window_vibe {
            None => {
                // 🎬 First window — establish baseline, take a bold first step into the unknown
                self.the_previous_window_vibe = Some(the_median);
                self.the_current_request_size += self.the_heading * self.the_step_size;
                self.the_current_request_size = self
                    .the_current_request_size
                    .clamp(self.the_floor, self.the_ceiling);

                debug!(
                    "🧗 Hill climber: first window median={:.0} B/s, stepping to {} bytes",
                    the_median, self.the_current_request_size as usize
                );
            }
            Some(the_prev) => {
                if the_prev == 0.0 {
                    self.the_previous_window_vibe = Some(the_median);
                    return self.the_current_request_size;
                }

                let the_delta = (the_median - the_prev) / the_prev;

                if the_delta > self.the_improvement_bar {
                    // 🚀 Improved! Keep going same direction — the grass IS greener over there
                    self.the_previous_window_vibe = Some(the_median);
                    self.the_current_request_size += self.the_heading * self.the_step_size;
                    self.the_boredom_counter = 0;

                    debug!(
                        "🚀 Hill climber: improved {:.1}% — stepping {} to {} bytes",
                        the_delta * 100.0,
                        if self.the_heading > 0.0 { "up" } else { "down" },
                        self.the_current_request_size as usize
                    );
                } else if the_delta < -self.the_improvement_bar {
                    // 🔄 Worsened! Reverse course + shrink step — we overshot the peak
                    self.the_heading *= -1.0;
                    self.the_step_size *= THE_GOLDEN_SHRINK;
                    self.the_previous_window_vibe = Some(the_median);
                    self.the_boredom_counter = 0;

                    if self.the_step_size < THE_MINIMUM_STEP_WORTH_TAKING {
                        self.the_seeker_found_peace = true;
                        info!(
                            "✅ Hill climber converged — step {:.0} < {} bytes. \
                             Peak found (or close enough). Time for a nap. 💤",
                            self.the_step_size, THE_MINIMUM_STEP_WORTH_TAKING as usize
                        );
                    } else {
                        self.the_current_request_size +=
                            self.the_heading * self.the_step_size;
                        debug!(
                            "🔄 Hill climber: worsened {:.1}% — reversed to {} bytes (step={:.0})",
                            the_delta * 100.0,
                            self.the_current_request_size as usize,
                            self.the_step_size
                        );
                    }
                } else {
                    // 😐 Noise band — stay put, count the boredom
                    self.the_previous_window_vibe = Some(the_median);
                    self.the_boredom_counter += 1;

                    if self.the_boredom_counter >= self.the_boredom_threshold {
                        // 🔍 Re-explore — been too quiet for too long, the landscape may have shifted
                        self.the_step_size = (self.the_current_request_size / 4.0).max(THE_MINIMUM_STEP_WORTH_TAKING);
                        self.the_heading = 1.0;
                        self.the_seeker_found_peace = false;
                        self.the_boredom_counter = 0;
                        self.the_current_request_size +=
                            self.the_heading * self.the_step_size;

                        info!(
                            "🔍 Hill climber re-exploring after {} quiet windows — \
                             step={:.0}, heading to {} bytes. Like a dog doing zoomies. 🐕",
                            self.the_boredom_threshold,
                            self.the_step_size,
                            self.the_current_request_size as usize
                        );
                    }
                }

                self.the_current_request_size = self
                    .the_current_request_size
                    .clamp(self.the_floor, self.the_ceiling);
            }
        }

        self.the_current_request_size
    }

    /// 💀 Process an error (429, timeout) — AIMD halve + cooldown + full reset.
    ///
    /// "When the sink says 429, you don't argue. You halve. You chill. You reflect.
    ///  Like a rejected Tinder match — swipe left on the request size." 💔
    fn on_error(&mut self) -> f64 {
        let the_old_size = self.the_current_request_size;
        self.the_current_request_size =
            (self.the_current_request_size * 0.5).max(self.the_floor);
        self.the_mandatory_chill_until = Instant::now() + THE_ERROR_COOLDOWN;
        self.reset_the_hill_climber();

        warn!(
            "💀 Error received — halved: {} → {} bytes. \
             Cooling down for {}s. Like putting ice on a burn. 🧊",
            the_old_size as usize,
            self.the_current_request_size as usize,
            THE_ERROR_COOLDOWN.as_secs()
        );

        self.the_current_request_size
    }

    /// 🔄 Reset the hill climber state — fresh start after a traumatic event.
    ///
    /// "The hill climber forgets everything and starts over.
    ///  Like a goldfish. Or me after a weekend." 🐠
    fn reset_the_hill_climber(&mut self) {
        // Step must be at least the minimum — otherwise a reset at the floor
        // produces step=32K < 64K → instant convergence → climber never climbs back
        self.the_step_size = (self.the_current_request_size / 4.0).max(THE_MINIMUM_STEP_WORTH_TAKING);
        self.the_heading = 1.0;
        self.the_throughput_samples.clear();
        self.the_window_start = Instant::now();
        self.the_previous_window_vibe = None;
        self.the_boredom_counter = 0;
        self.the_seeker_found_peace = false;
    }
}

impl Regulate for ThroughputSeeker {
    /// 🔄 Feed a GaugeReading, get an adjusted request size.
    ///
    /// DrainResult → compute throughput, run circuit breaker + hill climber.
    /// Error → immediate halve + cooldown + reset.
    /// CpuValue/LatencyMs → politely ignored. Not our department. 📋🦆
    fn regulate(&mut self, reading: GaugeReading, _since_last_checked_ms: Duration) -> f64 {
        match reading {
            GaugeReading::DrainResult {
                payload_bytes,
                latency_ms,
            } => {
                // 📊 bytes/sec = bytes / (ms / 1000) = bytes * 1000 / ms
                let the_throughput = if latency_ms == 0 {
                    // ⚡ Zero latency? Either time travel or an in-memory sink
                    payload_bytes as f64 * 1000.0
                } else {
                    payload_bytes as f64 / (latency_ms as f64 / 1000.0)
                };

                self.on_drain_complete(the_throughput)
            }
            GaugeReading::Error() => self.on_error(),
            GaugeReading::CpuValue(_) | GaugeReading::LatencyMs(_) => {
                // 🤷 Not our signal — return current size unchanged, like a cat ignoring commands
                self.the_current_request_size
            }
        }
    }
}

/// 📊 Compute the median of a mutable slice — the wisdom of the middle path.
///
/// Sorts in-place (the caller already drained the window, so mutation is free).
/// Middle value for odd-length, average of two middles for even-length.
/// Empty slice → 0.0, because even nothing has a median in this codebase. 🧘🦆
fn the_wisdom_of_the_middle(the_values: &mut [f64]) -> f64 {
    if the_values.is_empty() {
        return 0.0;
    }
    the_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let the_len = the_values.len();
    if the_len % 2 == 0 {
        (the_values[the_len / 2 - 1] + the_values[the_len / 2]) / 2.0
    } else {
        the_values[the_len / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔧 Test config with instant windows — ain't nobody got time for 5-second waits in CI ⏱️
    fn test_config() -> ThroughputSeekerConfig {
        ThroughputSeekerConfig {
            min_request_size_bytes: 131_072,
            initial_output_bytes: 4_194_304,
            window_duration_secs: 0,
            improvement_threshold_pct: 10.0,
            degradation_threshold_pct: 20.0,
            re_explore_after_windows: 30,
        }
    }

    /// 🔧 Shorthand: build a DrainResult from payload size + latency 📦
    fn drain_reading(payload_bytes: u64, latency_ms: u64) -> GaugeReading {
        GaugeReading::DrainResult {
            payload_bytes,
            latency_ms,
        }
    }

    /// 🔧 Feed a reading and return the output — the regulate() convenience wrapper 🎛️
    fn feed(seeker: &mut ThroughputSeeker, reading: GaugeReading) -> f64 {
        seeker.regulate(reading, Duration::from_millis(100))
    }

    /// 🧪 The one where it finds the peak.
    /// Feed ascending throughput, then worsening — seeker should climb up then react to decline.
    /// Like Goldilocks but with bytes/sec instead of porridge temperature. 🥣🦆
    #[test]
    fn the_one_where_it_finds_the_peak() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        // 🚀 Phase 1: improving throughput — 40→50→67 MB/s
        let the_initial = feed(&mut the_seeker, drain_reading(4_000_000, 100));
        feed(&mut the_seeker, drain_reading(4_000_000, 80));
        let the_ascending_peak = feed(&mut the_seeker, drain_reading(4_000_000, 60));

        // 🎯 Request size should have stepped up during ascending phase
        assert!(
            the_ascending_peak >= the_initial,
            "🎯 Improving throughput should increase request size — {} >= {}",
            the_ascending_peak, the_initial
        );

        // 📉 Phase 2: worsening throughput — 20 MB/s (5x worse than peak)
        for _ in 0..5 {
            feed(&mut the_seeker, drain_reading(4_000_000, 200));
        }

        let the_after_decline = the_seeker.the_current_request_size;

        // 🎯 After decline, seeker should have reduced (hill climber reversal or circuit breaker)
        assert!(
            the_after_decline < the_ascending_peak,
            "🎯 Declining throughput should reduce request size — {} < {}",
            the_after_decline, the_ascending_peak
        );
    }

    /// 🧪 The one where it settles and stops moving.
    /// Force enough reversals via alternating throughput until step < 64 KiB → converged.
    /// Like a pendulum that finally stops swinging. Or my will to debug CSS. 🎯🦆
    #[test]
    fn the_one_where_it_settles_and_stops_moving() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        // 🎬 Init EMAs at 40 MB/s baseline
        feed(&mut the_seeker, drain_reading(4_000_000, 100));

        // 🔄 Alternate 40 vs 30 MB/s — triggers hill climber reversals without circuit breaker
        // Initial step = 4MiB/4 = 1MiB. After ×0.618 per reversal:
        // 1048K → 648K → 400K → 247K → 153K → 94K → 58K < 64K → converged (6 reversals)
        for i in 0..20 {
            if i % 2 == 0 {
                feed(&mut the_seeker, drain_reading(4_000_000, 100));
            } else {
                feed(&mut the_seeker, drain_reading(4_000_000, 133));
            }
        }

        assert!(
            the_seeker.the_seeker_found_peace,
            "🎯 After many reversals, step should have shrunk below 64KiB — \
             step={:.0}, peace={}. The seeker needs more therapy.",
            the_seeker.the_step_size, the_seeker.the_seeker_found_peace
        );
    }

    /// 🧪 The one where the circuit breaker trips.
    /// Stable throughput, then sudden 10x drop → fast EMA crashes → TRIP → immediate halve.
    /// Like a stock market flash crash but for your bulk API. 📉🦆
    #[test]
    fn the_one_where_the_circuit_breaker_trips() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        // 📈 Build up stable 40 MB/s baseline — EMAs converge to 40
        for _ in 0..30 {
            feed(&mut the_seeker, drain_reading(4_000_000, 100));
        }

        let the_size_before_trip = the_seeker.the_current_request_size;

        // 💥 Sudden 10x throughput collapse — 4 MB/s
        for _ in 0..3 {
            feed(&mut the_seeker, drain_reading(4_000_000, 1000));
        }

        let the_size_after_trip = the_seeker.the_current_request_size;

        // 🎯 Circuit breaker should have halved request size
        assert!(
            the_size_after_trip < the_size_before_trip,
            "🎯 Circuit breaker should reduce request size on throughput collapse — \
             before={}, after={}. The breaker didn't break.",
            the_size_before_trip, the_size_after_trip
        );
    }

    /// 🧪 The one where the circuit breaker resets the climber.
    /// After trip: prev_median=None, boredom=0, peace=false. Full amnesia.
    /// Like a GPS recalculating after you ignore 5 "make a U-turn" warnings. 📡🦆
    #[test]
    fn the_one_where_the_circuit_breaker_resets_the_climber() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        // 📈 Build up state with stable throughput
        for _ in 0..30 {
            feed(&mut the_seeker, drain_reading(4_000_000, 100));
        }

        // 💥 Trip the circuit breaker with sudden collapse
        for _ in 0..3 {
            feed(&mut the_seeker, drain_reading(4_000_000, 1000));
        }

        // 🎯 Hill climber state should be fully reset
        assert!(
            the_seeker.the_previous_window_vibe.is_none(),
            "🎯 Circuit breaker should reset prev_median to None — the past is dead"
        );
        assert_eq!(
            the_seeker.the_boredom_counter, 0,
            "🎯 Circuit breaker should reset boredom counter to 0"
        );
        assert!(
            !the_seeker.the_seeker_found_peace,
            "🎯 Circuit breaker should reset convergence flag — no peace after war"
        );
    }

    /// 🧪 The one where safety halves on error.
    /// GaugeReading::Error() → immediate halve + cooldown. No questions asked. No appeals.
    /// Like your browser's "are you sure?" dialog, except this one actually does something. 💀🦆
    #[test]
    fn the_one_where_safety_halves_on_error() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        let the_initial_size = the_seeker.the_current_request_size;

        // 💀 Error hits
        let the_output = feed(&mut the_seeker, GaugeReading::Error());

        let the_expected = (the_initial_size * 0.5).max(131_072.0);
        assert!(
            (the_output - the_expected).abs() < 1.0,
            "🎯 Error should halve request size — expected {}, got {}",
            the_expected, the_output
        );

        // 🎯 Double error → halve again
        let the_double_output = feed(&mut the_seeker, GaugeReading::Error());
        let the_double_expected = (the_expected * 0.5).max(131_072.0);
        assert!(
            (the_double_output - the_double_expected).abs() < 1.0,
            "🎯 Double error should halve again — expected {}, got {}",
            the_double_expected, the_double_output
        );
    }

    /// 🧪 The one where it re-explores after settling.
    /// N consecutive noise-band windows → re-exploration with fresh step size.
    /// Like a dog that sat still for too long and suddenly decides it's zoomies time. 🐕🦆
    #[test]
    fn the_one_where_it_re_explores_after_settling() {
        let the_config = ThroughputSeekerConfig {
            re_explore_after_windows: 5,
            ..test_config()
        };
        let mut the_seeker = ThroughputSeeker::new(&the_config, 64_000_000.0);

        // 🎬 Reading 1: init EMAs
        feed(&mut the_seeker, drain_reading(4_000_000, 100));

        // 🧗 Reading 2: first window — establishes baseline, steps forward
        feed(&mut the_seeker, drain_reading(4_000_000, 100));

        // 😐 Readings 3-7: same throughput → noise band → boredom 1..5
        for _ in 0..5 {
            feed(&mut the_seeker, drain_reading(4_000_000, 100));
        }

        // 🎯 After 5 noise-band windows, re-exploration should have triggered —
        // step should be reset to x/4 (well above 64KiB minimum)
        assert!(
            the_seeker.the_step_size > THE_MINIMUM_STEP_WORTH_TAKING,
            "🎯 After re-exploration, step should be reset to x/4 (> 64KiB) — got {:.0}",
            the_seeker.the_step_size
        );
        assert_eq!(
            the_seeker.the_boredom_counter, 0,
            "🎯 Re-exploration should reset boredom counter"
        );
    }

    /// 🧪 The one where noise doesn't cause whiplash.
    /// Feed throughput with ±30% jitter around a mean — circuit breaker should not trip.
    /// Like noise-canceling headphones for your throughput signal. 🎧🦆
    #[test]
    fn the_one_where_noise_doesnt_cause_whiplash() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        // 📊 Jittery throughput: 4MB at varying latencies (77-133ms ≈ 30-52 MB/s)
        let the_latencies = [
            100, 130, 77, 110, 90, 125, 80, 105, 95, 120, 85, 100, 130, 77, 110, 90,
        ];

        for &latency in &the_latencies {
            feed(&mut the_seeker, drain_reading(4_000_000, latency));
        }

        // 🎯 Request size should be above the floor — no catastrophic collapse
        assert!(
            the_seeker.the_current_request_size >= the_seeker.the_floor,
            "🎯 Noisy throughput should not crash request size to floor — got {:.0}",
            the_seeker.the_current_request_size
        );
    }

    /// 🧪 The one where GC spikes don't fool the median.
    /// Test the median helper directly — outliers should be invisible to the middle path.
    /// Median: the Switzerland of statistics. Neutral. Unbothered. 🇨🇭🦆
    #[test]
    fn the_one_where_gc_spikes_dont_fool_the_median() {
        // 📊 Normal readings with one 10x outlier
        let mut the_values = vec![100.0, 100.0, 100.0, 1000.0, 100.0];
        let the_median = the_wisdom_of_the_middle(&mut the_values);

        assert!(
            (the_median - 100.0).abs() < f64::EPSILON,
            "🎯 Median of [100,100,100,1000,100] should be 100, got {} — \
             the outlier should be invisible. Like my dating profile. 💀",
            the_median
        );

        // 📊 Even-length with outlier
        let mut the_even = vec![100.0, 100.0, 1000.0, 100.0];
        let the_even_median = the_wisdom_of_the_middle(&mut the_even);
        assert!(
            (the_even_median - 100.0).abs() < f64::EPSILON,
            "🎯 Median of [100,100,1000,100] should be 100, got {}",
            the_even_median
        );

        // 📊 Empty — the void has a median of zero
        let mut the_void: Vec<f64> = vec![];
        assert!(
            the_wisdom_of_the_middle(&mut the_void).abs() < f64::EPSILON,
            "🎯 Median of empty vec should be 0"
        );

        // 📊 Single value — you are your own median
        let mut the_lonely = vec![42.0];
        assert!(
            (the_wisdom_of_the_middle(&mut the_lonely) - 42.0).abs() < f64::EPSILON,
            "🎯 Median of [42] should be 42"
        );
    }

    /// 🧪 The one where CpuValue and LatencyMs are politely ignored.
    /// ThroughputSeeker only cares about DrainResult. Everything else gets the cold shoulder.
    /// Like a cat when you call its name. 🐱🦆
    #[test]
    fn the_one_where_irrelevant_readings_are_ignored() {
        let mut the_seeker = ThroughputSeeker::new(&test_config(), 64_000_000.0);

        let the_initial = the_seeker.the_current_request_size;

        // 🤷 Feed CPU and Latency readings — should be ignored entirely
        let the_after_cpu = feed(&mut the_seeker, GaugeReading::CpuValue(95));
        let the_after_latency = feed(&mut the_seeker, GaugeReading::LatencyMs(999));

        assert!(
            (the_after_cpu - the_initial).abs() < f64::EPSILON,
            "🎯 CpuValue should not change request size — {} vs {}",
            the_after_cpu, the_initial
        );
        assert!(
            (the_after_latency - the_initial).abs() < f64::EPSILON,
            "🎯 LatencyMs should not change request size — {} vs {}",
            the_after_latency, the_initial
        );
    }
}
