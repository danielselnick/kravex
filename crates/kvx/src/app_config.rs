use anyhow::Context;
use serde::Deserialize;
// To load the configuration, so I don't have to manually parse environment variables or files. Bleh.
use figment::{Figment, providers::{Env, Format, Toml}};
#[derive(Debug, Deserialize)]
// Generic/common/core configuration values, applicable to any type of SOURCE: Elasticserach, OpensSearch, Mongo, your mom.
pub struct SourceConfig {
    
}

fn default_num_sink_workers() -> usize { 4 }

// Core/generic/common values, which can be used to all the SINKS: Elasticsearch, OpenSearch, Mongo, MielieSearch, boats (money sinks, get it?)
pub struct SinkConfig {
    #[serde(default = "default_num_sink_workers")]
    num_sink_workers: usize
}

pub struct ChannelConfig {
    
}

pub struct AppConfig {
    pub source_config: SourceConfig,
    pub sink_config: SinkConfig,
    pub channel_size: usize,
}