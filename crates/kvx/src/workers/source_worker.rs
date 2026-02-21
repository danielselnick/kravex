// human
use tokio::task::JoinHandle;
use anyhow::Result;
//
// crate:: meaning it's a crate in this scope
use crate
// workers:: meaning there's a module with this name
//    the module can be a mod.rs in a folder, or the folder_name.rs
//    for this project, I'm using folder_name with a folder_name.rs file for exports/pub mods
    ::workers
// worker: meaning the module in the module? it's the file in the module
    ::Worker;
// Worker:: meaning the _TYPE_ in this module. in this case, it's a type in the file worker.rs

pub struct SourceWorker {
    // TODO LIKE 2 mins ago
}

impl Worker for SourceWorker {
    fn start(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            println!("Hello worker!");
            Ok(())
        })
    }
}
