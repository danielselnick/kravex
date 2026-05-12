// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
//! 🚀 kvx — the core library crate, the beating heart, the engine room
//! where dreams of zero-config search migration become precisely-configured reality.
//!
//! 📦 This crate contains the supervisor, the workers, and all the existential
//! dread that comes with building a data migration tool that doesn't suck. 🦆
//!
//! ⚠️ "The singularity will happen, and it'll still use TOML"

pub mod config;
pub mod backends;
pub mod manifolds;
pub mod progress;
pub mod foreman;
pub mod taps;
pub mod regulators;
pub mod workers;
pub mod victory_laps;

use crate::config::AppConfig;
use crate::backends::elasticsearch::{ElasticsearchSink, ElasticsearchSource};
use crate::backends::file::{FileSink, FileSource};
use crate::backends::in_mem::{InMemorySink, InMemorySource};
use crate::backends::{SinkBackend, SourceBackend};
use crate::foreman::Foreman;
use crate::config::{SinkConfig, SourceConfig};
use crate::manifolds::ManifoldBackend;
use crate::taps::BarrelToDraftsTapper;
use crate::workers::GovernorConfig;
use anyhow::{Context, Result};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::SystemTime;
use tracing::info;


/// 🚀 The grand entry point. The big kahuna. The main event.
pub async fn run(app_config: AppConfig) -> Result<()> {
    let start_time = SystemTime::now();
    info!("🚀 KRAVEX IS BLASTING OFF — hold onto your indices, we are MIGRATING, baby!");

    // Build the backends from config
    // Note: We currently don't have implementations, so this will panic or fail when we add them.
    // We are passing an unimplemented mock mapping for now.
    let source_backend = from_source_config(&app_config)
        .await
        .context("Failed to create source backend")?;

    let sink_parallelism = app_config.runtime.sink_parallelism;
    let mut sink_backends = Vec::with_capacity(sink_parallelism);
    for _ in 0..sink_parallelism {
        sink_backends.push(
            from_sink_config(&app_config)
                .await
                .context("Failed to create sink backend")?,
        );
    }

    // 🔄 Resolve the tapper from source/sink config pair.
    // 🧠 Knowledge graph: BarrelToDraftsTapper::from_configs() matches (source, sink) → tapper.
    // File→ES = NdJsonToBulk, File→File = Passthrough, InMemory→InMemory = Passthrough, etc.
    let tapper =
        BarrelToDraftsTapper::from_configs(&app_config.source_config, &app_config.sink_config);

    // 🎼 Resolve the manifold from sink config.
    // 🧠 ES/File → NdjsonManifold, InMemory → JsonArrayManifold.
    // The Manifold casts raw barrels AND joins them into wire format. Two birds, one Cow. 🐄
    let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);

    // 📏 Extract max request size from sink config — the hard ceiling for drum size.
    let max_drum_size_bytes = app_config.sink_config.max_drum_size_bytes();

    // 🔧 Create the FlowKnob — shared atomic valve between Governor and refiners.
    // 🧠 GovernorConfig determines the initial value:
    //   - Static: fixed at output_bytes, never changes (no Governor spawned)
    //   - Latency: starts at initial_output_bytes, PID adjusts based on drain latency
    let the_governator = match &app_config.governor {
        GovernorConfig::Static(cfg) => cfg.output_bytes,
        GovernorConfig::Latency(cfg) => cfg.initial_output_bytes,
        GovernorConfig::Throughput(cfg) => cfg.initial_output_bytes,
    };
    let the_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(the_governator));

    info!(
        "🎛️ Governor mode: {} — initial flow: {} bytes",
        &app_config.governor,
        the_governator
    );

    // 📏 Extract pipeline name and total_expected_bytes for progress reporting.
    // File sources know their size upfront; everything else is a mystery. 🎭
    let (pipeline_name, total_expected_bytes) = match &source_backend {
        SourceBackend::File(fs) => (fs.source_config.file_name.clone(), fs.file_size),
        SourceBackend::Elasticsearch(_) => ("elasticsearch".to_string(), 0),
        SourceBackend::InMemory(_) => ("in-memory".to_string(), 0),
    };

    let foreman = Foreman::new(app_config.clone());
    foreman
        .start_workers(
            source_backend,
            sink_backends,
            tapper,
            manifold,
            the_flow_knob,
            &app_config.governor,
            max_drum_size_bytes,
            pipeline_name,
            total_expected_bytes,
        )
        .await?;

    info!("{}", victory_laps::victory_lap(start_time.elapsed()?));
    Ok(())
}

async fn from_source_config(config: &AppConfig) -> Result<SourceBackend> {
    match &config.source_config {
        // -- 📂 The File arm: ancient, reliable, and smells faintly of 2003.
        // -- Like a filing cabinet that somehow learned async/await.
        SourceConfig::File(file_cfg) => {
            let src = FileSource::new(file_cfg.clone()).await?;
            Ok(SourceBackend::File(src))
        }
        // -- 🧠 The InMemory arm: blazing fast, lives and dies with the process.
        // -- No persistence. No regrets. No disk. Very YOLO.
        SourceConfig::InMemory(_) => {
            let src = InMemorySource::new().await?;
            Ok(SourceBackend::InMemory(src))
        }
        // -- 📡 The Elasticsearch arm: HTTP calls, JSON parsing, and the constant
        // -- fear of a 429 response that ruins your Thursday afternoon.
        SourceConfig::Elasticsearch(es_cfg) => {
            let src = ElasticsearchSource::new(es_cfg.clone()).await?;
            Ok(SourceBackend::Elasticsearch(src))
        }
    }
}

async fn from_sink_config(config: &AppConfig) -> Result<SinkBackend> {
    match &config.sink_config {
        // -- 📂 File sink: data goes in, data stays in. It's basically a digital shoebox
        // -- under the bed. Hope you labeled it.
        SinkConfig::File(file_cfg) => {
            let sink = FileSink::new(file_cfg.clone()).await?;
            Ok(SinkBackend::File(sink))
        }
        // -- 🧠 InMemory sink: it holds all your data, beautifully, until the process
        // -- ends and takes everything with it like a sandcastle at high tide. 🌊
        SinkConfig::InMemory(_) => {
            let sink = InMemorySink::new().await?;
            Ok(SinkBackend::InMemory(sink))
        }
        // -- 📡 Elasticsearch sink: data goes in at the speed of HTTP, which is to say,
        // -- "fast enough until it isn't." May your bulk indexing be ever green. 🌿
        SinkConfig::Elasticsearch(es_cfg) => {
            let sink = ElasticsearchSink::new(es_cfg.clone()).await?;
            Ok(SinkBackend::Elasticsearch(sink))
        }
    }
}

/// 🛑 Stops the migration.
///
/// No really. That's it. `Ok(())`. That's the whole function.
///
/// You might ask: "doesn't this do nothing?" and you would be correct.
/// This function is a philosophical statement. A meditation on impermanence.
/// Someday it will gracefully shut down workers, drain channels, flush buffers,
/// and file its taxes. Today is not that day.
///
/// "The wisest thing I ever wrote was `Ok(())`." — this function, probably.
pub async fn stop() -> Result<()> {
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct Barrel(pub String);

impl Deref for Barrel {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Barrel {
    fn from(s: String) -> Self {
        Barrel(s)
    }
}

// 📦 A fully assembled, wire-ready drum — the final form before I/O.
#[derive(Debug, Clone, PartialEq)]
pub struct Drum(pub String);

impl Deref for Drum {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Drum {
    fn from(s: String) -> Self {
        Drum(s)
    }
}

impl PartialEq<&str> for Drum {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, PartialEq)]
pub struct Draft(pub String);
impl Deref for Draft {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Draft {
    fn from(s: String) -> Self {
        Draft(s)
    }
}

impl PartialEq<&str> for Draft {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// 🔧 The FlowKnob — a shared atomic valve that controls drum size.
///
/// The Governor writes it. The refiners read it. Nobody else touches it.
/// Like the office thermostat, except this one actually works. 🌡️
pub type FlowKnob = Arc<AtomicUsize>;

pub enum GaugeReading {
    DrainResult { drum_bytes: u64, latency_ms: u64 },
    Error()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RuntimeConfig, SinkConfig, SourceConfig};


    /// 🧪 Full pipeline integration: InMemory→Passthrough→InMemory.
    /// Four raw docs in (as one newline-delimited barrel), one JSON array drum out.
    ///
    /// 🧠 InMemory source returns one barrel: "{"doc":1}\n{"doc":2}\n{"doc":3}\n{"doc":4}".
    /// Passthrough returns the entire barrel as-is.
    /// JsonArrayManifold wraps it as [barrel_content].
    ///
    /// 🐄 Zero-copy verification: passthrough borrows from the buffered barrel, no per-doc alloc.
    #[tokio::test]
    async fn the_one_where_four_docs_made_it_home_safely() -> Result<()> {
        let app_config = AppConfig {
            runtime: RuntimeConfig {
                pumper_to_refiner_capacity: 10,
                refiner_to_drainer_capacity: 10,
                sink_parallelism: 1,
                refiner_count: 1,
            },
            source_config: SourceConfig::InMemory(()),
            sink_config: SinkConfig::InMemory(()),
            drainer: Default::default(),
            governor: Default::default(),
        };

        let source = SourceBackend::InMemory(InMemorySource::new().await?);
        let sink_inner = InMemorySink::new().await?;
        let sink = SinkBackend::InMemory(sink_inner.clone());

        // 🔄 InMemory→InMemory resolves to Passthrough tapper
        let tapper = BarrelToDraftsTapper::from_configs(
            &app_config.source_config,
            &app_config.sink_config,
        );

        // 🎼 InMemory sink → JsonArrayManifold: [item,item,...]
        let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);

        // 📏 Max request size from sink config
        let max_drum_size_bytes = app_config.sink_config.max_drum_size_bytes();

        // 🔧 No regulator for tests — static flow knob at max 🎚️
        let the_test_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(max_drum_size_bytes));

        let the_governor_config = GovernorConfig::default();
        let foreman = Foreman::new(app_config);
        foreman
            .start_workers(source, vec![sink], tapper, manifold, the_test_flow_knob, &the_governor_config, max_drum_size_bytes, "test-pipeline".to_string(), 0)
            .await?;

        // 📦 Refiner received 1 barrel (4 docs newline-delimited), passthrough-tapped and joined into JSON array.
        // Refiner accumulates raw barrels → manifold.join(plenum, tapper) → drum on ch2 → Drainer relays to sink.
        // 🧠 Passthrough treats entire barrel as one item → drum = '[{"doc":1}\n{"doc":2}\n{"doc":3}\n{"doc":4}]'
        // The barrel content includes newlines because passthrough doesn't split — that's by design!
        let received = sink_inner.received.lock().await;
        assert_eq!(received.len(), 1, "Should have received exactly 1 drum");

        let the_drum = &received[0];
        // 📄 Passthrough returns the whole barrel as one item, so JSON array wraps the entire barrel
        let expected = format!(
            "[{}]",
            [r#"{"doc":1}"#, r#"{"doc":2}"#, r#"{"doc":3}"#, r#"{"doc":4}"#].join("\n")
        );
        assert_eq!(
            the_drum, &expected,
            "InMemory sink should receive a JSON array wrapping the passthrough barrel"
        );

        Ok(())
    }

    /// 🧪 Full pipeline integration: ES→PitToBulk→NdjsonManifold→ES (all in-memory).
    ///
    /// 🎬 COLD OPEN — INT. DATA CENTER — 3:17 AM
    /// *[Two Elasticsearch clusters sit across from each other in a dimly lit rack.]*
    /// *["I have documents," whispers the source. "I have capacity," replies the sink.]*
    /// *[Between them, PitToBulk cracks its knuckles. "Let's dance."]*
    ///
    /// This test exercises the full ES→ES migration path:
    /// - Source emits ES `_search` PIT response envelopes (2 barrels, 3 hits total)
    /// - PitToBulk tapper extracts hits → `_bulk` NDJSON action+source pairs
    /// - NdjsonManifold joins drafts with `\n`
    /// - InMemorySink captures the final `_bulk` drum for assertion
    ///
    /// 🧠 The trick: InMemorySource holds ES-format barrels, but config enums say
    /// `Elasticsearch` so tapper/manifold resolution follows the ES→ES code path.
    /// No HTTP. No clusters. No 3am barrels. Just pure pipeline verification. 🦆
    #[tokio::test]
    async fn the_one_where_elasticsearch_docs_survive_the_pit_to_bulk_gauntlet() -> Result<()> {
        use crate::backends::elasticsearch::{ElasticsearchSourceConfig, ElasticsearchSinkConfig};
        use crate::backends::{CommonSourceConfig, CommonSinkConfig};

        // 🔧 ES config structs — used ONLY for tapper/manifold resolution, not actual connections.
        // These URLs are faker than a three-dollar bill. The pipeline doesn't care.
        // "In a world where configs lied... one test dared to trust the enum dispatch."
        let app_config = AppConfig {
            runtime: RuntimeConfig {
                pumper_to_refiner_capacity: 10,
                refiner_to_drainer_capacity: 10,
                sink_parallelism: 1,
                refiner_count: 1,
            },
            source_config: SourceConfig::Elasticsearch(ElasticsearchSourceConfig {
                url: "http://source-cluster-that-doesnt-exist:9200".to_string(),
                index: "test-index".to_string(),
                username: None,
                password: None,
                api_key: None,
                common_config: CommonSourceConfig::default(),
            }),
            sink_config: SinkConfig::Elasticsearch(ElasticsearchSinkConfig {
                url: "http://sink-cluster-also-fictional:9200".to_string(),
                username: None,
                password: None,
                api_key: None,
                index: Some("destination-index".to_string()),
                common_config: CommonSinkConfig::default(),
            }),
            drainer: Default::default(),
            governor: Default::default(),
        };

        // 📡 Barrel 1: Two hits from the "movies" index — one with routing, because spicy data is best data
        let the_first_pit_response = Barrel(r#"{"hits":{"hits":[{"_index":"movies","_id":"neo_1","_source":{"title":"The Matrix","year":1999,"tagline":"Welcome to the real world"}},{"_index":"movies","_id":"inception_2","_routing":"scifi_shard","_source":{"title":"Inception","year":2010,"tagline":"Your mind is the scene of the crime"}}]}}"#.to_string());

        // 📡 Barrel 2: One hit from a DIFFERENT index — tests cross-index preservation through the pipeline
        // Because real migrations don't always stay in one index. Life is messy. Data is messier.
        let the_second_pit_response = Barrel(r#"{"hits":{"hits":[{"_index":"classics","_id":"casa_3","_source":{"title":"Casablanca","year":1942,"tagline":"Here is looking at you, kid"}}]}}"#.to_string());

        // 🏗️ Wire the actual backends — InMemory with ES-format barrels
        let source = SourceBackend::InMemory(
            InMemorySource::with_barrels(vec![the_first_pit_response, the_second_pit_response]),
        );
        let sink_inner = InMemorySink::new().await?;
        let sink = SinkBackend::InMemory(sink_inner.clone());

        // 🔄 ES→ES config resolution → PitToBulk tapper (extracts hits from _search envelope)
        let tapper = BarrelToDraftsTapper::from_configs(
            &app_config.source_config,
            &app_config.sink_config,
        );

        // 🎼 ES sink config → NdjsonManifold (action\nsource\n per hit)
        let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);

        // 📏 Max request size from sink config — with default 64MB, all 3 hits fit in one drum
        let max_drum_size_bytes = app_config.sink_config.max_drum_size_bytes();

        // 🔧 Static flow knob — no regulator, full throttle, send it and pray 🙏
        let the_test_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(max_drum_size_bytes));

        let the_governor_config = GovernorConfig::default();
        let foreman = Foreman::new(app_config);
        foreman
            .start_workers(
                source,
                vec![sink],
                tapper,
                manifold,
                the_test_flow_knob,
                &the_governor_config,
                max_drum_size_bytes,
                "es-to-es-pit-to-bulk-gauntlet".to_string(),
                0,
            )
            .await?;

        // 📦 Collect all drums and concatenate — resilient to Refiner batching decisions
        let received = sink_inner.received.lock().await;
        assert!(
            !received.is_empty(),
            "💀 The sink received nothing. The pipeline is a black hole. Check your wiring."
        );

        let the_entire_bulk_body: String = received.iter().map(|s| s.as_str()).collect();

        // ✅ Must end with \n — ES _bulk API is picky about trailing newlines like a grammar teacher
        assert!(
            the_entire_bulk_body.ends_with('\n'),
            "💀 Bulk body must end with \\n — ES will reject this faster than a bad Tinder profile"
        );

        let the_bulk_lines: Vec<&str> = the_entire_bulk_body.lines().collect();

        // 🎯 3 hits × 2 lines each (action + source) = 6 lines total
        assert_eq!(
            the_bulk_lines.len(),
            6,
            "💀 Expected 6 lines (3 hits × 2 lines), got {}. The pipeline ate some docs or hallucinated extras.",
            the_bulk_lines.len()
        );

        // ✅ Every line must be valid JSON — corruption is not a feature, it's a felony
        for (i, line) in the_bulk_lines.iter().enumerate() {
            let _parsed: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| anyhow::anyhow!("💀 Line {i} is not valid JSON: '{line}' — error: {e}"))?;
        }

        // 🎬 Hit 1: The Matrix — action line + source doc
        let the_matrix_action: serde_json::Value = serde_json::from_str(the_bulk_lines[0])?;
        assert_eq!(the_matrix_action["index"]["_index"], "movies", "💀 Hit 1 _index mismatch");
        assert_eq!(the_matrix_action["index"]["_id"], "neo_1", "💀 Hit 1 _id mismatch");
        assert!(
            the_matrix_action["index"].get("_routing").is_none(),
            "💀 Hit 1 should NOT have _routing — it wasn't in the source"
        );
        let the_matrix_doc: serde_json::Value = serde_json::from_str(the_bulk_lines[1])?;
        assert_eq!(the_matrix_doc["title"], "The Matrix", "💀 Hit 1 source doc title mismatch");
        assert_eq!(the_matrix_doc["year"], 1999, "💀 Hit 1 source doc year mismatch");

        // 🎬 Hit 2: Inception — with _routing, the spicy metadata
        let the_inception_action: serde_json::Value = serde_json::from_str(the_bulk_lines[2])?;
        assert_eq!(the_inception_action["index"]["_index"], "movies", "💀 Hit 2 _index mismatch");
        assert_eq!(the_inception_action["index"]["_id"], "inception_2", "💀 Hit 2 _id mismatch");
        assert_eq!(
            the_inception_action["index"]["_routing"], "scifi_shard",
            "💀 Hit 2 _routing mismatch — the routing survived PitToBulk but died in the manifold? Investigate."
        );
        let the_inception_doc: serde_json::Value = serde_json::from_str(the_bulk_lines[3])?;
        assert_eq!(the_inception_doc["title"], "Inception", "💀 Hit 2 source doc title mismatch");

        // 🎬 Hit 3: Casablanca — from a DIFFERENT index, proving cross-index migration works
        let the_casablanca_action: serde_json::Value = serde_json::from_str(the_bulk_lines[4])?;
        assert_eq!(
            the_casablanca_action["index"]["_index"], "classics",
            "💀 Hit 3 _index should be 'classics' — cross-index preservation failed"
        );
        assert_eq!(the_casablanca_action["index"]["_id"], "casa_3", "💀 Hit 3 _id mismatch");
        let the_casablanca_doc: serde_json::Value = serde_json::from_str(the_bulk_lines[5])?;
        assert_eq!(the_casablanca_doc["title"], "Casablanca", "💀 Hit 3 source doc title mismatch");
        assert_eq!(the_casablanca_doc["year"], 1942, "💀 Hit 3 source doc year mismatch");

        // 🎯 Order preservation: Matrix → Inception → Casablanca (barrel 1 before barrel 2)
        // If this fails, either the pipeline is reordering or we're in a parallel universe
        // where Casablanca came before The Matrix. Both are concerning.
        // 🦆 Collect owned Strings because the parsed Values are temporaries that drop after each closure call
        let the_titles: Vec<String> = [1, 3, 5]
            .iter()
            .map(|&i| {
                let doc: serde_json::Value = serde_json::from_str(the_bulk_lines[i]).unwrap();
                doc["title"].as_str().unwrap().to_string()
            })
            .collect();
        assert_eq!(
            the_titles,
            vec!["The Matrix", "Inception", "Casablanca"],
            "💀 Document order not preserved — the pipeline is playing DJ with our data"
        );

        Ok(())
    }
}
