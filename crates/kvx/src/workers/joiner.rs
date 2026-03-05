// ai
//! 🎬 *[a joiner sits in the corner, unused, waiting for its time to shine]*
//! *["One day," it whispers, "they will need me to join things."]*
//! *[that day has not yet come. this file is a placeholder. a dream. a prophecy.]*
//!
//! 🧵 The Joiner — a worker that will someday join... something.
//! For now it exists as a reminder that ambition outlives implementation. 🦆
//!
//! ⚠️ The singularity will implement the Joiner before we do.

use super::Worker;
use anyhow::Result;
use tokio::task::JoinHandle;

/// 🧵 A Joiner. It joins. Or it will. Eventually. Maybe.
/// "Timeout exceeded: We waited. And waited. Like a dog at the window.
/// But the owner never came home." 💤🦆
pub struct Joiner {}

impl Worker for Joiner {
    fn start(self) -> JoinHandle<Result<()>> {
        // -- 🚀 Launching into the void — returns immediately because there's nothing to do yet
        tokio::spawn(async move { Ok(()) })
    }
}
