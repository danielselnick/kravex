//! 🚀 kvx — the core library crate, the beating heart, the engine room
//! where dreams of zero-config search migration become mildly-configured reality.
//!
//! 📦 This crate contains the supervisor, the workers, and all the existential
//! dread that comes with building a data migration tool for fun. 🦆
//!
//! ⚠️ "The singularity will happen before this crate reaches 1.0"

// -- 🗑️ TODO: clean up the dedz (dead code, not the grateful kind)
#![allow(dead_code, unused_variables, unused_imports)]
pub mod config;
pub mod backends;
pub mod manifolds;
pub mod progress;
pub mod foreman;
pub mod casts;
pub mod regulators;
pub mod workers;

use crate::config::AppConfig;
use crate::backends::elasticsearch::{ElasticsearchSink, ElasticsearchSource};
use crate::backends::file::{FileSink, FileSource};
use crate::backends::in_mem::{InMemorySink, InMemorySource};
use crate::backends::meilisearch::MeilisearchSink;
use crate::backends::{SinkBackend, SourceBackend};
use crate::foreman::Foreman;
use crate::config::{RuntimeConfig, SinkConfig, SourceConfig};
use crate::manifolds::ManifoldBackend;
use crate::casts::PageToEntriesCaster;
use crate::regulators::pressure_gauge::FlowKnob;
use crate::workers::FlowMasterConfig;
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

    // 🔄 Resolve the caster from source/sink config pair.
    // 🧠 Knowledge graph: DocumentCaster::from_configs() matches (source, sink) → caster.
    // File→ES = NdJsonToBulk, File→File = Passthrough, InMemory→InMemory = Passthrough, etc.
    let caster =
        PageToEntriesCaster::from_configs(&app_config.source_config, &app_config.sink_config);

    // 🎼 Resolve the manifold from sink config.
    // 🧠 ES/File → NdjsonManifold, InMemory → JsonArrayManifold.
    // The Manifold casts raw feeds AND joins them into wire format. Two birds, one Cow. 🐄
    let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);

    // 📏 Extract max request size from sink config — the hard ceiling for payload size.
    let max_request_size_bytes = app_config.sink_config.max_request_size_bytes();

    // 🔧 Create the FlowKnob — shared atomic valve between FlowMaster and joiners.
    // 🧠 FlowMasterConfig determines the initial value:
    //   - Static: fixed at output_bytes, never changes (no FlowMaster spawned)
    //   - Latency: starts at initial_output_bytes, PID adjusts based on drain latency
    //   - CPU: starts at initial_output_bytes, PID adjusts based on cluster CPU pressure
    let the_initial_flow = match &app_config.flow_master {
        FlowMasterConfig::Static(cfg) => cfg.output_bytes,
        FlowMasterConfig::Latency(cfg) => cfg.initial_output_bytes,
        FlowMasterConfig::CPU(cfg) => cfg.initial_output_bytes,
        FlowMasterConfig::Throughput(cfg) => cfg.initial_output_bytes,
    };
    let the_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(the_initial_flow));

    info!(
        "🎛️ FlowMaster mode: {:?} — initial flow: {} bytes",
        std::mem::discriminant(&app_config.flow_master),
        the_initial_flow
    );

    // 📏 Extract pipeline name and total_expected_bytes for progress reporting.
    // File sources know their size upfront; everything else is a mystery. 🎭
    let (pipeline_name, total_expected_bytes) = match &source_backend {
        SourceBackend::File(fs) => (fs.source_config.file_name.clone(), fs.file_size),
        SourceBackend::Elasticsearch(_) => ("elasticsearch".to_string(), 0),
        SourceBackend::InMemory(_) => ("in-memory".to_string(), 0),
    };

    // 🔍 Override pipeline name if sink is Meilisearch — so the progress bar says "→ meilisearch"
    let pipeline_name = match &app_config.sink_config {
        SinkConfig::Meilisearch(ms) => format!("{} → meilisearch/{}", pipeline_name, ms.index_uid),
        _ => pipeline_name,
    };

    let foreman = Foreman::new(app_config.clone());
    foreman
        .start_workers(
            source_backend,
            sink_backends,
            caster,
            manifold,
            the_flow_knob,
            &app_config.flow_master,
            max_request_size_bytes,
            pipeline_name,
            total_expected_bytes,
        )
        .await?;

    info!(
        "🎉 MIGRATION COMPLETE! Took: {:#?} — not bad for a Rust crate that was \"almost done\" six sprints ago 🦆",
        start_time.elapsed()?
    );
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
        // -- 🔍 Meilisearch sink: JSON arrays in, async tasks out. Like DoorDash but for search indices.
        // -- Bearer token auth, task polling, and the quiet confidence of a search engine that
        // -- auto-creates indices like it's nobody's business. Because it isn't. 🦆
        SinkConfig::Meilisearch(ms_cfg) => {
            let sink = MeilisearchSink::new(ms_cfg.clone()).await?;
            Ok(SinkBackend::Meilisearch(sink))
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
pub struct Page(pub String);

impl Deref for Page {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Page {
    fn from(s: String) -> Self {
        Page(s)
    }
}

// 📦 A fully assembled, wire-ready payload — the final form before I/O.
#[derive(Debug, Clone, PartialEq)]
pub struct Payload(pub String);

impl Deref for Payload {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Payload {
    fn from(s: String) -> Self {
        Payload(s)
    }
}

impl PartialEq<&str> for Payload {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, PartialEq)]
pub struct Entry(pub String);
impl Deref for Entry {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Entry {
    fn from(s: String) -> Self {
        Entry(s)
    }
}

impl PartialEq<&str> for Entry {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

pub enum GaugeReading {
    CpuValue(usize),
    LatencyMs(usize),
    DrainResult { payload_bytes: u64, latency_ms: u64 },
    Error()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RuntimeConfig, SinkConfig, SourceConfig};
    use crate::backends::{CommonSourceConfig, CommonSinkConfig};
    use crate::backends::meilisearch::MeilisearchSinkConfig;
    use crate::backends::file::FileSourceConfig;

    /// 🧪 Full pipeline integration: InMemory→Passthrough→InMemory.
    /// Four raw docs in (as one newline-delimited feed), one JSON array payload out.
    ///
    /// 🧠 InMemory source returns one feed: "{"doc":1}\n{"doc":2}\n{"doc":3}\n{"doc":4}".
    /// Passthrough returns the entire feed as-is.
    /// JsonArrayManifold wraps it as [feed_content].
    ///
    /// 🐄 Zero-copy verification: passthrough borrows from the buffered feed, no per-doc alloc.
    #[tokio::test]
    async fn the_one_where_four_docs_made_it_home_safely() -> Result<()> {
        let app_config = AppConfig {
            runtime: RuntimeConfig {
                pumper_to_joiner_capacity: 10,
                joiner_to_drainer_capacity: 10,
                sink_parallelism: 1,
                joiner_parallelism: 1,
            },
            source_config: SourceConfig::InMemory(()),
            sink_config: SinkConfig::InMemory(()),
            drainer: Default::default(),
            flow_master: Default::default(),
        };

        let source = SourceBackend::InMemory(InMemorySource::new().await?);
        let sink_inner = InMemorySink::new().await?;
        let sink = SinkBackend::InMemory(sink_inner.clone());

        // 🔄 InMemory→InMemory resolves to Passthrough caster
        let caster = PageToEntriesCaster::from_configs(
            &app_config.source_config,
            &app_config.sink_config,
        );

        // 🎼 InMemory sink → JsonArrayManifold: [item,item,...]
        let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);

        // 📏 Max request size from sink config
        let max_request_size_bytes = app_config.sink_config.max_request_size_bytes();

        // 🔧 No regulator for tests — static flow knob at max 🎚️
        let the_test_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(max_request_size_bytes));

        let the_flow_master_config = FlowMasterConfig::default();
        let foreman = Foreman::new(app_config);
        foreman
            .start_workers(source, vec![sink], caster, manifold, the_test_flow_knob, &the_flow_master_config, max_request_size_bytes, "test-pipeline".to_string(), 0)
            .await?;

        // 📦 Joiner received 1 feed (4 docs newline-delimited), passthrough-cast and joined into JSON array.
        // Joiner buffers raw feeds → manifold.join(buffer, caster) → payload on ch2 → Drainer relays to sink.
        // 🧠 Passthrough treats entire feed as one item → payload = '[{"doc":1}\n{"doc":2}\n{"doc":3}\n{"doc":4}]'
        // The feed content includes newlines because passthrough doesn't split — that's by design!
        let received = sink_inner.received.lock().await;
        assert_eq!(received.len(), 1, "Should have received exactly 1 payload");

        let the_payload = &received[0];
        // 📄 Passthrough returns the whole feed as one item, so JSON array wraps the entire feed
        let expected = format!(
            "[{}]",
            [r#"{"doc":1}"#, r#"{"doc":2}"#, r#"{"doc":3}"#, r#"{"doc":4}"#].join("\n")
        );
        assert_eq!(
            the_payload, &expected,
            "InMemory sink should receive a JSON array wrapping the passthrough feed"
        );

        Ok(())
    }

    /// 🧪 Full pipeline integration: File→NdJsonSplit→JsonArray→MeilisearchSink.
    /// Three NDJSON docs from a temp file, through the caster, into a mocked Meilisearch.
    ///
    /// 🎬 COLD OPEN — INT. INTEGRATION TEST SUITE — 2 AM
    /// *[A temp file is born. Three JSON docs are written. The pipeline awakens.]*
    /// *["Are we really going to test the entire pipeline?" asks the test runner.]*
    /// *["Yes," says the developer. "We are professionals. Sort of."]*
    ///
    /// 🧠 File source reads NDJSON → NdJsonSplit splits into individual entries →
    /// JsonArrayManifold joins as [doc1,doc2,doc3] → MeilisearchSink POSTs to wiremock →
    /// Task polling confirms success. The circle of data. 🦁
    #[tokio::test]
    async fn the_one_where_ndjson_docs_migrate_to_meilisearch_like_birds_flying_south() -> Result<()> {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::{method, path, header};
        use std::io::Write;

        // 🎭 Phase 1: Set the stage — temp file with 3 NDJSON docs
        let mut the_temp_file = tempfile::NamedTempFile::new()
            .context("💀 Failed to create temp file. The OS said 'no.' The test said 'fair.'")?;
        let the_doc_a = r#"{"id":1,"title":"The Matrix","year":1999}"#;
        let the_doc_b = r#"{"id":2,"title":"Inception","year":2010}"#;
        let the_doc_c = r#"{"id":3,"title":"Interstellar","year":2014}"#;
        writeln!(the_temp_file, "{}", the_doc_a)?;
        writeln!(the_temp_file, "{}", the_doc_b)?;
        write!(the_temp_file, "{}", the_doc_c)?;
        the_temp_file.flush()?;

        // 📡 Phase 2: Stand up the mock Meilisearch server
        let the_mock_server = MockServer::start().await;

        // 🔧 Mount health check — "I'm alive, thanks for asking"
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"available"}"#))
            .named("meili_health")
            .mount(&the_mock_server)
            .await;

        // 🔍 Mount index check — "yes the index exists, stop asking"
        Mock::given(method("GET"))
            .and(path("/indexes/movies"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"uid":"movies","primaryKey":"id"}"#
            ))
            .named("meili_index_check")
            .mount(&the_mock_server)
            .await;

        // 📡 Mount document POST — returns 202, fire and forget, no task polling
        Mock::given(method("POST"))
            .and(path("/indexes/movies/documents"))
            .respond_with(ResponseTemplate::new(202).set_body_string(
                r#"{"taskUid":7}"#
            ))
            .expect(1..)
            .named("meili_document_post")
            .mount(&the_mock_server)
            .await;

        // 🔧 Phase 3: Build configs
        let the_file_path = the_temp_file.path().to_str().unwrap().to_string();
        let the_source_config = SourceConfig::File(FileSourceConfig {
            file_name: the_file_path,
            common_config: CommonSourceConfig::default(),
        });
        let the_sink_config = SinkConfig::Meilisearch(MeilisearchSinkConfig {
            url: the_mock_server.uri(),
            api_key: None,
            index_uid: "movies".to_string(),
            primary_key: None,
            common_config: CommonSinkConfig::default(),
        });

        let app_config = AppConfig {
            runtime: RuntimeConfig {
                pumper_to_joiner_capacity: 10,
                joiner_to_drainer_capacity: 10,
                sink_parallelism: 1,
                joiner_parallelism: 1,
            },
            source_config: the_source_config.clone(),
            sink_config: the_sink_config.clone(),
            drainer: Default::default(),
            flow_master: Default::default(),
        };

        // 🏗️ Phase 4: Build backends
        let source = FileSource::new(match &the_source_config {
            SourceConfig::File(cfg) => cfg.clone(),
            _ => unreachable!(),
        }).await?;
        let source_backend = SourceBackend::File(source);

        let sink = MeilisearchSink::new(match &the_sink_config {
            SinkConfig::Meilisearch(cfg) => cfg.clone(),
            _ => unreachable!(),
        }).await?;
        let sink_backend = SinkBackend::Meilisearch(sink);

        // 🔄 File→Meilisearch resolves to NdJsonSplit caster
        let caster = PageToEntriesCaster::from_configs(&app_config.source_config, &app_config.sink_config);
        assert!(
            matches!(caster, PageToEntriesCaster::NdJsonSplit(_)),
            "💀 File → Meilisearch should resolve to NdJsonSplit, got {:?}", caster
        );

        // 🎼 Meilisearch sink → JsonArrayManifold: [doc1,doc2,doc3]
        let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);
        assert!(
            matches!(manifold, ManifoldBackend::JsonArray(_)),
            "💀 Meilisearch sink should resolve to JsonArrayManifold"
        );

        let max_request_size_bytes = app_config.sink_config.max_request_size_bytes();
        let the_test_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(max_request_size_bytes));
        let the_flow_master_config = FlowMasterConfig::default();

        // 🚀 Phase 5: Run the full pipeline
        let foreman = Foreman::new(app_config);
        foreman
            .start_workers(
                source_backend,
                vec![sink_backend],
                caster,
                manifold,
                the_test_flow_knob,
                &the_flow_master_config,
                max_request_size_bytes,
                "test-file-to-meili".to_string(),
                0,
            )
            .await?;

        // ✅ Phase 6: Verify — the mock server received at least one document POST
        // wiremock's `expect(1..)` on the POST mock will panic if it wasn't called.
        // If we got here without panic, the pipeline successfully:
        // 1. Read 3 NDJSON lines from the temp file
        // 2. Split them via NdJsonSplit into 3 individual Entry items
        // 3. Joined them via JsonArrayManifold into a JSON array payload
        // 4. Gzipped and POSTed the JSON array to the mocked Meilisearch /documents endpoint
        // 🎉 The data migrated. Fire and forget. The developer slept. 🦆

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
    /// - Source emits ES `_search` PIT response envelopes (2 pages, 3 hits total)
    /// - PitToBulk caster extracts hits → `_bulk` NDJSON action+source pairs
    /// - NdjsonManifold joins entries with `\n`
    /// - InMemorySink captures the final `_bulk` payload for assertion
    ///
    /// 🧠 The trick: InMemorySource holds ES-format pages, but config enums say
    /// `Elasticsearch` so caster/manifold resolution follows the ES→ES code path.
    /// No HTTP. No clusters. No 3am pages. Just pure pipeline verification. 🦆
    #[tokio::test]
    async fn the_one_where_elasticsearch_docs_survive_the_pit_to_bulk_gauntlet() -> Result<()> {
        use crate::backends::elasticsearch::{ElasticsearchSourceConfig, ElasticsearchSinkConfig};
        use crate::backends::{CommonSourceConfig, CommonSinkConfig};

        // 🔧 ES config structs — used ONLY for caster/manifold resolution, not actual connections.
        // These URLs are faker than a three-dollar bill. The pipeline doesn't care.
        // "In a world where configs lied... one test dared to trust the enum dispatch."
        let app_config = AppConfig {
            runtime: RuntimeConfig {
                pumper_to_joiner_capacity: 10,
                joiner_to_drainer_capacity: 10,
                sink_parallelism: 1,
                joiner_parallelism: 1,
            },
            source_config: SourceConfig::Elasticsearch(ElasticsearchSourceConfig {
                url: "http://source-cluster-that-doesnt-exist:9200".to_string(),
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
            flow_master: Default::default(),
        };

        // 📡 Page 1: Two hits from the "movies" index — one with routing, because spicy data is best data
        let the_first_pit_response = Page(r#"{"hits":{"hits":[{"_index":"movies","_id":"neo_1","_source":{"title":"The Matrix","year":1999,"tagline":"Welcome to the real world"}},{"_index":"movies","_id":"inception_2","_routing":"scifi_shard","_source":{"title":"Inception","year":2010,"tagline":"Your mind is the scene of the crime"}}]}}"#.to_string());

        // 📡 Page 2: One hit from a DIFFERENT index — tests cross-index preservation through the pipeline
        // Because real migrations don't always stay in one index. Life is messy. Data is messier.
        let the_second_pit_response = Page(r#"{"hits":{"hits":[{"_index":"classics","_id":"casa_3","_source":{"title":"Casablanca","year":1942,"tagline":"Here is looking at you, kid"}}]}}"#.to_string());

        // 🏗️ Wire the actual backends — InMemory with ES-format pages
        let source = SourceBackend::InMemory(
            InMemorySource::with_pages(vec![the_first_pit_response, the_second_pit_response]),
        );
        let sink_inner = InMemorySink::new().await?;
        let sink = SinkBackend::InMemory(sink_inner.clone());

        // 🔄 ES→ES config resolution → PitToBulk caster (extracts hits from _search envelope)
        let caster = PageToEntriesCaster::from_configs(
            &app_config.source_config,
            &app_config.sink_config,
        );

        // 🎼 ES sink config → NdjsonManifold (action\nsource\n per hit)
        let manifold = ManifoldBackend::from_sink_config(&app_config.sink_config);

        // 📏 Max request size from sink config — with default 64MB, all 3 hits fit in one payload
        let max_request_size_bytes = app_config.sink_config.max_request_size_bytes();

        // 🔧 Static flow knob — no regulator, full throttle, send it and pray 🙏
        let the_test_flow_knob: FlowKnob = Arc::new(AtomicUsize::new(max_request_size_bytes));

        let the_flow_master_config = FlowMasterConfig::default();
        let foreman = Foreman::new(app_config);
        foreman
            .start_workers(
                source,
                vec![sink],
                caster,
                manifold,
                the_test_flow_knob,
                &the_flow_master_config,
                max_request_size_bytes,
                "es-to-es-pit-to-bulk-gauntlet".to_string(),
                0,
            )
            .await?;

        // 📦 Collect all payloads and concatenate — resilient to Joiner batching decisions
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

        // 🎯 Order preservation: Matrix → Inception → Casablanca (page 1 before page 2)
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
