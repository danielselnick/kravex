// ai
// Note: The PID controller (pid_bytes_to_ms.rs) is licensed under LICENSE-EE (BSL 1.1).
// The trait definition and static controller are MIT licensed.
//! 🎬 *[INT. CONTROL ROOM — NIGHT]*
//! *[A lone engineer stares at a wall of blinking monitors. Each one shows a different bulk request,
//! crawling across the wire at wildly different speeds. The throughput graph looks like an EKG
//! during a horror movie. A single tear rolls down their cheek.]*
//!
//! *["There has to be a better way," they whisper.]*
//!
//! *[SMASH CUT TO: A PID controller. It's beautiful. It's elegant. It adjusts byte sizes
//! based on measured latencies. The graphs smooth out. The engineer smiles. The borrow checker
//! nods approvingly from the shadows.]*
//!
//! 📦 **Controllers** — the brains behind adaptive throttling in kravex.
//!
//! 🧠 Knowledge graph:
//! - `ThrottleController` trait: the interface that all throttling strategies implement.
//!   Decouples the SinkWorker from any specific strategy — it just calls `measure()` after
//!   each send and reads `output()` for the next buffer size target.
//! - `StaticThrottleController`: the OG. Fixed bytes. No feedback. Like cruise control
//!   on a car with no speedometer. But hey, it works when you know your road.
//! - `PidControllerBytesToMs`: the fancy one. Proportional-Integral-Derivative feedback loop
//!   that adjusts byte output based on measured request duration. Borrowed from control theory
//!   (and a C# implementation that shall not be named).
//! - `ThrottleControllerBackend`: enum dispatch pattern (same as SinkBackend, SourceBackend).
//!   Resolved from config at startup, handed to SinkWorker. Zero-cost dispatch via match.
//!
//! ⚠️ The singularity will arrive before we finish tuning these PID gains. But at least
//! the code will be well-structured when our robot overlords review it. 🦆
//!
//! "In a world where bulk requests must be sized... one module dared to control."
//!   — Kravex: The Throttling (2026, rated PG-13 for mild derivative gain)

mod pid_bytes_to_ms;
mod static_throttle;

pub(crate) use pid_bytes_to_ms::PidControllerBytesToMs;
pub(crate) use static_throttle::StaticThrottleController;

/// 🎯 The universal interface for throttle controllers.
///
/// 🧠 Knowledge graph:
/// - Every throttle strategy (static, PID, future token-bucket, etc.) implements this trait.
/// - SinkWorker owns a `ThrottleControllerBackend` and calls `measure()` + `output()` each cycle.
/// - `measure(duration_ms)`: feed the latest observed request duration into the controller.
///   For static controllers, this is a no-op. For PID, this drives the feedback loop.
/// - `output()`: returns the recommended byte size for the next bulk request.
///   The SinkWorker uses this as its dynamic `max_request_size_bytes`.
///
/// The trait is intentionally minimal — measure in, bytes out.
/// Like a vending machine, but for throughput decisions. 🚀
pub(crate) trait ThrottleController: std::fmt::Debug + Send {
    /// 📡 Feed a measured request duration (in milliseconds) into the controller.
    ///
    /// For static controllers: "cool story bro" (no-op).
    /// For PID controllers: this is the *raison d'être* — the feedback signal
    /// that drives proportional, integral, and derivative corrections. 🔄
    fn measure(&mut self, duration_ms: f64);

    /// 📏 Get the current recommended payload size in bytes.
    ///
    /// After each `measure()` call, this value may change (PID) or stay the same (static).
    /// The SinkWorker reads this to decide when to flush its page buffer. 🎯
    fn output(&self) -> usize;
}

// ============================================================
// 🏗️ ThrottleControllerBackend — the enum dispatcher
// ============================================================

/// 📦 Enum dispatch for throttle controllers.
///
/// 🧠 Knowledge graph: follows the exact same pattern as `SinkBackend` and `SourceBackend`:
/// concrete types wrapped in an enum, delegating via match. Resolved from `ThrottleConfig`
/// at startup in `lib.rs` or `app_config.rs`. Handed to `SinkWorker::new()`.
///
/// "He who dispatches via enum, avoids dyn Trait in production." — Ancient Rust proverb 🚀
#[derive(Debug)]
pub(crate) enum ThrottleControllerBackend {
    /// 🧊 Fixed byte size. No feedback loop. The classic.
    Static(StaticThrottleController),
    /// 🧠 PID control: bytes out, milliseconds in. The future is now.
    PidBytesToMs(PidControllerBytesToMs),
}

impl ThrottleController for ThrottleControllerBackend {
    fn measure(&mut self, duration_ms: f64) {
        match self {
            ThrottleControllerBackend::Static(c) => c.measure(duration_ms),
            ThrottleControllerBackend::PidBytesToMs(c) => c.measure(duration_ms),
        }
    }

    fn output(&self) -> usize {
        match self {
            ThrottleControllerBackend::Static(c) => c.output(),
            ThrottleControllerBackend::PidBytesToMs(c) => c.output(),
        }
    }
}

impl ThrottleControllerBackend {
    /// 🧊 Build a static controller — the "I know what I'm doing" option.
    pub(crate) fn new_static(fixed_bytes: usize) -> Self {
        ThrottleControllerBackend::Static(StaticThrottleController::new(fixed_bytes))
    }

    /// 🧠 Build a PID controller — the "let the math do the driving" option.
    pub(crate) fn new_pid(
        set_point_ms: f64,
        initial_output_bytes: usize,
        min_bytes: usize,
        max_bytes: usize,
    ) -> Self {
        ThrottleControllerBackend::PidBytesToMs(PidControllerBytesToMs::new(
            set_point_ms,
            initial_output_bytes,
            min_bytes,
            max_bytes,
        ))
    }
}
