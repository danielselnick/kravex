//! # 📡 THE ELASTICSEARCH BACKEND
//!
//! *Previously, on Kravex...*
//!
//! 🎬 COLD OPEN — INT. SERVER ROOM — 3:47 AM
//!
//! The monitoring dashboard glows amber in the dark. One engineer, alone,
//! stares into the abyss of a RED cluster. The abyss stares back and
//! offers a 429. Our hero's coffee has gone cold. Their Slack notifications
//! have reached triple digits. Somewhere in the distance, a PagerDuty alert
//! fires for something completely unrelated, and yet: it hurts.
//!
//! "I'll just reindex it," they whispered. "It'll be fast," they said.
//! "Elasticsearch scales horizontally," someone lied, once, at a conference.
//!
//! 🚀 This module sends your precious documents into the elastic void.
//! It is equal parts HTTP client, bulk API whisperer, and coping mechanism.
//! It accepts bytes. It rejects nothing (except your credentials, probably).
//! It does not judge. It flushes. It moves on. We should all be so lucky.
//!
//! ⚠️ NOTE: If you are reading this at 3am during an incident, take a breath.
//! The data is fine. Probably. The cluster is fine. Mostly. You are fine.
//! Debatable.
//!
//! 🦆 (mandatory duck, no context provided, none shall be requested)
//!
//! — *"In the beginning was the bulk request, and the bulk request was with Elasticsearch,
//!    and the bulk request was Elasticsearch."*
//!    — Book of NDJSON, verse 1:1

// -- 🔧 Standard-issue time stuff. Duration: because "30 seconds" is a vibes-based measurement
// -- until the compiler asks you to be specific about it. Rude, but fair.
use std::time::Duration;
// -- 💀 anyhow: the coping mechanism of error handling. "I don't know what went wrong, but here is a
// -- Context chain that reads like my therapy notes." — anyhow's README, probably.
use anyhow::{Context, Result};
// -- 🧵 async_trait: because Rust's async story is "almost there" in the same way my garage
// -- reorganization project has been "almost there" since 2019.
use async_trait::async_trait;
// -- 📦 serde: the ancient art of turning bytes into structs and back again. Like alchemy, but
// -- it actually works. Unlike alchemy. RIP those alchemists fr.
use serde::Deserialize;
// -- 🚀 tracing: for structured logs that future-you will ctrl+f through at 3am, desperately.
// -- debug! and trace! — the two modes of "I swear I know what this is doing."
use tracing::{debug, trace};

// -- 🏗️ The local souls this module depends on. They did not ask to be imported.
// -- They were called. They answered. This is their burden.
use crate::backends::{Sink, Source};
use crate::progress::ProgressMetrics;
use crate::supervisors::config::{CommonSinkConfig, CommonSourceConfig};

// -- 📡 ElasticsearchSourceConfig — "It's just Elasticsearch", she said, before the cluster went red.
// Moved here from supervisors/config.rs because configs should live near the thing they configure.
// -- Wild concept, I know. Next up: socks living near feet.
//
// 🔧 auth is tri-modal: username+password, api_key, or "I hope anonymous works" (it won't).
// The `common_config` field carries the boring but important stuff: batch sizes, timeouts, etc.
// -- It's the unsung hero. The bassist of this band. Underappreciated. Vital.
#[derive(Debug, Deserialize, Clone)]
pub struct ElasticsearchSourceConfig {
    /// 📡 The URL of your Elasticsearch cluster. Include scheme + port. Yes, all of it.
    /// No, `localhost` alone is not enough. Yes, I know it worked in dev. Yes, I know.
    pub url: String,
    /// 🔒 Username for basic auth. Optional, like flossing. You know you should have one.
    #[serde(default)]
    pub username: Option<String>,
    /// 🔒 Password. If this is in plaintext in your config file, I've already filed a complaint
    /// with the Department of Security Choices.
    #[serde(default)]
    pub password: Option<String>,
    /// 🔒 API key auth — the fancy way. Preferred over basic auth. Like using a card instead of
    /// cash. Or a key fob instead of a key. Or a retinal scanner instead of a key fob.
    /// Point is: hierarchy. This field respects hierarchy.
    #[serde(default)]
    pub api_key: Option<String>,
    /// 📦 Common source settings — the bureaucratic paperwork of data migration.
    /// Max batch size, timeouts, etc. Not glamorous. Essential. Like the appendix.
    #[serde(default)]
    pub common_config: CommonSourceConfig,
}

// -- 🚰 ElasticsearchSinkConfig — "What's the DEAL with index names?" — Jerry Seinfeld, if he were a DevOps engineer.
// -- The `index` field is Option<String> because sometimes you live dangerously and let each doc decide its fate.
//
// ⚠️ Per-doc index routing: each Hit can carry its own `_index` field, which overrides this config.
// This means a single sink can write to multiple indices if your source data is spicy enough.
// -- Whether that's a feature or a cry for help depends entirely on your use case.
#[derive(Debug, Deserialize, Clone)]
pub struct ElasticsearchSinkConfig {
    /// 📡 Where to send the bodies. Uh. The documents. Where to send the documents.
    pub url: String,
    /// 🔒 Username. The bouncer at the club. Except the club is a database.
    #[serde(default)]
    pub username: Option<String>,
    /// 🔒 Password. "password123" is not a password. It is a confession.
    #[serde(default)]
    pub password: Option<String>,
    /// 🔒 API key — the velvet rope variant of authentication.
    #[serde(default)]
    pub api_key: Option<String>,
    /// 📦 The default target index. Optional because each document can carry its own `_index`.
    /// If both are None, `transform_into_bulk` will bail with an existential error message.
    /// You've been warned. The existential error message is very existential.
    pub index: Option<String>,
    /// 🔧 Common sink config: max batch size in bytes, and other life decisions.
    #[serde(flatten, default)]
    pub common_config: CommonSinkConfig,
}

/// 📦 The source side of the Elasticsearch backend.
///
/// This struct holds a config and a progress tracker, and currently does approximately
/// nothing useful in production because `next_batch` returns empty. 🐛
/// It is, however, a *very* well-intentioned nothing. The vibes are all correct.
/// The scaffolding is artisan-grade. The potential is immense. The implementation is... pending.
///
/// No cap, this will slap once scroll/search_after lands. We believe in it. We believe in you.
pub(crate) struct ElasticsearchSource {
    #[allow(dead_code)]
    // -- 🔧 config kept for when next_batch finally stops ghosting us and actually scrolls.
    // -- Marked dead_code because rustc has opinions and no chill.
    config: ElasticsearchSourceConfig,
    // 📊 progress tracker — total_size is 0 because elasticsearch doesn't tell us upfront.
    // -- it's fine. we're fine. we'll show what we can. no percent, no ETA. just vibes.
    // TODO: implement _count query on init so we can actually show progress like adults
    progress: ProgressMetrics,
}

// -- 🎭 manual Debug impl because ProgressMetrics has a ProgressBar inside it,
// -- and ProgressBar from indicatif is a diva that doesn't want to derive Debug.
// -- Same pattern as FileSource. Consistency: the thing your tech lead asks for and nobody does.
impl std::fmt::Debug for ElasticsearchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 🔧 We carefully omit `progress` here because indicatif::ProgressBar does not implement
        // -- Debug. It's a whole thing. Don't ask. Actually do ask — it's a good story about
        // -- why we can't have nice derive macros sometimes. Short version: channels. Long version:
        // -- also channels, but with more feelings.
        f.debug_struct("ElasticsearchSource")
            .field("config", &self.config)
            .finish() // 🚀 progress omitted — it's in there, trust us, no cap
    }
}

impl ElasticsearchSource {
    /// 🚀 Constructs a new `ElasticsearchSource`.
    ///
    /// Currently: allocates a ProgressMetrics with `total_size = 0` because we have
    /// no idea how many docs are waiting for us — Elasticsearch does not greet us at the
    /// door with a number. It's mysterious like that. Enigmatic. A little rude, honestly.
    ///
    /// "How much data?" "Yes." — Elasticsearch, every time.
    ///
    /// ⚠️ Future improvement: fire a `_count` query here so we can show a real ETA
    /// instead of an existential void on the progress bar.
    pub(crate) async fn new(config: ElasticsearchSourceConfig) -> Result<Self> {
        // 📡 total_size = 0: unknown until we scroll through everything.
        // -- Classic elasticsearch — "how much data is there?" — "yes"
        // -- It's fine. We'll count as we go. Like eating chips and not checking how many are left.
        let progress = ProgressMetrics::new(config.url.clone(), 0);
        Ok(Self { config, progress })
    }
}

#[async_trait]
impl Source for ElasticsearchSource {
    /// 📡 Returns the next batch of raw document strings from Elasticsearch.
    ///
    /// Currently returns an empty vec faster than you can say "scroll API."
    /// It's aspirational. It's a placeholder with excellent posture.
    /// The borrow checker is fully satisfied. The product manager is not.
    async fn next_batch(&mut self) -> Result<Vec<String>> {
        // TODO: Implement search_after — the glow-up we deserve.
        Ok(vec![])
    }
}

/// 📡 The sink side of the Elasticsearch backend — pure I/O, zero buffering.
///
/// `ElasticsearchSink` accepts a fully rendered NDJSON payload string and POSTs it
/// to the `_bulk` API. That's it. No internal buffer. No transform logic.
/// The SinkWorker upstream handles transform + binary collect + size management.
///
/// 🧠 Knowledge graph: Sinks are I/O-only abstractions now. This one does HTTP POST.
/// The FileSink does file write. The InMemorySink does Vec push.
/// Buffering, transforming, and collecting moved to SinkWorker. Clean separation.
///
/// Internally holds:
/// - `client`: the HTTP muscle 💪 — reused across requests
/// - `sink_config`: auth, URL, index targeting info
///
/// 🚰 Think of this as the drain at the end of a data pipeline. The last stop.
/// Knock knock. Who's there? HTTP POST. HTTP POST who? HTTP POST your NDJSON
/// and hope the cluster's in a good mood.
#[derive(Debug)]
pub(crate) struct ElasticsearchSink {
    client: reqwest::Client,
    sink_config: ElasticsearchSinkConfig,
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
    /// ⚠️ Basic auth is used for the connectivity ping. API key is used for the index check.
    /// Pick your auth adventure, but be consistent about it in your config.
    pub(crate) async fn new(config: ElasticsearchSinkConfig) -> Result<Self> {
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
        let c = config.clone();
        client
            .get(&c.url)
            .basic_auth(c.username.unwrap_or_default(), c.password)
            .send()
            .await?;

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

    /// 📡 Fires a `_bulk` POST request with the given NDJSON body.
    ///
    /// This is the actual HTTP call that makes documents leave our process and enter
    /// Elasticsearch's warm embrace. Or cold rejection. Depends on the status code.
    ///
    /// Auth is applied here: API key takes priority over basic auth, same as index check.
    /// If the response is not 2xx, we bail with enough detail to file a reasonable postmortem.
    ///
    /// 🔄 This function does not retry. Retries are the caller's problem. Good luck.
    async fn submit_bulk_request(&self, request_body: String) -> Result<()> {
        // -- 📡 Build the bulk endpoint URL. The `_bulk` API: Elasticsearch's loading dock.
        // -- NDJSON only — no JSON arrays, no XML, no CSV, no hand-coded tab-separated values.
        // -- NDJSON. The only format Elasticsearch respects. Truly the format of people who
        // -- wanted JSON but also wanted to feel slightly superior about it.
        let bulk_url = format!("{}/_bulk", self.sink_config.url.trim_end_matches('/'));
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
            .body(request_body)
            .send()
            .await
            // -- 💀 "Failed to send bulk request" — micro-fiction, act one.
            // -- We gathered the documents. We serialized them. We built the NDJSON.
            // -- We formed the HTTP request with artisanal care. We called .send().
            // -- And the network layer, that capricious deity of bytes and routing tables,
            // -- looked upon our work... and dropped the packet. No response. No closure.
            // -- Just an Err. Like sending a love letter and getting a ECONNRESET back.
            .context("💀 The bulk request never made it to Elasticsearch. We launched the payload into the network and the network responded with what can only be described as 'not vibing with it.' Check connectivity, check timeouts, and check your feelings.")?;

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
        } else {
            // -- ✅ Sent! Gone! Into the index! No cap, this function absolutely slapped.
            trace!(
                "🚀 Bulk request landed successfully — documents have left the building, Elvis-style"
            );
        }

        Ok(())
    }

}

#[async_trait]
impl Sink for ElasticsearchSink {
    /// 📡 POST the fully rendered NDJSON payload to /_bulk. Pure I/O. No buffering. No drama.
    ///
    /// The SinkWorker upstream already transformed each doc and binary-collected them into
    /// a single NDJSON payload string. We just fire it into the elastic void.
    /// "In a world where sinks had too many responsibilities... one refactor dared to simplify."
    async fn send(&mut self, payload: String) -> Result<()> {
        debug!(
            "📡 Sending {} bytes to /_bulk — the payload has left the building, Elvis-style",
            payload.len()
        );
        self.submit_bulk_request(payload).await
            .context("💀 The bulk submission stumbled at the finish line. The NDJSON was rendered with love, the SinkWorker did its job, and the HTTP layer said 'nah.' Check connectivity. Check your cluster. Check your horoscope.")?;
        Ok(())
    }

    /// 🗑️ Nothing to flush — we don't buffer. The SinkWorker sends complete payloads.
    /// Close is a no-op. The HTTP client drops cleanly. The connections pool says goodbye.
    /// Knock knock. Who's there? Nobody. The sink is closed. Go home. 🦆
    async fn close(&mut self) -> Result<()> {
        debug!("🗑️ Elasticsearch sink closing — no buffer to flush, just vibes to release");
        Ok(())
    }
}
