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
- Execution not measurable: `bench/servers/` contains only README.md (no jars).

| Unit | Piece | Round | Verdict | Evidence | Artifact |
|------|-------|-------|---------|----------|----------|
| P1 | Relayout bench/ → tests/benchmarks/ | — | pending | | |
| P2 | Server provisioning (download + fallback, central dir, ref-extract decoupled) | — | pending | | |
| P3 | Versioned report history + compare over history | — | pending | | |
| P4 | Perf: root build works, build time improved, startup measured | — | pending | | |

## Round log
- (none yet)

## Open questions for the human
- (none)