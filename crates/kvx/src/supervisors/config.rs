// ai
//! 🏚️ *[the config types have left the building. forwarding address: `app_config`.]*
//! *[this file is now a lobby with a sign that says "they moved".]*
//! *[the borrow checker accepts the re-exports. the module system is at peace.]*
//!
//! 🔄 **Compatibility shim** — `crate::supervisors::config::*` now re-exports from their true homes.
//!
//! 🧠 Knowledge graph:
//! - `RuntimeConfig`, `SourceConfig`, `SinkConfig` → moved to `crate::app_config`
//! - `CommonSinkConfig`, `CommonSourceConfig` → moved to `crate::backends::common_config`
//! - This shim exists for `file_source.rs` which contains the literal string "human" and
//!   therefore cannot be modified per CLAUDE.md law. It will be deleted once that file is updated.
//!
//! ⚠️ TODO (human must action): Update `backends/file/file_source.rs` line 12:
//!   OLD: `use crate::supervisors::config::{CommonSinkConfig, CommonSourceConfig};`
//!   NEW: `use crate::backends::{CommonSinkConfig, CommonSourceConfig};`
//!   Then delete this file and remove `pub mod config;` from `supervisors.rs`.
//!
//! "He who re-exports instead of deletes, ships faster but refactors twice."
//!   — Ancient Rust proverb, written on a sticky note no one will remove 🦆

// 🔄 Application-level configs — now citizens of app_config
pub use crate::app_config::{RuntimeConfig, SinkConfig, SourceConfig};

// 🔄 Backend-primitive configs — now citizens of backends::common_config
// pub(crate) here because CommonSinkConfig/CommonSourceConfig are pub(crate) in backends 🔒
pub(crate) use crate::backends::{CommonSinkConfig, CommonSourceConfig};
