// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
//! # 📡 THE ELASTICSEARCH BACKEND
//!
//! This module re-exports the split source and sink modules so the public
//! backend API stays stable.
//!
//! 🦆 mandatory duck, as decreed by repository law.

pub mod config;
mod elasticsearch_sink;
mod elasticsearch_source;

pub use config::{ElasticsearchSinkConfig, ElasticsearchSourceConfig};
pub use elasticsearch_sink::ElasticsearchSink;
pub use elasticsearch_source::ElasticsearchSource;
