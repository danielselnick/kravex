// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// AI
//! 📊🚀🦆 progress/ — The Mission Control of migration visibility.
//!
//! *[EXT. MISSION CONTROL — NIGHT]*
//! *[Banks of monitors flicker. A lone engineer stares at terminal output.]*
//! *["How fast are we going?" they whisper. The progress bar answers.]*
//!
//! Thin API surface for spawning the progress reporter. The real rendering
//! lives in `renderer.rs`. Cluster stats polling lives in `cluster_stats.rs`.
//! This file just wires config → pollers → reporter. Like a matchmaker,
//! but for structs.

pub mod cluster_stats;
mod renderer;

// -- 📦 Re-exports: the only things the outside world needs from us
pub use renderer::DrainMetrics;

use crate::backends::config::{SinkConfig, SourceConfig};
use crate::config::AppConfig;
use cluster_stats::ClusterStatsPoller;
use renderer::ProgressReporter;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// 🚀 Spawns a tokio task that ticks the progress reporter every 500ms.
///
/// Returns a JoinHandle — the Foreman should .abort() this after all real workers complete.
/// The reporter is a leaf display task: it reads atomics, renders to terminal, and sleeps.
/// Aborting it is safe and expected. Like pulling the plug on a screensaver. 🖥️
///
/// Internally matches on `app_config.source_config` and `app_config.sink_config` to decide
/// whether to create ClusterStatsPollers for ES/OS clusters. Non-ES backends get None.
/// The ProgressReporter owns the polling lifecycle — no external background tasks needed.
///
/// "In the beginning there was no progress bar. And the developer stared into the void.
///  And the void did not stare back, because there was no render loop." — Genesis 0:0 🦆
pub fn spawn_progress_reporter(
    pipeline_name: String,
    drain_metrics: Arc<DrainMetrics>,
    total_expected_bytes: u64,
    app_config: &AppConfig,
) -> JoinHandle<()> {
    // -- 🔍 Inspect source config — if it's ES, build a poller to spy on the cluster
    let the_source_poller = match &app_config.source_config {
        SourceConfig::Elasticsearch(cfg) => Some(ClusterStatsPoller::from_es_config(
            &cfg.url,
            cfg.username.as_deref(),
            cfg.password.as_deref(),
            cfg.api_key.as_deref(),
        )),
        // -- 📂 File/InMemory sources don't have cluster stats. They have feelings, but no stats.
        _ => None,
    };

    // -- 🔍 Inspect sink config — same deal, ES gets a poller, everyone else gets existential dread
    let the_sink_poller = match &app_config.sink_config {
        SinkConfig::Elasticsearch(cfg) => Some(ClusterStatsPoller::from_es_config(
            &cfg.url,
            cfg.username.as_deref(),
            cfg.password.as_deref(),
            cfg.api_key.as_deref(),
        )),
        // -- 🗑️ Non-ES sinks: no cluster to monitor. Blissful ignorance.
        _ => None,
    };

    tokio::spawn(async move {
        let mut the_reporter = ProgressReporter::new(
            pipeline_name,
            drain_metrics,
            total_expected_bytes,
            the_source_poller,
            the_sink_poller,
        );
        loop {
            // -- 💤 sleep 500ms — fast enough to feel responsive, slow enough to not burn CPU
            tokio::time::sleep(Duration::from_millis(500)).await;
            the_reporter.tick().await;
        }
        // -- 🏁 unreachable: this loop runs until aborted by the Foreman.
        // -- Like a hamster wheel, it doesn't stop on its own. 🐹
    })
}
