// ai
//! 🎬 *[a payload arrives on ch2. the drainer doesn't flinch.]*
//! *[it sends. it doesn't ask questions. it doesn't buffer. it just sends.]*
//! *["I used to be complex," it whispers. "I had a manifold. A caster. A buffer."]*
//! *["Now I'm just a relay. And honestly? I've never been happier."]* 🗑️🚀🦆
//!
//! 📦 The Drainer — async I/O worker that receives assembled payloads from ch2
//! and sends them to the sink. That's it. That's the whole job.
//!
//! ```text
//! Joiner(s) (std::thread) → ch2 → Drainer(s) (tokio::spawn) → Sink (HTTP/file/memory)
//! ```
//!
//! 🧠 Knowledge graph: the Drainer was once a complex beast that buffered raw feeds,
//! cast them via DocumentCaster, joined them via Manifold, AND sent them to the sink.
//! That CPU-bound work now lives in the Joiner (on std::thread). The Drainer has been
//! liberated. It is now a thin async relay: recv payload → send to sink → repeat.
//! Like a bouncer who only checks wristbands — the hard work happened at the door.
//!
//! ⚠️ The singularity will drain data at the speed of light. We drain at the speed of HTTP.

use super::Worker;
use crate::backends::{Sink, SinkBackend};
use crate::regulators::signals::{DrainErrorKind, PipelineSignal};
use anyhow::{Context, Result};
use async_channel::Receiver;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

// -- 🔄 Retry constants: the "please try again" philosophy of distributed systems
// -- Like asking your kid to clean their room — you know it'll take at least 3 attempts
/// 🔄 Maximum number of retries before we accept our fate (4 total attempts)
const THE_LAST_STRAW: u32 = 3;
/// ⏱️ Base delay in seconds — just enough time to contemplate your choices
const PATIENCE_SEED_SECS: f64 = 1.0;
/// 📈 Backoff multiplier — exponential, like your anxiety during an outage
const ANXIETY_MULTIPLIER: f64 = 2.0;
/// 🧱 Maximum delay cap in seconds — even patience has its limits, like a toddler at a restaurant
const EVEN_PATIENCE_HAS_LIMITS_SECS: f64 = 30.0;

/// 🗑️ The Drainer: thin async relay from ch2 to sink.
///
/// Receives pre-assembled payload Strings from joiners via ch2,
/// sends them to the sink. No buffering, no casting, no manifold.
/// Pure I/O. Like a postman who delivers but never reads the mail. 📬
///
/// 📜 Lifecycle:
/// 1. **Recv**: assembled payload String from ch2 (async)
/// 2. **Send**: payload → Sink::send (HTTP POST, file write, memory push)
/// 3. **Repeat** until ch2 closes (all joiners done)
/// 4. **Close**: Sink::close — flush and finalize 🦆
#[derive(Debug)]
pub struct Drainer {
    /// 📥 ch2 receiver — assembled payloads from the joiner thread pool
    rx: Receiver<String>,
    /// 🚰 The final destination — where payloads go to live their best life (or die trying)
    sink: SinkBackend,
    /// 📡 Signal horn — optional feedback channel to the FlowMaster.
    /// When present, the drainer reports success/failure/429s through this channel.
    /// When absent (no regulator), the drainer drains in blissful silence. 🤫
    the_signal_horn: Option<mpsc::Sender<PipelineSignal>>,
}

impl Drainer {
    /// 🏗️ Construct a Drainer — just a receiver and a sink, like a mailbox with plumbing. 🚰
    ///
    /// "Give a drainer a payload, it sends for a millisecond.
    ///  Give a drainer a channel, it sends until the joiners die." — Ancient proverb 🦆
    pub fn new(
        rx: Receiver<String>,
        sink: SinkBackend,
        the_signal_horn: Option<mpsc::Sender<PipelineSignal>>,
    ) -> Self {
        Self { rx, sink, the_signal_horn }
    }
}

impl Worker for Drainer {
    /// 🚀 Start the drainer — recv from ch2, send to sink, retry if the sink gets moody.
    ///
    /// "You miss 100% of the sends you don't retry." — Wayne Gretzky — Michael Scott 🏒
    fn start(mut self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            debug!("📥 Drainer started — recv from ch2 → send to sink, simple life");

            loop {
                match self.rx.recv().await {
                    Ok(the_payload) => {
                        debug!("📄 Drainer received {} byte payload from ch2", the_payload.len());

                        // 📡 Send the assembled payload to the sink with retry. Because networks
                        // are about as reliable as a cat coming when called.
                        // Skip empty payloads — belt AND suspenders 🩳
                        if !the_payload.is_empty() && the_payload != "[]" {
                            // 🧠 Clone the payload before the retry loop because send() takes
                            // ownership. Each retry needs its own copy, like forwarding the same
                            // passive-aggressive email to HR multiple times.
                            let mut my_therapist_says_move_on: u32 = 0;
                            let the_payload_bytes = the_payload.len();

                            loop {
                                // 🎰 Each attempt gets a fresh clone — send() consumes the String
                                // like a black hole consumes hope
                                let the_payload_clone = the_payload.clone();

                                // ⏱️ Time the drain attempt for latency reporting to FlowMaster
                                let the_stopwatch = std::time::Instant::now();

                                match self.sink.send(the_payload_clone).await {
                                    Ok(()) => {
                                        // 📡 Report success to FlowMaster (if the horn exists)
                                        // try_send — never block the drainer for a signal. Like
                                        // texting "made it home safe" — nice to send, not essential. 📱
                                        if let Some(ref the_horn) = self.the_signal_horn {
                                            let _ = the_horn.try_send(PipelineSignal::DrainSuccess {
                                                payload_bytes: the_payload_bytes,
                                                latency_ms: the_stopwatch.elapsed().as_millis() as u64,
                                            });
                                        }
                                        break;
                                    }
                                    Err(the_bad_news) => {
                                        // 📡 Classify the error and report to FlowMaster before retry logic.
                                        // Phase 1: string parsing (anyhow messages). Phase 2: structured SinkResponse.
                                        // "He who parses error strings, walks a fragile path." — Ancient proverb 🧊
                                        if let Some(ref the_horn) = self.the_signal_horn {
                                            let the_error_msg = format!("{:#}", the_bad_news);
                                            let the_signal = classify_drain_error(
                                                &the_error_msg,
                                                the_payload_bytes,
                                            );
                                            let _ = the_horn.try_send(the_signal);
                                        }

                                        if my_therapist_says_move_on >= THE_LAST_STRAW {
                                            // 💀 All retries exhausted. Time to accept the void.
                                            return Err(the_bad_news).context(format!(
                                                "💀 Drainer exhausted all {} retries sending to sink. \
                                                 We tried. We really tried. Like a 90s modem connecting \
                                                 to AOL — eventually you just give up and go outside.",
                                                THE_LAST_STRAW + 1
                                            ));
                                        }

                                        // ⏱️ Exponential backoff: delay = min(base * multiplier^attempt, max)
                                        // Like the increasing time between texts when someone ghosts you
                                        let the_coping_delay_secs = (PATIENCE_SEED_SECS
                                            * ANXIETY_MULTIPLIER.powi(my_therapist_says_move_on as i32))
                                        .min(EVEN_PATIENCE_HAS_LIMITS_SECS);

                                        my_therapist_says_move_on += 1;

                                        warn!(
                                            "⚠️ Drainer send attempt {}/{} failed: {:#}. \
                                             Retrying in {:.1}s... 🔄 \
                                             (The sink said 'nah' but we're not taking no for an answer. Yet.)",
                                            my_therapist_says_move_on,
                                            THE_LAST_STRAW + 1,
                                            the_bad_news,
                                            the_coping_delay_secs
                                        );

                                        tokio::time::sleep(std::time::Duration::from_secs_f64(
                                            the_coping_delay_secs,
                                        ))
                                        .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // 🏁 ch2 closed — all joiners are done. Close the sink and exit.
                        debug!("🏁 Drainer: ch2 closed. All joiners done. Closing sink. Goodnight. 💤");
                        self.sink
                            .close()
                            .await
                            .context("💀 Drainer failed to close sink — the farewell was awkward")?;
                        return Ok(());
                    }
                }
            }
        })
    }
}

/// 🔍 Classify a drain error from the anyhow error message string.
///
/// Phase 1: string parsing. Phase 2 (future): structured `SinkResponse` from the Sink trait.
/// This is fragile, yes. Like a house of cards. But it works today,
/// and today is all we've got. "He who ships imperfect code, ships." — Ancient proverb 📜
///
/// Classification rules:
/// - Contains "429" → TooManyRequests (the sink is overwhelmed)
/// - Contains "timeout" or "timed out" → DrainError(Timeout)
/// - Contains "connection" → DrainError(ConnectionError)
/// - Matches HTTP status pattern (3-digit number after "status:") → DrainError(HttpStatus)
/// - Everything else → DrainError(Other) — the universal shrug 🤷🦆
fn classify_drain_error(the_error_msg: &str, payload_bytes: usize) -> PipelineSignal {
    let the_lowered = the_error_msg.to_lowercase();

    if the_lowered.contains("429") {
        // 🛑 The big one — sink said "TOO MANY REQUESTS"
        PipelineSignal::TooManyRequests { payload_bytes }
    } else if the_lowered.contains("timeout") || the_lowered.contains("timed out") {
        // ⏱️ The sink is slow, not angry — reduce flow gently
        PipelineSignal::DrainError {
            payload_bytes,
            error_kind: DrainErrorKind::Timeout,
        }
    } else if the_lowered.contains("connection") {
        // 🔌 Network blip — the tubes are kinked
        PipelineSignal::DrainError {
            payload_bytes,
            error_kind: DrainErrorKind::ConnectionError,
        }
    } else {
        // 🤷 No idea. Log it and move on. Like most of adulting.
        PipelineSignal::DrainError {
            payload_bytes,
            error_kind: DrainErrorKind::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::sink::flaky_mock::FlakySink;

    /// 🧪 Helper: create a drainer with a flaky sink and a channel pre-loaded with payloads.
    /// Like setting up a Rube Goldberg machine, but for testing retry logic.
    fn summon_the_test_drainer(
        flaky_sink: FlakySink,
        payloads: Vec<&str>,
    ) -> (Drainer, FlakySink) {
        let (tx, rx) = async_channel::bounded(payloads.len().max(1));

        // 📬 Pre-load the channel with payloads, then drop the sender to close it
        for p in payloads {
            tx.send_blocking(p.to_string()).unwrap();
        }
        drop(tx);

        let the_witness = flaky_sink.clone();
        let drainer = Drainer::new(rx, SinkBackend::FlakyMock(flaky_sink), None);
        (drainer, the_witness)
    }

    #[tokio::test]
    async fn the_one_where_send_fails_and_retries_save_the_day() {
        // 🎬 *[Scene: a sink fails twice, then succeeds. Like every printer ever.]*
        // 🧠 Validates that transient failures don't kill the pipeline —
        // the drainer should retry up to THE_LAST_STRAW times before giving up.
        let flaky = FlakySink::new(2); // 💥 Fail 2 times, succeed on 3rd
        let (drainer, witness) = summon_the_test_drainer(flaky, vec!["test payload"]);

        let honestly_who_knows = drainer.start().await.unwrap();
        assert!(honestly_who_knows.is_ok(), "💀 Drainer should succeed after retries");
        assert_eq!(
            witness.successful_sends.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "🎯 Exactly one payload should have made it through"
        );
    }

    #[tokio::test]
    async fn the_one_where_all_retries_are_exhausted() {
        // 🎬 *[Scene: the sink never cooperates. Like trying to reason with a toddler.]*
        // 🧠 After THE_LAST_STRAW+1 attempts, the drainer should propagate the error.
        let flaky = FlakySink::new(THE_LAST_STRAW + 1); // 💥 Fail more than max retries
        let (drainer, witness) = summon_the_test_drainer(flaky, vec!["doomed payload"]);

        let the_sad_truth = drainer.start().await.unwrap();
        assert!(the_sad_truth.is_err(), "💀 Should fail after exhausting all retries");

        let the_error_message = format!("{:#}", the_sad_truth.unwrap_err());
        assert!(
            the_error_message.contains("exhausted"),
            "🎯 Error should mention exhaustion: {}",
            the_error_message
        );
        assert_eq!(
            witness.successful_sends.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "🎯 No payloads should have succeeded"
        );
    }

    #[tokio::test]
    async fn the_one_where_first_attempt_works() {
        // 🎬 *[Scene: everything works on the first try. Suspicious. Very suspicious.]*
        // 🧠 Happy path — no failures, no retries, no drama. Like a code review
        // with zero comments. It happens. Rarely. But it happens.
        let flaky = FlakySink::new(0); // ✅ No failures — a miracle
        let (drainer, witness) =
            summon_the_test_drainer(flaky, vec!["payload_one", "payload_two", "payload_three"]);

        let honestly_who_knows = drainer.start().await.unwrap();
        assert!(honestly_who_knows.is_ok(), "✅ Should succeed without retries");
        assert_eq!(
            witness.successful_sends.load(std::sync::atomic::Ordering::Relaxed),
            3,
            "🎯 All three payloads should arrive"
        );
    }

    #[tokio::test]
    async fn the_one_where_empty_payloads_are_skipped() {
        // 🎬 *[Scene: the drainer receives empty payloads and does nothing. Efficiency king.]*
        // 🧠 Empty strings and "[]" should be skipped entirely — no send, no retry.
        let flaky = FlakySink::new(0);
        let (drainer, witness) = summon_the_test_drainer(flaky, vec!["", "[]", "real data"]);

        let honestly_who_knows = drainer.start().await.unwrap();
        assert!(honestly_who_knows.is_ok());
        assert_eq!(
            witness.successful_sends.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "🎯 Only the non-empty payload should be sent"
        );
    }

    /// 🧪 The one where error classification detects a 429.
    /// The most important signal in the pipeline — "TOO FAST, HUMAN." 🛑🦆
    #[test]
    fn the_one_where_429_is_classified_as_too_many_requests() {
        let the_signal = classify_drain_error(
            "💀 Elasticsearch said '429 Too Many Requests' — bulk rejected",
            4_000_000,
        );
        match the_signal {
            PipelineSignal::TooManyRequests { payload_bytes } => {
                assert_eq!(payload_bytes, 4_000_000, "🎯 Payload bytes should match");
            }
            other => panic!("🎯 Expected TooManyRequests, got {:?}", other),
        }
    }

    /// 🧪 The one where timeout errors get their own classification.
    /// Timeouts are different from 429s — the sink isn't angry, just slow. Like government. ⏱️🦆
    #[test]
    fn the_one_where_timeout_is_classified_correctly() {
        // 📡 "timeout" keyword
        let the_signal = classify_drain_error("operation timeout after 120s", 2_000_000);
        match the_signal {
            PipelineSignal::DrainError { error_kind: DrainErrorKind::Timeout, .. } => {}
            other => panic!("🎯 Expected Timeout, got {:?}", other),
        }

        // 📡 "timed out" variant
        let the_signal = classify_drain_error("request timed out", 1_000_000);
        match the_signal {
            PipelineSignal::DrainError { error_kind: DrainErrorKind::Timeout, .. } => {}
            other => panic!("🎯 Expected Timeout for 'timed out', got {:?}", other),
        }
    }

    /// 🧪 The one where connection errors are classified.
    /// Network blips happen. Like hiccups but for TCP. 🔌
    #[test]
    fn the_one_where_connection_error_is_classified() {
        let the_signal = classify_drain_error("connection refused to localhost:9200", 1_000_000);
        match the_signal {
            PipelineSignal::DrainError { error_kind: DrainErrorKind::ConnectionError, .. } => {}
            other => panic!("🎯 Expected ConnectionError, got {:?}", other),
        }
    }

    /// 🧪 The one where unknown errors fall through to Other.
    /// The universal "¯\_(ツ)_/¯" of error classification. 🤷🦆
    #[test]
    fn the_one_where_unknown_errors_are_other() {
        let the_signal = classify_drain_error("something completely unexpected happened", 500_000);
        match the_signal {
            PipelineSignal::DrainError { error_kind: DrainErrorKind::Other, .. } => {}
            other => panic!("🎯 Expected Other, got {:?}", other),
        }
    }

    /// 🧪 The one where the drainer sends signals through the horn.
    /// Proof that the feedback loop works end-to-end (within the drainer). 📡🦆
    #[tokio::test]
    async fn the_one_where_drainer_sends_success_signals() {
        let (signal_tx, mut signal_rx) = tokio::sync::mpsc::channel::<PipelineSignal>(16);
        let (tx, rx) = async_channel::bounded(2);
        tx.send("test payload".to_string()).await.unwrap();
        drop(tx);

        let flaky = FlakySink::new(0);
        let drainer = Drainer::new(rx, SinkBackend::FlakyMock(flaky), Some(signal_tx));

        let the_result = drainer.start().await.unwrap();
        assert!(the_result.is_ok(), "💀 Drainer should succeed");

        // 📡 Should have received a DrainSuccess signal
        let the_signal = signal_rx.try_recv().expect("🎯 Should have received a signal");
        match the_signal {
            PipelineSignal::DrainSuccess { payload_bytes, .. } => {
                assert_eq!(payload_bytes, 12, "🎯 Payload bytes should be len of 'test payload'");
            }
            other => panic!("🎯 Expected DrainSuccess, got {:?}", other),
        }
    }
}
