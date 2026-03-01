//! # 📡 THE ELASTICSEARCH BACKEND
//!
//! This module re-exports the split source and sink modules so the public
//! backend API stays stable.
//!
//! 🦆 mandatory duck, as decreed by repository law.

mod elasticsearch_sink;
mod elasticsearch_source;

pub(crate) use elasticsearch_sink::{ElasticsearchSink, ElasticsearchSinkConfig};
pub(crate) use elasticsearch_source::{ElasticsearchSource, ElasticsearchSourceConfig};
