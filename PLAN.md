# Plan: Complete Pipeline Terminology Rename → Unified Refinery Motif

## Context

The codebase underwent a partial refactor to rename pipeline components according to a
consistent plumbing/refinery mental model. Several old terms remain in the codebase and
need to be updated to fully realize the new naming. This plan catalogs every location.

## Key Finding

**The largest incomplete rename is `Joiner` → `Refiner`** — the struct, module, config field,
benchmarks, and all documentation references still use the old name. Everything else
(`Tapper`, `Barrel`, `Draft`, `Drum`, `Governor`, `Foreman`, `Manifold`) is already
renamed or newly introduced. Only comments, idioms ("cast" → "tap", "buffer" → "plenum"),
and a few terminological stragglers remain.

---

## Files to Modify

### Critical — structural renames

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/workers/joiner.rs` | Module, struct `Joiner`, all comments, test names | Rename file to `refiner.rs`, rename struct to `Refiner`, update all internal references |
| `crates/kvx-lib/src/workers/mod.rs` | `mod joiner; pub use joiner::Joiner;` | Update to `mod refiner; pub use refiner::Refiner;` |
| `crates/kvx-lib/benches/joiner_bench.rs` | Entire file uses old term | Rename to `refiner_bench.rs`, update all content |

### Config — field names and serde aliases

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/config.rs` | Field `joiner_parallelism`, serde alias `num_joiner_workers` | Rename to `refiner_count` (add `alias = "joiner_parallelism"` for compat) |
| `crates/kvx-lib/src/config.rs` | Field `pumper_to_joiner_capacity` | Rename to `pumper_to_refiner_capacity` |
| `crates/kvx-lib/src/config.rs` | Field `joiner_to_drainer_capacity` | Rename to `refiner_to_drainer_capacity` |
| All `configs/*.toml` | `joiner_parallelism`, `pumper_to_joiner_capacity`, `joiner_to_drainer_capacity` | Update field names |
| All `demo/*.toml` | Same fields | Update field names |

### Foreman — spawns Joiners, references Joiner everywhere

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/foreman.rs` | `workers::Joiner::new(...)`, `the_joiner_thread_handles`, comments | Update to `Refiner`, rename variables |

### Drainer — references Joiners in comments

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/workers/drainer.rs` | Doc comments: "Joiner(s) (std::thread) → ch2", "joiner thread pool" | Update to Refiner |

### Governor — references Joiners and FlowKnob in comments

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/workers/governor.rs` | Comments: "FlowKnob that Joiners read" | Update to Refiner |

### Manifolds — comment cleanup

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/manifolds/mod.rs` | Doc: "buffer" → "Plenum", "tapper.cast(barrel)" → "tapper.tap(barrel)" | Update terminology |
| `crates/kvx-lib/src/manifolds/backend.rs` | Comments: "cast" → "tap" | Update terminology |

### Taps — comment cleanup

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/taps/mod.rs` | Trait doc: "Cast a raw barrel", comment "the cast is free" | Update to "Tap a raw barrel" |
| `crates/kvx-lib/src/taps/pit_to_bulk.rs` | Comment: "Joiner calls `tapper.cast(barrel)`" | Update |
| `crates/kvx-lib/src/taps/ndjson_to_bulk.rs` | Comment: "cast it into the bulk dimension" | Update verb |

### lib.rs — integration test references

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/lib.rs` | Comments: "Joiner received 1 barrel", "Joiner buffers" | Update to Refiner |
| `crates/kvx-lib/src/lib.rs` | Doc: "FlowKnob — The Governor writes it. The joiners read it." | "joiners" → "refiners" |
| `crates/kvx-lib/src/lib.rs` | Test `RuntimeConfig` values: `joiner_parallelism`, `joiner_to_drainer_capacity` | Update field names |

### regulators — comment

| File(s) | Issue | Action |
|---------|-------|--------|
| `crates/kvx-lib/src/regulators/mod.rs` | Doc: "Joiner reads flow knob on every flush check" | Update to "Refiner reads..." |

### Documentation — README files

| File(s) | Issue | Action |
|---------|-------|--------|
| `README.md` | Architecture diagram, Terminology table, Configuration reference — "Joiner" everywhere | Update to "Refiner" |
| `crates/kvx-lib/README.md` | All references | Update |
| `crates/kvx-lib/src/README.md` | All references | Update |
| `crates/kvx-lib/src/workers/README.md` | All references | Update |
| `crates/kvx-lib/src/manifolds/README.md` | All references | Update |
| `crates/kvx-lib/src/regulators/README.md` | `FlowKnob` + `Joiner` references | Update |

### Demo configs

| File(s) | Issue | Action |
|---------|-------|--------|
| `demo/*.toml` | Any `joiner_parallelism`, `pumper_to_joiner_capacity` | Update field names |

---

## Reuse

The following renames have **already been completed** — no changes needed:

| Old | New | Status |
|-----|-----|--------|
| `page` → `Barrel` | ✅ Done — `crate::Barrel` exists as a newtype |
| `document/record in transit` → `Draft` | ✅ Done — `crate::Draft` exists as a newtype |
| `Request Body / Payload` → `Drum` | ✅ Done — `crate::Drum` exists as a newtype |
| `Caster` → `Tapper` + `BarrelToDraftsTapper` | ✅ Done — trait + enum already renamed |
| `Buffer` → `Plenum` | ⚠️ Partially — type renamed but comments still say "buffer" |
| `Flow Master` → `Governor` | ✅ Done — with serde `alias = "flow_master"` for config compat |
| `Page Channel` / `Request Body Channel` → ch1/ch2 | ✅ Internal shorthand kept |
| `Foreman` | ✅ Stays (the plumber / foreman fits the motif) |
| `Manifold` | ✅ Stays |
| `Regulator` | ✅ Stays |
| `GaugeReading` | ✅ Stays |
| `Drainer` | ✅ Stays |
| `Pumper` | ✅ Stays |

### Preserved backwards-compatible serde aliases (no need to remove):
- `pumper_to_joiner_capacity` ← also accepts `channel_size`, `queue_capacity`
- `joiner_to_drainer_capacity` ← also accepts `drum_channel_capacity`
- `refiner_count` ← will also accept `num_joiner_workers` (old), `joiner_parallelism` (old)
- `GovernorConfig` ← also accepts `flow_master` TOML section name

---

## Steps

### Step 1: Rename `Joiner` struct + module → `Refiner`
- [ ] Rename `workers/joiner.rs` → `workers/refiner.rs`
- [ ] Rename struct `Joiner` → `Refiner` in `refiner.rs`
- [ ] Update all doc comments (test names, module doc, inline comments)
- [ ] Rename `the_joiner_thread` / `the_joiner_thread_handles` variables

### Step 2: Rename config fields + serde aliases
- [ ] `joiner_parallelism` → `refiner_count` (add alias `joiner_parallelism` for compat)
- [ ] `pumper_to_joiner_capacity` → `pumper_to_refiner_capacity` (add alias)
- [ ] `joiner_to_drainer_capacity` → `refiner_to_drainer_capacity` (add alias)
- [ ] `num_joiner_workers` alias → keep as is (backward compat)

### Step 3: Update all references across source files
- [ ] `workers/mod.rs` — module declaration + re-export
- [ ] `foreman.rs` — `Joiner::new(...)`, variables, comments
- [ ] `drainer.rs` — doc comments
- [ ] `governor.rs` — doc comments
- [ ] `regulators/mod.rs` — doc comments
- [ ] `manifolds/mod.rs` — comments (buffer → Plenum, cast → tap)
- [ ] `manifolds/backend.rs` — comments
- [ ] `taps/mod.rs` — comments (cast → tap)
- [ ] `taps/pit_to_bulk.rs` — comments
- [ ] `taps/ndjson_to_bulk.rs` — comments
- [ ] `lib.rs` — comments, test RuntimeConfig values
- [ ] `progress/renderer.rs` — comments (FlowKnob pattern)
- [ ] `backends/sink.rs` — comments (cast → tap)
- [ ] `backends/file/mod.rs` — comments
- [ ] `backends/elasticsearch/elasticsearch_sink.rs` — comments
- [ ] `backends/in_mem/in_mem_sink.rs` — comments

### Step 4: Rename benchmark file
- [ ] Rename `benches/joiner_bench.rs` → `benches/refiner_bench.rs`
- [ ] Update all references inside (test names, variable names, imports)

### Step 5: Update TOML config files
- [ ] `configs/kvx.toml` — field names
- [ ] `configs/kvx_file.toml` — field names
- [ ] `configs/kvx_file_to_esdb.toml` — field names + `flow_master` comment
- [ ] `demo/*.toml` — field names

### Step 6: Update documentation
- [ ] `README.md` — architecture diagram, terminology table, config reference
- [ ] `crates/kvx-lib/README.md`
- [ ] `crates/kvx-lib/src/README.md`
- [ ] `crates/kvx-lib/src/workers/README.md`
- [ ] `crates/kvx-lib/src/manifolds/README.md`
- [ ] `crates/kvx-lib/src/regulators/README.md`

### Step 7: Verify compilation + tests pass
- [ ] `cargo check --workspace` — make sure all renames resolve
- [ ] `cargo test --workspace` — all tests pass after the rename
- [ ] `cargo clippy --workspace` — no lint regressions

---

## Verification

1. **`cargo check --workspace`** — all path references in `use` statements and `mod` declarations must resolve.
2. **`cargo test --workspace`** — full test suite including integration tests (`lib.rs`) and benchmark module.
3. **`cargo doc --no-deps`** — ensure no broken doc links from README renames.
4. **Manual grep sweep** — verify zero remaining instances of:
   - `Joiner` / `joiner` (case-insensitive, code + comments)
   - `flow_master` / `FlowMaster` (beyond the intentional backward-compat alias)
   - Old "cast" verb in pipeline context (comments)
   - Old "buffer" noun referring to the Plenum in pipeline context
5. **Config loading tests** — verify TOML files with old field names still load via serde aliases.