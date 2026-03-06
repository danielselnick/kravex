// ai
//! 🎬 *[INT. PIPELINE — THE RADIO CRACKLES]*
//! *[A drainer sends a distress signal. A manometer reports the pressure.]*
//! *[The FlowMaster hears everything. It always hears everything.]*
//! *["Talk to me, Goose."]* 📡🔧🦆
//!
//! 📦 PipelineSignal — the feedback vocabulary between pipeline workers and the FlowMaster.
//!
//! 🧠 Knowledge graph:
//! - Drainers produce: DrainSuccess, TooManyRequests, DrainError
//! - Manometer produces: CpuReading
//! - FlowMaster consumes all signals via `tokio::sync::mpsc::Receiver`
//! - Channel is bounded (256) — producers use `try_send()` to never block the pipeline
//! - Signals are fire-and-forget from the producer's perspective
//!
//! ⚠️ The singularity will communicate via telepathy. We use enums.

use std::fmt;

// ============================================================
// 🎛️ PipelineSignal — the feedback vocabulary
// ============================================================

/// 📡 Signals flowing from pipeline workers to the FlowMaster.
///
/// Like a walkie-talkie network: drainers report success/failure, the manometer
/// reports CPU readings, and the FlowMaster adjusts the FlowKnob accordingly.
/// "Breaker breaker one-nine, we got a 429 on the eastbound bulk endpoint." 🚛
#[derive(Debug)]
pub enum PipelineSignal {
    /// 📊 CPU reading from the manometer — the cluster's vital signs
    /// Arrives every poll_interval_secs (default 3s) like a heartbeat monitor 💓
    CpuReading { percent: f64 },

    /// 🛑 HTTP 429 Too Many Requests — the sink said "ENOUGH"
    /// This is the emergency brake. FlowMaster halves the knob immediately.
    /// Like a bouncer at a club: "Nobody else gets in until further notice." 🚫
    TooManyRequests { payload_bytes: usize },

    /// ✅ Successful drain — the payload made it home safely
    /// Currently used for debug logging. Future: latency-based secondary regulation.
    /// Like a delivery confirmation from the post office, except it actually arrives. 📬
    DrainSuccess { payload_bytes: usize, latency_ms: u64 },

    /// 💀 Drain error (non-429) — something went wrong, but not the "too fast" kind
    /// Timeouts get special treatment (25% reduction). Other errors are just logged.
    /// Like a doctor categorizing injuries: "This one needs surgery. That one needs a band-aid." 🏥
    DrainError { payload_bytes: usize, error_kind: DrainErrorKind },
}

// ============================================================
// 💀 DrainErrorKind — error taxonomy for the FlowMaster
// ============================================================

/// 💀 Classification of drain errors — because "it broke" is not an actionable diagnosis.
///
/// The FlowMaster uses this to decide how aggressively to throttle:
/// - Timeout → reduce by 25% (the sink is overwhelmed but not rejecting)
/// - ConnectionError → log only (network blip, not a flow problem)
/// - HttpStatus → log with status code (unexpected server error)
/// - Other → log and move on (the universal "¯\_(ツ)_/¯" response)
///
/// "He who does not classify errors, debugs in production with println." — Ancient proverb 📜
#[derive(Debug)]
pub enum DrainErrorKind {
    /// ⏱️ The sink took too long to respond — like waiting for a government office to process anything
    Timeout,
    /// 🔌 Connection failed — the tubes of the internet have kinked
    ConnectionError,
    /// 📡 HTTP status that isn't 429 — the server spoke, but what it said was rude
    HttpStatus(u16),
    /// 🤷 Something else entirely — we don't know and at this point we're afraid to ask
    Other,
}

impl fmt::Display for PipelineSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineSignal::CpuReading { percent } =>
                write!(f, "📊 CpuReading({:.1}%)", percent),
            PipelineSignal::TooManyRequests { payload_bytes } =>
                write!(f, "🛑 TooManyRequests({}B)", payload_bytes),
            PipelineSignal::DrainSuccess { payload_bytes, latency_ms } =>
                write!(f, "✅ DrainSuccess({}B, {}ms)", payload_bytes, latency_ms),
            PipelineSignal::DrainError { payload_bytes, error_kind } =>
                write!(f, "💀 DrainError({}B, {:?})", payload_bytes, error_kind),
        }
    }
}

impl fmt::Display for DrainErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrainErrorKind::Timeout => write!(f, "Timeout"),
            DrainErrorKind::ConnectionError => write!(f, "ConnectionError"),
            DrainErrorKind::HttpStatus(code) => write!(f, "HttpStatus({})", code),
            DrainErrorKind::Other => write!(f, "Other"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The one where signals construct without drama.
    /// If an enum variant can't be created, Rust has bigger problems than we do. 🦆
    #[test]
    fn the_one_where_signals_are_born_without_complications() {
        // 📊 CPU reading — the manometer's heartbeat
        let the_heartbeat = PipelineSignal::CpuReading { percent: 72.5 };
        assert!(format!("{}", the_heartbeat).contains("72.5"));

        // 🛑 429 — the emergency brake
        let the_rejection = PipelineSignal::TooManyRequests { payload_bytes: 4_194_304 };
        assert!(format!("{}", the_rejection).contains("4194304"));

        // ✅ Success — a rare breed in distributed systems
        let the_miracle = PipelineSignal::DrainSuccess { payload_bytes: 1024, latency_ms: 42 };
        assert!(format!("{}", the_miracle).contains("42ms"));

        // 💀 Error — the old familiar
        let the_inevitable = PipelineSignal::DrainError {
            payload_bytes: 2048,
            error_kind: DrainErrorKind::Timeout,
        };
        assert!(format!("{}", the_inevitable).contains("Timeout"));
    }

    /// 🧪 The one where error kinds display their inner truth.
    /// Debug output is for machines. Display output is for humans at 3am. 🌙
    #[test]
    fn the_one_where_error_kinds_explain_themselves() {
        assert_eq!(format!("{}", DrainErrorKind::Timeout), "Timeout");
        assert_eq!(format!("{}", DrainErrorKind::ConnectionError), "ConnectionError");
        assert_eq!(format!("{}", DrainErrorKind::HttpStatus(503)), "HttpStatus(503)");
        assert_eq!(format!("{}", DrainErrorKind::Other), "Other");
    }

    /// 🧪 The one where Debug works because derive(Debug) is doing the heavy lifting.
    /// If derive(Debug) ever breaks, we have bigger problems. Like the singularity. 🤖🦆
    #[test]
    fn the_one_where_debug_formatting_exists() {
        let the_signal = PipelineSignal::CpuReading { percent: 99.9 };
        let the_debug = format!("{:?}", the_signal);
        assert!(the_debug.contains("CpuReading"), "🎯 Debug should contain variant name");

        let the_error = DrainErrorKind::HttpStatus(418);
        let the_debug = format!("{:?}", the_error);
        assert!(the_debug.contains("418"), "🎯 Debug should contain the status code — I'm a teapot ☕");
    }
}
