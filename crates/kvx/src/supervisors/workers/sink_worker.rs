// human
use tokio::task::JoinHandle;
use anyhow::Result;
use crate::supervisors::workers::Worker;
use crate::supervisors::config::SinkWorkerConfig;
pub(in crate::supervisors) struct SinkWorker {
    // ALSO TODO 3 mins ago
    config : SinkWorkerConfig,
}

impl SinkWorker {
    pub(in crate::supervisors) fn new(config: SinkWorkerConfig) -> Self {
        SinkWorker { config }
    }
}

impl Worker for SinkWorker {
    fn start(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            Ok(())
        })
    }
}