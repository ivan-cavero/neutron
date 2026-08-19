# Neutron — dual-track gauntlet workbench

> Live round log. LEAD: pi (main checkout). Two parallel tracks in native git worktrees.
> Bar is untouchable; builders never grade themselves; critics are fresh per round.
> **ACTIVE 18 Aug 2026 — run-048 continued on THIS PC (Windows); parallel worldgen
> agent active on `crates/neutron-worldgen/` (user-confirmed). run-047 history below.**

## Run 048 (active) — resume: A3 (bench perf) + B2/B3 (worldgen parity)

Bar: verbatim in `runs/run-048.md`. Budget: 3 rounds/unit, A3 ∥ B2, B3 after B2,
1 smoothing per track, merge+push. Ownership: A3 = `tests/benchmarks/**`;
B2/B3 = `crates/neutron-*/**` + `tools/nbt-ref/` (gitignored); LEAD = STATE/workbench/runs.
**NOTE (R4 audit): B3 was re-derived directly on `main` by the parallel worldgen agent —
the `run048-worldgen` worktree/branch does not exist on this PC.**

| Unit | Round | Verdict | Evidence | Artifact |
| --- | --- | --- | --- | --- |
| A3 bench perf (101 fix, root gate, times) | R1 | ✅ PASS (blind critic 239b4fb6) | workbench R1 log below | 3e13bfe..12de2d9 + smoothing, merged to main |
| B2 refs + baseline (424242/12345/777) | R1 | ✅ PASS (blind critic 98008157) | `runs/run-048-evidence-baseline.txt` | — |
| B3 777 regression + lush/pale ≥80% | R3 (this PC) | 🔨 re-derived on main (parallel agent); **FAIL on recall** (57.14% < 80%, LEAD re-measured); blind critic PENDING | commits 7fcfd06/e72a87e/355c3d5 (NOT pushed) | main @ 355c3d5 |
| B4 vegetation gap closure | ⏳ next | — | design: R4 log entry below | — |

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
- R1 (LEAD): **merge smoke test GREEN** — 241/241 on merged main (47 protocol,
  7 world, 24 server, 65 sim, 39 world, 59 worldgen), all ok. Committed
  workbench + baseline evidence (4a41f5b). **Push BLOCKED: no GitHub creds**
  (HTTPS needs token, SSH key not registered, no gh CLI) — OPEN for human.
- R2 B3 (builder cc25f22d): **30-min timeout hit** mid-diagnosis (lush/pale
  per-chunk tool written, lushdiag-424242.txt). LEAD read the gaps:
  **big_dripleaf neu=0 everywhere (feature absent)**, cave_vines under,
  pale_hanging_moss over-generated (wrong≈neu), clay/moss wrong positions.
  **Resumed** (e111a02b) with plan: commit WIP first, port features in vanilla
  order, ratchet 3 seeds per change.
- R2 B3 (resumed): committed WIP 1e0a30a (diagnostic + gap analysis); now
  investigating tree geometry (tree_bases/tree_draws/bio_dump diag examples,
  untracked) — pale_oak_log/leaves missing+wrong. Still active.
- R2 B3 (builder): ratchet after biome fix — 424242 97.36% (bar ✓), 12345
  97.81% (✓), **777 98.29% (+2.0pp vs 96.29 baseline)**; lush recall 54.17→
  57.43%. Full workspace tests green (15 suites, 0 failed).
- R3 B3 (builder, resumed twice): **DONE (builder-verified)** — 6 commits
  (1e0a30a..eec1d1d). Root cause of 777 "regression": the ~99.4% historical
  claim is NOT reproducible (pre-U5 dc71940 = 96.32% vs fresh ref); real bug =
  biome source applied peaksAndValleys to ridge noise, vanilla uses RAW
  (probe −0.8113) → 777 96.29→98.29%, lifts all seeds. Lush/pale ports:
  vegetation_patch Java HashSet order, random_offset x,y,z, env_scan drop,
  FeatureSorter indices 1:1. Final: 424242 97.36% ✓ · 12345 97.81% ✓ · 777
  98.29% ratchet ✓ · **recall 57.43% (bar ≥80% NOT met)** · clay 411/493 ·
  59/59 + workspace green. Residual gap: trees 3338 + clay 1914 missing,
  claimed terrain/scheduler-coupled (ring-first experiment worse 54.45%).
  Evidence: runs/run-048-evidence-B3.txt + /tmp/rp-*-final.txt. → **B3 blind
  critic launched** (44205f88) — expects FAIL on recall, checks ratchet/biome/
  no-tampering honestly.
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
- R3 (parallel worldgen agent, THIS PC — user-confirmed ownership): B3 re-derived
  directly on `main` (3 commits, NOT pushed): 7fcfd06 ([profile.test] opt-level=3 —
  workspace tests 13.5min+ → 1m18s; region_parity/lush_pale examples parallel,
  byte-identical output), e72a87e (raw ridge noise fix for `climate_at_block` +
  random_offset xz,xz,y order — claimed 424242 97.33%, recall 55.85%), 355c3d5
  (vegetation_patch Java HashSet order port `JavaBlockPosSet` — claimed 424242
  97.36%, 12345 97.81%, 777 98.31%, recall 57.14%). No blind critic on this PC.
- R4 (LEAD audit §2.5, this session): STATE/workbench/run-048 were stale (claimed
  PC-2 branch `run048-worldgen` @ eec1d1d + recall 57.43% + critic in flight —
  none exist here). Verified git (main @ 355c3d5, clean at first check) and
  **re-measured everything live** (release examples, refs on disk): 424242
  **97.36%** ✓ · 12345 **97.81%** ✓ · 777 **98.31%** ✓ (ratchet) · lush/pale recall
  **57.14%** (bar ≥80% NOT met) · clay 411/497 · tests 241/241 (15 suites).
  Gap analysis (B4 design): 8 572 missing cells → trees (leaves 2178 + log 1190) +
  clay 1965 = 62%; **3 dispatch no-ops** (multiface_growth/glow_lichen,
  root_system/rooted_azalea_tree, vines/classic_vines); **RNG-stream desync**
  evidence (center-chunk leaf counts ≈ vanilla 392 vs 396 but 3×3 positions ≠ →
  variable RNG consumption on accept/reject shifts later attempts). STATE.md +
  workbench + run-048 updated (append-only). Push still blocked.
- R4 (20:11 +02:00): parallel agent working RIGHT NOW — uncommitted
  `crates/neutron-worldgen/Cargo.toml` + `examples/feature_index_probe.rs`.
  NOT touched (AGENTS.md §5.5). Wait for its commit before B4 builders touch the crate.
- R5 (21:0x +02:00, LEAD + human authorization): **D0-D4 detection infra built**
  (the "detect any change on version bump" stack from STATE B4 design):
  - `mc-decompiler extract-data <version>` (tools/ — human explicitly asked me to
    build it): extracts `data/minecraft/worldgen/**` (bundler-safe, in-memory) +
    semantic diff vs the crate tree. LIVE evidence (this PC, jars on disk):
    **26.2 → MATCH 654 / CHANGED 0 / JAR-ONLY 309 / CRATE-ONLY 1**
    (`noise_settings_overworld.json` rename vs jar `noise_settings/overworld.json`)
    = crate data 100% in sync; **26.3-snapshot-8 → CHANGED 302, JAR-ONLY 703
    (new biome `dappled_forest`), CRATE-ONLY 227** = detection works end-to-end.
  - `crates/neutron-worldgen/tests/dispatch_coverage.rs`: every sorter-reachable
    configured feature must dispatch / be step-6 batch / be whitelisted. First run
    found 43 orphans in 23 types; verified against source → 14 implemented
    (10 dispatch + 4 batch), **30 gaps confessed with reasons**; test GREEN.
    Real findings the test surfaced: `ice_patch` (disk) at step 4 + `ore_infested`
    at step 7 are outside the step-6 batch (new known gaps); the 3 B4 no-ops
    (glow_lichen/root_system/vines) now covered by the whitelist, so any NEW
    type in 26.3 → red at D2 naming the feature.
  - Verification: `cargo test --workspace` **242/242 green** (incl. new test);
    mc-decompiler workspace builds+tests green. Docs updated (README tools list,
    runs/README D0-D4 template + history, STATE §D0-D4 infra). Parallel agent's
    uncommitted files untouched.
  - **PUSH: SUCCESS** (21:2x +02:00) — `git push origin main` OK
    (`ddb9e7c..c8468a6`); main == origin/main 0/0. The historical "no creds"
    block is obsolete (creds configured on this machine). Pushed: B3 commits
    7fcfd06/e72a87e/355c3d5 (parallel agent) + c8468a6 (this infra). STATE.md
    push-blocked claim corrected (append-only). Agent's WIP (Cargo.toml,
    feature_index_probe.rs, tree.rs) remains uncommitted on disk, untouched.

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
- R5 (B4 builder session 19 Aug 23:00-00:45, this PC): tree-type bug + T5 categorization + order derivation (partial) + T4 ports + diagnostics.
  - **T5 categorization** (subagent, read-only): per-seed mismatches categorized — TREES ~2390 (desync), FEATURE-veg ~808, CLAY ~473, SCULK 456, ORE ~393, CARVER 162, SURFACE ~123. Root cause found: BlockId lacked birch/spruce/jungle/acacia/mangrove/cherry log+leaves → every non-oak tree placed OAK blocks.
  - **Tree-type bug FIXED** (8775d20): 12 new BlockIds (101-112) + protocol ids + is_log/is_air_or_leaves/valid_tree_pos. **777 98.31 → 98.58% (+0.27pp)**. 424242 97.32%, 12345 97.81% unchanged.
  - **Climate verified exact** (c78c1cf): neon climate targets match vanilla at (4,y,4) seed 424242. biome source is NOT the divergence source; surface mismatches are desync trees.
  - **T3 order (partial, 2 subagents)**: for (0,0) seed 424242: (7,6) tree at **draw 8** (NOT 13 — parent's MARKER-A assumption wrong: free-height rejections consume height RNG, shifting stream). Before-neighbors of (0,0): **(-1,0) and (0,-1) only**. `vanilla_spawn` order measured WORSE (54.40%) — full 9-chunk order derivation incomplete (subagents timed out). Water filter fix (correct OCEAN_FLOOR) measured WORSE alone (-0.52pp) — must be applied WITH the full order (order+filter are a package). The root mechanism: the before-neighbors' plant spillover at draw positions → water-filter reject → no tree RNG → stream aligned. Without the order, the neutron terrain lacks the spillover → filter rejects differently → stream desyncs.
  - **T4 ports** (7 types, whitelist 27→19): vines, root_system, sea_pickle+seagrass+kelp (3→6 entries), block_blob, blue_ice. All ratchet green, recall 57.50%.
  - **deco_stream_probe**: climate/surface/column/trunks + skip-tree-draws gate (T3 tooling). Verified: climate targets EXACT match; terrain (stone/dirt) matches; surface mismatches are desync trees.

---

## Run 049 (active) — B4 parallel: full 9-chunk order (T3) ∥ remaining T4 ports

Bar: verbatim in `runs/run-049.md`. Budget: 2 rounds/task, parallel worktrees,
LEAD merges, blind critic on merged result. Ownership: `neutron-wt-t3` (branch
`b4-t3-order`) = T3 order; `neutron-wt-t4` (branch `b4-t4-ports`) = T4 ports;
LEAD = STATE/workbench/runs; refs shared read-only via symlinks.

| Unit | Round | Verdict | Evidence | Artifact |
| --- | --- | --- | --- | --- |
| T3 full 9-chunk order + water filter | R1 | 🔨 in flight (builder 844d6b68) | — | b4-t3-order |
| T4 remaining ports (whitelist 19→~5) | R1 | 🔨 in flight (builder 844d6b68) | — | b4-t4-ports |

## Round log (run 049)

- R0 (LEAD audit §2.5): main @ 8249a67 clean. Refs verified on disk
  (wt-worldgen worktree, 529 chunks × 3 seeds, region mca present). Baseline
  re-measured live: 424242 **97.33%** · 12345 **97.80%** · 777 **98.54%** ·
  lush/pale recall **57.96%** · clay 411/497 · tests 241/241 (matches STATE).
  Stale `tmp_order_probe` Cargo.toml entry (d331ee3, file gitignored) broke
  `cargo build --examples` → removed (8249a67, same fix as c901340).
  Worktrees `neutron-wt-t3`/`neutron-wt-t4` created from main; refs + 26.2 jar
  symlinked in (read-only). run-049 written. Two builders launched in parallel:
  T3 (full order derivation, rebuild lost tmp_order_probe as gitignored) and
  T4 (11 ports, measure-each, revert-on-regression per glow_lichen precedent).

- R1 (19 Aug 10:0x): first fan-out 844d6b68 FAILED — both builders hit the 30-min
  default subagent timeout (release builds + measurements need more). T3 left
  usable tooling on disk (strip+trunk-compare modes in deco_stream_probe,
  is_vegetal_family pub) — LEAD fixed a missing `wb` binding, verified it
  builds, validated the method live, committed as 9689cdc on b4-t3-order.
  T4 left nothing. LEAD findings from live validation:
  - trunk-compare matched-count is TRIVIAL (vanilla trunks pre-loaded in the
    buffer) — the real signal is the DRAW STREAM (accepted (x,z) per draw vs
    vanilla trunk columns).
  - probe vs FULL vanilla terrain: (0,0) draw 8 = (11,11), NOT vanilla (7,6)
    → full terrain is the wrong baseline; the center's own output must be
    stripped too (its features absent at its draw time).
  - stripping the 6 after-neighbors does NOT change (0,0)'s stream (their
    features don't reach its draw positions); stripping all 8 DOES → consistent
    with before-set {(-1,0),(0,-1)}.
  - water filter fix (OCEAN_FLOOR = blocks_motion) still unapplied — package
    with the order.
  Relaunched both builders (3580424c) with 90-min budget + continuation
  context (T3: derivation method + validated findings; T4: fresh, measure-each).

- R2 (19 Aug 12:2x): segunda fan-out (3580424c) terminó — AMBOS builders hicieron
  timeout a los 90 min con trabajo parcial. T3: 3 commits de tooling (water
  filter OCEAN_FLOOR fix, full step-9 stream mode, strip-center-trees) + análisis
  profundo del RNG (draw stream vs trunks; hallazgo clave: el strip del center a
  air elimina el spillover de los before-neighbors que causa los rejects —
  corregido con strip-center-trees que quita solo logs/leaves del center).
  T4: sin commits (solo una entrada de ejemplo vacía revertida).
  LEAD verificación: con after6+center-trees strip (fill=air): 5 rejects de 16
  draws (1,4,7,10,15) — ratio ~1/4 coincide con el finding del workbench.
  **PERO: cero commits de paridad en main hoy. El error fue dar tareas gigantes
  (9 chunks + 11 ports) a los builders.**
- R2 (corrección): relanzados con tareas ACOTADAS (85725fd4): T3a = derivar
  SOLO el before-set de (0,0) y verificar draw stream vs vanilla; T4a = portar
  SOLO 3 features fáciles con medición por-port. Presupuesto 50-90 min.

## Run 050 (closed) — orden REFUTADO; cave-biome root cause

- Pull limpio (backup local). Refs 424242 provisionados (spawn (0,0)).
  Baseline: 97.34% · recall 58.43% · clay 411/435.
- Scheduler 26.2 decompilado: FEATURES = radius 1 + vecinos a CARVERS; cola FIFO
  por nivel; orden del spawn = espiral + hash-wavefront (ChunkTracker) →
  RUN-DEPENDIENTE (PC-2: {(-1,0),(0,-1)}; esta máquina: {1,0;-1,1}).
- Orden NO es el lever: spiral +0.55pp; search 2-opt convergió en objetivo
  compartido pero la medición real no se movió (58.46%); centro con before-set
  exacto 50.9% pero el total empeora. Modelo compartido ≠ independiente (solo
  el centro coincide).
- Water filter fix: −0.54pp → revertido (ratchet).
- **CAUSA RAÍZ**: cave-biomes. 2.43% de celdas 4×4×4 mismatched, TODAS en
  secciones de cueva (y −48..96): vanilla=lush_caves, Neutron=pale_garden
  (boundary de depth ~14% de columnas más alto). lush_caves_clay/moss/vines
  llevan filtro `minecraft:biome` → se rechazan en esas celdas → gaps de
  clay/moss/vines = 34% del recall. Mecanismo verificado (volumen 6×).
- Commits: refactor decorate_region_origin_major + modos spiral/custom +
  probes (strips + biomes) + pubs. Tmp borrados. Tests 242/242.
- Próximo: corregir depth/offset del cave-biome (T1), re-medir.
