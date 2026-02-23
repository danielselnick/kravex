//! 🚀 kvx — the core library crate, the beating heart, the engine room
//! where dreams of zero-config search migration become mildly-configured reality.
//!
//! 📦 This crate contains the supervisor, the workers, and all the existential
//! dread that comes with building a data migration tool for fun. 🦆
//!
//! ⚠️ "The singularity will happen before this crate reaches 1.0"

// 🗑️ TODO: clean up the dedz (dead code, not the grateful kind)
#![allow(dead_code, unused_variables, unused_imports)]
mod supervisors;
pub mod app_config;
use anyhow::{Context, Result};
use crate::app_config::AppConfig;
use crate::supervisors::Supervisor;

/// 🚀 The grand entry point. The big kahuna. The main event.
///
/// 🔧 Takes an AppConfig, creates a Supervisor, and then... well...
/// currently just vibes. Like a DJ who set up all the equipment
/// but forgot to bring any music. 🎶
///
/// 💀 If this fails, check your config. Then check it again.
/// Then blame DNS. It's always DNS.
pub async fn run(app_config: AppConfig) -> Result<()> {
    // 🏗️ Load it — ✅ done (thanks, main.rs, you absolute legend)
    // 🚀 Do it — 🔄 in progress (for a very generous definition of "progress")
    let supervisor = Supervisor::new(app_config.clone());
    // 🎯 TODO: supervisor.start_workers().await? — but not today.
    // today we rest. today we return Ok(()). today is a good day.
    Ok(())
}

pub async fn stop() -> Result<()> {
    // Send a control channel message to the supervisor
    Ok(())
}
