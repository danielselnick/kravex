//! 🧵 Workers: the backbone of kravex, the unsung heroes, the ones who actually
//! do the work while the Supervisor takes all the credit in the sprint retro.
//!
//! 🚀 This module is like a factory floor, except instead of hard hats
//! we wear `#[derive(Debug)]` and instead of OSHA violations
//! we have borrow checker violations. 🦆
//!
//! ⚠️ "If you're reading this, the code review went poorly."

// 🎉 anyhowwwww.... it's useful! Like duct tape for error handling.
// This is pretty much across the whole world of kravex —
// the universal donor of Result types 🩸
use anyhow::{Context, Result};
use tokio::task::JoinHandle;
use serde::Deserialize;

mod sink_worker;
use sink_worker::SinkWorker;
mod source_worker;
use source_worker::SourceWorker;
use crate::app_config::AppConfig;

/// 🏗️ A background worker, that does work. duh.
///
/// 🎯 The trait that all workers must implement, like a social contract
/// but enforced by the compiler instead of polite society.
///
/// "What's the DEAL with lifetime annotations? You borrow something,
///  you give it back. It's not that hard, Jerry!" — Seinfeld, on Rust
pub trait Worker {
    /// 🚀 Start the worker. Returns a JoinHandle because we trust
    /// but verify. Mostly verify. Okay, we don't trust at all.
    fn start(self) -> JoinHandle<Result<()>>;
}

/// 🚰 Factory function for sink workers.
/// Currently does nothing, which is peak minimalism. ✨
/// TODO: win the lottery, retire, delete this crate
fn new_sink_worker() {}

/// 🧵 Spawn all the workers into the async void and pray 🙏
///
/// 🔄 Takes a config because workers need to know things,
/// like how many of them there should be, and whether the
/// mortgage payment motivation is high enough to keep going.
fn start_workers(config : AppConfig) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        // 📦 Assemble the team! Like the Avengers, but async
        let sink_worker = SinkWorker::new(config.sink_worker_config.clone());
        let source_worker = SourceWorker::new(config.source_worker_config.clone());
        // 💀 TODO: actually .start() these workers
        // right now they're just standing around the water cooler
        // discussing last night's episode of "The Borrow Checker Diaries"
        Ok(())
    })
}

