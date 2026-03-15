use std::io::Write;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::header::HeaderValue;
use tracing::{debug, info, warn};

use crate::Payload;
use crate::backends::Sink;
use super::config::MeilisearchSinkConfig;

/// 🔍 The Meilisearch sink — raw reqwest + gzip, fire-and-forget. No SDK, no task polling, no drama.
///
/// We tried the SDK life. It pinned reqwest 0.12 and caused dual-compilation.
/// So we went full artisanal — hand-rolled HTTP like the Meilisearch importer tool does.
/// Gzip the payload, POST it, check the status code, move on with our lives.
/// Like a food truck: fast, compressed, no reservations needed. 🌮🦆
///
/// Internally holds:
/// - `the_http_client`: workspace reqwest 0.13 — one client to rule them all
/// - `the_precomputed_documents_url`: `{base}/indexes/{uid}/documents` — pre-baked, zero alloc per request
/// - `the_precomputed_auth_header`: `Bearer {key}` — computed once, reused forever
///
/// 🧠 Knowledge graph:
/// - Sink trait impl: `drain()` gzip-compresses payload, POSTs with Content-Encoding: gzip
/// - Fire-and-forget: Meilisearch returns 202 with taskUid, we don't poll the task
/// - Auth: Bearer token in pre-computed header, injected on every POST
///
/// "In a world where search SDKs caused dependency conflicts...
/// one sink said 'I'll do it myself' and reached for raw reqwest." 🎬
pub struct MeilisearchSink {
    // -- 📡 The HTTP client — workspace reqwest, no dependency conflict, no drama
    the_http_client: reqwest::Client,
    // -- 🔗 Pre-baked documents URL — computed once because format!() per request is for amateurs
    the_precomputed_documents_url: String,
    // -- 🔒 Pre-baked auth header — None for dev instances running wild and free
    the_precomputed_auth_header: Option<HeaderValue>,
}

impl std::fmt::Debug for MeilisearchSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // -- 🔍 reqwest::Client and HeaderValue don't derive Debug nicely, so we improvise like jazz musicians
        f.debug_struct("MeilisearchSink")
            .field("the_precomputed_documents_url", &self.the_precomputed_documents_url)
            .field("has_auth", &self.the_precomputed_auth_header.is_some())
            .finish()
    }
}

#[async_trait]
impl Sink for MeilisearchSink {
    /// 📡 Gzip the JSON array payload and POST it — fire and forget, baby.
    ///
    /// 1. Compress payload bytes with flate2 GzEncoder
    /// 2. POST with Content-Type: application/json + Content-Encoding: gzip
    /// 3. Check 2xx → Ok. Non-2xx → read body, bail with error.
    /// 4. No task polling. Meilisearch queues it. We trust the process. 🙏
    async fn drain(&mut self, payload: Payload) -> Result<()> {
        let the_raw_bytes = payload.0.into_bytes();
        let the_uncompressed_len = the_raw_bytes.len();

        // 🫁 Phase 1: Gzip compress — squeeze those bytes like a stress ball
        let mut the_gzip_encoder = GzEncoder::new(Vec::new(), Compression::default());
        the_gzip_encoder.write_all(&the_raw_bytes)
            .context("💀 Gzip encoder choked on the payload. The bytes went in but didn't come out compressed. Like trying to vacuum-seal a watermelon.")?;
        let the_compressed_bytes = the_gzip_encoder.finish()
            .context("💀 Gzip finalization failed. The encoder started strong but couldn't stick the landing. A metaphor for most of my PRs.")?;

        debug!(
            "🫁 Compressed {} → {} bytes ({:.0}% reduction) — like packing for vacation but actually fitting everything",
            the_uncompressed_len,
            the_compressed_bytes.len(),
            (1.0 - the_compressed_bytes.len() as f64 / the_uncompressed_len as f64) * 100.0
        );

        // 📡 Phase 2: POST the gzipped payload — fire and forget
        let mut the_request = self.the_http_client
            .post(&self.the_precomputed_documents_url)
            .header("Content-Type", "application/json")
            .header("Content-Encoding", "gzip")
            .body(the_compressed_bytes);

        if let Some(ref the_auth_header) = self.the_precomputed_auth_header {
            the_request = the_request.header("Authorization", the_auth_header.clone());
        }

        let the_response = the_request.send().await
            .context("💀 POST to Meilisearch failed. The network said no. Like asking your crush to prom via HTTP and getting a TCP RST.")?;

        // 🎯 Phase 3: Check status — 2xx means queued, anything else means rejected
        let the_status = the_response.status();
        if !the_status.is_success() {
            let the_rejection_letter = the_response.text().await.unwrap_or_else(|_| "<body unreadable>".to_string());
            anyhow::bail!(
                "💀 Meilisearch returned {} — the documents were turned away at the velvet rope. Body: {}. This is the data equivalent of 'we regret to inform you.'",
                the_status,
                the_rejection_letter
            );
        }

        debug!("✅ Meilisearch accepted the payload (202) — documents are queued, our job is done 🏡");

        Ok(())
    }

    /// 🗑️ Nothing to flush — we don't buffer. The Drainer sends complete payloads.
    /// Close is a no-op. Like saying goodbye to a search engine that was never really yours.
    /// The reqwest client drops cleanly. The connection pool waves from the window. 🪟
    async fn close(&mut self) -> Result<()> {
        debug!("🗑️ Meilisearch sink closing — no buffer to flush, just search relevance to mourn");
        Ok(())
    }
}

impl MeilisearchSink {
    /// 🚀 Stand up a new `MeilisearchSink` with raw reqwest — no SDK, no baggage, just HTTP.
    ///
    /// 1. Build reqwest client with tcp_nodelay + pool_idle_timeout (matching ES/OS sinks)
    /// 2. Health check: GET /health — confirm Meilisearch is conscious
    /// 3. Index check: GET /indexes/{uid} — warn on 404 (Meilisearch auto-creates)
    /// 4. Pre-compute documents URL + auth header — zero alloc per request
    ///
    /// "He who constructs without health-checking, debugs in production" — Ancient Proverb 📜
    pub async fn new(config: MeilisearchSinkConfig) -> Result<Self> {
        let the_base_url = config.url.trim_end_matches('/').to_string();

        // 🔧 Build the HTTP client — tcp_nodelay for latency, pool_idle_timeout for cleanup
        let the_http_client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_idle_timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .context("💀 reqwest::Client::builder() failed. This is like failing to open a web browser. Check your TLS stack. Check your life choices.")?;

        // 🔒 Pre-compute auth header — once and done, like a good tattoo decision
        let the_precomputed_auth_header = config.api_key.as_deref().map(|the_key| {
            HeaderValue::from_str(&format!("Bearer {}", the_key))
                .expect("💀 API key contains invalid header characters. Who hurt you?")
        });

        // 📡 Health check — GET /health, expect {"status":"available"}
        let mut the_health_request = the_http_client.get(format!("{}/health", the_base_url));
        if let Some(ref the_auth) = the_precomputed_auth_header {
            the_health_request = the_health_request.header("Authorization", the_auth.clone());
        }
        the_health_request.send().await
            .context("💀 Meilisearch health check failed. The server is either down, unreachable, or pretending not to be home. We knocked. Nobody answered.")?
            .error_for_status()
            .context("💀 Meilisearch health endpoint returned non-2xx. It's alive but unhappy. Like Mondays.")?;

        // 🔍 Index existence check — GET /indexes/{uid}
        let mut the_index_request = the_http_client.get(format!("{}/indexes/{}", the_base_url, config.index_uid));
        if let Some(ref the_auth) = the_precomputed_auth_header {
            the_index_request = the_index_request.header("Authorization", the_auth.clone());
        }
        match the_index_request.send().await {
            Ok(the_resp) if the_resp.status().is_success() => {
                debug!("✅ Index '{}' exists and is ready for documents — like an empty filing cabinet awaiting chaos", config.index_uid);
            }
            Ok(the_resp) if the_resp.status().as_u16() == 404 => {
                warn!(
                    "⚠️ Index '{}' not found — Meilisearch will auto-create it on first document POST. Proceeding with the reckless optimism of a startup founder.",
                    config.index_uid
                );
            }
            Ok(the_resp) => {
                warn!(
                    "⚠️ Index check for '{}' returned {} — unexpected but not fatal. Pressing on like a determined salmon upstream.",
                    config.index_uid, the_resp.status()
                );
            }
            Err(the_err) => {
                warn!(
                    "⚠️ Index check for '{}' failed: {} — couldn't verify but we'll try anyway. YOLO engineering.",
                    config.index_uid, the_err
                );
            }
        }

        // 🔗 Pre-compute documents URL — zero format!() per request
        let mut the_precomputed_documents_url = format!("{}/indexes/{}/documents", the_base_url, config.index_uid);

        // 🔑 Append primaryKey query param if configured — tells Meilisearch which field is the unique ID
        // Without this, Meilisearch infers it from the first doc (looks for *id fields).
        // Datasets like NOAA with no top-level *id field need this or they get `missing_document_id` errors.
        if let Some(ref the_primary_key) = config.primary_key {
            the_precomputed_documents_url.push_str(&format!("?primaryKey={}", the_primary_key));
        }

        info!(
            "🔍 Meilisearch sink initialized — target: {} — raw reqwest + gzip, no SDK, no task polling, just vibes",
            the_precomputed_documents_url
        );

        Ok(Self {
            the_http_client,
            the_precomputed_documents_url,
            the_precomputed_auth_header,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::CommonSinkConfig;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, header};

    /// 🔧 Build a minimal MeilisearchSinkConfig pointing at the mock server.
    fn make_config(mock_url: &str) -> MeilisearchSinkConfig {
        MeilisearchSinkConfig {
            url: mock_url.to_string(),
            api_key: None,
            index_uid: "test-meili-idx".to_string(),
            primary_key: None,
            common_config: CommonSinkConfig::default(),
        }
    }

    /// 🔧 Mount the health endpoint — Meilisearch's way of saying "I'm alive and well, thanks for asking"
    async fn mount_health_check(mock_server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"available"}"#))
            .named("health_check")
            .mount(mock_server)
            .await;
    }

    /// 🔧 Mount the index check — returns 200 for "yes the index exists, stop asking"
    async fn mount_index_check(mock_server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/indexes/test-meili-idx"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"uid":"test-meili-idx","primaryKey":"id"}"#
            ))
            .named("index_check")
            .mount(mock_server)
            .await;
    }

    // ================================================================
    // 🧪 GROUP A: Constructor — Health + Index Checks
    // ================================================================

    /// 🧪 Constructor succeeds when health and index checks both return 200.
    #[tokio::test]
    async fn the_one_where_meilisearch_is_alive_and_the_index_exists() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        let _sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        // ✅ Sink constructed — raw reqwest, no SDK middleman
        Ok(())
    }

    /// 🧪 Constructor still succeeds when index doesn't exist (Meilisearch auto-creates).
    #[tokio::test]
    async fn the_one_where_the_index_is_missing_but_we_press_on_with_reckless_optimism() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;

        // 📡 Index check returns 404 — we warn and continue
        Mock::given(method("GET"))
            .and(path("/indexes/test-meili-idx"))
            .respond_with(ResponseTemplate::new(404).set_body_string(
                r#"{"message":"Index `test-meili-idx` not found.","code":"index_not_found","type":"invalid_request"}"#
            ))
            .named("index_404")
            .mount(&mock_server)
            .await;

        let _sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        Ok(())
    }

    /// 🧪 Constructor fails when health check is unreachable.
    #[tokio::test]
    async fn the_one_where_meilisearch_ghosted_us_on_the_health_check() {
        // 📡 No mock server — connection refused
        let the_result = MeilisearchSink::new(make_config("http://127.0.0.1:1")).await;
        assert!(the_result.is_err(), "💀 Should fail when Meilisearch is unreachable");
    }

    // ================================================================
    // 🧪 GROUP B: Bearer Token Auth
    // ================================================================

    /// 🧪 API key is sent as Bearer token in all requests.
    #[tokio::test]
    async fn the_one_where_the_bearer_token_opens_the_velvet_rope() -> Result<()> {
        let mock_server = MockServer::start().await;

        // 📡 Health check expects Bearer auth
        Mock::given(method("GET"))
            .and(path("/health"))
            .and(header("Authorization", "Bearer meili-master-key-42"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"available"}"#))
            .expect(1)
            .named("authed_health")
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/indexes/test-meili-idx"))
            .and(header("Authorization", "Bearer meili-master-key-42"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"uid":"test-meili-idx","primaryKey":"id"}"#
            ))
            .expect(1)
            .named("authed_index")
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.api_key = Some("meili-master-key-42".to_string());

        let _sink = MeilisearchSink::new(config).await?;
        Ok(())
    }

    // ================================================================
    // 🧪 GROUP C: Document POST — Fire and Forget
    // ================================================================

    /// 🧪 send() gzip-compresses and POSTs JSON array, gets 202, returns Ok.
    #[tokio::test]
    async fn the_one_where_documents_make_it_to_the_promised_land() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        // 📡 Document POST returns 202 — fire and forget, no task polling needed
        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .respond_with(ResponseTemplate::new(202).set_body_string(
                r#"{"taskUid":1337}"#
            ))
            .expect(1)
            .named("document_post")
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        let the_payload = Payload(r#"[{"id":1,"title":"Test Doc"}]"#.to_string());
        sink.drain(the_payload).await?;
        Ok(())
    }

    /// 🧪 send() includes Content-Encoding: gzip header and actually compresses the payload.
    #[tokio::test]
    async fn the_one_where_gzip_squeezes_bytes_like_a_stress_ball() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        // 📡 Verify Content-Encoding: gzip header is present
        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .and(header("Content-Encoding", "gzip"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(202).set_body_string(r#"{"taskUid":42}"#))
            .expect(1)
            .named("gzip_post")
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        let the_payload = Payload(r#"[{"id":1,"title":"Gzip test — compressing dreams since 1992"}]"#.to_string());
        sink.drain(the_payload).await?;
        Ok(())
    }

    /// 🧪 send() fails when Meilisearch returns non-2xx (e.g., 400 Bad Request).
    #[tokio::test]
    async fn the_one_where_meilisearch_rejects_our_documents_like_a_bouncer_at_a_club() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        // 📡 POST returns 400 — bad payload, no entry, go home
        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .respond_with(ResponseTemplate::new(400).set_body_string(
                r#"{"message":"Invalid JSON","code":"bad_request","type":"invalid_request"}"#
            ))
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        let the_result = sink.drain(Payload(r#"not even json lol"#.to_string())).await;
        assert!(the_result.is_err(), "💀 Non-2xx response should propagate as error");
        Ok(())
    }

    // ================================================================
    // 🧪 GROUP D: close() — The Grand Finale of Nothing
    // ================================================================

    /// 🧪 close() is a no-op and always succeeds. Like a participation trophy. 🏆
    #[tokio::test]
    async fn the_one_where_close_does_absolutely_nothing_and_is_proud_of_it() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        sink.close().await?;
        Ok(())
    }

    // ================================================================
    // 🧪 GROUP E: Primary Key — Labeling Your Lunchbox
    // ================================================================

    /// 🧪 primary_key appends ?primaryKey=custom_id to the documents URL.
    /// Without this, NOAA data screams `missing_document_id` into the void.
    #[tokio::test]
    async fn the_one_where_primary_key_rides_shotgun_in_the_query_string() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        // 📡 Document POST must hit the URL with ?primaryKey=custom_id
        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .and(wiremock::matchers::query_param("primaryKey", "custom_id"))
            .respond_with(ResponseTemplate::new(202).set_body_string(r#"{"taskUid":1338}"#))
            .expect(1)
            .named("primary_key_post")
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.primary_key = Some("custom_id".to_string());

        let mut sink = MeilisearchSink::new(config).await?;
        sink.drain(Payload(r#"[{"custom_id":"abc","title":"Primary key vibes"}]"#.to_string())).await?;
        Ok(())
    }

    /// 🧪 No primary_key means no query param — Meilisearch auto-infers from *id fields.
    #[tokio::test]
    async fn the_one_where_no_primary_key_means_meilisearch_figures_it_out() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        // 📡 Document POST hits /documents with no query string
        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .respond_with(ResponseTemplate::new(202).set_body_string(r#"{"taskUid":1339}"#))
            .expect(1)
            .named("no_pk_post")
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        sink.drain(Payload(r#"[{"geonameid":123,"name":"Auto-detect city"}]"#.to_string())).await?;
        Ok(())
    }

    // ================================================================
    // 🧪 GROUP F: Edge Cases — Where Dreams Go to Be Tested
    // ================================================================

    /// 🧪 Trailing slash in URL is handled gracefully.
    #[tokio::test]
    async fn the_one_where_trailing_slashes_are_trimmed_like_a_fancy_hedge() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        let config = make_config(&format!("{}/", mock_server.uri()));
        let _sink = MeilisearchSink::new(config).await?;
        // ✅ No double slashes — clean URL guaranteed
        Ok(())
    }
}
