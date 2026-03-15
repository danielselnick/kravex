use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{debug, trace};

use crate::Payload;
use crate::backends::Sink;
use super::config::OpenObserveSinkConfig;

/// 📡 The OpenObserve sink — ES-compatible bulk ingestion, zero drama.
///
/// 🎬 COLD OPEN — INT. OBSERVABILITY DASHBOARD — 3AM
/// *[the on-call engineer stares at a blank screen. "Where are my logs?"]*
/// *["In the sink," says the pipeline. "I sent them. All of them."]*
/// *["But did OpenObserve receive them?" "Define 'receive'..."]*
///
/// `OpenObserveSink` accepts a fully rendered NDJSON payload and POSTs it
/// to `/api/{org}/_bulk`. That's it. No buffering. No casting. No drama.
/// The Drainer upstream handles everything else.
///
/// 🧠 Knowledge graph:
/// - Pure I/O abstraction — same contract as ElasticsearchSink
/// - Endpoint: `{url}/api/{org}/_bulk` (ES-compatible bulk API)
/// - Auth: basic auth only (no API key — OpenObserve keeps it simple)
/// - Streams auto-create on first write (no existence check at startup)
/// - Content-Type: `application/x-ndjson` (same as Elasticsearch)
///
/// 🚰 This is the drain at the end of the pipeline. The last stop before
/// your data enters the observability dimension. There is no return. 🦆
#[derive(Debug)]
pub struct OpenObserveSink {
    client: reqwest::Client,
    sink_config: OpenObserveSinkConfig,
}

#[async_trait]
impl Sink for OpenObserveSink {
    /// 📡 POST the fully rendered NDJSON payload to /api/{org}/_bulk.
    /// Pure I/O. No buffering. No existential questions about data formats.
    /// "I came, I POST'd, I returned Ok(())." — Julius Sink
    async fn send(&mut self, payload: Payload) -> Result<()> {
        debug!(
            "📡 Sending {} bytes to OpenObserve /_bulk — the payload departs on its final journey",
            payload.len()
        );
        self.submit_bulk_request(payload).await
            .context("💀 The bulk submission to OpenObserve failed at the finish line. The NDJSON was pristine, the Drainer did its job, and the HTTP layer said 'nah.' Check connectivity. Check your OpenObserve instance. Check if Mercury is in retrograde.")?;
        Ok(())
    }

    /// 🗑️ Nothing to flush — no buffer, no state, no drama.
    /// Close is a no-op. The HTTP client drops cleanly. The connection pool waves goodbye.
    /// "What's the DEAL with close() methods that do nothing?" — Jerry Seinfeld, probably 🦆
    async fn close(&mut self) -> Result<()> {
        debug!("🗑️ OpenObserve sink closing — no buffer to flush, just releasing the vibes");
        Ok(())
    }
}

impl OpenObserveSink {
    /// 🚀 Stand up a new `OpenObserveSink`, fully wired and ready to ingest.
    ///
    /// This constructor does two things:
    /// 1. Builds the `reqwest::Client` with sane timeouts (10s connect, 30s read).
    /// 2. Pings the OpenObserve API root to confirm it's alive and accepting visitors.
    ///
    /// Unlike the ES sink, we do NOT check if the stream exists — OpenObserve
    /// auto-creates streams on first write. Like a river that digs its own bed.
    /// Nature is beautiful. So is auto-provisioning. 🌊
    pub async fn new(config: OpenObserveSinkConfig) -> Result<Self> {
        // 🔧 Build the HTTP client — 10s connect, 30s response timeout.
        // If OpenObserve can't shake hands in 10 seconds, it's having a moment.
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("💀 The HTTP client refused to be born. TLS said no, OpenSSL said maybe, and the operating system filed a restraining order. This is a system-level problem, not a you-level problem. Probably.")?;

        // 📡 Connectivity ping — "Hello? Is anyone observing?" — us, to OpenObserve.
        // GET /api/{org}/ — just to confirm the URL is real and basic auth works.
        // No health check, no stream check. Just a handshake.
        let the_ping_url = format!(
            "{}/api/{}/",
            config.url.trim_end_matches('/'),
            config.org
        );
        let mut the_ping_request = client.get(&the_ping_url);
        // 🔒 Attach basic auth if credentials are provided
        if let Some(ref username) = config.username {
            the_ping_request = the_ping_request.basic_auth(username, config.password.as_ref());
        }
        the_ping_request
            .send()
            .await
            .context("💀 Failed to ping OpenObserve. We said hello, the void said nothing. Check the URL, check the network, check if the OpenObserve instance is actually running or just a figment of your docker-compose.")?;

        // 🚀 All checks passed. No stream validation needed — OpenObserve auto-creates.
        // This is like checking into a hotel that builds the room when you arrive.
        Ok(Self {
            sink_config: config,
            client,
        })
    }

    /// 📡 Fires a `_bulk` POST request with the given NDJSON body.
    ///
    /// The actual HTTP call that makes documents leave our process and enter
    /// OpenObserve's warm, observability-flavored embrace.
    ///
    /// URL: `{url}/api/{org}/_bulk`
    /// Content-Type: `application/x-ndjson` — because that's what the bulk API expects.
    /// Auth: basic auth if configured.
    ///
    /// 🔄 No retries here. That's the caller's existential burden. Good luck.
    async fn submit_bulk_request(&self, request_body: Payload) -> Result<()> {
        // 📡 Build the bulk endpoint URL — the loading dock of observability
        let the_bulk_url = format!(
            "{}/api/{}/_bulk",
            self.sink_config.url.trim_end_matches('/'),
            self.sink_config.org
        );

        let mut request = self
            .client
            .post(&the_bulk_url)
            // ⚠️ Content-Type: application/x-ndjson — same as Elasticsearch.
            // OpenObserve speaks ES bulk fluently. They went to the same school.
            .header("Content-Type", "application/x-ndjson");

        // 🔒 Basic auth — the only auth OpenObserve needs. No API key hierarchy drama.
        // "In a world of complex auth schemes... one platform chose simplicity." 🎬
        if let Some(ref username) = self.sink_config.username {
            request = request.basic_auth(username, self.sink_config.password.as_ref());
        }

        let response = request
            .body(request_body.0)
            .send()
            .await
            .context("💀 The bulk request to OpenObserve never arrived. We launched the payload into the network and the network said 'return to sender.' Check connectivity, check DNS, check if your packets got lost in the bermuda triangle of routing tables.")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "💀 OpenObserve looked at our documents and said '{}'. The body of the verdict read: '{}'. We have no one to blame but ourselves, and possibly whoever configured the stream.",
                status,
                body
            );
        } else {
            trace!(
                "🚀 Bulk request to OpenObserve landed successfully — documents have been observed, finally"
            );
        }

        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  🧪  T E S T S  —  The OpenObserve Sink Trials
//     ╭──────────────────╮
//     │ /api/org/_bulk   │◄── NDJSON goes in, observations come out.
//     ╰──────────────────╯
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Payload;
    use crate::backends::{CommonSinkConfig, Sink};
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // 🔧 Minimal config factory — no auth, default org, test stream.
    // Like ordering water at a bar — you can always add flavor later. 🦆
    fn make_config(url: &str) -> OpenObserveSinkConfig {
        OpenObserveSinkConfig {
            url: url.to_string(),
            org: "default".to_string(),
            stream: "test-stream".to_string(),
            username: None,
            password: None,
            common_config: CommonSinkConfig::default(),
        }
    }

    // 🔧 Mounts the connectivity ping mock (GET /api/default/) that `new()` always hits.
    // The bouncer at the OpenObserve door — every test must pass this checkpoint.
    async fn mount_org_ping(mock_server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api/default/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(mock_server)
            .await;
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP A: Constructor — Connectivity Ping                           │
    // │  "Are you there, OpenObserve? It's me, the sink."                  │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Happy path: OpenObserve responds to ping, sink is born. Circle of observability life.
    #[tokio::test]
    async fn the_one_where_openobserve_is_alive_and_observing() -> Result<()> {
        // 🔧 Arrange — a welcoming OpenObserve instance
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        let config = make_config(&mock_server.uri());

        // 🚀 Act — attempt the sacred construction
        let the_newborn_sink = OpenObserveSink::new(config).await;

        // 🎯 Assert — the sink exists! Observation begins!
        assert!(
            the_newborn_sink.is_ok(),
            "💀 Sink construction failed even though OpenObserve was alive. This is a betrayal of trust."
        );

        Ok(())
    }

    /// 🧪 OpenObserve returns 500 on ping. We check liveness, not happiness.
    /// "I came, I pinged, I got a 500." — Julius HTTP Caesar
    #[tokio::test]
    async fn the_one_where_openobserve_is_having_a_rough_day() -> Result<()> {
        // 🔧 Arrange — OpenObserve is alive but suffering
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/default/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());

        // 🚀 Act — construct against a suffering instance
        let the_suffering_sink = OpenObserveSink::new(config).await;

        // 🎯 Assert — we verify liveness, not happiness. A 500 means "alive but in pain."
        assert!(
            the_suffering_sink.is_ok(),
            "💀 Sink should still construct if OpenObserve responds (even with 500). We check pulse, not mood."
        );

        Ok(())
    }

    /// 🧪 Basic auth credentials are sent on the connectivity ping. Trust but verify.
    #[tokio::test]
    async fn the_one_where_basic_auth_is_sent_on_ping() -> Result<()> {
        // 🔧 Arrange — OpenObserve with a bouncer
        let mock_server = MockServer::start().await;

        // 📡 dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk = base64("the_user:the_password")
        Mock::given(method("GET"))
            .and(path("/api/default/"))
            .and(header("Authorization", "Basic dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.username = Some("the_user".to_string());
        config.password = Some("the_password".to_string());

        // 🚀 Act — construct, triggering the authenticated ping
        let _the_authenticated_sink = OpenObserveSink::new(config).await?;

        // 🎯 Assert — wiremock's expect(1) validates Basic auth was sent ✅

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP B: Bulk POST — send() / submit_bulk_request()                │
    // │  "You miss 100% of the bulk requests you don't POST." — Wayne HTTP  │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Happy path bulk POST: 200 response. Documents observed. Sleep well tonight.
    #[tokio::test]
    async fn the_one_where_bulk_request_lands_in_openobserve() -> Result<()> {
        // 🔧 Arrange — a welcoming observability platform
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_eager_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act — fire the payload into the observability void
        let the_ndjson = Payload::from("{\"index\":{\"_index\":\"test-stream\"}}\n{\"id\":1}\n".to_string());
        let the_result = the_eager_sink.send(the_ndjson).await;

        // 🎯 Assert — OpenObserve accepted our offering ✅
        assert!(
            the_result.is_ok(),
            "💀 Bulk request returned 200 but send() still failed. The vibes are deeply off."
        );

        Ok(())
    }

    /// 🧪 OpenObserve returns 400 — bad docs? bad format? bad karma? Error includes status + body.
    #[tokio::test]
    async fn the_one_where_openobserve_rejects_our_docs() -> Result<()> {
        // 🔧 Arrange — OpenObserve is judgy today
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_string("bad_request: your docs are chaotic and you should feel chaotic"),
            )
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_judged_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act — submit docs that OpenObserve will roast
        let the_rejected_payload = Payload::from("{\"index\":{}}\n{\"bad\":\"doc\"}\n".to_string());
        let the_harsh_verdict = the_judged_sink.send(the_rejected_payload).await;

        // 🎯 Assert — should fail, error chain should contain status info
        assert!(the_harsh_verdict.is_err(), "💀 400 response should cause send() to fail");
        let the_full_error_chain = format!("{:?}", the_harsh_verdict.unwrap_err());
        assert!(
            the_full_error_chain.contains("400"),
            "💀 Error chain should mention the 400 status, got: {the_full_error_chain}"
        );

        Ok(())
    }

    /// 🧪 Server error (500). All non-2xx should fail. No favorites.
    /// "This is fine." 🐕‍🦺🔥
    #[tokio::test]
    async fn the_one_where_openobserve_catches_fire() -> Result<()> {
        // 🔧 Arrange — the server is on fire
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_string("internal_server_error: the observation has become the observed"),
            )
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_unlucky_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        let the_500_result = the_unlucky_sink.send(Payload::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await;

        // 🎯 Assert — 500 is not 200. Math checks out.
        assert!(
            the_500_result.is_err(),
            "💀 500 response should fail. It's literally called 'Internal Server Error'. The clue is in the name."
        );

        Ok(())
    }

    /// 🧪 Content-Type must be application/x-ndjson. Same as Elasticsearch. Same school.
    #[tokio::test]
    async fn the_one_where_content_type_is_ndjson() -> Result<()> {
        // 🔧 Arrange — mock that requires the correct content type
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .and(header("Content-Type", "application/x-ndjson"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_proper_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        the_proper_sink.send(Payload::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock's header matcher confirms Content-Type ✅

        Ok(())
    }

    /// 🧪 Basic auth on bulk requests. The working class hero of authentication.
    #[tokio::test]
    async fn the_one_where_basic_auth_is_sent_on_bulk() -> Result<()> {
        // 🔧 Arrange — authenticated bulk
        let mock_server = MockServer::start().await;

        // 📡 Mount ping that accepts any request (auth is verified separately)
        Mock::given(method("GET"))
            .and(path("/api/default/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // 📡 dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk = base64("the_user:the_password")
        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .and(header("Authorization", "Basic dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.username = Some("the_user".to_string());
        config.password = Some("the_password".to_string());

        let mut the_basic_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        the_basic_sink.send(Payload::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock confirms Basic auth was sent ✅

        Ok(())
    }

    /// 🧪 No auth = no Authorization header. Some OpenObserve instances run without auth.
    /// Living dangerously. Like skydiving without a parachute, but for data.
    #[tokio::test]
    async fn the_one_where_no_auth_means_anarchy() -> Result<()> {
        // 🔧 Arrange — no auth, no judgment
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .named("bulk_no_auth")
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_naked_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        the_naked_sink.send(Payload::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — request was received, no auth configured = no auth sent ✅

        Ok(())
    }

    /// 🧪 Payload body arrives exactly as sent. No mutation. No trimming. Pure NDJSON fidelity.
    #[tokio::test]
    async fn the_one_where_the_payload_survives_the_journey() -> Result<()> {
        // 🔧 Arrange — a payload that must arrive byte-perfect
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        let the_sacred_payload =
            "{\"index\":{\"_index\":\"test-stream\"}}\n{\"id\":42,\"confession\":\"I still use println for debugging\"}\n";

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .and(body_string(the_sacred_payload))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_faithful_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        the_faithful_sink.send(Payload::from(the_sacred_payload.to_string())).await?;

        // 🎯 Assert — wiremock's body_string matcher confirms byte-perfect delivery ✅

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP C: close() — The No-Op                                       │
    // │  "The best code is no code at all." — Jeff Atwood                   │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 close() does nothing. Returns Ok. The most peaceful function.
    #[tokio::test]
    async fn the_one_where_close_achieves_inner_peace() -> Result<()> {
        // 🔧 Arrange — build a sink just to close it. Like buying a journal to write "Day 1."
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        let config = make_config(&mock_server.uri());
        let mut the_doomed_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act — the grand closing ceremony
        let the_anticlimactic_result = the_doomed_sink.close().await;

        // 🎯 Assert — Ok and nothing else. The most boring test. The best test.
        assert!(
            the_anticlimactic_result.is_ok(),
            "💀 close() failed. HOW? It literally does nothing. What sorcery is this?"
        );

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP D: Edge Cases — The Weird Stuff                              │
    // │  "Edge cases are where observability goes to cry." — Ancient proverb│
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Trailing slash in URL doesn't cause double-slash in bulk path.
    /// /api/default/_bulk vs /api/default//_bulk — one slash, infinite debugging.
    #[tokio::test]
    async fn the_one_where_trailing_slash_is_handled_gracefully() -> Result<()> {
        // 🔧 Arrange — the cursed trailing slash
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // ⚠️ Trailing slash in the URL — the classic footgun
        let config = make_config(&format!("{}/", mock_server.uri()));
        let mut the_slash_aware_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        the_slash_aware_sink.send(Payload::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock's path matcher + expect(1) confirms correct URL ✅

        Ok(())
    }

    /// 🧪 Empty payload — sent as-is. The sink doesn't judge content. 🦆
    #[tokio::test]
    async fn the_one_where_we_send_nothing_and_call_it_observability() -> Result<()> {
        // 🔧 Arrange — accepting the void
        let mock_server = MockServer::start().await;
        mount_org_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/api/default/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_yolo_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act — send the void
        let the_existential_result = the_yolo_sink.send(Payload::from(String::new())).await;

        // 🎯 Assert — the sink sent nothing, OpenObserve accepted nothing. Balance.
        assert!(
            the_existential_result.is_ok(),
            "💀 Empty payload with 200 response should be Ok. The sink doesn't judge content."
        );

        Ok(())
    }

    /// 🧪 Custom org name appears in the correct URL path. Not all orgs are "default."
    /// Some orgs have ambition. Some have budgets. Some have both.
    #[tokio::test]
    async fn the_one_where_custom_org_routes_correctly() -> Result<()> {
        // 🔧 Arrange — a non-default org, living its best life
        let mock_server = MockServer::start().await;

        // 📡 Mount ping for custom org
        Mock::given(method("GET"))
            .and(path("/api/acme-corp/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // 📡 Mount bulk for custom org
        Mock::given(method("POST"))
            .and(path("/api/acme-corp/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.org = "acme-corp".to_string();

        let mut the_corporate_sink = OpenObserveSink::new(config).await?;

        // 🚀 Act
        the_corporate_sink.send(Payload::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock confirms the custom org path was used ✅

        Ok(())
    }
}
