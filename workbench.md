# F0 — Fundamentos y harness · gauntlet workbench

**Bar (verbatim from ROADMAP.md):**
> un agente distinto al builder ejecuta `bench/run.ps1` desde cero en Windows y Linux y reproduce el baseline B0 (vanilla 26.2 / Paper / Pumpkin); 10 bots de join simultáneo sin kicks (p95 < 5 s); cps ±30% consistente con baselines publicados.

**Budget:** 3-5 rondas · ~1-2 semanas · wall-clock: until bar clears
**Status:** Round 5 · 6 ago 2026 · **Code fixes for smoother bugs in progress**

## Units

| # | Unit | Independent? | Bar (per unit) |
|---|------|-------------|----------------|
| U1 | `bench/run.ps1` + `bench/run.sh` — orchestration harness | No (share structure) | Script levanta servidor → bota joins → miden cps → produce JSON + tabla |
| U2 | `bench/bots/join-bench/` — bot script multi-conexión | No (same dir) | 10 bots simultáneos conectan sin kick, p95 join < 5 s |
| U3 | `bench/README.md` — docs del harness | No (references U1/U2) | Cubre setup, ejecución, interpretación, referencias de baselines |

## Round log

| Round | U1 harness | U2 bots | U3 docs | Notes |
|-------|-----------|---------|---------|-------|
| 1 | FAIL (3 critical + 8 major) | FAIL (5 high/medium) | — | Harness dead pipeline; unit inconsistency |
| 2 | FAIL (1 bash bug + code-smells) | FAIL (1 comment inaccuracy) | — | Fixing critic findings |
| 3 | PASS ✅ | PASS ✅ | → IN PROGRESS | U1/U2 converged; U3 in progress |
| — | PASS ✅ (R2) | PASS ✅ (R2) | FAIL (4 FAIL + 5 WARN) | U3 docs need more depth |
| 4 | — | — | FAIL (4 FAIL + 5 WARN) | Docs critic rejects |
| 5 | FAIL (2 smoother bugs) | — | FAIL | Smoother found: no `--version` to bot, `* 1000` converts ms→ms×1000 |
| 6 | PASS ✅ (smoother fixes verified) | — | PASS ✅ (5 doc fixes) | U1/U2/U3 all converge — ready for final smoother |

### Round 1 — U1 FAIL details
- **Critical:** `--latency-file` arg never recognized by bot (should be `--output`); bot output JSON format doesn't match harness parser; latency values in seconds but expected in ms
- **Major:** Memory watcher 30s < warmup 60s; no TPS/cps measurement; bash seed precision; bash `input` vs slurp; missing memory watcher and per-run markdown

### Round 1 — U2 FAIL details
- **High:** Unit inconsistency — p50/p95/p99 in seconds, perBot in ms
- **High:** Graceful shutdown broken
- **Medium:** Meaningless 5s warmup
- **Medium:** Default version "26.2" may not be valid mineflayer data

### Round 2 — U1 FAIL (1 issue)
- `${run_idx+1}` parameter expansion bug → always showed "Run 1" in markdown

### Round 2 — U2 FAIL (1 issue)
- Stagger comment said "~20ms" but math gives 18ms for 10 bots

### Round 3 — PASS (U1 + U2)
- U1: `${run_idx+1}` → `$((run_idx + 1))` ✅
- U2: "~20ms" → "~18ms" ✅

### Round 4 — U3 FAIL (4 FAIL + 5 WARN)
- C2ME confusion in server column, Rust build time undocumented, --world no example, CI example broken

### Round 5 — Smoother findings (critical code bugs)
1. **Harness never passes `--version` to bot** → bots use wrong protocol version (1.21.11 instead of 26.2)
2. **Harness multiplies latency by 1000** → bot already outputs ms, so values are 1000× too large
3. README claims `level-name = <run-id>` but code writes full path (minor)
4. README shows `per_run.txt` as universal but it's bash-only (minor)

## Open questions for the human

- **Rust installed?** Neutron requires `cargo build --release -p neutron-cli` (5-15 min first build)
- **Minecraft servers available?** For the actual B0 bar, we need vanilla 26.2 / Paper / Pumpkin binaries
- **Node.js + Java 25?** Required to run the harness

---
*Last updated: 6 ago 2026 — Round 6 in progress (fixing smoother bugs)*