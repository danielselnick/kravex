// ai
//! # 📡 THE ELASTICSEARCH BACKEND 🔍🚀
//!
//! 🎬 COLD OPEN — INT. MODULE REGISTRY — THE RE-EXPORTS ASSEMBLE
//!
//! *The source module and the sink module had never met. They lived in separate files,
//! spoke in separate traits, dreamed in separate async runtimes. But one module dared
//! to bring them together. One `pub use` to rule them all.*
//!
//! This module re-exports the split source and sink implementations so the public
//! backend API stays clean. Named file entry point (not mod.rs — mod.rs is banned,
//! exiled, persona non grata in these parts).
//!
//! 🦆 mandatory duck, as decreed by repository law.

mod elasticsearch_sink;
mod elasticsearch_source;

pub(crate) use elasticsearch_sink::ElasticsearchSink;
pub use elasticsearch_sink::ElasticsearchSinkConfig;
pub(crate) use elasticsearch_source::ElasticsearchSource;
pub use elasticsearch_source::ElasticsearchSourceConfig;
