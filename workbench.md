# Benchmarks reorganization — gauntlet workbench

**Goal**: reorganize/refactor the project's benchmark setup: move `bench/` out of the root, add server provisioning (auto-download + local fallback), add a versioned report history, decouple tools from bench jars, fix the slow/broken build.

**Bar (verbatim)**:
1. `bench/` no longer exists in the repo root; everything lives in `tests/benchmarks/`, documented in AGENTS.md §4.
2. `cargo test --workspace` green in the root workspace AND the benchmarks workspace (or a documented combined gate running both); no regression in the 59 worldgen tests.
3. `neutron-bench servers download <type> <version>` provisions jars (auto + local fallback) into a central managed dir; zero tools reference `bench/servers` (`grep -r "bench/servers" tools/` = 0).
4. Every run writes a timestamped, versioned report into `tests/benchmarks/results/history/`; `compare` works over the history.
5. The benchmarks workspace builds in release from the repo root (toolchain resolved — the current azalea nightly panic disappears); build time measured before/after.

**Budget**: 4 pieces, ≤3 rounds each, 1 smoothing pass. Standard.

**Baseline (measured by LEAD, pre-loop)**:
- `cargo build --release --manifest-path bench/Cargo.toml` from root: **FAILS** (azalea build.rs panics: requires nightly; rust-toolchain.toml not picked up from root cwd). 1:01 min elapsed to failure.
- Execution not measurable: `bench/servers/` contained only README.md (no jars).
- After P1: benchmarks release build works from `tests/benchmarks/` cwd (9.55s with warm cache, 52 MB binary); root build from root still fails on azalea toolchain unless cd'd (documented cwd-based build).

| Unit | Piece | Round | Verdict | Evidence | Artifact |
|------|-------|-------|---------|----------|----------|
| P1 | Relayout bench/ → tests/benchmarks/ | 1 | **PASS** | critic: worldgen 59/59 (378s), release build fresh binary, 32 renames, docs clean grep | commit 9f8b3e4 |
| P2 | Server provisioning (download + fallback, central dir, ref-extract decoupled) | 1 | pending | | |
| P3 | Versioned report history + compare over history | 1 | pending | | |
| P4 | Perf: root build works, build time improved, startup measured | 1 | pending | | |

## Round log
- R1 P1: builder timeout waiting on azalea release build (30 min) → LEAD killed orphans (7 neutron_server @300% CPU), fixed pre-existing libc build break (server.rs:175 used libc::kill without dep), rebuilt (9.55s warm), critic blind PASS 6/6.
- R1 Wave 2 (parallel worktrees): FAILED — harness worktree isolation did not bind children to their worktrees (all 3 wrote to the main checkout; 0 files in the worktrees). P2 produced non-compiling code on main, P3/P4 interleaved. LEAD stopped the workflow, stashed the mess (stash@{0}), removed worktrees. Lesson: P2/P3 share main.rs → SERIALIZE. Relaunching serial, no worktrees, strict per-piece file ownership.
- R2 P2: in progress (serial).

## Open questions for the human
- (none)