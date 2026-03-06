// ai
// 🧠 The lines of NDJSON are raw json docs — they have no bulk action metadata.
// 📡 This caster adds the ES bulk index action line before each doc.
use anyhow::Result;

use crate::casts::Caster;

/// 📡 Casts raw NDJSON docs into ES bulk format (action line + source doc).
/// Like a bouncer at a club — "you can't come in without your action line, buddy." 🦆
#[derive(Debug, Clone, Copy)]
pub struct NdJsonToBulk {}

impl Caster for NdJsonToBulk {
    #[inline]
    fn cast(&self, feed: &str) -> Result<String> {
        
    }
}
