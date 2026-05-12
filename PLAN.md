# 🗑️ Code Cleanup Plan — "Taking Out the Trash"

## Context

The codebase has undergone significant iteration — renames (`Page → Draft`, `FlowMaster → Governor`, `kvx → kvx-lib`), removed features (Meilisearch, OpenObserve, cpu_pressure gauge/regulator), and structural changes (configs moved to co-located backend files). The result is accumulated dead code, orphaned modules, unused constants, suppressed warnings, and stale comments that no longer match reality. Time to clean house.

## Key Observations

1. **`#![allow(dead_code, unused_variables, unused_imports)]`** — These crate-level allows suppress ALL dead/unused warnings across both `lib.rs` and `kvx-cli/src/main.rs`. They were needed during rapid iteration but are now masking real problems.

2. **Numerous renames left debris** — Old names and comments referencing `Page`, `FlowMaster`, `kvx` (pre-rename), `cpu_pressure`, `regulator` (old field name in config), `Drainer` doing CPU work (now in `Refiner`).

3. **Orphaned/unused code** — Constants defined but never referenced, `pub` visibility on internal types, unused functions, redundant `#[serde(default)]` patterns.

4. **Massive comment volume** — Every file has lengthy AI-generated movie-script doc comments. While entertaining, they significantly bloat the source and make real documentation harder to find.

## Approach

Work crate-by-crate, module-by-module, in dependency order. Each step removes the `#![allow(...)]` from that specific scope, builds, fixes warnings by removing or `#[allow]`-tagging specific items (only after determining they're truly dead), and commits. This is safer than a single big-bang removal because each compile failure reveals exactly one problem at a time.

## Files to Modify

### Primary targets (dead code removal):

| File | What to Clean |
|------|---------------|
| `crates/kvx-lib/src/lib.rs` | Remove `#![allow(dead_code, unused_variables, unused_imports)]` — this is the main gate. Also remove `stop()` (empty no-op function) |
| `crates/kvx-cli/src/main.rs` | Remove `#![allow(dead_code, unused_variables, unused_imports)]` |
| `crates/kvx-lib/src/taps/pit_to_bulk.rs` | Remove unused constants `_HIT_ID_FIELD`, `_HIT_INDEX_FIELD`, `_HIT_ROUTING_FIELD` |
| `crates/kvx-lib/src/backends/file/file_sink.rs` | Remove unused `_sink_config` field (stored but never read after construction) |
| `crates/kvx-lib/src/regulators/pid_controller.rs` | Remove unused `_HIT_ROUTING_FIELD` reference if exists |
| `crates/kvx-lib/src/victory_laps.rs` | Consider trimming excessive victory messages (50+ scrolls is fun but adds build time/complexity for test verification) |
| `crates/kvx-lib/src/progress/mod.rs` | Remove unused import `tokio::task::JoinHandle` if present |
| `crates/kvx-lib/src/progress/cluster_stats.rs` | Remove unused import `std::sync::atomic::AtomicUsize` if present |
| `crates/kvx-lib/src/backends/file/file_source.rs` | Remove unused `total_bytes_from_file` variable (assigned but never used outside tracing) |

### Config/schema cleanup:

| File | What to Clean |
|------|---------------|
| `crates/kvx-lib/src/regulators/config.rs` | `default_min_request_size_bytes()` and `default_initial_output_bytes()` — check if they're used or duplicated elsewhere |
| `crates/kvx-lib/src/backends/config.rs` | `default_max_barrel_size_docs()` and `default_max_barrel_size_bytes()` — serde default vs `Default` trait have different values. Doc says this is intentional but should be reconciled or documented with a clear decision |
| `crates/kvx-lib/src/backends/config.rs` | `default_max_drum_size_bytes()` — same dueling-defaults issue |

### Stale comments to update/remove:

Many comments reference old architecture, wrong module locations, or are just movie scripts. Specific high-value targets:

- `lib.rs` — comments mention "unimplemented mock mapping for now" — pipeline is working
- `backends/file/file_source.rs` — "we rolled our own buffering" internals changed
- `backends/sink.rs` — says "Drainer buffers" but Drainer no longer buffers
- `taps/mod.rs` — says "Barrels go in, Drafts come out" but concept has been renamed
- `workers/mod.rs` — "stop logic yet to be written" is stale (refers to `stop()`)
- All README.md files in subdirectories — check if still accurate

**Note**: Comments are lower priority than dead code. We clean obvious stale comments as we touch files for dead code removal, but don't do a standalone comment-edit pass.

### Documentation files:

| File | Action |
|------|--------|
| Multiple `README.md` files in `crates/kvx-lib/src/` subdirectories | Brief review for accuracy |
| `PLAN.md` in repo root | Write new plan over this file (done) |

## Reuse

No existing functions or utilities need to be reused for this cleanup. This is deletion work.

## Steps

### Phase 1: Prepare (safe structural cleanup)

- [ ] 1.1 — Build the project with `cargo build` and `cargo test` to establish a known-good baseline.
- [ ] 1.2 — Run `cargo clippy` to see current warnings (likely zero due to `#![allow]`).
- [ ] 1.3 — Run `cargo +nightly udeps` or `cargo-unused-features` if available to detect unused workspace deps (optional).

### Phase 2: Remove crate-level `#![allow]` from `kvx-cli`

- [ ] 2.1 — In `crates/kvx-cli/src/main.rs`, remove `#![allow(dead_code, unused_variables, unused_imports)]`.
- [ ] 2.2 — `cargo build` in the CLI crate. Fix any warnings (likely none since main.rs is thin).
- [ ] 2.3 — Commit.

### Phase 3: Remove crate-level `#![allow]` from `kvx-lib`

- [ ] 3.1 — In `crates/kvx-lib/src/lib.rs`, remove `#![allow(dead_code, unused_variables, unused_imports)]`.
- [ ] 3.2 — `cargo build`. The build will fail on the first dead/unused item. Fix it.
  - **Dead items policy**: For each warning:
    - If trivially unused (constants, variables, imports) → delete.
    - If private function/struct that is clearly never called → delete.
    - If `pub` function/struct that is unused within the crate but part of the intended public API → add `#[allow(dead_code)]` on just that item, with a brief comment.
    - If a variant/enum is unused → delete unless it's part of a planned feature (add `#[allow]` with comment).
  - **`stop()` function**: This is a public no-op function in `lib.rs`. The comment admits it does nothing. If no caller exists outside the crate, remove it. If it's part of the intended public API, keep it with a `#[allow(dead_code)]`.
- [ ] 3.3 — Iterate until clean build. Commit.

### Phase 4: Specific cleanup targets

- [ ] 4.1 — Remove unused constants from `pit_to_bulk.rs` (`_HIT_ID_FIELD`, `_HIT_INDEX_FIELD`, `_HIT_ROUTING_FIELD`).
- [ ] 4.2 — Remove unused `_sink_config` field from `FileSink` struct (stored but never read after `new()`).
- [ ] 4.3 — Remove unused `total_bytes_from_file` variable in `file_source.rs` `pump()` (assigned but only used in a trace that doesn't use it).
- [ ] 4.4 — Remove `GaugeReading::Error()` variant if unused (depends on whether ThroughputSeeker actually uses it — yes it does, keep it).
- [ ] 4.5 — Audit `pub` visibility on internal types and reduce to `pub(crate)` where possible (reducing public API surface to intentional boundaries).
- [ ] 4.6 — Commit.

### Phase 5: Config default reconciliation

- [ ] 5.1 — `CommonSourceConfig` has different serde defaults (10k docs/10MB) vs `Default` trait (1k docs/1MB). Decide which is canonical and eliminate the duplication.
  - **Recommendation**: Make serde defaults match the `Default` trait (1k/1MB) — these are safer and more conservative. Document the discrepancy and pick one.
- [ ] 5.2 — `CommonSinkConfig` has serde default 10MB vs `Default` impl 64MB. Same issue. Pick one (recommend: keep `Default` = 64MB, serde default = 64MB).
- [ ] 5.3 — Commit.

### Phase 6: Verify

- [ ] 6.1 — `cargo build` — clean build, zero warnings.
- [ ] 6.2 — `cargo test --workspace` — all tests pass.
- [ ] 6.3 — `cargo clippy --all-targets` — clean.
- [ ] 6.4 — Run the integration tests (the ones using `wiremock` for ES sink/source).

### Phase 7: Optional stretch goals

- [ ] 7.1 — Trim `victory_laps.rs` down to ~20 messages (keep the best ones, remove the padding).
- [ ] 7.2 — Remove stale `README.md` files inside `src/` subdirectories that just duplicate module-level doc comments.
- [ ] 7.3 — Run `cargo +nightly udeps` for unused dependency detection.

## Verification

1. **`cargo build --workspace`** — must compile with zero warnings and zero errors.
2. **`cargo test --workspace`** — all 140+ tests must pass.
3. **`cargo clippy --all-targets`** — clean output.
4. **Manual check**: The two `#[tokio::test]` integration tests in `lib.rs` and the ES sink/source tests use `wiremock` and exercise the full pipeline — they must pass.
5. **No `#![allow(dead_code)]` at crate level should remain** — any remaining dead code suppression must be item-level with a justification comment.