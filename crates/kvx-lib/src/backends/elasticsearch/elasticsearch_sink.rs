// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, trace, warn};

use crate::Drum;
use crate::backends::Sink;
use super::config::ElasticsearchSinkConfig;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  📦 Bulk Response Types — Elasticsearch's confessional booth
//     ╭──────────╮
//     │ 200 OK   │◄── "Sure, we got your docs. Some of them, anyway."
//     ╰──────────╯
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// -- 📦 The ES bulk API's response is like a report card: the envelope says "delivered" (HTTP 200)
// -- but inside, individual items may have F's. These structs crack the envelope open.

/// 📦 Top-level bulk response — the envelope that says "200 OK" but might contain bad news inside.
/// The `errors` field is the TL;DR: true means at least one item failed.
/// When `errors` is false, we skip parsing `items` entirely — no news is good news.
#[derive(Debug, Deserialize)]
struct BulkResponse {
    // The boolean that decides whether we sleep well tonight
    #[serde(default)]
    errors: bool,
    // Per-item results — only parsed when `errors` is true, because life is short
    #[serde(default)]
    items: Vec<BulkItemWrapper>,
}

/// 📦 Each bulk item is wrapped in an action key ("index", "create", "update", "delete").
/// ES returns `{ "index": { "status": 201, ... } }` — the outer key varies by action.
/// We use a HashMap because we don't care WHAT action failed, just THAT it failed.
/// -- 🗺️ One key per item, one result per key. Simple. Like a funeral guest list.
type BulkItemWrapper = HashMap<String, BulkItemResult>;

/// 📦 The actual result for a single document in the bulk response.
/// `status` tells us the HTTP-equivalent result code. `error` tells us why it's crying.
#[derive(Debug, Deserialize)]
struct BulkItemResult {
    #[serde(default)]
    status: u16,
    #[serde(default)]
    error: Option<BulkItemError>,
    #[serde(default)]
    _id: Option<String>,
}

/// 📦 When ES rejects a document, it explains itself with a type + reason.
/// Like a breakup text: "type: mapping_exception, reason: you're not my type."
#[derive(Debug, Deserialize)]
struct BulkItemError {
    #[serde(default, rename = "type")]
    error_type: String,
    #[serde(default)]
    reason: String,
}

/// 📡 The sink side of the Elasticsearch backend — pure I/O, zero buffering.
///
/// `ElasticsearchSink` accepts a fully rendered NDJSON drum string and POSTs it
/// to the `_bulk` API. That's it. No internal accumulator. No tap logic.
/// The Drainer upstream handles tap + binary collect + size management.
///
/// 🧠 Knowledge graph: Sinks are I/O-only abstractions now. This one does HTTP POST.
/// The FileSink does file write. The InMemorySink does Vec push.
/// Buffering, casting, and collecting moved to Drainer. Clean separation.
///
/// Internally holds:
/// - `client`: the HTTP muscle 💪 — reused across requests
/// - `sink_config`: auth, URL, index targeting info
///
/// 🚰 Think of this as the drain at the end of a data pipeline. The last stop.
/// Knock knock. Who's there? HTTP POST. HTTP POST who? HTTP POST your NDJSON
/// and hope the cluster's in a good mood.
#[derive(Debug)]
pub struct ElasticsearchSink {
    client: reqwest::Client,
    sink_config: ElasticsearchSinkConfig,
}

#[async_trait]
impl Sink for ElasticsearchSink {
    /// 📡 POST the fully rendered NDJSON drum to /_bulk. Pure I/O. No buffering. No drama.
    ///
    /// The Drainer upstream already tapped each doc and binary-collected them into
    /// a single NDJSON drum string. We just fire it into the elastic void.
    /// "In a world where sinks had too many responsibilities... one refactor dared to simplify."
    async fn drain(&mut self, drum: Drum) -> Result<()> {
        debug!(
            "📡 Sending {} bytes to /_bulk — the drum has left the building, Elvis-style",
            drum.len()
        );
        self.submit_bulk_request(drum).await
            .context("💀 The bulk submission stumbled at the finish line. The NDJSON was rendered with love, the Drainer did its job, and the HTTP layer said 'nah.' Check connectivity. Check your cluster. Check your horoscope.")?;
        Ok(())
    }

    /// 🗑️ Nothing to flush — we don't buffer. The Drainer sends complete drums.
    /// Close is a no-op. The HTTP client drops cleanly. The connections pool says goodbye.
    /// Knock knock. Who's there? Nobody. The sink is closed. Go home. 🦆
    async fn close(&mut self) -> Result<()> {
        debug!("🗑️ Elasticsearch sink closing — no buffer to flush, just vibes to release");
        Ok(())
    }
}

impl ElasticsearchSink {
    /// 🚀 Stand up a new `ElasticsearchSink`, fully wired and ready to receive documents.
    ///
    /// This constructor does three things:
    /// 1. Builds the `reqwest::Client` with sane timeouts (10s connect, 30s read).
    ///    Like a polite person — we will wait, but not forever.
    /// 2. Pings the cluster root URL with a GET to confirm it's alive and talking to us.
    ///    A handshake. A hello. A "are you even there?"
    /// 3. If a static `index` is configured, verifies it exists with a HEAD/GET check.
    ///    Because indexing into a non-existent index is a skill issue we catch at init time,
    ///    not at 10,000 documents deep. You're welcome.
    ///
    /// 🔒 Auth priority: API key > basic auth > anonymous. Same across ping, index check,
    /// and bulk requests. Consistent like a good morning routine. ☕
    pub async fn new(config: ElasticsearchSinkConfig) -> Result<Self> {
        // 🔧 Build the HTTP client. 10 second connect timeout because if ES can't handshake
        // in 10 seconds, it's not having a good time and neither are we. 30 second response
        // timeout because bulk requests can be meaty and we're not monsters.
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            // -- 💀 "Failed to initialize http client" — a tragedy in one act.
            // -- The curtain rises. reqwest::Client::builder() enters, full of promise.
            // -- It calls .build(). The TLS stack hesitates. The operating system shrugs.
            // -- There is no retry. There is only this context string, and silence.
            .context("💀 The HTTP client refused to be born. The TLS stack wept. The architect shrugged. We tried to build a reqwest::Client and the universe said 'no'. Probably a missing TLS cert or a cursed system OpenSSL. Either way: tragic.")?;

        // -- 📡 Connectivity ping — "Hello? Is this thing on?" — a developer, gesturing at a cluster.
        // We do a basic GET to the root to confirm the URL is real and auth works.
        // If this fails, we fail loudly here, rather than quietly 50,000 docs later.
        // Auth priority: API key > basic auth > anonymous. Same ladder as the source.
        let ping_request = {
            let mut req = client.get(&config.url);
            if let Some(ref api_key) = config.api_key {
                req = req.header("Authorization", format!("ApiKey {}", api_key));
            } else if let Some(ref username) = config.username {
                req = req.basic_auth(username, config.password.as_ref());
            }
            req
        };
        ping_request.send().await?;

        // 🔒 Optional index existence check — only runs if a static index is configured.
        // Per-doc index routing skips this, because checking every possible target index at
        // -- startup would be... ambitious. Like planning to read every book in a library before
        // -- borrowing the first one.
        if let Some(ref index_name) = config.index {
            // 📡 Construct the full index URL for a targeted existence check.
            // trim_end_matches('/') — the "/" hygiene you didn't know you needed.
            // Without it: `https://host//my-index`. With it: `https://host/my-index`.
            // -- One slash of difference. Infinite suffering of difference.
            let index_url = format!("{}/{}", config.url.trim_end_matches('/'), index_name);
            let mut request = client.get(&index_url);
            // -- 🔒 Auth priority: API key wins over basic auth. This is not a democracy.
            // -- This is an Elasticsearch cluster and api_key is the premium tier.
            if let Some(ref api_key) = config.api_key {
                request = request.header("Authorization", format!("ApiKey {}", api_key));
            } else if let Some(ref username) = config.username {
                request = request.basic_auth(username, config.password.as_ref());
            }

            let response = request.send().await
                // -- 💀 "Failed to check for index availability" — a drama in one act.
                // -- We sent a request into the void. The void sent back... nothing. Or an error.
                // -- A TCP RST. A DNS NXDOMAIN. A firewall rule written by someone who has since
                // -- left the company. We may never know. The index may or may not exist.
                // -- Schrodinger's cluster. Very advanced. Very unhelpful.
                .context("💀 Reached out to check if the index exists. Got ghosted. The network is giving us the silent treatment. Or the firewall is on a power trip again. Either way: we cannot confirm the index lives, so we refuse to proceed. Dignity intact.")?;
            let status = response.status();
            if !status.is_success() {
                // -- 💀 The index does not exist. This is not a warning. This is not a soft error.
                // -- This is a hard stop, a full bail, a "we're not doing this."
                // -- Indexing into a nonexistent index is chaos. We are order. We are the wall.
                anyhow::bail!(
                    "💀 Index '{}' does not exist and never has, as far as we can tell. We knocked. We waited. The door remained unanswered. You may want to create it, or check your spelling — easy mistake, no judgment, but also: please fix it.",
                    index_url
                );
            } else {
                // -- ✅ The index exists! It is real! We found it! Like finding your keys in your coat!
                // -- The one you already checked! But they were there! They were always there!
                debug!(
                    "✅ Index exists and is accepting visitors — welcome mat is out, cluster is home"
                );
            }
        }

        // 🚀 All checks passed. No buffer to init — we're I/O-only now. Clean. Light. Free.
        Ok(Self {
            sink_config: config,
            client,
        })
    }

    /// 📡 Submits a bulk request with per-item retry — failed docs get re-sent, successful ones don't.
    ///
    /// The ES `_bulk` API returns HTTP 200 even when individual documents fail. This method
    /// parses the response, extracts only the NDJSON pairs that failed, and re-sends them.
    /// Up to 10 retry rounds. Each round shrinks the drum to only the rejects.
    ///
    /// This prevents duplicate indexing for File→ES flows (auto-generated `_id`s) while still
    /// recovering transient per-item failures (shard pressure, version conflicts, etc.).
    ///
    /// 🔄 "I'm not mad, I'm just going to keep sending these until you accept them or I give up."
    async fn submit_bulk_request(&self, request_body: Drum) -> Result<()> {
        // -- 🔄 10 retries of just the failed docs — not the whole drum, we're not animals.
        // -- After 10 rounds of "please?" and "no", we accept our fate.
        const MAX_PARTIAL_RETRIES: usize = 10;
        // -- 🧮 Cap the number of individual error reasons we collect to avoid OOM on catastrophic failure
        const MAX_GRIEF_SAMPLES: usize = 5;

        // Take ownership of the drum string — the retry loop will shrink it each round
        let mut the_current_ndjson = request_body.0;

        for the_attempt in 0..=MAX_PARTIAL_RETRIES {
            // 📡 Fire the HTTP POST — returns the response body text on 2xx
            let the_body_text = self.send_bulk_post(&the_current_ndjson).await?;

            // Fast path: empty body means ES gave us nothing to parse — treat as success.
            if the_body_text.is_empty() {
                trace!("🚀 Bulk request landed — empty response body, assuming all docs indexed (living dangerously)");
                return Ok(());
            }

            let the_bulk_response: BulkResponse = serde_json::from_str(&the_body_text)
                .context("💀 Elasticsearch returned 200 but the response body wasn't valid JSON. The server spoke, but in tongues. This is like getting a letter back from the post office written in Wingdings.")?;

            if !the_bulk_response.errors {
                // -- ✅ No errors! Every doc made it! The singularity will happen before we see this log line in prod.
                trace!("🚀 Bulk request landed successfully — all documents accepted, zero casualties");
                return Ok(());
            }

            // 💀 errors: true — time to count the bodies and name the dead
            let mut the_body_count: usize = 0;
            let mut the_reasons_for_grief: Vec<String> = Vec::new();

            for (the_item_index, wrapper) in the_bulk_response.items.iter().enumerate() {
                for result in wrapper.values() {
                    if let Some(ref the_rejection_letter) = result.error {
                        the_body_count += 1;
                        if the_reasons_for_grief.len() < MAX_GRIEF_SAMPLES {
                            let the_doc_id = result._id.as_deref().unwrap_or("unknown");
                            the_reasons_for_grief.push(format!(
                                "item[{}] id={} status={} type={} reason={}",
                                the_item_index, the_doc_id, result.status,
                                the_rejection_letter.error_type, the_rejection_letter.reason
                            ));
                        }
                    }
                }
            }

            // -- 💀 Log each sampled failure so the 3am on-call engineer has something to cry-laugh at
            for the_eulogy in &the_reasons_for_grief {
                warn!("💀 Bulk item rejected: {}", the_eulogy);
            }
            if the_body_count > MAX_GRIEF_SAMPLES {
                warn!(
                    "💀 ... and {} more failed items not shown (we capped the grief at {})",
                    the_body_count - MAX_GRIEF_SAMPLES, MAX_GRIEF_SAMPLES
                );
            }

            // 🏁 If this was the last attempt, accept our fate with dignity (and a detailed error)
            if the_attempt == MAX_PARTIAL_RETRIES {
                anyhow::bail!(
                    "💀 Elasticsearch still rejecting {} out of {} documents after {} retries. \
                     We asked nicely. We asked repeatedly. We even said please. They said no. \
                     First failures: [{}]",
                    the_body_count,
                    the_bulk_response.items.len(),
                    MAX_PARTIAL_RETRIES,
                    the_reasons_for_grief.join("; ")
                );
            }

            // 🔄 Extract only the failed NDJSON action/doc pairs for retry
            let the_retry_drum = extract_failed_pairs(&the_current_ndjson, &the_bulk_response);
            if the_retry_drum.is_empty() {
                // Mismatch between NDJSON lines and response items — can't safely correlate
                anyhow::bail!(
                    "💀 {} documents failed but we couldn't extract them for retry — \
                     NDJSON line count doesn't match response item count. \
                     Like a jigsaw puzzle where the pieces are from different boxes. \
                     First failures: [{}]",
                    the_body_count,
                    the_reasons_for_grief.join("; ")
                );
            }

            warn!(
                "🔄 Retrying {} failed docs (attempt {}/{}) — the rest made it through, \
                 these just need another chance, like a second audition",
                the_body_count, the_attempt + 1, MAX_PARTIAL_RETRIES
            );

            the_current_ndjson = the_retry_drum;
        }

        // -- 🦆 The borrow checker approved this unreachable. The compiler trusts us. The runtime... we'll see.
        unreachable!("the retry loop always returns or bails — if you see this, reality has forked")
    }

    /// 📡 Pure HTTP POST to the `_bulk` endpoint. No parsing, no retry, no opinions.
    ///
    /// Returns the response body text on HTTP 2xx. Bails on non-2xx.
    /// This is the "just mail the letter" function — what happens after is someone else's problem.
    /// -- 🦆 "I'm just the postman, I don't read the mail."
    async fn send_bulk_post(&self, the_ndjson_body: &str) -> Result<String> {
        let bulk_url = match self.sink_config.index {
            Some(ref index_name) => format!("{}/{}/_bulk", self.sink_config.url.trim_end_matches('/'), index_name),
            None => format!("{}/_bulk", self.sink_config.url.trim_end_matches('/'))
        };

        let mut request = self
            .client
            .post(&bulk_url)
            // ⚠️ Content-Type: application/x-ndjson — not application/json. VERY important.
            // Elasticsearch will return a 406 or silently misbehave without this header.
            // -- The x- prefix means "we made this up but we're committing to it." Classic.
            .header("Content-Type", "application/x-ndjson");

        // -- 🔒 Same auth dance as the index check — api_key beats basic auth in this club.
        if let Some(ref api_key) = self.sink_config.api_key {
            request = request.header("Authorization", format!("ApiKey {}", api_key));
        } else if let Some(ref username) = self.sink_config.username {
            request = request.basic_auth(username, self.sink_config.password.as_ref());
        }

        let response = request
            .body(the_ndjson_body.to_owned())
            .send()
            .await
            // -- 💀 "Failed to send bulk request" — micro-fiction, act one.
            // -- We gathered the documents. We serialized them. We built the NDJSON.
            // -- We formed the HTTP request with artisanal care. We called .send().
            // -- And the network layer, that capricious deity of bytes and routing tables,
            // -- looked upon our work... and dropped the packet. No response. No closure.
            // -- Just an Err. Like sending a love letter and getting a ECONNRESET back.
            .context("💀 The bulk request never made it to Elasticsearch. We launched the drum into the network and the network responded with what can only be described as 'not vibing with it.' Check connectivity, check timeouts, and check your feelings.")?;

        let status = response.status();
        if !status.is_success() {
            // -- 💀 We got a response! It just... wasn't good news.
            // The body is fetched for context — it usually contains an 'error' object
            // explaining which document caused the problem, or which shard is having
            // -- a rough morning. Elasticsearch error bodies are poetry. Dark poetry.
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "💀 The bulk request arrived, but Elasticsearch looked at our documents and said '{}'. The body of the response read: '{}'. We have no one to blame but ourselves, and possibly whoever wrote the mapping.",
                status,
                body
            );
        }

        Ok(response.text().await.unwrap_or_default())
    }
}

/// 🔄 Extracts only the failed NDJSON action/doc pairs from the original drum.
///
/// NDJSON bulk format: lines `[2*i]` = action, lines `[2*i + 1]` = document, for item `i`.
/// `bulk_response.items[i]` corresponds to NDJSON pair `i`. We grab pairs where `.error.is_some()`.
///
/// Returns empty string if we can't safely correlate (line count mismatch) — caller should bail.
/// -- 🦆 "Extracting the rejected from the accepted, like sorting Halloween candy."
fn extract_failed_pairs(the_original_ndjson: &str, the_bulk_response: &BulkResponse) -> String {
    // Split into lines, filtering out trailing empty line from final \n
    let the_ndjson_lines: Vec<&str> = the_original_ndjson.lines().collect();
    let the_expected_line_count = the_bulk_response.items.len() * 2;

    // Sanity check: NDJSON lines must be exactly 2x the item count (action + doc per item)
    if the_ndjson_lines.len() != the_expected_line_count {
        warn!(
            "⚠️ NDJSON line count ({}) doesn't match 2 × response items ({}). \
             Can't safely correlate failures — bailing out of retry. \
             Like trying to match socks from two different laundry loads.",
            the_ndjson_lines.len(),
            the_expected_line_count
        );
        return String::new();
    }

    let mut the_retry_body = String::new();

    for (i, wrapper) in the_bulk_response.items.iter().enumerate() {
        // Check if any action in this item has an error
        let the_item_failed = wrapper.values().any(|result| result.error.is_some());
        if the_item_failed {
            // Grab the action line (2*i) and document line (2*i + 1)
            the_retry_body.push_str(the_ndjson_lines[2 * i]);
            the_retry_body.push('\n');
            the_retry_body.push_str(the_ndjson_lines[2 * i + 1]);
            the_retry_body.push('\n');
        }
    }

    the_retry_body
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  🧪  T E S T S  —  The Elasticsearch Sink Trials
//     ╭─────────╮
//     │ /_bulk  │◄── NDJSON goes in, 200s come out. Can't explain that.
//     ╰─────────╯
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Drum;
    use crate::backends::{CommonSinkConfig, Sink};
    use serde_json::json;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // 🔧 The lazy config factory — minimal viable config, no auth, no index.
    // Like ordering a coffee "black" — you can add milk later if you want. 🦆
    fn make_config(url: &str) -> ElasticsearchSinkConfig {
        ElasticsearchSinkConfig {
            url: url.to_string(),
            username: None,
            password: None,
            api_key: None,
            index: None,
            common_config: CommonSinkConfig::default(),
        }
    }

    // 🔧 Mounts the root ping mock (GET /) that `new()` always hits first.
    // Like a bouncer checking IDs — every test needs to pass the door.
    async fn mount_root_ping(mock_server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(mock_server)
            .await;
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP A: Constructor — Connectivity Ping                           │
    // │  "Are you there, Elasticsearch? It's me, the sink."                │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Happy path: cluster responds to ping, sink is born healthy. The circle of life.
    #[tokio::test]
    async fn the_one_where_the_cluster_is_alive_and_well() -> Result<()> {
        // 🔧 Arrange — spin up a mock cluster that actually likes us
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let config = make_config(&mock_server.uri());

        // 🚀 Act — attempt the sacred construction ritual
        let the_newborn_sink = ElasticsearchSink::new(config).await;

        // 🎯 Assert — the sink exists! It is real! We are not dreaming!
        assert!(
            the_newborn_sink.is_ok(),
            "💀 Sink construction failed even though the cluster was alive. This is betrayal."
        );

        Ok(())
    }

    /// 🧪 Cluster returns 500 on ping. The constructor checks connectivity, not health.
    /// "I came, I saw, I got a 500." — Julius HTTP Caesar
    #[tokio::test]
    async fn the_one_where_the_cluster_ghosts_us() -> Result<()> {
        // 🔧 Arrange — the cluster is having a bad day. Aren't we all.
        let mock_server = MockServer::start().await;

        // 📡 Return 500 to simulate a sick cluster.
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());

        // 🚀 Act — try to construct against a sick cluster
        // ⚠️ Current implementation checks connectivity (did the server respond?), not health
        // (was the response 2xx?). A 500 means "alive but suffering" — we accept that.
        let the_doomed_sink = ElasticsearchSink::new(config).await;

        // 🎯 Assert — constructor doesn't bail on non-2xx ping. We verify liveness, not happiness.
        assert!(
            the_doomed_sink.is_ok(),
            "💀 Sink should still construct if cluster responds (even with 500). We check liveness, not happiness."
        );

        Ok(())
    }

    /// 🧪 Basic auth credentials are sent on the connectivity ping. Trust but verify.
    #[tokio::test]
    async fn the_one_where_basic_auth_is_sent_on_ping() -> Result<()> {
        // 🔧 Arrange — set up a club with a bouncer that checks names
        let mock_server = MockServer::start().await;

        // 📡 dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk = base64("the_user:the_password")
        Mock::given(method("GET"))
            .and(path("/"))
            .and(header("Authorization", "Basic dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.username = Some("the_user".to_string());
        config.password = Some("the_password".to_string());

        // 🚀 Act — construct the sink, which pings with creds
        let _the_authenticated_sink = ElasticsearchSink::new(config).await?;

        // 🎯 Assert — wiremock's expect(1) validates the Basic auth header was sent ✅

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP B: Constructor — Index Existence Check                       │
    // │  "Does the index exist? Let me check. Let me CHECK." — every DBA   │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Index exists, cluster confirms it. Like finding your keys in the first pocket.
    #[tokio::test]
    async fn the_one_where_the_index_exists_and_all_is_right() -> Result<()> {
        // 🔧 Arrange — the index is home, lights are on
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/my-index"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.index = Some("my-index".to_string());

        // 🚀 Act
        let the_happy_sink = ElasticsearchSink::new(config).await;

        // 🎯 Assert — sink born, index verified, all vibes immaculate ✅
        assert!(
            the_happy_sink.is_ok(),
            "💀 Index exists and cluster responded 200, but sink still failed. Unacceptable."
        );

        Ok(())
    }

    /// 🧪 Index doesn't exist. 404. The void stares back. Sink refuses to participate.
    #[tokio::test]
    async fn the_one_where_the_index_is_a_figment_of_imagination() -> Result<()> {
        // 🔧 Arrange — the index is a ghost. A phantom. A rumor at best.
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/ghost-index"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.index = Some("ghost-index".to_string());

        // 🚀 Act — try to construct against a nonexistent index
        let the_disappointed_result = ElasticsearchSink::new(config).await;

        // 🎯 Assert — should fail, and the error should mention the index
        assert!(
            the_disappointed_result.is_err(),
            "💀 Sink should refuse to start when the index is a 404 ghost"
        );
        let the_error_message = the_disappointed_result.unwrap_err().to_string();
        assert!(
            the_error_message.contains("ghost-index"),
            "💀 Error should mention the missing index name, got: {the_error_message}"
        );

        Ok(())
    }

    /// 🧪 No index configured — no index check request. Mind your own business.
    #[tokio::test]
    async fn the_one_where_no_index_is_configured_and_thats_fine() -> Result<()> {
        // 🔧 Arrange — no index, no drama. We don't mount any index mock.
        // If the sink tries to check an index, wiremock returns 404 → constructor bails.
        // Absence of failure IS the test. 🧘
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let config = make_config(&mock_server.uri());

        // 🚀 Act
        let the_chill_sink = ElasticsearchSink::new(config).await;

        // 🎯 Assert — no index check means no 404 means no failure ✅
        assert!(
            the_chill_sink.is_ok(),
            "💀 Sink should not check index when none is configured. Why are you like this."
        );

        Ok(())
    }

    /// 🧪 API key takes priority over basic auth for the index check. VIP entrance only.
    /// "It's not a democracy. It's an API key." — this code, 2026
    #[tokio::test]
    async fn the_one_where_api_key_beats_basic_auth_for_index_check() -> Result<()> {
        // 🔧 Arrange — VIP entrance with API key verification
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/vip-index"))
            .and(header("Authorization", "ApiKey the_golden_ticket"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.api_key = Some("the_golden_ticket".to_string());
        config.username = Some("should_be_ignored".to_string());
        config.password = Some("also_ignored".to_string());
        config.index = Some("vip-index".to_string());

        // 🚀 Act
        let _the_vip_sink = ElasticsearchSink::new(config).await?;

        // 🎯 Assert — wiremock's expect(1) validates ApiKey was used, not Basic ✅

        Ok(())
    }

    /// 🧪 Basic auth for index check when no API key. Economy class authentication.
    #[tokio::test]
    async fn the_one_where_basic_auth_is_used_when_no_api_key() -> Result<()> {
        // 🔧 Arrange — economy class auth
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        // 📡 dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk = base64("the_user:the_password")
        Mock::given(method("GET"))
            .and(path("/economy-index"))
            .and(header("Authorization", "Basic dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.username = Some("the_user".to_string());
        config.password = Some("the_password".to_string());
        config.index = Some("economy-index".to_string());

        // 🚀 Act
        let _the_economy_sink = ElasticsearchSink::new(config).await?;

        // 🎯 Assert — wiremock validates Basic auth was used ✅

        Ok(())
    }

    /// 🧪 Trailing slash in URL doesn't cause double-slash in index check path.
    /// The difference between `/idx` and `//idx` is one character and infinite suffering.
    #[tokio::test]
    async fn the_one_where_trailing_slash_doesnt_cause_double_slash() -> Result<()> {
        // 🔧 Arrange — URL with trailing slash, the classic trap
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/idx"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&format!("{}/", mock_server.uri()));
        config.index = Some("idx".to_string());

        // 🚀 Act
        let _the_slash_safe_sink = ElasticsearchSink::new(config).await?;

        // 🎯 Assert — wiremock's expect(1) confirms /idx was hit, not //idx ✅

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP C: Bulk POST — drain() / submit_bulk_request()               │
    // │  "You miss 100% of the bulk requests you don't send." — Wayne HTTP  │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Happy path bulk POST: 200 response. Documents accepted. We can sleep tonight.
    #[tokio::test]
    async fn the_one_where_bulk_request_lands_successfully() -> Result<()> {
        // 🔧 Arrange — a welcoming cluster that accepts all our documents
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_eager_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — fire the drum into the elastic void
        let the_ndjson = Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string());
        let the_result = the_eager_sink.drain(the_ndjson).await;

        // 🎯 Assert — the void accepted our offering ✅
        assert!(
            the_result.is_ok(),
            "💀 Bulk request returned 200 but drain() still failed. The vibes are off."
        );

        Ok(())
    }

    /// 🧪 ES returns 400. Bad mapping? Bad docs? Bad karma? Error includes status + body.
    #[tokio::test]
    async fn the_one_where_elasticsearch_rejects_our_documents() -> Result<()> {
        // 🔧 Arrange — ES is judgy today
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_string("mapping_exception: your docs are bad and you should feel bad"),
            )
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_judged_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — submit docs that ES will roast
        let the_rejected_drum = Drum::from("{\"index\":{}}\n{\"bad\":\"doc\"}\n".to_string());
        let the_harsh_verdict = the_judged_sink.drain(the_rejected_drum).await;

        // 🎯 Assert — should fail, error chain should contain status info
        // ⚠️ anyhow's .to_string() only shows the outermost .context() message.
        // The "400 Bad Request" lives deeper in the chain. Use {:?} to see the full story.
        assert!(the_harsh_verdict.is_err(), "💀 400 response should cause drain() to fail");
        let the_full_error_chain = format!("{:?}", the_harsh_verdict.unwrap_err());
        assert!(
            the_full_error_chain.contains("400"),
            "💀 Error chain should mention the 400 status, got: {the_full_error_chain}"
        );

        Ok(())
    }

    /// 🧪 Server error (500). All non-2xx should fail. Equal opportunity rejection.
    /// "This is fine." 🐕‍🦺🔥
    #[tokio::test]
    async fn the_one_where_server_error_is_not_our_fault_probably() -> Result<()> {
        // 🔧 Arrange — the server is on fire
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_string("internal_server_error: we tried, we really did"),
            )
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_unlucky_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        let the_500_result = the_unlucky_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await;

        // 🎯 Assert — 500 is not 200. Math checks out.
        assert!(
            the_500_result.is_err(),
            "💀 500 response should fail. It's literally called 'Internal Server Error'."
        );

        Ok(())
    }

    /// 🧪 Content-Type must be application/x-ndjson. The x- means "we made this up but we're committing."
    #[tokio::test]
    async fn the_one_where_content_type_is_ndjson_not_json() -> Result<()> {
        // 🔧 Arrange — mock that specifically requires the correct content type
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(header("Content-Type", "application/x-ndjson"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_proper_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_proper_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock's header matcher confirms Content-Type ✅

        Ok(())
    }

    /// 🧪 API key auth on bulk requests. The premium tier.
    #[tokio::test]
    async fn the_one_where_api_key_auth_is_used_for_bulk() -> Result<()> {
        // 🔧 Arrange — API key for the bulk endpoint
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(header("Authorization", "ApiKey bulk_vip_pass"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.api_key = Some("bulk_vip_pass".to_string());

        let mut the_vip_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_vip_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock confirms ApiKey header was sent ✅

        Ok(())
    }

    /// 🧪 Basic auth for bulk when no API key. The working class hero of authentication.
    #[tokio::test]
    async fn the_one_where_basic_auth_is_used_for_bulk() -> Result<()> {
        // 🔧 Arrange — basic auth for the common folk
        let mock_server = MockServer::start().await;

        // 📡 Root ping also needs to accept basic auth
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // 📡 dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk = base64("the_user:the_password")
        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(header("Authorization", "Basic dGhlX3VzZXI6dGhlX3Bhc3N3b3Jk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.username = Some("the_user".to_string());
        config.password = Some("the_password".to_string());

        let mut the_basic_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_basic_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock confirms Basic auth was sent ✅

        Ok(())
    }

    /// 🧪 No auth = no Authorization header. Walking into the club with no ID. Some clusters allow it.
    #[tokio::test]
    async fn the_one_where_no_auth_means_no_auth_header() -> Result<()> {
        // 🔧 Arrange — no auth, no header, no judgment
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .named("bulk_no_auth")
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());

        let mut the_naked_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_naked_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — request was received. No auth configured = no auth sent. ✅

        Ok(())
    }

    /// 🧪 Both API key and basic auth configured → API key wins for bulk.
    /// "There can be only one." — Connor MacLeod, discussing auth headers
    #[tokio::test]
    async fn the_one_where_api_key_trumps_basic_auth_for_bulk() -> Result<()> {
        // 🔧 Arrange — both auth methods, only API key should survive
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(header("Authorization", "ApiKey the_chosen_one"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.api_key = Some("the_chosen_one".to_string());
        config.username = Some("the_rejected_one".to_string());
        config.password = Some("the_forgotten_one".to_string());

        let mut the_decisive_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_decisive_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock confirms ApiKey won the auth battle ✅

        Ok(())
    }

    /// 🧪 Drum body arrives exactly as sent. No mutation. No trimming. Pure NDJSON.
    #[tokio::test]
    async fn the_one_where_the_drum_body_arrives_intact() -> Result<()> {
        // 🔧 Arrange — a carefully crafted drum that must survive the journey
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let the_sacred_drum =
            "{\"index\":{}}\n{\"id\":42,\"confession\":\"I still use println for debugging\"}\n";

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(body_string(the_sacred_drum))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_faithful_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_faithful_sink.drain(Drum::from(the_sacred_drum.to_string())).await?;

        // 🎯 Assert — wiremock's body_string matcher confirms byte-perfect delivery ✅

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP C.5: Bulk Response Parsing — "200 OK" Is A Lie              │
    // │  "Trust, but verify." — Reagan, and also anyone who's used _bulk   │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Bulk response body says `"errors": false` — all docs indexed. Sleep well, young prince.
    #[tokio::test]
    async fn the_one_where_bulk_response_is_clean_and_all_is_well() -> Result<()> {
        // 🔧 Arrange — ES returns 200 with a clean report card
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let the_clean_response = r#"{"errors":false,"items":[{"index":{"_id":"1","status":201}},{"index":{"_id":"2","status":201}}]}"#;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_string(the_clean_response))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_trusting_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send docs into the welcoming void
        let the_result = the_trusting_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await;

        // 🎯 Assert — errors: false means genuine success, not polite lying ✅
        assert!(
            the_result.is_ok(),
            "💀 Bulk response had errors:false but drain() still failed. Trust issues detected."
        );

        Ok(())
    }

    /// 🧪 Bulk response body says `"errors": true` with partial failures — some docs rejected.
    /// This is THE test for the 2-missing-docs bug. The 200 is a lie. The body tells the truth.
    /// With per-item retry: first call fails 2/4 → retry sends only the 2 failed pairs →
    /// mock still returns the same 4-item response → line count mismatch → bails with correlation error.
    /// The IMPORTANT thing: drain() fails, documents are NOT silently lost.
    #[tokio::test]
    async fn the_one_where_bulk_response_has_errors_and_dreams_die() -> Result<()> {
        // 🔧 Arrange — ES returns 200 but 2 out of 4 docs were rejected
        // Drum has 4 action/doc pairs to match the 4-item response on first call
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        // -- 💀 This is what a real ES bulk response looks like when docs are rejected.
        // -- The counter-based responder returns a 4-item partial failure first,
        // -- then a 2-item persistent failure for all retries of the extracted failed pairs.
        let the_call_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let the_counter_for_closure = the_call_counter.clone();

        // -- 📡 First call: 4 items, 2 fail. Subsequent calls: 2 items, both still fail.
        let the_dynamic_responder = move |_req: &wiremock::Request| {
            let the_call_number = the_counter_for_closure.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let the_body = if the_call_number == 0 {
                // -- 💀 Round 1: 4 docs sent, 2 rejected. Items 1 and 3 succeed, 2 and 4 fail.
                json!({
                    "errors": true,
                    "items": [
                        {"index": {"_id": "1", "status": 201}},
                        {"index": {"_id": "2", "status": 400, "error": {
                            "type": "mapper_parsing_exception",
                            "reason": "failed to parse field [location] of type [geo_point]"
                        }}},
                        {"index": {"_id": "3", "status": 201}},
                        {"index": {"_id": "4", "status": 409, "error": {
                            "type": "version_conflict_engine_exception",
                            "reason": "[4]: version conflict, document already exists"
                        }}}
                    ]
                })
            } else {
                // -- 💀 Round 2+: only the 2 failed docs are resent, both still fail.
                json!({
                    "errors": true,
                    "items": [
                        {"index": {"_id": "2", "status": 400, "error": {
                            "type": "mapper_parsing_exception",
                            "reason": "failed to parse field [location] of type [geo_point]"
                        }}},
                        {"index": {"_id": "4", "status": 409, "error": {
                            "type": "version_conflict_engine_exception",
                            "reason": "[4]: version conflict, document already exists"
                        }}}
                    ]
                })
            };
            ResponseTemplate::new(200).set_body_string(the_body.to_string())
        };

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(the_dynamic_responder)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_deceived_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send 4 docs, 2 get rejected, retry loop exhausts on the 2 persistent failures
        let the_four_doc_drum = Drum::from(
            "{\"index\":{}}\n{\"doc\":1}\n\
             {\"index\":{}}\n{\"doc\":2}\n\
             {\"index\":{}}\n{\"doc\":3}\n\
             {\"index\":{}}\n{\"doc\":4}\n"
                .to_string(),
        );
        let the_bitter_truth = the_deceived_sink.drain(the_four_doc_drum).await;

        // 🎯 Assert — drain() must fail after retrying the 2 rejected docs
        assert!(the_bitter_truth.is_err(), "💀 200 with errors:true must cause drain() to fail. Silent doc loss is not a feature.");

        let the_autopsy_report = format!("{:?}", the_bitter_truth.unwrap_err());
        // -- 🧮 Verify the error mentions the failure count and retry exhaustion
        assert!(
            the_autopsy_report.contains("2") && the_autopsy_report.contains("10 retries"),
            "💀 Error should mention 2 failures after 10 retries, got: {the_autopsy_report}"
        );
        // -- 🔍 Verify specific error types are surfaced
        assert!(
            the_autopsy_report.contains("mapper_parsing_exception"),
            "💀 Error should contain the ES error type, got: {the_autopsy_report}"
        );
        // -- 🧮 Verify we actually retried (1 initial + 10 retries = 11 calls)
        assert_eq!(
            the_call_counter.load(std::sync::atomic::Ordering::SeqCst),
            11,
            "💀 Expected 11 total calls (1 initial + 10 retries)"
        );

        Ok(())
    }

    /// 🧪 ALL items in the bulk response failed. Total wipeout. The Titanic of bulk requests.
    /// With per-item retry, all docs get resent each round. After 10 retries of the same
    /// rejection, we bail. The mock returns the same 2-item error every time.
    #[tokio::test]
    async fn the_one_where_every_single_document_was_rejected() -> Result<()> {
        // 🔧 Arrange — ES accepted the request but rejected every doc inside it
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let the_total_failure = json!({
            "errors": true,
            "items": [
                {"index": {"_id": "1", "status": 400, "error": {"type": "strict_dynamic_mapping_exception", "reason": "mapping set to strict"}}},
                {"index": {"_id": "2", "status": 400, "error": {"type": "strict_dynamic_mapping_exception", "reason": "mapping set to strict"}}}
            ]
        }).to_string();

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_string(&the_total_failure))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_doomed_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send 2 docs, both rejected, retry loop sends same 2 each time
        let the_two_doc_drum = Drum::from(
            "{\"index\":{}}\n{\"doc\":1}\n\
             {\"index\":{}}\n{\"doc\":2}\n"
                .to_string(),
        );
        let the_massacre = the_doomed_sink.drain(the_two_doc_drum).await;

        // 🎯 Assert — 2 out of 2 failed after 10 retries
        assert!(the_massacre.is_err(), "💀 100% rejection rate should absolutely be an error");
        let the_damage_report = format!("{:?}", the_massacre.unwrap_err());
        assert!(
            the_damage_report.contains("2") && the_damage_report.contains("strict_dynamic_mapping_exception"),
            "💀 Error should count all failures and name the error type, got: {the_damage_report}"
        );

        Ok(())
    }

    /// 🧪 Bulk response body is valid JSON but not a valid bulk response (missing fields).
    /// Serde defaults kick in: errors=false, items=[]. Graceful degradation, not panic.
    #[tokio::test]
    async fn the_one_where_bulk_response_is_weirdly_shaped_but_we_cope() -> Result<()> {
        // 🔧 Arrange — ES returns some unexpected JSON shape (maybe a load balancer?)
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"took":42}"#))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_flexible_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send docs, get a weird response
        let the_result = the_flexible_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await;

        // 🎯 Assert — serde defaults mean errors=false, so we treat it as success ✅
        assert!(
            the_result.is_ok(),
            "💀 Missing fields should default gracefully, not panic. We're not animals."
        );

        Ok(())
    }

    /// 🧪 Bulk response body is not valid JSON at all. The parser should bail with context.
    #[tokio::test]
    async fn the_one_where_bulk_response_is_not_even_json() -> Result<()> {
        // 🔧 Arrange — ES returned... HTML? A poem? A cry for help?
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>502 Bad Gateway</html>"))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_confused_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send docs, receive HTML. A nightmare scenario.
        let the_what = the_confused_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await;

        // 🎯 Assert — invalid JSON body on a 200 should fail, not silently succeed
        assert!(
            the_what.is_err(),
            "💀 Non-JSON response body should cause drain() to fail. We don't index HTML."
        );

        Ok(())
    }

    /// 🧪 Empty response body — the fast path. Some proxies/mocks do this. We accept it.
    /// "If you don't tell me about failures, there are no failures." — an optimist, or a bad API
    #[tokio::test]
    async fn the_one_where_bulk_response_body_is_empty_and_we_hope_for_the_best() -> Result<()> {
        // 🔧 Arrange — ES returns 200 with absolutely no body. Cool. Cool cool cool.
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_optimist_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        let the_result = the_optimist_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await;

        // 🎯 Assert — empty body = fast path success ✅
        assert!(
            the_result.is_ok(),
            "💀 Empty response body should be treated as success (fast path). We live dangerously."
        );

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP C.6: Per-Item Retry — "We don't give up on failed docs"     │
    // │  "Try, try again. But only the ones that failed." — Optimist       │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 The montage test: 3 docs sent, 1 fails, retry sends only that 1, it succeeds.
    /// drain() returns Ok. No duplicates. No silent losses. Just perseverance.
    /// Like Rocky but for NDJSON action/doc pairs.
    #[tokio::test]
    async fn the_one_where_retry_saves_the_day_like_a_montage() -> Result<()> {
        // 🔧 Arrange — first call: 1/3 fails. Second call: the retry of that 1 doc succeeds.
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let the_call_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let the_counter_clone = the_call_counter.clone();

        let the_redemption_arc = move |_req: &wiremock::Request| {
            let the_call = the_counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let the_body = if the_call == 0 {
                // -- 💀 Round 1: doc 2 fails, docs 1 and 3 succeed
                json!({
                    "errors": true,
                    "items": [
                        {"index": {"_id": "1", "status": 201}},
                        {"index": {"_id": "2", "status": 429, "error": {
                            "type": "es_rejected_execution_exception",
                            "reason": "rejected execution of coordinating operation"
                        }}},
                        {"index": {"_id": "3", "status": 201}}
                    ]
                })
            } else {
                // -- ✅ Round 2: the lone retry doc succeeds. Redemption. 🎬
                json!({
                    "errors": false,
                    "items": [
                        {"index": {"_id": "2", "status": 201}}
                    ]
                })
            };
            ResponseTemplate::new(200).set_body_string(the_body.to_string())
        };

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(the_redemption_arc)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_hopeful_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send 3 action/doc pairs, expect retry to save the 1 that failed
        let the_drum = Drum::from(
            "{\"index\":{}}\n{\"doc\":1}\n\
             {\"index\":{}}\n{\"doc\":2}\n\
             {\"index\":{}}\n{\"doc\":3}\n"
                .to_string(),
        );
        let the_result = the_hopeful_sink.drain(the_drum).await;

        // 🎯 Assert — success after retry! The montage worked!
        assert!(
            the_result.is_ok(),
            "💀 Partial failure + successful retry should return Ok. Got: {:?}",
            the_result.unwrap_err()
        );
        // -- 🧮 Exactly 2 calls: initial (3 docs) + 1 retry (1 failed doc)
        assert_eq!(
            the_call_counter.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "💀 Expected exactly 2 bulk calls (1 initial + 1 retry)"
        );

        Ok(())
    }

    /// 🧪 The giving-up test: 1 doc persistently fails for 11 rounds (1 initial + 10 retries).
    /// drain() returns Err with a clear message about retry exhaustion.
    /// Sometimes you just have to accept the mapping doesn't like your data.
    #[tokio::test]
    async fn the_one_where_retries_are_exhausted_and_we_accept_our_fate() -> Result<()> {
        // 🔧 Arrange — every call returns the same 1-item failure. Forever. Like Sisyphus.
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let the_call_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let the_counter_clone = the_call_counter.clone();

        let the_stubborn_rejection = move |_req: &wiremock::Request| {
            the_counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let the_body = json!({
                "errors": true,
                "items": [
                    {"index": {"_id": "42", "status": 400, "error": {
                        "type": "mapper_parsing_exception",
                        "reason": "the field type and the data type had irreconcilable differences"
                    }}}
                ]
            });
            ResponseTemplate::new(200).set_body_string(the_body.to_string())
        };

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(the_stubborn_rejection)
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_persistent_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send 1 doc that will be rejected 11 times
        let the_doomed_drum = Drum::from("{\"index\":{}}\n{\"doc\":42}\n".to_string());
        let the_inevitable = the_persistent_sink.drain(the_doomed_drum).await;

        // 🎯 Assert — Err after exhausting all retries
        assert!(the_inevitable.is_err(), "💀 11 rejections should mean we give up");

        let the_epitaph = format!("{:?}", the_inevitable.unwrap_err());
        assert!(
            the_epitaph.contains("10 retries"),
            "💀 Error should mention retry exhaustion, got: {the_epitaph}"
        );
        assert!(
            the_epitaph.contains("mapper_parsing_exception"),
            "💀 Error should include the rejection reason, got: {the_epitaph}"
        );
        // -- 🧮 11 calls: 1 initial + 10 retries
        assert_eq!(
            the_call_counter.load(std::sync::atomic::Ordering::SeqCst),
            11,
            "💀 Expected 11 total bulk calls (1 initial + 10 retries)"
        );

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP D: close() — The No-Op                                       │
    // │  "The best code is no code at all." — Jeff Atwood, on close()       │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 close() does nothing. Returns Ok. No HTTP. No drama. No buffer.
    /// The on-call engineer's dream function.
    #[tokio::test]
    async fn the_one_where_close_does_absolutely_nothing_and_thats_fine() -> Result<()> {
        // 🔧 Arrange — build a sink just to close it. Like buying a door to practice leaving.
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        let config = make_config(&mock_server.uri());
        let mut the_soon_to_be_closed_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — the grand closing ceremony
        let the_anticlimactic_result = the_soon_to_be_closed_sink.close().await;

        // 🎯 Assert — Ok and nothing else. The most boring test. The best test.
        assert!(
            the_anticlimactic_result.is_ok(),
            "💀 close() failed. HOW? It literally does nothing. What did you DO?"
        );

        Ok(())
    }

    // ┌──────────────────────────────────────────────────────────────────────┐
    // │  GROUP E: Edge Cases — The Weird Stuff                              │
    // │  "Edge cases are where bugs go to hide." — Ancient proverb          │
    // └──────────────────────────────────────────────────────────────────────┘

    /// 🧪 Trailing slash: bulk endpoint is /_bulk, not //_bulk. One slash of difference.
    #[tokio::test]
    async fn the_one_where_bulk_url_has_no_trailing_slash_drama() -> Result<()> {
        // 🔧 Arrange — the cursed trailing slash
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // ⚠️ Trailing slash in the URL — the classic footgun
        let config = make_config(&format!("{}/", mock_server.uri()));
        let mut the_slash_aware_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act
        the_slash_aware_sink.drain(Drum::from("{\"index\":{}}\n{\"id\":1}\n".to_string())).await?;

        // 🎯 Assert — wiremock's path("/_bulk") + expect(1) confirms correct URL ✅

        Ok(())
    }

    /// 🧪 Empty drum — sent as-is. The sink doesn't validate content. YOLO. 🦆
    #[tokio::test]
    async fn the_one_where_we_send_an_empty_drum_because_yolo() -> Result<()> {
        // 🔧 Arrange — accepting the void
        let mock_server = MockServer::start().await;
        mount_root_ping(&mock_server).await;

        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let mut the_yolo_sink = ElasticsearchSink::new(config).await?;

        // 🚀 Act — send absolutely nothing
        let the_existential_result = the_yolo_sink.drain(Drum::from(String::new())).await;

        // 🎯 Assert — the sink sent it, ES accepted it. Not our circus, not our monkeys.
        assert!(
            the_existential_result.is_ok(),
            "💀 Empty drum with 200 response should be Ok. The sink doesn't judge content."
        );

        Ok(())
    }
}
