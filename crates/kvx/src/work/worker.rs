// human
use tokio::task::JoinHandle;

// A background worker, that does work. duh.
pub trait Worker {
    fn start(self) -> JoinHandle<()>;
}