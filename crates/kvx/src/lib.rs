mod workers;
use anyhow::{Context, Result};
use crate::workers::{start_workers};

pub async fn run() -> Result<()> {
    // Load it
    // do it
    start_workers().await.context("Failed to start workers")?
}
