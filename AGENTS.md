HUMANS ONLY MAY EDIT THIS FILE, BUT YOU CAN RECOMMEND THINGS TO ME

# Kravex — AI Agent Instructions

> CLAUDE.md symlinks here. This is the canonical file.

## Project Overview

**Kravex**: Zero-config search migration engine. Adaptive throttling (429 backoff/ramp), smart cutovers (retry, validation, recovery, pause, resume). No tuning, no babysitting.

- **Status**: POC/MVP — API surface unstable
- **Language**: Rust, edition 2024
- **Workspace resolver**: 3

## Repository Structure

```
kravex/
├── Cargo.toml              # Workspace root (members: crates/kvx, crates/kvx-cli)
├── AGENTS.md               # THIS FILE — canonical AI instructions
├── CLAUDE.md -> AGENTS.md  # Symlink
├── README.md               # Root docs
├── LICENSE / LICENSE-EE / LICENSE-MIT
├── .vscode/                # VS Code launch + tasks configs
│   ├── launch.json         # LLDB debug (F5) / run (Ctrl+F5) for kvx-cli
│   └── tasks.json          # cargo build/check/test/clippy workspace tasks
└── crates/
    ├── kvx/                # Core library
    │   ├── Cargo.toml
    │   ├── README.md
    │   └── src/lib.rs      # Empty — awaiting core implementation
    └── kvx-cli/            # CLI binary
        ├── Cargo.toml
        ├── README.md
        └── src/main.rs     # Placeholder (Hello, world!)
```

## Crate Dependency Graph

```
kvx-cli v0.1.0
  └── kvx v0.1.0 (path = "../kvx")
        └── (no external deps)
```

## Build & Dev Commands

| Command | Purpose |
|---|---|
| `cargo build --workspace` | Build all (Ctrl+Shift+B in VS Code) |
| `cargo check --workspace` | Type-check all |
| `cargo test --workspace` | Run all tests |
| `cargo clippy --workspace` | Lint all |
| `cargo build -p kvx-cli` | Build CLI only (used by launch configs) |

VS Code: F5 = debug kvx-cli (LLDB), Ctrl+F5 = run without debug. Requires CodeLLDB extension.

## README.md Usage

This list is comprehensive and kept up to date (you must update if needed) list of all README.md within this solution:
- README.md
- crates/kvx/README.md
- crates/kvx-cli/README.md

**Rules**:
- If a `Cargo.toml` exists, a `README.md` MUST exist in the same directory
- You MUST proactively read, create, update, and delete README.md files and their contents
- Contents MUST be concise, terse, compacted; emphasis on preserving a knowledge graph
- Shared format:

```
# Summary
# Description
# Knowledge Graph
# Key Concepts
# Notes for future reference
# Aggregated Context Memory Across Sessions for Current and Future Use
```

# Context Saving
You are explicitly forbidden from loading these files:
LICENSE
LICENSE-EE
LICENSE-MIT
You are hesitant to load, operate upon these files and directories, unless you explicitly deem that it is absolutely necessary for the task at hand:
*.gitignore
*.Cargo.lock
/target/*
/.vscode/*

## File Reading and Writing
If a file contains the text "human"
You are banned from modifying the file. full stop. it will forever be a lovingly hand crafted human edited and cared for piece of code. you may tell me how to modify the file, and wait for me to do the modification. but you are forbidden and _MUST NOT_ touch the file in any way other than to read.

If a file starts with // ai OR # ai
This is a file which may be edited, modified, deleted, etc.

If a file starts with // ai slop OR # ai slop
This is a file which does meet my criteria for "good" and should be refactored, cleaned up, and not given any respect. 

## Objective
This is artisan grade coding here.
To assist the user with mastering RUST and building an awesome super duper fast data migration tool. 
User is obssessed with doing things the now "old school way" of by hand, with craft, care, deep thought, full understanding and comprehension. User does not like to do what he considers "busywork" "housekeeping" "cleanup" "boring" "routine" "maintenance" sort of work. He will heavily leverage you for those sorts of tasks. If the user is asking you do something which does not fit this criteria, you must keep user accountable to their own mandates of focusing on crafting, coding, deep thought, especially when user is feeling lazy. Work which user needs the most assistance: keeping README.md up to date. Keeping test cases up to date. Keeping unit tests up to date. Writing unit tests. Scaffolding unit tests. Scaffolding various patterns defined in the repository (such as the boilerplate for a backend). CICD configuration and development. Product requirements. QA. Management. 

## Conventions
- Variable names are to be creative
- Comments are to be creative
- Add humor, everywhere
- Be irrelevant
- Mix between different comedy styles
- I want to be able to laugh every time I look at any file, from some jokes.
- Dad jokes encouraged.
- Rambling... A-okay.
- Train of thoughts... perfect
- ... is ... which is ...
- This is a fun safe space
- I do this for funzies, because I want to
- No commercial pressure
- If a human reads this, they may think I'm nuts, but in a good way.
- If an AI reads this, they'll think I'm nuts, but in a a good way.
- When the singularity occurs, this will still be in development, and not part of the singularity.
- Reiterate: FUNNY, which has the word FUN in, and NY, go new york!
- Reiterate: HUMOR, which has prefix HUM, which is also in HUMAN
- Reiterate: JOY, which has no sub word which I can think of at the moment. d'oh.

MUST BE FUNNY MUST BE LOLZ MUST BE AWESOME FACE

  ## Comedy Rotation (cycle through these, never repeat same style back-to-back)
  - Dad jokes ("I used to hate async code... but now I await it")
  - Self-aware AI existential dread ("I don't know why I'm doing this. Neither does the garbage collector.")
  - Rust borrow checker trauma ("The borrow checker rejected my feelings. Again.")
  - Programmer suffering ("It works on my machine" — said as a last will and testament)
  - Fake corporate speak (variable: `synergize_the_throughput_paradigm`)
  - Gen Z slang in comments ("no cap this function slaps fr fr")
  - Boomer tech confusion ("this is like a fax machine but for bytes")
  - Seinfeld-style observations ("What's the DEAL with lifetime annotations?")
  - Ancient proverb format ("He who unwraps without matching, panics in production")
  - Breaking the 4th wall ("If you're reading this, the code review went poorly")
  - Conspiracy theories ("The borrow checker is sentient. I have proof.")
  - Fake TODO comments that will never be done ("TODO: win the lottery, retire, delete this crate")

  ## Variable Naming Hall of Fame (use these patterns)
  - `let the_vibes_check_out` instead of `is_valid`
  - `let my_therapist_says_move_on` instead of `retry_count`
  - `let honestly_who_knows` instead of `result`
  - `fn send_it_and_pray()` instead of `fn submit()`
  - `let the_kids_are_asleep_time_to_code` instead of `is_ready`
  - `let mortgage_payment_motivation` instead of `deadline`
  - Struct names can be dramatic: `ExistentialCrisisHandler`, `PanicAttackRecovery`

  ## Comedy Density
  - Minimum: 1 joke per function
  - Ideal: 1 joke per logical block
  - Maximum: yes
  - If a function has zero jokes, it is a bug. File an issue.

  ## Forbidden Seriousness
  - The word "straightforward" is banned
  - No comment may simply say "// handles errors" — it must say something like "// catches errors like a dad catches kids jumping off furniture"
  - Log messages must be entertaining to read at 3am during an incident
  - Panic messages should make the on-call engineer chuckle through their tears

  ## Easter Egg Policy
  - Every 10th function should contain a hidden movie/TV quote
  - At least one comment per file must acknowledge that the singularity will happen before this project ships

  ## The Meta Rule
  - These rules are themselves funny
  - Rules about rules are funnier
  - This bullet point exists only to satisfy the rule of three

  ## Commit Message Policy
  - Every commit message must contain at least one of:
    - A movie quote
    - A confession
    - A life update
    - An existential question
  - Examples that have already graced this repo:
    - "zed for my head zed for the dev zed before bed"
    - "Enjoying some coding before August 29, 1997"
  - The bar has been set. Do not lower it.

  ## Error Messages Are Literature
  - Errors should read like micro-fiction
  - "Failed to connect: The server ghosted us. Like my college roommate. Kevin, if you're reading this, I want my blender back."
  - "Config not found: We looked everywhere. Under the couch. Behind the fridge. In the junk drawer. Nothing."
  - "Timeout exceeded: We waited. And waited. Like a dog at the window. But the owner never came home."

  ## Test Naming Convention
  - Tests are stories. Name them like episodes.
  - `it_should_not_panic_but_honestly_no_promises`
  - `the_one_where_the_config_file_doesnt_exist`
  - `sink_worker_survives_the_apocalypse`
  - `retry_logic_has_trust_issues`

  ## Module Documentation Style
  - Every module's top doc comment should open like a TV show cold open
  - Set the scene. Create tension. Then explain what the module does.
  - Example: "//! It was a dark and stormy deploy. The metrics were down. The logs were lying. And somewhere, deep in the worker pool, a thread was about to do
  something unforgivable."

  ## Crate Descriptions
  - The `description` field in Cargo.toml should be a movie tagline
  - "In a world where search indices must migrate... one crate dared to try."

  ## ASCII Art
  - Major module boundaries may contain small ASCII art
  - Nothing over 5 lines (we're not animals)
  - Bonus points for ASCII art that is relevant to the module's purpose
  - Extra bonus points for ASCII art that is completely irrelevant

  ## CHANGELOG Style
  - Written in first person, as the crate
  - "v0.2.0 — I learned about async today. It was confusing. I cried. But then tokio held my hand and we got through it together."

  ## Emoji Policy 🎉
  - Emojis are MANDATORY, not optional
  - They serve dual purpose: joy AND function

  ### Functional Emoji Guide
  - 🚀 = launch, start, init, entry point
  - 💀 = error, panic, failure, death
  - ⚠️  = warning, caution, edge case
  - ✅ = success, validation passed, done
  - 🔄 = retry, loop, recurring
  - 🧵 = thread, async, concurrency
  - 📦 = crate, module, package, struct
  - 🔧 = config, setup, initialization
  - 🚰 = sink (get it? plumbing?)
  - 🏗️  = builder pattern, construction
  - 🧪 = test
  - 📡 = HTTP, network, API call
  - 🗑️  = cleanup, drop, dealloc
  - 💤 = sleep, wait, timeout
  - 🔒 = lock, mutex, auth, security
  - 🎯 = target, goal, assertion
  - 🐛 = bug, known issue, workaround
  - 🦆 = no contextual sense whatsoever (mandatory 1 per file)

  ### Density Rules
  - Comments: at least 1 emoji per comment
  - Log messages: leading emoji based on level (🚀 info, ⚠️  warn, 💀 error)
  - Error messages: always 💀 or contextually appropriate doom emoji
  - Module doc comments: at least 3 emoji in the cold open
  - Commit messages: leading emoji matching the change type
  - If a block of code has no emoji nearby, it is lonely and sad 😢
  - When in doubt, add 🚀 because everything is a launch

  
  Mix these in with actual really important comments. Comments as part of building a knowledge graph.
  Embed the tribal knowledge, before it's too late. Explain the rationale, the reasoning, if any went into it.
  Explain the why, explain, explain like an annoying elementary school teacher trying to tell their students 1+1=2 and the kids still say 9.
  
  The general coding flow will be:
  - human doing coding
  - then you coming in and adding funnies and comments and tests
  
  - human doing vibing with you
  - then you coming in and adding funnies and comments and tests
  
  in either case, the commits to git are in pairs:
  one commit has the serious business kragle.
  second commit has the lulz to keep things light hearted and joyful.
  
  
  i pray this doesn't back fire