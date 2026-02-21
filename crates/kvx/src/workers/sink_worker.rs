// human
use tokio::task::JoinHandle;
use anyhow::Result;
use crate::workers::Worker;
pub struct SinkWorker {
    // ALSO TODO 3 mins ago
}

impl SinkWorker {
    pub fn new() -> Self {
        SinkWorker { }
    }
}

impl Worker for SinkWorker {
    fn start(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            Ok(())
        })
    }
}