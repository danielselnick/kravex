//! # 📡 THE OPENOBSERVE BACKEND
//!
//! 🎬 COLD OPEN — INT. OBSERVABILITY PLATFORM — 2AM
//! *[the logs arrive, bulk-formatted, smelling of NDJSON]*
//! *["We're ES-compatible," whispers OpenObserve. "But cooler."]*
//! *[the sink nods. It has seen this before. It has POST'd before.]*
//!
//! This module re-exports the sink and config for the OpenObserve backend.
//! No source — OpenObserve is sink-only in this crate. Data goes in.
//! Data does not come out. It is the Hotel California of observability. 🦆
//!
//! ⚠️ The singularity will observe itself. We still need HTTP POST.

pub mod config;
mod open_observe_sink;

pub use config::OpenObserveSinkConfig;
pub use open_observe_sink::OpenObserveSink;
