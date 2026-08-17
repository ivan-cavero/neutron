# Neutron server — review note for the next units

Status of the server-facing bars as of this commit (branch `ivan-cavero/server-worldgen`).
Evidence files live in `logs/b1-relaunch/` (boot3.log, join2.log, join3.log, status_ping.out).

## What works

- **Boot**: clean startup with the `Done` marker. `2026-08-17T16:38:04.741315Z INFO Done (0.0s)!`
  (logs/b1-relaunch/boot3.log). Worldgen NoiseRouter builds in ~2-4 ms.
- **Status ping**: full 26.2 server-list ping round-trips. `STATUS PING OK` — decoded JSON
  reports `version 26.2 / protocol 776`, `players max=20 online=0`, description
  `"Neutron — live worldgen"`, and the pong echoes the ping payload exactly
  (logs/b1-relaunch/status_ping.out). Harness: `tests/e2e-server/status_ping.py`.
- **Join to Play with real chunks**: protocol-level login (handshake → login → configuration →
  play) succeeds; login finished in 0.4 ms, 29 RegistryData packets, SyncPosition, CenterChunk,
  then real level-chunk data. Best run (logs/b1-relaunch/join2.log, RESULT: PASS):
  - `Chunks received: 21`, `Time to first chunk: 602.7ms` (warm), first chunk 59 477 bytes.
  - Packets per 10s window `[21, 3, 4, 3, 2]` — the burst is the initial chunk batch.
- **TPS**: the tick loop now holds ~20 TPS. Measured every 200 ticks (logs/b1-relaunch/boot3.log,
  81 samples over ~13.5 min incl. a client session): min 19.97, max 20.03, bulk 19.99-20.01.

## What was broken and fixed

- **TPS drift (server bug)**: the tick loop used `tokio::time::sleep(50ms)`, which drifts: each
  sleep starts after the previous tick's work, and on Windows tokio timers resolve to the next
  ~15.6 ms boundary (~62 ms per tick). Measured 16.07-16.10 TPS in the pre-fix runs
  (logs/b1/boot.log, `tick=200 tps="16.09"` at 16:01:13). **Fix**: `tokio::time::interval(50ms)`
  reschedules each tick relative to the previous deadline, so slow bodies and coarse timers cannot
  drag the long-run rate down (crates/neutron-server/src/tick.rs). Post-fix: 20.01 TPS with
  client load (boot3.log `tick=400 tps="20.01"` at 16:38:24 while SmokeBot was connected).
  A `tick rate` INFO line now reports measured TPS every 200 ticks.
- **Probe decode errors (harness bug, not server)**: early join runs failed with hundreds of
  `packet too short for header` decode errors (logs/b1/join1.log, RESULT: FAIL). Root cause was
  harness-side: the read buffer was zero-filled and NOT truncated on read timeout, so a partial
  frame at the end of a buffer merged with the next read into garbage. **Fix**: truncate the
  buffer to consumed bytes on timeout (tests/e2e-server/main.rs). Post-fix runs decode cleanly
  (join2.log/join3.log, RESULT: PASS, zero decode errors).

## Still flaky / open

- **Cold first-chunk latency is worldgen speed (B3's domain, crates/neutron-worldgen)**: first
  chunk takes 13.1 s cold (join3.log `Time to first chunk: 13144.4ms`, first run 16.3 s) vs
  602.7 ms warm (join2.log). The long cold time is the worldgen worker cold path; the warm-time
  difference (13.1 s vs 0.6 s) is the dominant user-visible cost of a join.
- **Chunk count varies run to run** (21 in join2 vs 7 in join3): depends on how far the
  radius/view-distance progression gets during the session. Not a regression, just timing.
- **Unknown/unhandled play packets**: one each of `0x26`, `0x40`, `0x61` during a session
  (join2.log/join3.log). Harmless so far (likely damage/entity/actionbar-ish), but unhandled.
- **`failed to send keepalive ... channel closed` WARN spam** in server logs when a client
  disconnects mid-session (logs/b1/boot.log). Cosmetic; could be downgraded to debug.
- **Harness TPS estimate**: the e2e harness's keepalive-cadence TPS estimate still reads 16.07 —
  that number was measured against the old drifting loop; re-run against the fixed server if the
  estimate matters (the server's own `tick rate` line is authoritative).
- **Worldgen is "not 1:1 yet"** (server chat on join) — live worldgen, not final terrain.
- **Harness workspace**: `tests/e2e-server` is its OWN workspace (empty `[workspace]` in its
  Cargo.toml), not a member of the root workspace, so `cargo test --workspace` at the root does
  not cover it. Build/run it via `cargo run --manifest-path tests/e2e-server/Cargo.toml`. It
  depends on `neutron-protocol` by path and compiles cleanly.

## How to re-run

```bash
# unit + integration tests (root workspace)
cargo test --workspace

# e2e harness (own workspace)
cargo run --manifest-path tests/e2e-server/Cargo.toml -- join --duration 60
python tests/e2e-server/status_ping.py --port 25565
```