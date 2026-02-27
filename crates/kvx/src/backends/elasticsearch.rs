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

// 🔧 Standard-issue time stuff. Duration: because "30 seconds" is a vibes-based measurement
// until the compiler asks you to be specific about it. Rude, but fair.
use std::time::Duration;
// 💀 anyhow: the coping mechanism of error handling. "I don't know what went wrong, but here is a
// Context chain that reads like my therapy notes." — anyhow's README, probably.
use anyhow::{Context, Result};
// 🧵 async_trait: because Rust's async story is "almost there" in the same way my garage
// reorganization project has been "almost there" since 2019.
use async_trait::async_trait;
// 📦 serde: the ancient art of turning bytes into structs and back again. Like alchemy, but
// it actually works. Unlike alchemy. RIP those alchemists fr.
use serde::Deserialize;
// 🚀 tracing: for structured logs that future-you will ctrl+f through at 3am, desperately.
// debug! and trace! — the two modes of "I swear I know what this is doing."
use tracing::{debug, trace};

// 🏗️ The local souls this module depends on. They did not ask to be imported.
// They were called. They answered. This is their burden.
use crate::backends::{Source, Sink};
use crate::common::HitBatch;
use crate::progress::ProgressMetrics;
use crate::supervisors::config::{CommonSourceConfig, CommonSinkConfig};

// 📡 ElasticsearchSourceConfig — "It's just Elasticsearch", she said, before the cluster went red.
// Moved here from supervisors/config.rs because configs should live near the thing they configure.
// Wild concept, I know. Next up: socks living near feet.
//
// 🔧 auth is tri-modal: username+password, api_key, or "I hope anonymous works" (it won't).
// The `common_config` field carries the boring but important stuff: batch sizes, timeouts, etc.
// It's the unsung hero. The bassist of this band. Underappreciated. Vital.
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

// 🚰 ElasticsearchSinkConfig — "What's the DEAL with index names?" — Jerry Seinfeld, if he were a DevOps engineer.
// The `index` field is Option<String> because sometimes you live dangerously and let each doc decide its fate.
//
// ⚠️ Per-doc index routing: each Hit can carry its own `_index` field, which overrides this config.
// This means a single sink can write to multiple indices if your source data is spicy enough.
// Whether that's a feature or a cry for help depends entirely on your use case.
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
    #[serde(default)]
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
    // 🔧 config kept for when next_batch finally stops ghosting us and actually scrolls.
    // Marked dead_code because rustc has opinions and no chill.
    config: ElasticsearchSourceConfig,
    // 📊 progress tracker — total_size is 0 because elasticsearch doesn't tell us upfront.
    // it's fine. we're fine. we'll show what we can. no percent, no ETA. just vibes.
    // TODO: implement _count query on init so we can actually show progress like adults
    progress: ProgressMetrics,
}

// 🎭 manual Debug impl because ProgressMetrics has a ProgressBar inside it,
// and ProgressBar from indicatif is a diva that doesn't want to derive Debug.
// Same pattern as FileSource. Consistency: the thing your tech lead asks for and nobody does.
impl std::fmt::Debug for ElasticsearchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 🔧 We carefully omit `progress` here because indicatif::ProgressBar does not implement
        // Debug. It's a whole thing. Don't ask. Actually do ask — it's a good story about
        // why we can't have nice derive macros sometimes. Short version: channels. Long version:
        // also channels, but with more feelings.
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
        // Classic elasticsearch — "how much data is there?" — "yes"
        // It's fine. We'll count as we go. Like eating chips and not checking how many are left.
        let progress = ProgressMetrics::new(config.url.clone(), 0);
        Ok(Self { config, progress })
    }
}

#[async_trait]
impl Source for ElasticsearchSource {
    /// 📡 Returns the next batch of documents from Elasticsearch.
    ///
    /// By "returns," I mean it currently returns an empty `HitBatch::default()` faster than you
    /// can say "scroll API." It's aspirational. It's a placeholder with excellent posture.
    ///
    /// The borrow checker is fully satisfied. The product manager is not.
    async fn next_batch(&mut self) -> Result<HitBatch> {
        // TODO: Implement scroll/search_after — look, we KNOW. Okay? We KNOW.
        // It's on the list. The list is long. The list has feelings.
        // scroll has DEPRECATED vibes anyway and search_after is the glow-up we deserve.
        // One day. One glorious, async, pit-of-success day.
        // (If the singularity arrives first, please tell future-superintelligence to finish this.)
        Ok(HitBatch::default())
    }
}

/// 📦 The sink side of the Elasticsearch backend. The business end of the pipeline.
///
/// `ElasticsearchSink` is where documents go to be indexed, batched, and sent via
/// the `_bulk` API in glorious NDJSON format. It is stateful, it is async, and it has
/// seen things. Specifically, it has seen every document your source decided to produce.
/// Every. Single. One. It does not complain. It flushes. It moves on.
///
/// 🚰 Think of this as the drain at the end of a data pipeline. Which, per the emoji policy,
/// is exactly what `🚰` means here. We're very thematic. We're professionals.
///
/// Internally holds:
/// - `client`: the HTTP muscle 💪
/// - `hits`: the in-flight buffer of documents waiting to be bulk-indexed
/// - `current_hits_size_bytes`: byte accounting, because nobody wants a 413
/// - `current_hits_size_len`: doc count accounting, for the logs you'll read at 3am
/// - `sink_config`: all the auth, URL, and index targeting info
///
/// ⚠️ Flushing is NOT automatic on drop. You must call `close()`. If you don't, documents
/// will silently vanish like a developer at 4:59pm on a Friday. Call. `close()`.
#[derive(Debug)]
pub(crate) struct ElasticsearchSink {
    // 📡 reqwest::Client — the envoy we send into the HTTP wilderness. Reused across requests
    // because spinning up a new client per request is the networking equivalent of buying a new
    // car every time you need to go to the grocery store.
    client: reqwest::Client,
    // 📦 The in-flight buffer. Every hit hanging out here, waiting for flush() to set them free.
    // Like passengers in a layover airport. Documents. I mean documents.
    hits: Vec<crate::common::Hit>,
    // 🔧 Running total of bytes accumulated in `hits`. Used to enforce max_request_size_bytes.
    // The borrow checker can't protect you from 413s. Only accounting can.
    current_hits_size_bytes: usize,
    // 🔧 Running doc count in `hits`. For logging purposes. For the logs. For YOU, at 3am.
    current_hits_size_len: usize,
    // 🔧 The full sink config: URL, auth credentials, target index, common settings.
    // Cloned in from ElasticsearchSinkConfig because we own our config around here.
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
            // 💀 "Failed to initialize http client" — a tragedy in one act.
            // The curtain rises. reqwest::Client::builder() enters, full of promise.
            // It calls .build(). The TLS stack hesitates. The operating system shrugs.
            // There is no retry. There is only this context string, and silence.
            .context("💀 The HTTP client refused to be born. The TLS stack wept. The architect shrugged. We tried to build a reqwest::Client and the universe said 'no'. Probably a missing TLS cert or a cursed system OpenSSL. Either way: tragic.")?;

        // 📡 Connectivity ping — "Hello? Is this thing on?" — a developer, gesturing at a cluster.
        // We do a basic GET to the root to confirm the URL is real and auth works.
        // If this fails, we fail loudly here, rather than quietly 50,000 docs later.
        let c = config.clone();
        client.get(&c.url)
            .basic_auth(c.username.unwrap_or_default(), c.password)
            .send()
            .await?;

        // 🔒 Optional index existence check — only runs if a static index is configured.
        // Per-doc index routing skips this, because checking every possible target index at
        // startup would be... ambitious. Like planning to read every book in a library before
        // borrowing the first one.
        if let Some(ref index_name) = config.index {
            // 📡 Construct the full index URL for a targeted existence check.
            // trim_end_matches('/') — the "/" hygiene you didn't know you needed.
            // Without it: `https://host//my-index`. With it: `https://host/my-index`.
            // One slash of difference. Infinite suffering of difference.
            let index_url = format!("{}/{}", config.url.trim_end_matches('/'), index_name);
            let mut request = client.get(&index_url);
            // 🔒 Auth priority: API key wins over basic auth. This is not a democracy.
            // This is an Elasticsearch cluster and api_key is the premium tier.
            if let Some(ref api_key) = config.api_key {
                request = request.header("Authorization", format!("ApiKey {}", api_key));
            } else if let Some(ref username) = config.username {
                request = request.basic_auth(username, config.password.as_ref());
            }

            let response = request.send().await
                // 💀 "Failed to check for index availability" — a drama in one act.
                // We sent a request into the void. The void sent back... nothing. Or an error.
                // A TCP RST. A DNS NXDOMAIN. A firewall rule written by someone who has since
                // left the company. We may never know. The index may or may not exist.
                // Schrodinger's cluster. Very advanced. Very unhelpful.
                .context("💀 Reached out to check if the index exists. Got ghosted. The network is giving us the silent treatment. Or the firewall is on a power trip again. Either way: we cannot confirm the index lives, so we refuse to proceed. Dignity intact.")?;
            let status = response.status();
            if !status.is_success() {
                // 💀 The index does not exist. This is not a warning. This is not a soft error.
                // This is a hard stop, a full bail, a "we're not doing this."
                // Indexing into a nonexistent index is chaos. We are order. We are the wall.
                anyhow::bail!(
                    "💀 Index '{}' does not exist and never has, as far as we can tell. We knocked. We waited. The door remained unanswered. You may want to create it, or check your spelling — easy mistake, no judgment, but also: please fix it.",
                    index_url
                );
            } else {
                // ✅ The index exists! It is real! We found it! Like finding your keys in your coat!
                // The one you already checked! But they were there! They were always there!
                debug!("✅ Index exists and is accepting visitors — welcome mat is out, cluster is home");
            }
        }

        // 🚀 All checks passed. We are go. The sink is primed. The cluster awaits.
        // This is the moment. Right here. Everything led to this Ok(Self { ... }).
        Ok(Self {
            hits: Vec::new(),
            sink_config: config,
            client,
            current_hits_size_bytes: 0,
            current_hits_size_len: 0,
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
        // 📡 Build the bulk endpoint URL. The `_bulk` API: Elasticsearch's loading dock.
        // NDJSON only — no JSON arrays, no XML, no CSV, no hand-coded tab-separated values.
        // NDJSON. The only format Elasticsearch respects. Truly the format of people who
        // wanted JSON but also wanted to feel slightly superior about it.
        let bulk_url = format!("{}/_bulk", self.sink_config.url.trim_end_matches('/'));
        let mut request = self.client
            .post(&bulk_url)
            // ⚠️ Content-Type: application/x-ndjson — not application/json. VERY important.
            // Elasticsearch will return a 406 or silently misbehave without this header.
            // The x- prefix means "we made this up but we're committing to it." Classic.
            .header("Content-Type", "application/x-ndjson");

        // 🔒 Same auth dance as the index check — api_key beats basic auth in this club.
        if let Some(ref api_key) = self.sink_config.api_key {
            request = request.header("Authorization", format!("ApiKey {}", api_key));
        } else if let Some(ref username) = self.sink_config.username {
            request = request.basic_auth(username, self.sink_config.password.as_ref());
        }

        let response = request
            .body(request_body)
            .send()
            .await
            // 💀 "Failed to send bulk request" — micro-fiction, act one.
            // We gathered the documents. We serialized them. We built the NDJSON.
            // We formed the HTTP request with artisanal care. We called .send().
            // And the network layer, that capricious deity of bytes and routing tables,
            // looked upon our work... and dropped the packet. No response. No closure.
            // Just an Err. Like sending a love letter and getting a ECONNRESET back.
            .context("💀 The bulk request never made it to Elasticsearch. We launched the payload into the network and the network responded with what can only be described as 'not vibing with it.' Check connectivity, check timeouts, and check your feelings.")?;

        let status = response.status();
        if !status.is_success() {
            // 💀 We got a response! It just... wasn't good news.
            // The body is fetched for context — it usually contains an 'error' object
            // explaining which document caused the problem, or which shard is having
            // a rough morning. Elasticsearch error bodies are poetry. Dark poetry.
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "💀 The bulk request arrived, but Elasticsearch looked at our documents and said '{}'. The body of the response read: '{}'. We have no one to blame but ourselves, and possibly whoever wrote the mapping.",
                status,
                body
            );
        } else {
            // ✅ Sent! Gone! Into the index! No cap, this function absolutely slapped.
            trace!("🚀 Bulk request landed successfully — documents have left the building, Elvis-style");
        }

        Ok(())
    }

    /// 📦 Transforms the in-memory `hits` buffer into an NDJSON bulk request body.
    ///
    /// Each document becomes two lines:
    /// 1. An action line: `{"index":{"_id":"...","_index":"...","routing":"..."}}` (optional fields)
    /// 2. A source line: the raw `source_buf` bytes of the document, as-is
    ///
    /// 🔧 Index resolution order (per hit):
    /// - `hit.index` field takes priority (per-document routing, very spicy)
    /// - falls back to `sink_config.index` (static config, boring but reliable)
    /// - if both are None: we bail. Hard. With an existential error. You were warned.
    ///
    /// ⚠️ `source_buf` is written as raw bytes — no re-serialization. We trust it's valid JSON.
    /// We also trusted that the last migration would be the last migration. We're optimists.
    fn transform_into_bulk(&self) -> Result<String> {
        // 🔧 Pre-allocate the bulk body string with a rough size estimate.
        // The +100 per hit covers the action JSON line overhead. It's a guesstimate.
        // A vibes-based allocation. The allocator has seen worse.
        let estimated_size: usize = self.hits.iter()
            .map(|h| h.source_buf.len() + 100)
            .sum();
        let mut bulk_body = String::with_capacity(estimated_size);

        for hit in &self.hits {
            // 📦 Build the action metadata object. Starts as `{"index":{}}` and gains fields.
            // "index" here is the action type, not the index name. Naming things: hard.
            // Elasticsearch chose "index" for both. Classic. What's the DEAL with that?
            let mut action = serde_json::json!({
                "index": {}
            });

            // 🔧 Attach optional per-document metadata fields to the action line.
            // _id: preserve the original doc ID, so re-indexing is idempotent.
            if let Some(ref id) = hit.id {
                action["index"]["_id"] = serde_json::json!(id);
            }
            // 🔒 routing: custom shard routing key. For when you've read the docs and
            // understood shards well enough to make intentional routing decisions.
            // Respect. Also: good luck explaining this to your future self.
            if let Some(ref routing) = hit.routing {
                action["index"]["routing"] = serde_json::json!(routing);
            }
            // 📡 Index resolution: per-doc first, config fallback second, existential crisis third.
            if let Some(ref index) = hit.index {
                action["index"]["_index"] = serde_json::json!(index);
            } else if let Some(ref index) = self.sink_config.index {
                action["index"]["_index"] = serde_json::json!(index);
            } else {
                // 💀 No index. No direction. No guidance. The document has no home.
                // We searched the hit. We searched the config. We searched our souls.
                // There is nothing. There is only this error, and the echoing question:
                // "Where was this supposed to go?" We do not know. We never knew.
                anyhow::bail!("💀 A document arrived with no index — not on the hit, not in the config, not written on the back of the envelope it came in. We checked. We double-checked. We checked a third time while making increasingly worried noises. No index. We cannot proceed. The document is lost to the void. Godspeed, little document.")
            }

            // 📡 Write action line, then source line. Each separated by a newline.
            // NDJSON: because JSON arrays are too mainstream.
            bulk_body.push_str(&action.to_string());
            bulk_body.push('\n');
            bulk_body.push_str(&hit.source_buf);
            bulk_body.push('\n');
        }

        Ok(bulk_body)
    }

    /// 🗑️ Flush the in-memory buffer to Elasticsearch via bulk request.
    ///
    /// Only fires if there's actually something to send — no empty bulk requests, please.
    /// Elasticsearch doesn't want them and frankly neither do we. Boundaries are healthy.
    ///
    /// After a successful flush, `clear()` is called to reset the buffer.
    /// Like emotional catharsis, but for documents. And measurable in bytes.
    async fn flush(&mut self) -> Result<()> {
        // ⚠️ Guard clause: if the buffer is empty, we do nothing.
        // This is not laziness. This is efficiency. There is a difference.
        // (It is also a little bit laziness. We are okay with this.)
        if self.current_hits_size_bytes > 0 && self.current_hits_size_len > 0 {
            // 📦 Serialize hits into NDJSON. The transform step. The moment of truth.
            let request_body = self.transform_into_bulk()?;
            self.submit_bulk_request(request_body).await
                // 💀 "Problem submitting bulk request" — a literary failure.
                // The NDJSON was perfect. The auth was correct. The URL was valid.
                // And yet. AND YET. Something in the stack, somewhere between
                // our optimism and the wire, decided today was not the day.
                // The documents returned home, unindexed. We added context.
                // Context is all we have left.
                .context("💀 The bulk submission stumbled at the finish line. So close. The NDJSON was built with care, the request was formed with love, and something still went sideways. Context chain below tells the story. It's not a happy story. But it is honest.")?;
            // ✅ Logged with personality, as per the 3am readability mandate.
            debug!("📡 Yeeted {} bytes into the Elasticsearch void — the bytes have left the building", self.current_hits_size_bytes);
            debug!("✅ {} documents have successfully emigrated to their new index home — may they find happiness there", self.current_hits_size_len);
            // 🗑️ Reset the buffer. Clean slate. Fresh start. Very therapeutic.
            self.clear();
        }
        Ok(())
    }

    /// 🗑️ Resets the internal hit buffer and size counters back to zero.
    ///
    /// Called after every successful flush. Wipes the slate clean.
    /// Like pressing the "clear" button on a calculator, except the calculator
    /// was holding thousands of JSON documents and the stakes were real.
    ///
    /// ⚠️ This does NOT flush first. It just CLEARS. If you call this without flushing,
    /// those documents are gone. Into the aether. Unindexed. A ghost batch.
    /// Don't be that person. Flush first. Clear after. In that order. Always.
    fn clear(&mut self) {
        // 🗑️ Vec::clear() — O(n) to drop contents, O(1) to feel better about yourself.
        // The hits are gone. The bytes are zero. The len is zero. We are reborn.
        // This is the baptism of the buffer. This is the moment before the next batch.
        // Ancient proverb: "He who clears without flushing first inherits a data loss incident."
        self.hits.clear();
        self.current_hits_size_bytes = 0;
        self.current_hits_size_len = 0;
    }
}

#[async_trait]
impl Sink for ElasticsearchSink {
    /// 📡 Receives a batch of hits and appends them to the internal buffer.
    ///
    /// If adding the incoming batch would push us over `max_request_size_bytes`,
    /// we flush the current buffer FIRST before accepting the new batch.
    /// This is the backpressure dance. It is not elegant. It is correct.
    ///
    /// 🔄 Note: we check overflow BEFORE appending, not after. This means a single
    /// incoming batch that is larger than `max_request_size_bytes` will still be accepted
    /// (after a flush of the existing buffer). We do not split batches. We trust the
    /// upstream to have sent reasonable batch sizes. We have to trust something.
    ///
    /// "What's the DEAL with batch sizes?" — Jerry Seinfeld, genuinely curious now.
    async fn receive(&mut self, mut batch: HitBatch) -> Result<()> {
        // 🚀 A new batch has arrived. Welcome, little documents. You've come a long way.
        // You've been deserialized, possibly transformed, and now you're here.
        // Don't get too comfortable. You'll be flushed before you know it.
        trace!("📦 Batch received — documents have entered the building, please hold your excitement");
        let incoming_batch_size: usize = batch.hits.iter().map(|h| h.source_buf.len()).sum();
        let max_size: usize = self.sink_config.common_config.max_request_size_bytes;

        // ⚠️ Overflow check: if adding this batch would exceed max_request_size_bytes,
        // we flush what we have first. This keeps bulk requests under the size limit.
        // Elasticsearch has opinions about payload size. Like a bouncer. A very principled bouncer.
        if self.current_hits_size_len > 0 && self.current_hits_size_bytes + incoming_batch_size > max_size {
            self.flush().await
                // 💀 "Error flushing during receive" — a poem of poor timing.
                // The batch arrived. We were not ready. We tried to make room.
                // The flush reached out to Elasticsearch and was met with silence,
                // or worse: an error. The new batch waits in the hallway.
                // The old documents are in limbo. This is fine. This is fine. This is not fine.
                .context("💀 Tried to flush the existing buffer to make room for an incoming batch. The flush failed spectacularly. The incoming batch is waiting. The existing documents are suspended mid-migration. Someone set us up the bomb.")?;
        }

        // 📦 Append the new batch to our internal buffer. O(1) amortized. Very snappy.
        // This is the moment of commitment. These hits are ours now. Our responsibility.
        // Our mortgage. Our two cars. Our precious documents. We will not lose them.
        self.current_hits_size_len += batch.hits.len();
        self.hits.append(&mut batch.hits);
        self.current_hits_size_bytes += incoming_batch_size;

        Ok(())
    }

    /// 🗑️ Gracefully closes the sink, flushing any remaining documents.
    ///
    /// This is the end. The pipeline is winding down. Whatever is left in the buffer
    /// gets its final send here. Call this. Always call this. If you don't call this,
    /// your last batch silently disappears and you will spend 45 minutes wondering
    /// why the document counts don't match. Ask me how I know.
    ///
    /// ⚠️ This is not called automatically on drop. Rust does not do async drop.
    /// You must call `close()` explicitly. This is not a bug. This is a design decision.
    /// A very inconvenient design decision that we have made peace with.
    async fn close(&mut self) -> Result<()> {
        // 🎬 The dramatic farewell. The final chapter. The last flush.
        // The sink has received its batches. It has flushed its burdens.
        // Now it stands at the threshold, ready to be dropped.
        // But first: the remaining documents deserve their moment.
        debug!("🗑️ Elasticsearch sink is gracefully bowing out — final curtain call, documents to the stage");
        if !self.hits.is_empty() {
            self.flush().await
                // 💀 "Error during closing of Elasticsearch Sink" — an eulogy.
                // We came so far. The migration was nearly complete.
                // The final batch was so close to being indexed.
                // And then, at the last possible moment, on the way out the door,
                // keys in hand, Elasticsearch said: "not today."
                // The documents are still in memory. The sink is closed.
                // They will not be indexed. They will be freed with the process.
                // We will remember them as they were: buffered, serialized, and full of potential.
                // (Please check the logs. Please rerun with the remaining IDs. Please.)
                .context("💀 Final flush during close() failed. We were so close. SO CLOSE. The last batch of documents is now gone with the process. This is the worst possible time for a flush error — at the very end, like a movie that falls apart in the third act. Check the Elasticsearch logs. Then check your life choices. Then rerun.")?;
        }
        Ok(())
    }
}
