use anyhow::{Context, Ok, Result};
use tracing::error;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let path_arg = match args.get(1) {
        Some(s) => s,
        None => &format!("kvx.toml")
    };
    
    let config_file = std::path::Path::new(path_arg);
    let config_file_path_which_is_validated_to_exist = match config_file.try_exists()
        .context(format!("Configuration file may not exist, couldn't find it. Double check that it exists, or maybe, it's an issue with pwd/cwd and relative paths. In that case, use an absolute path, to be absolutely certain, you are not messing this up. Was checking here: '{}'", config_file.display()))
    /* ? */ ? // Unwrap this, maybe
    {
        true => Some(config_file),
        false => None 
    };
    
    let app_config  = kvx::load_config(config_file_path_which_is_validated_to_exist)
        .context("In kvx-cli, main, we couldn't load the config file, take a look at the file, make sure it's correct. Make sure you didn't forget something obvious, dumas")
    /* ? */ ?;
    
    let result = kvx::run().await;
    
    if let Err(err) =  result {
        error!("error: {:#}", err);
        for cause in err.chain().skip(1) {
            error!("cause: {:#}", cause);
        }
        
        std::process::exit(1);
    }
    
    Ok(())
}
