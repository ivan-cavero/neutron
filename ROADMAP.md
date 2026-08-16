# Neutron — Roadmap

> Progress is measured in **BARS and ROUNDS**, not calendar time. A bar is a real,
> non-negotiable reference (checksum, benchmark, real server) that a blind critic
> inspects (Gauntlet Loop — see `AGENTS.md` §2).

## 0. How to read

- **Bar**: what the critic compares our artifact against (each phase's "Call of Duty").
  Not negotiable: met or not.
- **Rounds**: build → critic → fix cycles. No arbitrary cap: iterate until the bar wins,
  2 rounds without improvement, or budget exhausted.
- **Prompt**: each phase has a prompt template in `docs/prompts/` — copy-paste into pi.
  It details tasks to distribute via `subagent`, research via `web_search`, tracking via
  `todo`, and human gates via `ask_user_question`.
- **How to launch a run**: see `runs/README.md` (single source of truth).

## 1. Mojang cadence (verified — ARCHITECTURE.md Annex A §1)

| Version | Type | Date |
| --- | --- | --- |
| 1.21.11 "Mounts of Mayhem" | last 1.x (obfuscated jar, Java 21) | 9 Dec 2025 |
| 26.1 "Tiny Takeover" | unobfuscated jar, Java 25 | 24 Mar 2026 |
| 26.2 "Chaos Cubed" | **`main` target version today** | 16 Jun 2026 |
| 26.3 | in snapshots | Q3 2026 |
| ~26.x | ~3 drops/year + hotfixes | continuous |

> **D0-D4 pipeline**: when Mojang releases a new version, follow §4 to update `main` in ≤7 days.

## 2. Phases

| Phase | Status | Objective | Bar (summary) | Prompt |
| --- | --- | --- | --- | --- |
| F0 — Foundations & harness | ✅ COMPLETE | repo infra + first public baseline | harness reproducible on Win/Linux | — |
| F1 — Playable core | ✅ mostly | real player joins, plays, world persists in vanilla Anvil | bot 26.2 plays 10 min no kick; world opens in vanilla; fuzz 1M no panic; startup <2s; RAM <200MB | `docs/prompts/f1.md` |
| F2 — Worldgen parity 1:1 | 🔄 superseded by F2d | same seed → same world by checksum | xxHash64 identical on 50 golden seeds; cps >250 @16 threads | `docs/prompts/f2.md` |
| F2d — Byte-identical parity | 🔄 **ACTIVE** (run-046) | close remaining gaps for same-seed identical chunks | mechanism parity (human R43): same seeds/streams/algorithms; deterministic phases → 100% block match multi-seed | `docs/prompts/f2d.md` |
| F3 — Vanilla simulation | 🟡 A/B/C ✅, D pending | blocks, fluids, light, redstone, spawns, survival | positional golden suite 100% vs real vanilla; light arrays identical on 50 seeds | `docs/prompts/f3.md` |
| F4 — Scale 500-1000+ | ⏳ not started | 500 stable players; path to 1000+ | 500 bots 60 min → TPS 20.0, p99 tick <25ms; RAM/player <1MB | `docs/prompts/f4.md` |
| F5 — Mobs & AI | ⏳ not started | vanilla mob behavior + combat | E2E 20 min survival; spot-checks (creeper, zombie burn, enderman TP, dragon); 50 mobs/chunk no TPS regression | `docs/prompts/f5.md` |
| F6 — WASM + Lua plugins | ⏳ not started | secure-by-construction ecosystem | WASM panic doesn't kill server; fuel 10M opcodes; hot reload; 3 Bukkit conversions; <5µs/tick | `docs/prompts/f6.md` |
| F7 — Bedrock | ⏳ not started | Bedrock clients in the same world | real Bedrock client plays 10 min; Java+Bedrock coexist; no TPS impact | `docs/prompts/f7.md` |
| F8 — 1.0 | ⏳ not started | stable, verifiable, defensible release | full parity suite green on `main`; reproducible benchmarks; 72h uptime 100 players; 24h fuzz clean | `docs/prompts/f8.md` |

## 3. Timeline (est. rounds)

| Phase | Est. rounds | Can run parallel with |
| --- | --- | --- |
| F0 | 3-5 | — |
| F1 | 5-8 | — |
| F2 | 8-12 | — |
| F3 | 10-16 | F4 |
| F4 | 6-10 | F3 |
| F5 | 10-16 | F6, F7 |
| F6 | 8-12 | F5 |
| F7 | 6-10 | F5 |
| F8 | 6-10 | — |

Recalibrate after each phase based on real speed.

## 4. Version pipeline D0-D4 (SLA: `main` ≤7 days after Mojang release)

| Day | Step | Tool | Verification |
| --- | --- | --- | --- |
| D0 | Detect Mojang release | webhook/CI | — |
| D1 | Extract jar: registries, protocol, worldgen, assets | `tools/mc-extract` | diff vs previous; minecraft-data |
| D2 | Codegen → typed Rust | `tools/codegen` | clean `cargo check` |
| D3 | Regenerate golden data (chunks per seed, contraptions) | harness | xxHash64 checksums |
| D4 | Parity suite + benchmarks + release `main` | CI + human gate | parity 100% |

## 5. Risks & mitigations

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Redstone parity (positional, QC, 1.21.2+) | CRITICAL | positional golden suite from F3-A; against real server |
| Mob AI parity | HIGH | port from unobfuscated jar; automated spot-checks |
| Bukkit compat expectations | HIGH | honest layered strategy (F6); communication |
| Scale 1000+ | MEDIUM-HIGH | regional scheduler A/B (F4); continuous stress |
| Mojang cadence | MEDIUM | D0-D4 pipeline; regression tests |
| Scope creep | HIGH | bar per phase; separate backlog |
| Agent cost (tokens) | MEDIUM | budget guardrails; kill-switch; lean STATE.md |

## 6. Out of scope

Combat 1.8 · Forge/Fabric mods · 100% Bukkit plugins (layered only) · custom minigames ·
client FPS (see BENCHMARKS.md)
