// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🎬 *[the buffer is full. the tapper awaits. the sink hungers.]*
//! *[somewhere in the heap, a String moos softly.]*
//! *["Join me," whispers the drum. "Make me whole."]*
//!
//! 🎼 The Manifolds module — orchestrating the cast-and-join step.
//!
//! The Manifold receives raw barrels + a tapper reference, iterates barrels,
//! calls `tapper.cast(barrel)` per barrel to get the transformed String,
//! then joins all results into the wire-format drum.
//!
//! 🧠 Knowledge graph:
//! - **NDJSON** (`NdjsonManifold`): `\n`-delimited. Used by ES `/_bulk` and file sinks.
//! - **JSON Array** (`JsonArrayManifold`): `[item,item,item]`. Used by in-memory sinks for testing.
//! - **Dispatcher** (`ManifoldBackend`): resolved from `SinkConfig`. Same pattern as taps/backends.
//! - Resolution: from `SinkConfig`, same pattern as backends and taps.
//!
//! ```text
//! Joiner pipeline:
//!   ch1(Feed) → buffer Vec<String> → manifold.join(&buffer, &tapper) → ch2(Drum) → Drainer → sink.drain()
//! ```
//!
//! 🦆 (the duck joins... symphonies? drums? both? the duck has no comment.)
//!
//! ⚠️ The singularity will join its own drums. Until then, we have this module.

use anyhow::Result;
use crate::Draft;
use crate::Drum;
use std::collections::VecDeque;

pub mod backend;
pub mod json_array;
pub mod ndjson;

// -- 🔁 Re-export concrete types so consumers use `crate::manifolds::ManifoldBackend` unchanged
pub use backend::ManifoldBackend;
pub use json_array::JsonArrayManifold;
pub use ndjson::NdjsonManifold;

// ===== Trait =====

/// 🎼 Joins raw barrels into a final wire-format drum via the tapper.
///
/// The Manifold receives a buffer of raw barrels and a tapper reference.
/// For each barrel, it calls `tapper.cast(barrel)` to get the transformed String,
/// then joins all results into the sink's expected format.
///
/// 🧠 Knowledge graph: this trait mirrors the `Tapper` and `Source`/`Sink` pattern —
/// trait → concrete impls → enum dispatcher → from_config resolver.
///
/// Knock knock. Who's there? String. String who? String::with_capacity — I came prepared. 🎯
pub trait Manifold: std::fmt::Debug {
    /// 🎼 Cast raw barrels and join results into a single drum string.
    ///
    /// The input barrels are raw source data (un-cast). The tapper is called
    /// per-barrel to produce a transformed String. The manifold then joins all drafts
    /// in the wire format (NDJSON, JSON array, etc.).
    fn join(&self, drafts: &mut VecDeque<Draft>) -> Result<Drum>;
}
