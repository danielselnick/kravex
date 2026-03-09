//! # 🔍 THE MEILISEARCH BACKEND
//!
//! 🎬 COLD OPEN — INT. SEARCH ENGINE STARTUP — 3 AM
//! *[A JSON array payload arrives at the Meilisearch gate.]*
//! *["I'm here for the indexing?" it asks, clutching its documents.]*
//! *["202 Accepted," says the gate. "Take a number. We'll call you."]*
//! *[The payload sits. And waits. And polls. And waits some more.]*
//!
//! This module re-exports the sink-only Meilisearch backend.
//! No source (yet). Meilisearch is a write-mostly destination for now.
//!
//! 🦆 The duck doesn't search. The duck finds.

pub mod config;
mod meilisearch_sink;

pub use config::MeilisearchSinkConfig;
pub use meilisearch_sink::MeilisearchSink;
