
// This is pretty much across the whole world of kravex
// anyhowwwww.... it's useful!
use anyhow::Result;
use tokio::task::JoinHandle;

mod sink_worker;
use sink_worker::SinkWorker;
mod source_worker;


pub fn start_workers() -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let _sink_worker = SinkWorker::new();
        
        Ok(())
    })
}

// human

// A background worker, that does work. duh.
pub trait Worker {
    fn start(self) -> JoinHandle<Result<()>>;
}