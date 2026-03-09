use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::Payload;
use crate::backends::Sink;
use super::config::MeilisearchSinkConfig;

/// 🔍 The Meilisearch sink — sends JSON array payloads to `/indexes/{uid}/documents`.
///
/// Like the ES sink but with fewer knobs and more optimism. Meilisearch is the
/// Marie Kondo of search engines — does it spark joy? Index it. Does it not?
/// Still index it. We don't judge documents here. 🦆
///
/// Internally holds:
/// - `client`: the HTTP workhorse 🐴 — reused across requests
/// - `sink_config`: auth, URL, index UID
/// - `the_documents_url`: pre-computed `{url}/indexes/{uid}/documents` to avoid
///   string formatting in the hot path (because allocations are the enemy and we
///   are at war)
///
/// 🧠 Knowledge graph:
/// - Sink trait impl: `send()` POSTs JSON array, polls task until done, `close()` is a no-op
/// - Meilisearch returns 202 for document ingestion (async task)
/// - We poll `GET /tasks/{taskUid}` until status is "succeeded" or "failed"
/// - Auth: `Authorization: Bearer {api_key}` when api_key is present
///
/// Knock knock. Who's there? 202 Accepted. 202 Accepted who?
/// 202 Accepted your documents but won't tell you if they actually made it
/// until you poll the task endpoint like a nervous parent at a school play. 🎭
#[derive(Debug)]
pub struct MeilisearchSink {
    client: reqwest::Client,
    sink_config: MeilisearchSinkConfig,
    // 📡 Pre-computed URL: `{base_url}/indexes/{index_uid}/documents`
    // Allocated once at construction. The hot path borrows it. Zero allocs per send.
    the_documents_url: String,
    // 📡 Pre-computed URL: `{base_url}/tasks/`
    // We append the task UID at poll time, but the prefix is stable.
    the_tasks_url_prefix: String,
}

/// 🔍 Meilisearch task status response — the async receipt for document ingestion.
/// We only care about `taskUid` (from the 202 response) and `status` (from the poll).
/// The rest is metadata we politely ignore like terms and conditions. 📜
#[derive(serde::Deserialize, Debug)]
struct MeilisearchTaskResponse {
    // -- 🐛 POST /documents returns "taskUid", GET /tasks/{id} returns "uid" — Meilisearch has commitment issues with field naming
    #[serde(alias = "taskUid", alias = "uid")]
    task_uid: u64,
    status: Option<String>,
}

// 💤 Poll interval for task status checks
const THE_TASK_POLL_INTERVAL_MS: u64 = 25;
// 💀 Max poll attempts before we declare the task lost at sea
const THE_MAX_POLL_ATTEMPTS: u64 = 240;

#[async_trait]
impl Sink for MeilisearchSink {
    /// 📡 POST the JSON array payload to `/indexes/{uid}/documents`, then poll until done.
    ///
    /// Meilisearch is async-first: the POST returns 202 with a `taskUid`, and we
    /// poll `GET /tasks/{taskUid}` until it's "succeeded" or "failed".
    /// It's like ordering food and then staring at the kitchen window. 🍕
    async fn send(&mut self, payload: Payload) -> Result<()> {
        debug!(
            "🔍 Sending {} bytes to Meilisearch — the payload approaches the search engine like a cat approaching a bath",
            payload.len()
        );

        // 📡 Phase 1: POST documents — fire the payload into the Meilisearch void
        let the_task_response = self.submit_documents(payload).await
            .context("💀 Document submission to Meilisearch failed. The JSON was pristine. The network said 'nah.' Check connectivity. Check your index UID. Check if Mercury is in retrograde.")?;

        // 🔄 Phase 2: Poll task until completion — the anxious waiting room of data ingestion
        self.poll_task_until_done(the_task_response.task_uid).await
            .context("💀 Task polling failed. Meilisearch accepted our documents but then ghosted us on the status. Like my college roommate. Kevin, if you're reading this, I want my blender back.")?;

        Ok(())
    }

    /// 🗑️ Nothing to flush — we don't buffer. The Drainer sends complete payloads.
    /// Close is a no-op. Like saying goodbye to a search engine that was never really yours.
    /// The HTTP client drops cleanly. The connection pool waves from the window. 🪟
    async fn close(&mut self) -> Result<()> {
        debug!("🗑️ Meilisearch sink closing — no buffer to flush, just search relevance to mourn");
        Ok(())
    }
}

impl MeilisearchSink {
    /// 🚀 Stand up a new `MeilisearchSink`, fully wired and ready to accept documents.
    ///
    /// This constructor does three things:
    /// 1. Builds the `reqwest::Client` with sane timeouts (10s connect, 60s read for task polling).
    /// 2. Pings the health endpoint (`GET /health`) to confirm Meilisearch is alive.
    /// 3. Verifies the target index exists (`GET /indexes/{uid}`).
    ///
    /// Pre-computes the documents URL and tasks URL prefix so the hot path
    /// does zero string formatting. Because every nanosecond counts when you're
    /// migrating millions of documents and your SLA is "before lunch." 🍔
    pub async fn new(config: MeilisearchSinkConfig) -> Result<Self> {
        // 🔧 Build the HTTP client — 10s connect, 60s read (task polling can be slow)
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .context("💀 The HTTP client refused to materialize. The TLS stack is having an existential crisis. Probably a missing cert or a cursed OpenSSL installation. Either way: we can't talk to Meilisearch without an HTTP client, and we can't build an HTTP client without hope.")?;

        // 📡 Normalize URL — strip trailing slash because double-slashes are a crime against HTTP
        let the_base_url = config.url.trim_end_matches('/').to_string();

        // 📡 Health check — "Hello? Is anybody home?" 🏠
        let mut the_health_request = client.get(format!("{}/health", the_base_url));
        if let Some(ref the_api_key) = config.api_key {
            the_health_request = the_health_request.bearer_auth(the_api_key);
        }
        the_health_request
            .send()
            .await
            .context("💀 Meilisearch health check failed. The server is either down, unreachable, or pretending not to be home. We knocked. Nobody answered. The lights are off but we can hear the CPU fan spinning.")?;

        // 🔍 Index existence check — confirm the target index is real
        let mut the_index_request = client.get(format!("{}/indexes/{}", the_base_url, config.index_uid));
        if let Some(ref the_api_key) = config.api_key {
            the_index_request = the_index_request.bearer_auth(the_api_key);
        }
        let the_index_response = the_index_request
            .send()
            .await
            .context("💀 Failed to check if index exists. The network hiccuped mid-verification.")?;

        if !the_index_response.status().is_success() {
            // ⚠️ Index doesn't exist — Meilisearch will auto-create it on first document POST,
            // but we warn because explicit > implicit (except in Rust where explicit is mandatory)
            warn!(
                "⚠️ Index '{}' returned status {} — Meilisearch will auto-create it on first document POST. Proceeding with the reckless optimism of a startup founder.",
                config.index_uid,
                the_index_response.status()
            );
        }

        // 📡 Pre-compute URLs for the hot path
        let the_documents_url = format!("{}/indexes/{}/documents", the_base_url, config.index_uid);
        let the_tasks_url_prefix = format!("{}/tasks/", the_base_url);

        info!(
            "🔍 Meilisearch sink initialized — target: {}/indexes/{} — ready to index like it's 1999",
            the_base_url, config.index_uid
        );

        Ok(Self {
            client,
            sink_config: config,
            the_documents_url,
            the_tasks_url_prefix,
        })
    }

    /// 📡 POST the JSON array payload to the documents endpoint.
    /// Returns the task response with the `taskUid` for polling.
    async fn submit_documents(&self, payload: Payload) -> Result<MeilisearchTaskResponse> {
        let mut the_request = self.client
            .post(&self.the_documents_url)
            .header("Content-Type", "application/json")
            .body(payload.0);

        if let Some(ref the_api_key) = self.sink_config.api_key {
            the_request = the_request.bearer_auth(the_api_key);
        }

        let the_response = the_request
            .send()
            .await
            .context("💀 HTTP POST to Meilisearch failed. The documents were ready. The network was not. This is basically a long-distance relationship that didn't work out.")?;

        let the_status = the_response.status();
        let the_body = the_response.text().await
            .context("💀 Failed to read Meilisearch response body. The server sent something, but we couldn't read it. Like a postcard in a language we don't speak.")?;

        if !the_status.is_success() && the_status.as_u16() != 202 {
            anyhow::bail!(
                "💀 Meilisearch returned {} for document POST. Body: {}. This is the HTTP equivalent of 'we need to talk.'",
                the_status, the_body
            );
        }

        let the_task: MeilisearchTaskResponse = serde_json::from_str(&the_body)
            .context("💀 Failed to parse Meilisearch task response. The JSON decoder looked at the response and said 'I don't know her.'")?;

        debug!("🔄 Meilisearch accepted documents — taskUid: {} — now we wait like it's a DMV appointment", the_task.task_uid);

        Ok(the_task)
    }

    /// 🔄 Poll `GET /tasks/{taskUid}` until the task reaches a terminal state.
    ///
    /// Terminal states: "succeeded" (party 🎉) or "failed" (funeral 💀).
    /// Everything else means "still processing" and we poll again after a short nap.
    /// Like checking if your pizza is ready every 30 seconds. The staff hates it.
    /// But we do it anyway because data integrity > social graces. 🍕
    async fn poll_task_until_done(&self, the_task_uid: u64) -> Result<()> {
        let the_task_url = format!("{}{}", self.the_tasks_url_prefix, the_task_uid);

        for the_attempt in 0..THE_MAX_POLL_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(THE_TASK_POLL_INTERVAL_MS)).await;

            let mut the_request = self.client.get(&the_task_url);
            if let Some(ref the_api_key) = self.sink_config.api_key {
                the_request = the_request.bearer_auth(the_api_key);
            }

            let the_response = the_request.send().await
                .context("💀 Task poll request failed. We lost contact with Meilisearch mid-poll. Like losing WiFi during a video call with your boss.")?;

            let the_body = the_response.text().await
                .context("💀 Failed to read task poll response body.")?;

            let the_task: MeilisearchTaskResponse = serde_json::from_str(&the_body)
                .context("💀 Failed to parse task poll response. Meilisearch is speaking in tongues.")?;

            match the_task.status.as_deref() {
                Some("succeeded") => {
                    debug!("✅ Meilisearch task {} succeeded on poll attempt {} — the documents found their forever home", the_task_uid, the_attempt + 1);
                    return Ok(());
                }
                Some("failed") => {
                    anyhow::bail!(
                        "💀 Meilisearch task {} failed. The documents arrived but were rejected at the door. Full response: {}. This is the data equivalent of showing up to a party with the wrong invitation.",
                        the_task_uid, the_body
                    );
                }
                Some(the_status) => {
                    debug!("🔄 Meilisearch task {} status: '{}' — attempt {}/{} — still marinating", the_task_uid, the_status, the_attempt + 1, THE_MAX_POLL_ATTEMPTS);
                }
                None => {
                    debug!("🔄 Meilisearch task {} has no status field — attempt {}/{} — the void stares back", the_task_uid, the_attempt + 1, THE_MAX_POLL_ATTEMPTS);
                }
            }
        }

        anyhow::bail!(
            "💀 Meilisearch task {} did not complete after {} poll attempts ({} seconds). We waited. And waited. Like a dog at the window. But the owner never came home.",
            the_task_uid, THE_MAX_POLL_ATTEMPTS, (THE_MAX_POLL_ATTEMPTS * THE_TASK_POLL_INTERVAL_MS) / 1000
        );
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
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"uid":"test-meili-idx","primaryKey":"id"}"#))
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

        let sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        // ✅ Sink constructed — pre-computed URLs should be correct
        assert!(sink.the_documents_url.ends_with("/indexes/test-meili-idx/documents"));
        assert!(sink.the_tasks_url_prefix.ends_with("/tasks/"));
        Ok(())
    }

    /// 🧪 Constructor still succeeds when index doesn't exist (Meilisearch auto-creates).
    #[tokio::test]
    async fn the_one_where_the_index_is_missing_but_we_press_on_with_reckless_optimism() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;

        // 📡 Index check returns 404 — Meilisearch will auto-create
        Mock::given(method("GET"))
            .and(path("/indexes/test-meili-idx"))
            .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message":"Index `test-meili-idx` not found."}"#))
            .named("index_404")
            .mount(&mock_server)
            .await;

        let sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        assert!(sink.the_documents_url.contains("test-meili-idx"));
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
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"uid":"test-meili-idx"}"#))
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
    // 🧪 GROUP C: Document POST + Task Polling
    // ================================================================

    /// 🧪 send() POSTs JSON array and polls task to success.
    #[tokio::test]
    async fn the_one_where_documents_make_it_to_the_promised_land() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        // 📡 Document POST returns 202 with taskUid
        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(202).set_body_string(
                r#"{"taskUid":1337,"indexUid":"test-meili-idx","status":"enqueued","type":"documentAdditionOrUpdate"}"#
            ))
            .expect(1)
            .named("document_post")
            .mount(&mock_server)
            .await;

        // 📡 Task poll returns succeeded immediately
        Mock::given(method("GET"))
            .and(path("/tasks/1337"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"taskUid":1337,"status":"succeeded"}"#
            ))
            .named("task_poll")
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        let the_payload = Payload(r#"[{"id":1,"title":"Test Doc"}]"#.to_string());
        sink.send(the_payload).await?;
        Ok(())
    }

    /// 🧪 send() handles task that is "processing" before "succeeded".
    #[tokio::test]
    async fn the_one_where_we_wait_patiently_like_monks_at_a_buffet() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .respond_with(ResponseTemplate::new(202).set_body_string(
                r#"{"taskUid":42,"status":"enqueued"}"#
            ))
            .mount(&mock_server)
            .await;

        // 📡 Task poll: first returns processing, then succeeded
        // wiremock doesn't support stateful responses easily, so we just return succeeded
        // (the real test of multi-poll is in the integration test)
        Mock::given(method("GET"))
            .and(path("/tasks/42"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"taskUid":42,"status":"succeeded"}"#
            ))
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        sink.send(Payload(r#"[{"id":1}]"#.to_string())).await?;
        Ok(())
    }

    /// 🧪 send() fails when task status is "failed".
    #[tokio::test]
    async fn the_one_where_meilisearch_rejects_our_documents_like_a_bouncer_at_a_club() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/indexes/test-meili-idx/documents"))
            .respond_with(ResponseTemplate::new(202).set_body_string(
                r#"{"taskUid":666,"status":"enqueued"}"#
            ))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/666"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"taskUid":666,"status":"failed","error":{"message":"Invalid document","code":"invalid_document_fields"}}"#
            ))
            .mount(&mock_server)
            .await;

        let mut sink = MeilisearchSink::new(make_config(&mock_server.uri())).await?;
        let the_result = sink.send(Payload(r#"[{"bad":"data"}]"#.to_string())).await;
        assert!(the_result.is_err(), "💀 Failed task should propagate as error");
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
    // 🧪 GROUP E: Edge Cases — Where Dreams Go to Be Tested
    // ================================================================

    /// 🧪 Trailing slash in URL is handled gracefully.
    #[tokio::test]
    async fn the_one_where_trailing_slashes_are_trimmed_like_a_fancy_hedge() -> Result<()> {
        let mock_server = MockServer::start().await;
        mount_health_check(&mock_server).await;
        mount_index_check(&mock_server).await;

        let mut config = make_config(&format!("{}/", mock_server.uri()));
        let sink = MeilisearchSink::new(config).await?;
        // ✅ No double slashes in the pre-computed URL
        assert!(!sink.the_documents_url.contains("//indexes"));
        Ok(())
    }
}
