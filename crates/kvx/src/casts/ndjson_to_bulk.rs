// ai
// 🧠 The lines of NDJSON are raw json docs — they have no bulk action metadata.
// 📡 This caster adds the ES bulk index action line before each doc.
use anyhow::Result;

use crate::casts::Caster;
const THE_BULK_ACTION_LINE: &str = "{\"index\":{}}\n";

/// 📡 Casts raw NDJSON docs into ES bulk format (action line + source doc).
/// Like a bouncer at a club — "you can't come in without your action line, buddy." 🦆
#[derive(Debug, Clone, Copy)]
pub struct NdJsonToBulk {}

impl Caster for NdJsonToBulk {
    #[inline]
    fn cast(&self, feed: &str) -> Result<String> {
        // 📄 Split feed by newlines, cast each non-empty line into bulk format.
        // 🧠 Each line becomes: action_line\n{json_document}
        // -- "He who casts without an action line, gets a 400 from Elasticsearch." 💀
        // TODO: actually implement the bulk action line generation
        // -- for now, pass through like a speed bump that forgot to bump 🦆
        let mut result = String::with_capacity(feed.len() + feed.len() / 2);
        for line in feed.split('\n') {
            if !line.is_empty() {
                result.push_str(THE_BULK_ACTION_LINE);
                result.push_str(line);
                result.push('\n');
            }
        }
        Ok(result)
    }
}
