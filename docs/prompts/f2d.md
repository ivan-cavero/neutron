# F2d — Byte-identical parity (ACTIVE) — LEAD prompt

> Paste into pi. Read `STATE.md`, `runs/README.md`, `workbench.md`, and the latest
> `runs/run-NNN.md` first. Bar (human decision R43): **mechanism parity**.

## Context to read

- `tools/vanilla-extract/PARAMETERS.md` — exact vanilla parameters from the decompiled jar
- `crates/neutron-worldgen/src/` — current code
- `tools/parity-check/src/main.rs` — comparison tool
- `runs/run-046.md` — active run (cross-chunk input model)
- `STATE.md` — current state

## Bar (acceptance criteria)

- [ ] Same seeds/streams/algorithms as vanilla; deterministic phases → 100% block match multi-seed
- [ ] Vegetation/sculk → same RNG stream 1:1
- [ ] **Ratchet**: no regression on ANY seed (12345/424242/777)
- [ ] `cargo test --workspace` green; worldgen 59/59
- [ ] Bar untouchable: do not edit measurement examples/tests to pass

## Tasks (split into gradeable units, launch in parallel via subagent)

Each task: What (measurable) / AC (thresholds + ratchet) / Evidence (raw logs, hashes) /
DoD (what the blind critic runs from scratch).

Active gaps (run-046 R3): lush/pale recall 62.94% (bar ≥80%) — moss_block 1218,
cave_vines 735, clay missing; border diffs -7.5% (bar ≥30% down); **777 regression
96.29% (isolate: U5 model vs R3 trees)**.

## Gauntlet Loop

1. Each task is built against the bar.
2. Blind critic (`subagent`, clean context) verifies with real evidence (parity output,
   logs, tests it runs itself). Default REJECT.
3. FAIL → the single biggest gap → fix → repeat.
4. Every round re-measures ALL seeds (ratchet).
5. PASS only with blind-critic evidence; builder-verified is never PASS.

## Output

- `crates/neutron-worldgen/src/` updated
- `runs/run-NNN.md` with evidence + rounds
- `workbench.md` round log updated
- `STATE.md` updated (state, not history)
