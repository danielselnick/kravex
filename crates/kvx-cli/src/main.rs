//! 🚀 kvx-cli — the front door, the bouncer, the maitre d' of kravex.
//!
//! 🎬 *[narrator voice]* "It all started with a simple main() function..."
//! 📦 This binary crate is the thin CLI wrapper that loads config,
//! sets up logging, and then lets the real code do the heavy lifting.
//! Like a manager. 🦆

#![allow(dead_code, unused_variables, unused_imports)]
use anyhow::{Context, Ok, Result};
use tracing::error;
use tracing_subscriber::EnvFilter;

/// 🚀 main() — where it all begins. The genesis. The big bang.
/// The "I pressed F5 and held my breath" moment.
///
/// 🔧 Steps:
/// 1. Init tracing (so we can see what goes wrong, and when)
/// 2. Parse args (or don't, we're not picky)
/// 3. Load config (the moment of truth)
/// 4. Run the thing (send it and pray 🙏)
/// 5. Handle errors (cry)
#[tokio::main]
async fn main() -> Result<()> {
    // 📡 Set up tracing — because println! debugging is a lifestyle choice
    // we're trying to move past, like flip phones and cargo shorts
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 🎯 Grab the args like catching Pokémon — gotta get at least 1
    let args: Vec<String> = std::env::args().collect();
    let path_arg = match args.get(1) {
        Some(s) => s,
        None => &format!("kvx.toml"), // 🔧 default: the ol' reliable
    };

    // 🔒 Validate the config file exists before we get too emotionally attached
    let config_file = std::path::Path::new(path_arg);
    let config_file_path_which_is_validated_to_exist = match config_file.try_exists()
        .context(format!("💀 Configuration file may not exist, couldn't find it. Double check that it exists, or maybe, it's an issue with pwd/cwd and relative paths. In that case, use an absolute path, to be absolutely certain, you are not messing this up. Was checking here: '{}'", config_file.display()))
    /* ? */ ? // ⚠️ Unwrap this, maybe — like unwrapping a gift that might be socks
    {
        true => Some(config_file),  // ✅ Found it! Better than finding my car keys
        false => None               // 💤 Not there. Like my motivation on Mondays.
    };

    // 🔧 Load the config — this is the moment where we find out if the TOML is valid
    // or if someone put a tab where a space should be (looking at you, Kevin)
    let app_config  = kvx::app_config::load_config(config_file_path_which_is_validated_to_exist)
        .context("💀 In kvx-cli, main, we couldn't load the config file, take a look at the file, make sure it's correct. Make sure you didn't forget something obvious, dumas")
    /* ? */ ?;

    // 🚀 SEND IT. No take-backs. This is not a drill.
    // (okay it might be a drill, we're still in POC/MVP)
    let result = kvx::run(app_config).await;

    // 💀 Error handling: the part where we find out what went wrong
    // and print it in a way that's helpful at 3am
    if let Err(err) = result {
        error!("💀 error: {}", err);
        // -- 🧅 peel the onion of sadness, one tear-jerking layer at a time
        let mut the_vibes_are_giving_connection_issues = false;
        for cause in err.chain().skip(1) {
            error!("⚠️  cause: {}", cause);
            // -- 🕵️ sniff the cause like a truffle pig hunting for connection problems
            let cause_str = cause.to_string();
            if cause_str.contains("error sending request")
                || cause_str.contains("connection refused")
                || cause_str.contains("Connection refused")
                || cause_str.contains("tcp connect error")
                || cause_str.contains("dns error")
            {
                the_vibes_are_giving_connection_issues = true;
            }
        }

        // -- 📡 if it smells like a connection problem, it's probably a connection problem
        // -- like when your wifi icon has full bars but nothing loads
        if the_vibes_are_giving_connection_issues {
            error!(
                "🔧 hint: looks like a service isn't reachable. \
                Double-check that the backing service (Elasticsearch, database, etc.) \
                is actually running. If you're using Docker, try: \
                `docker ps` to see what's up, or `docker compose up -d` to resurrect it. \
                Even servers need a nudge sometimes. ☕"
            );
        }

        // 🗑️ Exit with prejudice. Process exitus maximus.
        std::process::exit(1);
    }

    // ✅ If we got here, everything worked. Pop the champagne. 🍾
    // (or at least close the terminal tab with a sense of accomplishment)
    Ok(())
}
