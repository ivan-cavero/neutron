# Neutron — dual-track gauntlet workbench

> Live round log. LEAD: pi (main checkout). Two parallel tracks in Orca worktrees.
> Bar is untouchable; builders never grade themselves; critics are fresh per round.

## Goal

1. **Track A (bench-refactor)**: refactor + improve the benchmark harness — fast,
   reliable multi-version server provisioning (jar download + local fallback),
   versioned report history, and a build/run that isn't painfully slow.
2. **Track B (server-worldgen)**: review the neutron server (test it properly) and
   continue world generation parity work (run-046 continuation).

## Bar (verbatim)

**Track A** (from previous workbench P1-P4 + user additions):
1. `bench/` no longer exists in the repo root; everything lives in `tests/benchmarks/`.
2. `cargo test --workspace` green in root workspace AND benchmarks workspace; no
   regression in the 59 worldgen tests.
3. `neutron-bench servers download <type> <version>` provisions jars (auto-download
   from Mojang manifest / Paper API + local fallback) into a central managed dir with
   a **multi-version layout**; zero tools reference `bench/servers` (`grep -r "bench/servers" tools/` = 0).
4. Every run writes a timestamped, versioned report into `tests/benchmarks/results/history/`;
   `compare` works over the history.
5. Benchmarks workspace builds in release from the repo root (toolchain resolved);
   build time measured before/after; benchmark wall-clock budget documented.

**Track B** (run-046 bar, human decision R43 — mechanism parity):
- Same seeds/streams/algorithms as vanilla. Deterministic phases → 100% block match
  multi-seed; vegetation/sculk → same RNG stream 1:1. **Do not edit measurement
  examples/tests to pass.**
- U5 AC: clay 424242 → ~497 (lush/pale missing ≤20%); border diffs ≥30% down on
  12345; REGION 424242 ≥97.28%, 12345 ≥97.75%; **777 no regression** (ratchet);
  tests 59/59.
- Server: boots, joinable, serves real chunks, TPS stable (smoke evidence).

## Budget

3 rounds/unit max, 2 tracks in parallel, 1 smoothing pass per track, then merge+push.
Stop: bar met, or 2 consecutive rounds with no improvement, or user says stop.

## Parallelism / ownership map

| Track | Worktree (branch) | Files owned | Builder | Critic |
| --- | --- | --- | --- | --- |
| A | `C:/Users/ivang/orca/workspaces/neutron/bench-refactor` (`ivan-cavero/bench-refactor`) | `tests/benchmarks/**` | gauntlet-builder | gauntlet-critic (fresh) |
| B | `C:/Users/ivang/orca/workspaces/neutron/server-worldgen` (`ivan-cavero/server-worldgen`) | `crates/neutron-server/**`, `crates/neutron-worldgen/**` | gauntlet-builder | gauntlet-critic (fresh) |
| LEAD | main checkout | `STATE.md`, `workbench.md`, `runs/` | — | — |

Rules: one writer per file; A and B touch disjoint dirs; lead merges branches into
main and pushes incrementally. `tools/` = human-owned, never touched.

## Units

| Unit | Scope | Verdict | Evidence | Artifact |
| --- | --- | --- | --- | --- |
| A1 | Multi-version server provisioning (`servers download <type> <version>`, Mojang/Paper API + local fallback, central dir, ref-extract decoupled) | — | | |
| A2 | Versioned report history + `compare` over history | — | | |
| A3 | Perf: root build works, build/runtime time before/after | — | | |
| B1 | Server review + bot smoke test (boot, join, chunks, TPS) | — | | |
| B2 | Reference extraction (Java 25) + baseline re-measure (424242/12345/777) | — | | |
| B3 | Worldgen: 777 regression isolate + lush/pale recall ≥80% | — | | |

## Round log

- R0 (LEAD): audit §2.5 — repo clean @ c566ece, in sync with origin/main. No stash
  (workbench claim stale). `tools/nbt-ref/` has NO reference worlds → run-046 numbers
  unverifiable, flagged. No jars, Java 21 only → Java 25 installed via winget
  (Temurin 25.0.4.7, user-approved). No target dirs (cold builds everywhere — the
  "slow benchmarks" baseline). Orca worktrees created:
  `bench-refactor` + `server-worldgen` (branch `ivan-cavero/*`). Workbench rewritten.
- R1 A1 (builder, bench-refactor): DONE — commit 36c13a4, clean tree. Real downloads
  verified: vanilla 26.2 (sha256 cdacdfb2…, 60,894,273 B, valid jar), paper 26.2 build
  112 STABLE (sha256 matches API), folia 26.2 build 4 BETA (matches API). Live finding:
  PaperMC v2 API sunset → fill.papermc.io/v3. Offline fallback via
  NEUTRON_BENCH_SERVERS_FALLBACK (10s connect/600s total timeouts). `servers list/status`,
  `run --version` resolution + legacy fallback, grep bench/servers = 0. Build: 8m52s cold,
  ~9s warm. Pumpkin: no release binaries → honest actionable error. → **critic launched**.
- R1 B1 (builder, server-worldgen): TIMED OUT at 30 min mid-investigation (join1 client
  decode errors — server bug vs harness?). Uncommitted on disk: tick.rs, worldgen
  micro-opts (zero gain → must revert), tests/e2e-server/ harness + status_ping.py.
  Resume unavailable (no recovery identity) → **fallback builder relaunched** (aa2d4953,
  90 min budget) with on-disk state.
- R1 B1 (fallback): in flight.
- R1 A1 CRITIC: **FAIL** — 5/6 criteria MET with live evidence (re-downloaded vanilla, sha1 vs Mojang manifest match; paper/folia sha256 vs live API match; fallback/timeouts/hostile inputs all clean). Criterion 1 fails on **pumpkin leg**: downloads ARM64 binary on x86_64 host (PE machine 0xaa64 verified); "no releases" error message is false (nightly release with X64+ARM64 assets exists). Bonus find: `run` exits 101 on pre-existing azalea/bevy_log logger panic in bot crate (not provisioning; scope for A3). → **A1 R2 builder launched** (arch-aware asset selection + honest error text).
- R2 A1 (builder, bench-refactor): DONE — commit 3b7b0ff (provision.rs, +31/-20). Arch-aware `pumpkin_target()` (ARCH+OS → asset labels), honest error text, explicit version→nightly mapping. LEAD spot-check: pumpkin.exe PE machine = **0x8664 (AMD64) ✓**. → **fresh R2 critic launched**.
- R1 B1 (fallback #1): TIMED OUT at 30 min again (timeout param not propagated; 90 min requested). But bars #1-4 PROVEN on disk: Done marker, status ping, join root-cause (PROBE bug — zero-fill buffer, not server), 21 real chunks, TPS fix sleep→interval = 20.01 (19.99-20.02 under load). Worldgen experiment reverted cleanly. → **FINALIZE fallback launched** (test suite + REVIEW.md + commits; warm builds).
- R2 A1 CRITIC: **PASS** — re-proved whole bar from clean state: fresh vanilla re-download sha1/size == Mojang manifest, folia/paper sha256 == live API, pumpkin PE = AMD64, offline/fallback/network-error paths bounded, hostile inputs clean, tree clean. A1 done (2 rounds).
- R1 B1 (finalize): DONE — 4 commits on ivan-cavero/server-worldgen: 6444286 (tick interval 20 TPS), 89550c4 (e2e join harness, zero decode errors), 7d45404 (status_ping.py), 2199b05 (REVIEW.md). `cargo test --workspace` 241/241 EXIT=0. → **B1 blind critic launched**.
- R2 A2 (builder, bench-refactor): in flight (versioned report history + compare over history).
- R2 A2 (builder): DONE — commits c0876c6 (history.rs, versioned reports under results/history/) + 16fb597 (compare over history, per-metric deltas + winners). 6 tests green, real runs produced history entries (vanilla-26.2-join-storm-small-1-*.json), ws_root anchoring verified from any cwd. → **A2 blind critic launched**.
- R1 B1 CRITIC: **PASS** — all 6 criteria met (boot Done marker 0.0s, status ping valid, real join to Play + chunks, independent TPS 20.00 no drift, 241/241 tests, REVIEW.md honest). Refutation found ONE real defect (doesn't fail bar as written): **disconnect state leak** — `run_reader` `?` on read error skips `remove_player`; RST leaves dead players in map → status ping reports stale `online: 4` with zero clients. → **B1b fix builder launched** (reproduce → fix → re-verify, both clean + forced disconnect).
- R2 A2 CRITIC: **PASS** — 5/5 criteria (versioned history schema, compare over history, history list, real run → history entry, builds green). Two blemishes flagged (live): missing metrics (0.0) marked winner over real measurements; ties get winner marker. → **A2 R2 fix builder launched** (winner logic: missing/absent values never win, ties no winner, unit tests).
- R2 A2 FIX: DONE — commit 519bc2e (reporter.rs +97/-16: EPS, is_measured, winner_index, delta_str; 4 new tests; 10 passed; re-run compare shows blemishes gone, real winners intact). → **A2 R2 critic launched**.
- R1 B1b FIX: DONE — commit 272e30b (connection.rs: read error → break, cleanup always runs; both cleanup paths confirmed in log before timeout). LEAD committed the builder's artifact + inspected diff (matches prescribed fix). → **B1b critic launched** (reproduce RST disconnect, 241 tests).
- R1 B1b CRITIC #1: died on tool typo (`rea`). Resume unavailable → **fallback critic relaunched** (5c9cf5ed).
- R2 A2 CRITIC #1: timed out at 30 min (no partial output; scope included live runs). Worktree clean at 519bc2e, history intact → **tight-scope critic relaunched** (331afe8b; logic-only, crafted JSON pairs, no live benchmark).

## Open questions for the human

- (none)