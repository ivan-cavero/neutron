# STATE — Neutron

> Read this first every session. Answers: where are we, what is the bar, what is the next action.
> History lives in `runs/` — this file only holds the current state.
> **Updated 18 Aug 2026** — run-048 active on PC-2 (Linux); session paused for PC switch.
> Resume boundary + full detail: `runs/run-048.md`. **PUSH BLOCKED on this PC (no GitHub creds).**

## Current phase

**run-048 (dual-track)**: Track A = benchmarks harness perf — **DONE** (A1-A3 PASS +
smoothing, merged to main). Track B = worldgen parity — B2 baseline PASS, B3 builder
DONE with strong numbers but **recall bar NOT met**; B3 critic IN FLIGHT (verdict
pending on the machine where it launched).

## Bars (unchanged)

- **Worldgen (human decision R43 — mechanism parity)**: same seeds/streams/algorithms as
  vanilla. Deterministic phases → 100% block match multi-seed; vegetation/sculk → same RNG
  stream 1:1. Do not edit measurement examples/tests to pass.
- **Benchmarks (run-047/048 Track A)**: multi-version provisioning, versioned report
  history + compare, builds green both workspaces, build/runtime measured. **MET.**

## Track A — benchmarks (`tests/benchmarks/`, own nightly workspace) — DONE

| Piece | Status | Commits (in main) |
| --- | --- | --- |
| A1 provisioning (multi-version, arch-aware pumpkin, fallback) | ✅ PASS | 36c13a4, 3b7b0ff |
| A2 versioned report history + compare over history | ✅ PASS | c0876c6, 16fb597, 519bc2e |
| A3 perf: exit-101 fix (OnceLock logger + LogPlugin disable), root gate `./bench`, measured times | ✅ PASS | 3e13bfe, 144b288, a77dc86, 12de2d9 (+smoothing 0067fe2, 522ecae, cc59594) |
| A4 smoothing | ✅ DONE | merged to main (ff) |

## Track B — worldgen parity (`crates/neutron-worldgen`) — B3 in progress

| Piece | Status | Evidence / commits |
| --- | --- | --- |
| B1 server review + B1b disconnect fix | ✅ PASS (run-047) | merged in main |
| B2 fresh 26.2 references + baseline (424242/12345/777) | ✅ PASS (blind critic) | `runs/run-048-evidence-baseline.txt` |
| B3 777 regression + lush/pale recall ≥80% | 🔨 builder DONE, **critic IN FLIGHT**; **bar NOT met (recall 57.43%)** | branch `run048-worldgen` @ eec1d1d (6 commits) |
| B4 smoothing | ⏳ after B3 closes | — |

## B3 results (builder-verified, critic verdict pending)

- **777 "regression" root-caused**: the ~99.4% historical claim was never reproducible
  (pre-U5 dc71940 = 96.32% vs fresh ref). Real bug fixed: `climate_at_block` used
  `peaksAndValleys` on ridge noise; vanilla 26.2 uses RAW ridge noise (probe −0.8113).
  → **777 96.29 → 98.29%**, lifts every seed.
- Lush/pale mechanism ports: vegetation_patch Java HashSet order, random_offset x,y,z,
  env_scan drop semantics, FeatureSorter indices 1:1. **Recall 53.03 → 57.43% (bar ≥80% NOT met).**
- Final: 424242 **97.36%** (≥97.28 ✓) · 12345 **97.81%** (≥97.75 ✓) · 777 **98.29%** (ratchet ✓) ·
  clay 411/493 · tests 59/59 + workspace green.
- Residual gap (B3 R2 target): trees 3338 + clay 1914 missing — claimed
  terrain/scheduler-coupled (free-height acceptance + tree RNG consumption depend on exact
  terrain + neighbor-first decoration order; ring-first experiment was worse 54.45%).
- Evidence: `runs/run-048-evidence-B3.txt`, `/tmp/rp-{424242,12345,777}-final.txt` (this PC).

## RESUME BOUNDARY (next PC — read first)

1. **Push is BLOCKED on PC-2 (this Linux box)**: no GitHub creds (HTTPS needs token, SSH
   key not registered, no gh CLI). Local-only commits: `main` @ **e3f52de**, branch
   `run048-worldgen` @ **eec1d1d** (NOT pushed). On the next PC: push or re-apply.
   If you continue on a PC WITHOUT these commits, re-derive from `runs/run-048.md` +
   `runs/run-048-evidence-B3.txt` + the workbench round log.
2. **B3 critic verdict is on PC-2** (async run 44205f88, session dir
   /tmp/pi-subagents-uid-1000/async-subagent-runs/44205f88-*): if you're on PC-2, read it
   (`subagent status` or the completion archive). Else, re-derive: re-run the 3
   region_parity examples + lush_pale_parity 424242 against the on-disk references and
   apply the same bar (ratchet + recall ≥80% → expect FAIL on recall).
3. **Runtime data does NOT travel** (gitignored): references (`tools/nbt-ref/vanilla-fresh-*`),
   jars (`tests/benchmarks/servers/vanilla/26.2/server.jar`), `target/`. Re-provision on the
   next PC: B2 recipe in `runs/run-048.md` (ref-extract with fresh tmp dir per seed +
   `--servers-dir` with `server-vanilla.jar`; Java 25 = JBR 25.0.2 works, no Adoptium needed).
4. **Next actions**:
   - Capture/re-derive the B3 critic verdict.
   - B3 R2: attack the residual lush/pale gap (trees + clay, neighbor-first decoration
     order hypothesis; ring-first already tried = worse). Ratchet all 3 seeds every round.
   - B4 smoothing → merge `run048-worldgen` into main → push → update workbench + STATE.
5. **Ownership**: LEAD owns STATE/workbench/runs/. A3 branch `run048-bench` is MERGED into
   main (clean). `tools/` = human-owned — the probe commits (e3f52de) are investigation
   artifacts (user-approved).

## Worldgen measurement status

- References on disk (PC-2): `tools/nbt-ref/vanilla-fresh-{424242,12345,777}/` (529 chunks
  each, hash-mode blocks, verified by B2 critic). 12345 spawn center = (6,-2); its (0,0)
  chunk is an air proto-chunk (invalid measurement target).
- Baseline (B2 PASS): REGION 424242 97.27% · 12345 97.79% · 777 96.29% · recall 53.03% ·
  clay 466 (vanilla 493). After B3: 97.36 / 97.81 / 98.29 / 57.43% / 411.

## System status

- **Tests**: 241 passed root workspace (47 protocol, 7 world, 24 server, 65 sim, 39 world, 59 worldgen) — verified on merged main after Track A.
- **Server**: `cargo run --release -p neutron-server -- --seed 12345 --view-distance 8` (B1 PASS in run-047).
- **F3**: FASE A ✅ B ✅ C ✅ D pending (not started).

## History (pointers — full details in each run file)

| Runs | Phase | Outcome |
| --- | --- | --- |
| run-000..043 | F0→F2d | harness → parity baseline → mechanism parity bar (R43) |
| run-044 | mechanism parity T1-T3 | ✅ aquifer/surface/sculk (blind-critic PASS) |
| run-045 | lush/pale dispatch | recall 11→49.6%; cross-chunk model isolated |
| run-046 | cross-chunk input model | U1 PASS; U5 R3 (777 regression, recall 62.94% claim — unverified) |
| run-047 | dual-track benchmarks + server/worldgen | A1/A2 PASS, B1/B1b PASS (merged); A3/B2/B3 pending |
| run-048 | resume on new PC | **ACTIVE — PAUSED for PC switch** |

## Key docs

- `AGENTS.md` — how we work (bar, gauntlet loop, tools)
- `ROADMAP.md` — phases, bars, prompt templates in `docs/prompts/`
- `workbench.md` — live round log for the active run
- `runs/run-048.md` — current run file with RESUME BOUNDARY + B2 recipe
- `runs/run-048-evidence-baseline.txt`, `runs/run-048-evidence-B3.txt` — B2/B3 evidence
- `crates/neutron-worldgen/WORLDGEN.md`, `WORLDGEN-PIPELINE.md`
- `crates/neutron-server/REVIEW.md` — server review evidence