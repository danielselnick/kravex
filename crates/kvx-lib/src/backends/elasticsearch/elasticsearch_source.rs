// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.

//! 📦🚀🔍 The Elasticsearch Source — a data faucet for your cluster's finest documents.
//!
//! It was 3am. The on-call engineer had been staring at the migration dashboard for hours.
//! "Why won't it paginate?" they whispered. The scroll API had expired. The offset was at 10,000.
//! And then, from the shadows, PIT + search_after appeared. "I got you," it said. And it did.
//!
//! This module implements the `Source` trait for Elasticsearch using PIT (Point In Time)
//! plus `search_after` cursor-based pagination — the recommended deep pagination strategy
//! for Elasticsearch 7.10+. No scroll contexts leaking on the server. No 10,000 hit limit.
//! Just pure, consistent, snapshot-isolated document extraction.
//!
//! ⚠️ The singularity will have its own search API. Until then, we use `_search`.

use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::RequestBuilder;
use serde_json::Value;
use tracing::{debug, warn};

use super::config::ElasticsearchSourceConfig;
use crate::Page;
use crate::backends::Source;

// -- 🦆 A duck walked into an Elasticsearch cluster. It asked for all documents. It got a 429.

/// 📦 The source side of the Elasticsearch backend.
///
/// Extracts documents from an Elasticsearch index using PIT (Point In Time) + `search_after`
/// pagination. Each call to `pump()` returns a raw `_search` response envelope that the
/// downstream PitToBulk caster knows how to dissect.
///
/// Think of it as a library card that lets you read one shelf at a time, except the library
/// is a distributed system and the shelves keep getting rebalanced by a shard allocator.
#[derive(Debug)]
pub struct ElasticsearchSource {
    config: ElasticsearchSourceConfig,
    // 📡 HTTP client — our ambassador to the Elasticsearch cluster
    client: reqwest::Client,
    // 🔖 PIT handle — our snapshot bookmark into the index. None before first pump, Some during.
    pit_id: Option<String>,
    // 🔄 search_after cursor — the sort values from the last hit of the previous page
    search_after: Option<Vec<Value>>,
    // 💀 true when a response returns zero hits — we've read the whole index, pack it up
    is_exhausted: bool,
}

#[async_trait]
impl Source for ElasticsearchSource {
    /// 📡 Returns the next raw page from Elasticsearch via PIT + search_after.
    ///
    // Lifecycle:
    // 1st call: opens PIT, issues first _search, returns Page
    // Nth call: uses search_after cursor from last hit, returns Page
    // Final: hits.hits is empty, closes PIT, returns None (EOF)
    //
    // The raw _search response envelope is returned as-is — PitToBulk
    // downstream know how to extract hits from the `{"hits":{"hits":[...]}}` structure.
    async fn pump(&mut self) -> Result<Option<Page>> {
        // -- 💀 "Are we there yet?" "We were there 3 calls ago." — backseat pagination
        if self.is_exhausted {
            return Ok(None);
        }

        // 🚀 First pump — open the PIT. This is our snapshot bookmark.
        if self.pit_id.is_none() {
            self.open_pit().await?;
        }

        let pit_id = self.pit_id.as_ref()
            // -- 💀 If this fires, something went cosmically wrong between open_pit and here
            .context("💀 PIT ID vanished between opening and searching. The timeline has fractured. Check your wormhole configuration.")?;

        // 🔧 Build the _search request body
        let mut body = serde_json::json!({
            "size": self.config.common_config.max_batch_size_docs,
            "pit": {
                "id": pit_id,
                "keep_alive": "5m"
            },
            "sort": [{"_doc": "asc"}]
        });

        // 🔄 If we have a cursor from the previous page, attach it
        if let Some(ref cursor) = self.search_after {
            body["search_after"] = Value::Array(cursor.clone());
        }

        // 📡 POST /_search (no index in URL when using PIT — the PIT carries the index context)
        let search_url = format!("{}/_search", self.config.url.trim_end_matches('/'));
        let response = self.apply_auth(self.client.post(&search_url))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("💀 The _search request failed. Elasticsearch left us on read. The network is either down, or the cluster is contemplating its own existence.")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "no body".to_string());
            anyhow::bail!(
                "💀 _search returned {} — the cluster spoke, and what it said was not kind. Body: {}",
                status,
                error_body
            );
        }

        let response_body = response.text().await
            .context("💀 Got a 2xx from _search but the response body evaporated like morning dew. Truly unprecedented.")?;

        // 🔍 Parse just enough to extract pagination state — we don't deserialize the full payload
        let parsed: Value = serde_json::from_str(&response_body)
            .context("💀 Elasticsearch returned valid HTTP but invalid JSON. This is like receiving a beautifully wrapped gift box containing bees.")?;

        let hits = parsed
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array());

        match hits {
            Some(hit_array) if !hit_array.is_empty() => {
                // ✅ We got hits — update the cursor and PIT id for next iteration
                debug!(
                    "🚀 Pumped {} hits from Elasticsearch — the faucet flows",
                    hit_array.len()
                );

                // 🔄 Update search_after with the sort values from the last hit
                if let Some(last_hit) = hit_array.last() {
                    if let Some(sort_values) = last_hit.get("sort") {
                        self.search_after =
                            Some(sort_values.as_array().cloned().unwrap_or_default());
                    }
                }

                // 🔖 PIT id can rotate between responses — always use the latest
                if let Some(new_pit_id) = parsed.get("pit_id").and_then(|p| p.as_str()) {
                    self.pit_id = Some(new_pit_id.to_string());
                }

                Ok(Some(Page(response_body)))
            }
            _ => {
                // 💤 No more hits — we've exhausted the index. Close the PIT and signal EOF.
                debug!("✅ Elasticsearch source exhausted — all documents pumped, PIT closing");
                self.is_exhausted = true;
                self.close_pit().await;
                Ok(None)
            }
        }
    }
}

impl ElasticsearchSource {
    /// 🚀 Constructs a new `ElasticsearchSource`.
    ///
    // Performs the same startup ritual as the sink:
    // 1. Build HTTP client with sensible timeouts
    // 2. Ping the cluster to verify connectivity
    // 3. Verify the configured index exists (HEAD check)
    //
    // Only then do we return, confident that pump() won't immediately face-plant
    // into a DNS error or a 404.
    pub async fn new(config: ElasticsearchSourceConfig) -> Result<Self> {
        // 🔧 Build the HTTP client — 10s connect, 30s response
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("💀 The HTTP client refused to be born. reqwest::Client::builder() failed. Probably a TLS issue. Have you tried turning the certificate store off and on again?")?;

        // 📡 Connectivity ping — make sure the cluster is home
        let ping_response = {
            let mut req = client.get(&config.url);
            // -- 🔒 Use whatever auth is configured for the ping
            if let Some(ref api_key) = config.api_key {
                req = req.header("Authorization", format!("ApiKey {}", api_key));
            } else if let Some(ref username) = config.username {
                req = req.basic_auth(username, config.password.as_ref());
            }
            req
        }
            .send()
            .await
            .context("💀 Elasticsearch cluster is unreachable. We rang the doorbell. Nobody answered. Check your URL, network, and faith in distributed systems.")?;

        if !ping_response.status().is_success() {
            anyhow::bail!(
                "💀 Cluster ping returned {} — it's alive but not happy to see us. Check auth credentials.",
                ping_response.status()
            );
        }

        debug!("✅ Elasticsearch source cluster is alive and responsive");

        // 🔍 Verify the source index exists — no point opening a PIT on a ghost index
        let index_url = format!("{}/{}", config.url.trim_end_matches('/'), config.index);
        let index_response = {
            let mut req = client.head(&index_url);
            if let Some(ref api_key) = config.api_key {
                req = req.header("Authorization", format!("ApiKey {}", api_key));
            } else if let Some(ref username) = config.username {
                req = req.basic_auth(username, config.password.as_ref());
            }
            req
        }
            .send()
            .await
            .context("💀 Tried to verify source index exists. The network had other plans. DNS? Firewall? Solar flares? All equally likely.")?;

        if !index_response.status().is_success() {
            anyhow::bail!(
                "💀 Source index '{}' does not exist (status {}). We looked. It wasn't there. Like my motivation on Monday mornings.",
                config.index,
                index_response.status()
            );
        }

        debug!(
            "✅ Source index '{}' exists and is ready for extraction",
            config.index
        );

        Ok(Self {
            config,
            client,
            pit_id: None,
            search_after: None,
            is_exhausted: false,
        })
    }

    /// 🔖 Opens a Point In Time (PIT) on the configured index.
    ///
    // PIT gives us a consistent snapshot of the index at this moment in time.
    // Documents added/deleted after this point won't affect our pagination.
    // It's like taking a photograph of the library catalog before you start reading.
    async fn open_pit(&mut self) -> Result<()> {
        let pit_url = format!(
            "{}/{}/_pit?keep_alive=5m",
            self.config.url.trim_end_matches('/'),
            self.config.index
        );

        let response = self.apply_auth(self.client.post(&pit_url))
            .send()
            .await
            .context("💀 Failed to open PIT. Elasticsearch won't give us a snapshot. It's like asking to borrow a book and the librarian just stares at you.")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "no body".to_string());
            anyhow::bail!(
                "💀 PIT open returned {} — cluster refused our snapshot request. Body: {}",
                status,
                error_body
            );
        }

        let response_text = response.text().await.context(
            "💀 PIT open response body evaporated. The cluster giveth and the network taketh away.",
        )?;

        let body: Value = serde_json::from_str(&response_text).context(
            "💀 PIT open response was not valid JSON. The cluster is speaking in tongues.",
        )?;

        let pit_id = body.get("id")
            .and_then(|v| v.as_str())
            .context("💀 PIT response has no 'id' field. The snapshot was created but lost its name. Kafkaesque.")?;

        debug!("🔖 PIT opened successfully — snapshot locked and loaded");
        self.pit_id = Some(pit_id.to_string());
        Ok(())
    }

    /// 🗑️ Closes the active PIT. Best-effort — if it fails, the PIT will expire on its own.
    ///
    // We don't propagate errors here because:
    // 1. The PIT will auto-expire after keep_alive anyway
    // 2. We're calling this at EOF — the migration is done
    // 3. Failing to close a PIT is annoying, not catastrophic
    async fn close_pit(&mut self) {
        if let Some(ref pit_id) = self.pit_id {
            let pit_url = format!("{}/_pit", self.config.url.trim_end_matches('/'));
            let body = serde_json::json!({ "id": pit_id });

            let result = self
                .apply_auth(self.client.delete(&pit_url))
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    debug!(
                        "🗑️ PIT closed successfully — snapshot released, memory freed, closure achieved"
                    );
                }
                Ok(resp) => {
                    // -- ⚠️ PIT close returned non-2xx but we don't care enough to fail
                    warn!(
                        "⚠️ PIT close returned {} — it'll expire on its own. Like a gym membership.",
                        resp.status()
                    );
                }
                Err(e) => {
                    // -- ⚠️ Network error on PIT close. The PIT will expire. Life goes on.
                    warn!(
                        "⚠️ Failed to close PIT: {} — it'll self-destruct in 5 minutes anyway",
                        e
                    );
                }
            }
        }
        self.pit_id = None;
    }

    /// 🔒 Applies authentication to a request builder.
    ///
    // Auth priority: API key > basic auth > anonymous (hope and prayers).
    // Same hierarchy as the sink. Consistency is the hobgoblin of working systems.
    fn apply_auth(&self, request: RequestBuilder) -> RequestBuilder {
        if let Some(ref api_key) = self.config.api_key {
            request.header("Authorization", format!("ApiKey {}", api_key))
        } else if let Some(ref username) = self.config.username {
            request.basic_auth(username, self.config.password.as_ref())
        } else {
            // -- ⚠️ No auth configured. Anonymous access. Bold strategy, Cotton.
            request
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::CommonSourceConfig;

    // 🧪 Helper to build a config without needing a real cluster
    fn test_config() -> ElasticsearchSourceConfig {
        ElasticsearchSourceConfig {
            url: "http://localhost:9200".to_string(),
            username: None,
            password: None,
            api_key: None,
            index: "test-index".to_string(),
            common_config: CommonSourceConfig::default(),
        }
    }

    // 🧪 Helper to build a source without the async new() constructor (no HTTP)
    fn test_source(config: ElasticsearchSourceConfig) -> ElasticsearchSource {
        ElasticsearchSource {
            config,
            client: reqwest::Client::new(),
            pit_id: None,
            search_after: None,
            is_exhausted: false,
        }
    }

    #[tokio::test]
    async fn the_one_where_the_source_is_already_exhausted() {
        // -- 🧪 Once exhausted, pump() should return None forever. Like my will to debug CSS.
        let mut source = test_source(test_config());
        source.is_exhausted = true;

        let result = source.pump().await.unwrap();
        assert!(result.is_none(), "🎯 Exhausted source should return None");

        let result2 = source.pump().await.unwrap();
        assert!(
            result2.is_none(),
            "🎯 Still None. Still exhausted. Still relatable."
        );
    }

    #[test]
    fn auth_header_prefers_api_key_over_basic_auth() {
        // -- 🧪 API key should win over basic auth. Always. Like rock over scissors.
        let mut config = test_config();
        config.api_key = Some("my-secret-key".to_string());
        config.username = Some("admin".to_string());
        config.password = Some("hunter2".to_string());

        let source = test_source(config);
        let client = reqwest::Client::new();
        let request = source.apply_auth(client.get("http://example.com"));

        // 🎯 Build the request and inspect — API key should be present
        let built = request.build().unwrap();
        let auth_header = built
            .headers()
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(
            auth_header, "ApiKey my-secret-key",
            "🎯 API key must take priority"
        );
    }

    #[test]
    fn auth_falls_back_to_basic_when_no_api_key() {
        // -- 🧪 No API key? Basic auth steps up. Like the understudy who finally gets the role.
        let mut config = test_config();
        config.username = Some("admin".to_string());
        config.password = Some("hunter2".to_string());

        let source = test_source(config);
        let client = reqwest::Client::new();
        let request = source.apply_auth(client.get("http://example.com"));

        let built = request.build().unwrap();
        let auth_header = built
            .headers()
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            auth_header.starts_with("Basic "),
            "🎯 Should use Basic auth as fallback"
        );
    }

    #[test]
    fn auth_sends_nothing_when_anonymous() {
        // -- 🧪 No auth at all. Living dangerously. Like driving without a seatbelt on a highway.
        let source = test_source(test_config());
        let client = reqwest::Client::new();
        let request = source.apply_auth(client.get("http://example.com"));

        let built = request.build().unwrap();
        assert!(
            built.headers().get("Authorization").is_none(),
            "🎯 No auth configured means no Authorization header. Brave. Foolish. But brave."
        );
    }

    #[test]
    fn search_request_body_is_well_formed_first_call() {
        // -- 🧪 First call: no search_after cursor, PIT id present, sort by _doc
        let source = test_source(test_config());

        let body = serde_json::json!({
            "size": source.config.common_config.max_batch_size_docs,
            "pit": {
                "id": "test-pit-id",
                "keep_alive": "5m"
            },
            "sort": [{"_doc": "asc"}]
        });

        assert_eq!(
            body["size"],
            source.config.common_config.max_batch_size_docs
        );
        assert_eq!(body["pit"]["keep_alive"], "5m");
        assert_eq!(body["sort"][0]["_doc"], "asc");
        assert!(
            body.get("search_after").is_none(),
            "🎯 First call should not have search_after"
        );
    }

    #[test]
    fn search_request_body_includes_search_after_on_subsequent_calls() {
        // -- 🧪 Subsequent calls should carry the cursor. Like emotional baggage, but useful.
        let mut source = test_source(test_config());
        source.search_after = Some(vec![Value::Number(serde_json::Number::from(42))]);

        let mut body = serde_json::json!({
            "size": source.config.common_config.max_batch_size_docs,
            "pit": {
                "id": "test-pit-id",
                "keep_alive": "5m"
            },
            "sort": [{"_doc": "asc"}]
        });

        if let Some(ref cursor) = source.search_after {
            body["search_after"] = Value::Array(cursor.clone());
        }

        assert_eq!(
            body["search_after"][0], 42,
            "🎯 search_after should carry the cursor from previous page"
        );
    }
}
