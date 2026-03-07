use anyhow::Result;
use async_trait::async_trait;

use crate::Page;
use crate::backends::Source;
use super::config::ElasticsearchSourceConfig;

/// 📦 The source side of the Elasticsearch backend.
///
/// This struct holds a config and currently does approximately
/// nothing useful in production because `next_batch` returns empty. 🐛
/// It is, however, a *very* well-intentioned nothing. The vibes are all correct.
/// The scaffolding is artisan-grade. The potential is immense. The implementation is... pending.
///
/// No cap, this will slap once scroll/search_after lands. We believe in it. We believe in you.
#[derive(Debug)]
pub struct ElasticsearchSource {
    #[allow(dead_code)]
    // -- 🔧 config kept for when next_batch finally stops ghosting us and actually scrolls.
    // -- Marked dead_code because rustc has opinions and no chill.
    config: ElasticsearchSourceConfig,
}

#[async_trait]
impl Source for ElasticsearchSource {
    /// 📡 Returns the next raw page from Elasticsearch.
    ///
    /// Currently returns `None` faster than you can say "scroll API."
    /// It's aspirational. It's a placeholder with excellent posture.
    /// The borrow checker is fully satisfied. The product manager is not.
    /// "He who stubs with None, deploys with hope." — Ancient scroll API proverb 📜
    async fn next_page(&mut self) -> Result<Option<Page>> {
        // TODO: Implement search_after — the glow-up we deserve. 🚀 🦆
        Ok(None)
    }
}

impl ElasticsearchSource {
    /// 🚀 Constructs a new `ElasticsearchSource`.
    ///
    /// Currently: allocates config and that's it. When search_after lands,
    /// this will do HTTP client setup, index validation, and maybe even a _count query
    /// so we can show real progress instead of existential uncertainty. 🦆
    ///
    /// "How much data?" "Yes." — Elasticsearch, every time.
    pub async fn new(config: ElasticsearchSourceConfig) -> Result<Self> {
        Ok(Self { config })
    }
}
