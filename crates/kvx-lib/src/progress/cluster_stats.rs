// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// AI
//! 📡🧠🦆 cluster_stats — The Cluster Whisperer.
//!
//! *[INT. SERVER ROOM — 2:47 AM]*
//! *[A lone poller reaches out to the Elasticsearch cluster.]*
//! *["How are you feeling?" it asks. The cluster responds with JSON.]*
//! *[Some say the singularity will arrive before this module ships. They may be right.]*
//!
//! Polls `_nodes/stats/process,jvm` from ES/OS clusters and extracts process CPU% and JVM heap%.
//! Stateless except for an HTTP client. Fire-and-forget via `tokio::spawn` — the
//! renderer kicks off a fetch each tick and harvests the result when it's ready.

use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use tokio::task::JoinHandle;

// =========================================================================
//  🔑 Auth — because clusters have trust issues
// =========================================================================

/// 🔒 Authentication for cluster stats polling.
///
/// Tri-modal like a Swiss Army knife: Basic, ApiKey, or just walk in like you own the place.
/// "He who polls without credentials, gets 401 in production." — Ancient proverb 📜
#[derive(Debug, Clone)]
pub enum ClusterAuth {
    // -- 🔑 username:password — the OG authentication, like a deadbolt on your front door
    Basic { username: String, password: String },
    // -- 🗝️ API key auth — fancier, like a keycard at a hotel you can't afford
    ApiKey(String),
    // -- 🔓 no auth — living on the edge, like deploying on Friday at 4:59pm
    None,
}

// =========================================================================
//  📸 Snapshot — one moment in time, frozen like Han Solo
// =========================================================================

/// 📊 A point-in-time snapshot of cluster health metrics.
///
/// Averaged across all nodes. Two numbers. That's it. That's the tweet. 🐦
/// CPU tells you if the cluster is sweating. JVM heap tells you if the GC is panicking.
#[derive(Debug, Clone, Copy)]
pub struct ClusterSnapshot {
    // -- 🧠 average CPU% across all nodes — 0-100, like a midterm grade
    pub cpu_percent: f64,
    // -- 📦 average JVM heap used% across all nodes — when this hits 90+ start praying
    pub jvm_heap_percent: f64,
}

// =========================================================================
//  📡 Poller — the struct that does the asking
// =========================================================================

/// 🏗️ Stateless-ish poller for cluster node stats.
///
/// Holds a URL, auth config, and an HTTP client. Call `fetch()` to spawn a background
/// task that hits `_nodes/stats/process,jvm` and returns a `ClusterSnapshot`.
/// The caller decides when to harvest the result. Like planting a garden and checking
/// on it when you feel like it. 🌱
///
/// "If you're reading this, the code review went poorly." 🦆
pub struct ClusterStatsPoller {
    // -- 📡 base URL of the cluster (e.g., "http://localhost:9200")
    url: String,
    // -- 🔒 auth credentials — cached from config at construction time
    auth: ClusterAuth,
    // -- 🌐 reqwest client — connection pooling, timeouts, the whole nine yards
    client: Client,
}

impl ClusterStatsPoller {
    /// 🏗️ Build a poller from Elasticsearch/OpenSearch config fields.
    ///
    /// Auth priority: api_key > username+password > none.
    /// Like a nightclub bouncer checking IDs in order of impressiveness. 🪪
    pub fn from_es_config(
        url: &str,
        username: Option<&str>,
        password: Option<&str>,
        api_key: Option<&str>,
    ) -> Self {
        // -- 🔑 resolve auth — api_key wins if present, then basic, then yolo
        let auth = if let Some(key) = api_key {
            ClusterAuth::ApiKey(key.to_string())
        } else if let Some(user) = username {
            ClusterAuth::Basic {
                username: user.to_string(),
                password: password.unwrap_or("").to_string(),
            }
        } else {
            ClusterAuth::None
        };

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("💀 Failed to build reqwest client — this is like failing to open a door. A door that was already open.");

        Self {
            url: url.trim_end_matches('/').to_string(),
            auth,
            client,
        }
    }

    /// 🚀 Kick off a background fetch. Returns a JoinHandle immediately.
    ///
    /// The spawned task hits `_nodes/stats/os,jvm`, parses the response, averages
    /// CPU and JVM heap% across all nodes. Non-blocking — the renderer calls this
    /// and checks back later, like ordering takeout and refreshing the tracker. 🍕
    pub fn fetch(&self) -> JoinHandle<Result<ClusterSnapshot>> {
        let the_client = self.client.clone();
        let the_url = self.url.clone();
        let the_auth = self.auth.clone();

        tokio::spawn(async move { fetch_cluster_stats(&the_client, &the_url, &the_auth).await })
    }
}

// =========================================================================
//  🔬 Internal fetch logic — the part that actually talks to the cluster
// =========================================================================

/// 📡 Fetch and parse cluster stats from `_nodes/stats/process,jvm`.
///
/// Averages process CPU% and JVM heap% across all reporting nodes.
/// Nodes that don't report a metric are skipped — like that one coworker
/// who never fills out their timesheet. We just work around them. 🦆
async fn fetch_cluster_stats(
    client: &Client,
    base_url: &str,
    auth: &ClusterAuth,
) -> Result<ClusterSnapshot> {
    let the_stats_url = format!("{}/_nodes/stats/process,jvm", base_url);

    // -- 📡 build the request — auth applied like seasoning on a steak
    let mut the_request_builder = client.get(&the_stats_url);
    match auth {
        ClusterAuth::Basic { username, password } => {
            the_request_builder = the_request_builder.basic_auth(username, Some(password));
        }
        ClusterAuth::ApiKey(key) => {
            the_request_builder =
                the_request_builder.header("Authorization", format!("ApiKey {}", key));
        }
        ClusterAuth::None => {}
    }

    let the_response = the_request_builder
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(
            "💀 Failed to reach _nodes/stats at {} — the cluster is giving us the silent treatment. Error: {}",
            the_stats_url, e
        ))?;

    let the_body = the_response.text().await.map_err(|e| {
        anyhow::anyhow!(
            "💀 Got a response from _nodes/stats but the body was unreadable — \
             like receiving a postcard written in cursive by a doctor. Error: {}",
            e
        )
    })?;

    let the_stats: NodeStatsResponse = serde_json::from_str(&the_body).map_err(|e| {
        anyhow::anyhow!(
            "💀 Failed to parse _nodes/stats JSON — expected os,jvm fields, \
             got something that looks like my kid drew it. Error: {}",
            e
        )
    })?;

    // -- 📊 Average CPU and JVM heap across all nodes that report them
    let mut the_cpu_sum = 0.0_f64;
    let mut the_cpu_count = 0_u64;
    let mut the_jvm_heap_sum = 0.0_f64;
    let mut the_jvm_count = 0_u64;

    for (_node_id, node) in &the_stats.nodes {
        // -- 🧠 CPU: process → cpu → percent (the JVM's own CPU, not the container's system CPU)
        if let Some(process) = &node.process {
            if let Some(cpu) = &process.cpu {
                the_cpu_sum += cpu.percent as f64;
                the_cpu_count += 1;
            }
        }
        // -- 📦 JVM heap: jvm → mem → heap_used_percent
        if let Some(jvm) = &node.jvm {
            if let Some(mem) = &jvm.mem {
                the_jvm_heap_sum += mem.heap_used_percent as f64;
                the_jvm_count += 1;
            }
        }
    }

    Ok(ClusterSnapshot {
        cpu_percent: if the_cpu_count > 0 {
            the_cpu_sum / the_cpu_count as f64
        } else {
            // -- ⚠️ no CPU data? return 0 — the cluster is either dead or very zen
            0.0
        },
        jvm_heap_percent: if the_jvm_count > 0 {
            the_jvm_heap_sum / the_jvm_count as f64
        } else {
            // -- ⚠️ no JVM data? return 0 — maybe it's a Go cluster in disguise
            0.0
        },
    })
}

// =========================================================================
//  📦 Serde structs — parsing the firehose of JSON into the two numbers we want
// =========================================================================

/// 🔬 Top-level response from `_nodes/stats/process,jvm`.
/// ES/OS return approximately 47 billion fields per node. We want exactly four.
/// This struct is the bouncer at the JSON nightclub. 🚪
#[derive(Debug, Deserialize)]
struct NodeStatsResponse {
    // -- 📦 map of node_id → node stats — the keys are opaque hashes, the values are gold
    nodes: HashMap<String, NodeStats>,
}

/// 📦 Per-node stats — we only care about process and jvm subsections.
#[derive(Debug, Deserialize)]
struct NodeStats {
    // -- 🧠 Process-level stats (the ES/OS JVM process specifically, not the container OS)
    process: Option<NodeProcessStats>,
    // -- ☕ JVM stats (heap, GC, threads)
    jvm: Option<NodeJvmStats>,
}

/// 🏭 Process-level stats from a node (the ES/OS JVM process).
#[derive(Debug, Deserialize)]
struct NodeProcessStats {
    // -- 🧠 CPU subsection — the process's own CPU, not the system/container CPU
    cpu: Option<NodeProcessCpuStats>,
}

/// 🧠 CPU stats from the ES/OS process itself.
/// Can exceed 100% on multi-core (e.g. 200% = fully utilizing 2 cores). 💪
#[derive(Debug, Deserialize)]
struct NodeProcessCpuStats {
    // -- 📊 process CPU usage percent — 0 to N×100 where N = cores. 150% means 1.5 cores worth of work.
    percent: u64,
}

/// ☕ JVM stats from a node.
#[derive(Debug, Deserialize)]
struct NodeJvmStats {
    // -- 📦 memory subsection — heap is the metric that matters
    mem: Option<NodeJvmMemStats>,
}

/// 📦 JVM memory stats from a node.
#[derive(Debug, Deserialize)]
struct NodeJvmMemStats {
    // -- 📊 heap used percent — 0-100. When this hits 90, the GC starts panic-collecting
    // -- like a hoarder who just got a visit from the health department 🏠
    heap_used_percent: u64,
}

// =========================================================================
//  🧪 Tests — because untested code is just a suggestion
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 🧪 The one where cluster stats JSON parses CPU and JVM heap correctly.
    /// Three nodes, three opinions on CPU and memory. Democracy in action. 🗳️🦆
    #[test]
    fn the_one_where_cluster_stats_json_parses_cpu_and_jvm_heap() {
        let the_json = r#"{
            "nodes": {
                "node_1": {
                    "process": { "cpu": { "percent": 30 } },
                    "jvm": { "mem": { "heap_used_percent": 40 } }
                },
                "node_2": {
                    "process": { "cpu": { "percent": 60 } },
                    "jvm": { "mem": { "heap_used_percent": 70 } }
                },
                "node_3": {
                    "process": { "cpu": { "percent": 90 } },
                    "jvm": { "mem": { "heap_used_percent": 50 } }
                }
            }
        }"#;

        let the_stats: NodeStatsResponse = serde_json::from_str(the_json).unwrap();
        // -- 📊 manually compute what fetch_cluster_stats would return
        let mut cpu_sum = 0.0_f64;
        let mut cpu_count = 0_u64;
        let mut jvm_sum = 0.0_f64;
        let mut jvm_count = 0_u64;

        for (_id, node) in &the_stats.nodes {
            if let Some(process) = &node.process {
                if let Some(cpu) = &process.cpu {
                    cpu_sum += cpu.percent as f64;
                    cpu_count += 1;
                }
            }
            if let Some(jvm) = &node.jvm {
                if let Some(mem) = &jvm.mem {
                    jvm_sum += mem.heap_used_percent as f64;
                    jvm_count += 1;
                }
            }
        }

        let avg_cpu = cpu_sum / cpu_count as f64;
        let avg_jvm = jvm_sum / jvm_count as f64;

        // -- 🎯 (30+60+90)/3 = 60.0 CPU, (40+70+50)/3 ≈ 53.33 JVM
        assert_eq!(cpu_count, 3);
        assert_eq!(jvm_count, 3);
        assert!(
            (avg_cpu - 60.0).abs() < 0.01,
            "CPU avg should be 60.0, got {}",
            avg_cpu
        );
        assert!(
            (avg_jvm - 53.333).abs() < 0.01,
            "JVM avg should be ~53.33, got {}",
            avg_jvm
        );
    }

    /// 🧪 The one where missing JVM stats returns only CPU.
    /// Some nodes are shy about their JVM feelings. We respect that. 🫣🦆
    #[test]
    fn the_one_where_missing_jvm_stats_returns_only_cpu() {
        let the_json = r#"{
            "nodes": {
                "chatty_node": {
                    "process": { "cpu": { "percent": 42 } },
                    "jvm": { "mem": { "heap_used_percent": 55 } }
                },
                "shy_node": {
                    "process": { "cpu": { "percent": 58 } }
                }
            }
        }"#;

        let the_stats: NodeStatsResponse = serde_json::from_str(the_json).unwrap();
        let mut cpu_sum = 0.0_f64;
        let mut cpu_count = 0_u64;
        let mut jvm_sum = 0.0_f64;
        let mut jvm_count = 0_u64;

        for (_id, node) in &the_stats.nodes {
            if let Some(process) = &node.process {
                if let Some(cpu) = &process.cpu {
                    cpu_sum += cpu.percent as f64;
                    cpu_count += 1;
                }
            }
            if let Some(jvm) = &node.jvm {
                if let Some(mem) = &jvm.mem {
                    jvm_sum += mem.heap_used_percent as f64;
                    jvm_count += 1;
                }
            }
        }

        // -- 🎯 both nodes report CPU, only one reports JVM
        assert_eq!(cpu_count, 2);
        assert_eq!(jvm_count, 1);
        assert!((cpu_sum / cpu_count as f64 - 50.0).abs() < 0.01);
        assert!((jvm_sum / jvm_count as f64 - 55.0).abs() < 0.01);
    }

    /// 🧪 The one where empty nodes returns neutral values.
    /// An empty cluster is a philosophical question. Zero nodes, zero problems. 🧘🦆
    #[test]
    fn the_one_where_empty_nodes_returns_neutral_values() {
        let the_json = r#"{ "nodes": {} }"#;

        let the_stats: NodeStatsResponse = serde_json::from_str(the_json).unwrap();

        // -- 🎯 no nodes → counts stay at zero
        assert!(the_stats.nodes.is_empty());
        // fetch_cluster_stats would return ClusterSnapshot { cpu_percent: 0.0, jvm_heap_percent: 0.0 }
        // because both counts are 0 → fallback to 0.0
    }

    /// 🧪 The one where process CPU exceeds 100% because multi-core is a lifestyle.
    /// Two nodes hammering all their cores. process.cpu.percent goes brrr. 🔥🦆
    #[test]
    fn the_one_where_process_cpu_exceeds_100_on_multicore() {
        let the_json = r#"{
            "nodes": {
                "beefy_node": {
                    "process": { "cpu": { "percent": 350 } },
                    "jvm": { "mem": { "heap_used_percent": 60 } }
                },
                "modest_node": {
                    "process": { "cpu": { "percent": 150 } },
                    "jvm": { "mem": { "heap_used_percent": 40 } }
                }
            }
        }"#;

        let the_stats: NodeStatsResponse = serde_json::from_str(the_json).unwrap();
        let mut cpu_sum = 0.0_f64;
        let mut cpu_count = 0_u64;

        for (_id, node) in &the_stats.nodes {
            if let Some(process) = &node.process {
                if let Some(cpu) = &process.cpu {
                    cpu_sum += cpu.percent as f64;
                    cpu_count += 1;
                }
            }
        }

        let avg_cpu = cpu_sum / cpu_count as f64;

        // -- 🎯 (350+150)/2 = 250.0 — yes, process CPU can be > 100%. That's the whole point.
        assert_eq!(cpu_count, 2);
        assert!(
            (avg_cpu - 250.0).abs() < 0.01,
            "CPU avg should be 250.0, got {}",
            avg_cpu
        );
    }
}
