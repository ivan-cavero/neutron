# Neutron — dual-track gauntlet workbench

> Live round log. LEAD: pi (main checkout). Two parallel tracks in native git worktrees.
> Bar is untouchable; builders never grade themselves; critics are fresh per round.
> **ACTIVE 18 Aug 2026 — run-048 on new PC (Linux). run-047 history below.**

## Run 048 (active) — resume: A3 (bench perf) + B2/B3 (worldgen parity)

Bar: verbatim in `runs/run-048.md`. Budget: 3 rounds/unit, A3 ∥ B2, B3 after B2,
1 smoothing per track, merge+push. Ownership: A3 = `tests/benchmarks/**`;
B2/B3 = `crates/neutron-*/**` + `tools/nbt-ref/` (gitignored); LEAD = STATE/workbench/runs.
Worktrees: `wt-bench` (run048-bench), `wt-worldgen` (run048-worldgen) — both off 3e63d5a.

| Unit | Round | Verdict | Evidence | Artifact |
| --- | --- | --- | --- | --- |
| A3 bench perf (101 fix, root gate, times) | — | ⏳ not started | | |
| B2 refs + baseline (424242/12345/777) | — | ⏳ not started | | |
| B3 777 regression + lush/pale ≥80% | — | ⏳ after B2 | | |

## Round log (run 048)

- R0 (LEAD audit §2.5): repo clean @ 3e63d5a, main == origin/main. STATE.md claims
  match git (all PASSED work merged). run-046 numbers still unverifiable (no reference
  worlds on disk — `tools/nbt-ref/` has only README.md + stale `vanilla1/` server dir
  from an old run). Java 25 present as JBR 25.0.2 (JetBrains) — will test against the
  vanilla jar before installing Adoptium. Bench release target warm in main checkout
  (built 17 Aug 15:33). Worktrees created. run-048.md written. Builds (bench release +
  ref-extract) launched in background. → provisioning in flight.
- R0 (env provision): bench release build 27.6s warm, ref-extract 16.2s. Vanilla 26.2
  jar downloaded (60,894,273 B, sha verified). JBR 25.0.2 boots vanilla 26.2 (Done
  marker + chunks saved) → **no Adoptium install needed**. Builders A3 + B2 launched
  in parallel (async).
- R1 B2 (builder): **run died on model API 502** mid-debug (ref-extract java spawn
  failing: "stdout closed" instantly on tmp-dir reuse with --keep-tmp; orphaned java
  held world/session.lock → retry collided). LEAD root-caused live: extraction WORKS
  from a fresh tmp dir (424242 → 529 chunks, EXIT 0, 50.4s boot). **B2 resumed**
  (9b947c38) with fresh-tmp recipe + cleanup instructions. A3 still running.
- R1 A3 (builder): **hit 30-min harness timeout** mid-investigation. Findings on disk:
  exit-101 panic = bevy_log GLOBAL subscriber conflict (bot 1 sets it, bots 2-10
  error); deeper: even bot-1 never connects (azalea `.start()` silent death). WIP
  (uncommitted): port plumbing config.rs/harness.rs/server.rs (server-port +
  query.port). **A3 resumed** (0458c6f4) with timebox: commit WIP → fix 101
  (priority) → 20-min timebox on azalea connectivity (else document as open item) →
  root gate + measured times.
- R1 A3 (resumed): port plumbing committed (3e13bfe). Breakthrough: bots now
  connect — azalea login → config state (ClientInformation sent, bench-0..7
  visible in debug log). 101 fix + connectivity fix in flight (bot crate
  client.rs/lib.rs/Cargo.toml uncommitted).
- R1 A3 (builder): **DONE (builder-verified)** — 5 commits (3e13bfe port plumbing,
  144b288 exit-101 fix = init global logger once + disable bevy_log LogPlugin in
  bot Apps, a77dc86 raise bot join timeouts for MC 26.2 config phase, 12de2d9
  root-gate ./bench wrapper + README + logger idempotency test, 09bd272 docs).
  Acceptance run REAL: server ready 44s, join-storm 10/10 bots connected
  (p50=26041ms — high but connecting), run completes, results JSON + history
  entry written, NO exit 101. Cold build 21.31s (379% CPU), root gate EXIT=0.
  Evidence: /tmp/bench-evidence/*.log. → **A3 blind critic launched** (239b4fb6).
- R1 A3 CRITIC: **PASS** — 5/5 verified live: tests 14/14 (incl.
  init_logging_is_idempotent); 101 fix principled (OnceLock + LogPlugin disable,
  grep exit(0)/catch_unwind = 0); real run BENCH_EXIT=0, 10/10 bots
  (p50=6755ms), results written, ~2m58s; root gate ./bench test + run --help
  from root on nightly 1.100.0; times real (cold 12:45.65, warm 21.31s, run
  3m25s documented); tree clean. Nits: 2 warnings (unused mut,
  SubscriberInitExt import), try_init err discarded (OK — OnceLock). **A3 DONE.**
  → **A4 smoothing launched** (5f06e9b9) on run048-bench.
- R1 A4 SMOOTHER: DONE — 3 harmonization commits (cc59594 README working-dir
  clarity, 522ecae bench wrapper help + sane failures, 0067fe2 warning fixes).
  Bot crate warning-free; remaining 10 warnings pre-existing at 3e63d5a.
  **TRACK A MERGED to main (ff cc59594)**; runs/run-048.md reconciled (LEAD
  version + A3/B2 results, 5d90620). Root workspace smoke test running
  (cargo test --workspace on merged main).
- R1 B2 (resumed builder 9b947c38): **DONE (builder-verified)** — references
  extracted for all 3 seeds (529 chunks each, reference.json present; 12345 spawn
  at (6,-2), its (0,0) chunk is a proto-chunk). Baseline measured (raw in
  baseline-evidence.txt → runs/run-048-evidence-baseline.txt):
  REGION 424242 (0,0) 97.27% (bar ≥97.28%, 0.01pp under); 12345 (6,-2) 97.79%
  (≥97.75% ✓) but border diffs 20.90pp (≥30%-down NOT met); **777 (0,0) 96.29% —
  regression reproduced EXACTLY**; clay 424242 = 466 (vanilla 493); lush_pale
  424242 recall 53.03% (run-046 claimed 62.94% — claim was optimistic); lush_pale
  12345 98.40%; tests 59/59. Excluded (documented): 12345 (0,0) proto-chunks;
  clay hardcodes 424242. → **B2 blind critic launched** (98008157).
- R1 B2 CRITIC: **PASS** — every criterion verified from scratch: 3 seeds × 529
  chunks, reference.json timestamps cross-checked to sub-second vs extraction
  logs; region_parity 424242 re-run → 97.27% byte-identical (all 9 chunks);
  lush_pale recall 53.03% byte-identical; exclusions independently verified
  (12345 (0,0) proto-chunk via chunk-dump; clay hardcodes 424242 at
  clay_overlap.rs:110); 0 commits on branch; tests 59/59 (55s). Nit: evidence
  file + servers-ref/ not gitignored (not code). **B2 DONE.** → **B3 builder
  launched** (cc25f22d) with verified baseline + run-046 context.

---

## Run 047 (closed) — dual-track: benchmarks refactor + server review/worldgen

> **SESSION PAUSED 17 Aug 2026 (user leaving; resume on another PC) — see runs/run-047.md RESUME B

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
| A1 | Multi-version server provisioning (`servers download <type> <version>`, Mojang/Paper API + local fallback, central dir, ref-extract decoupled) | ✅ PASS (R2) | critic: sha1/sha256 vs live APIs, pumpkin PE=AMD64, hostile inputs clean | 36c13a4, 3b7b0ff |
| A2 | Versioned report history + `compare` over history | ✅ PASS (R3) | hand-verified deltas; missing/tie never win; 10 tests | c0876c6, 16fb597, 519bc2e |
| A3 | Perf: root build works, build/runtime time before/after, exit-101 panic fix | ⏳ IN FLIGHT (relaunch 3cb4f881; died on wall/stream ×2) | | |
| B1 | Server review + bot smoke test (boot, join, chunks, TPS) | ✅ PASS (6/6) | Done 0.0s, ping, 21 chunks, TPS 20.00, 241/241 | 6444286, 89550c4, 7d45404, 2199b05 |
| B1b | Disconnect cleanup (critic-found defect) | ✅ PASS | RST → online 0 live; 241/241 | 272e30b |
| B2 | Reference extraction (Java 25) + baseline re-measure (424242/12345/777) | ⏳ IN FLIGHT (fc381ce1; no artifacts yet) | | |
| B3 | Worldgen: 777 regression isolate + lush/pale recall ≥80% | ⏳ PENDING (needs B2) | | |

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
- R2 A2 CRITIC #2 (tight scope): **PASS** — hand-verified real-history math: startup Δ −3687.8 (−21.1%) winner correct; join p50 0.0 unmeasured → Δ N/A no winner; TPS/CPS all-unmeasured sections skipped; crafted pairs (regression/improvement/absent-field/near-tie) all correct, no panics. A2 DONE (3 rounds incl. fix).
- R3 A3 (builder, bench-refactor): in flight — exit-101 bevy_log panic fix, root gate wrapper, times measured.
- R3 A3: first launch died on stream interruption (2 min in, no changes; tree clean) → **fallback relaunched** (3cb4f881).
- R1 B1b CRITIC (fallback): **PASS** — fix minimal (+9/-2), RST repro live: online 0 → join → 1 → taskkill /F → cleanup fires → 0; rapid cycle clean; 241/241 tests; tree clean at 272e30b. **B1 (server review) fully DONE.**
- R3 B2 (builder, server-worldgen): in flight — fresh 26.2 reference extraction (424242/12345/777) + baseline re-measure (region_parity/clay_overlap/lush_pale_parity). Resume-from-disk design (jars/worlds gitignored; if wall hits, next builder continues from disk).

## Open questions for the human

- (none)