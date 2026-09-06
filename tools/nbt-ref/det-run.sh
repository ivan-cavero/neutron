#!/usr/bin/env bash
# det-run.sh <tag> — determinism experiment: ONE vanilla fresh generation.
#
# Fixed procedure, identical for every tag: seed 424242, boot headless,
# forceload the canonical 16x16 square, settle, save-all flush, stop.
# Run twice (`det-run.sh a`, then `det-run.sh b`) and diff the two worlds
# with: parity vanilladiff --a <A>/world/.../region --b <B>/world/.../region
# If A == B cell-for-cell, vanilla decoration is deterministic in practice
# and 100% parity is achievable. Any diff = true scheduler nondeterminism.
#
# Env overrides:
#   DET_JAVA_FLAGS  extra JVM flags (e.g. -Dmax.bg.threads=1 for serialized
#                   decoration = deterministic by construction; use a distinct
#                   TAG like c1/c2 for those runs)
#   DET_SETTLE_SECS settle wait after forceload (default 150; single-threaded
#                   runs need ~600)
set -euo pipefail

TAG=${1:?usage: det-run.sh <a|b|c1|c2>}
SEED=424242
SETTLE=${DET_SETTLE_SECS:-150}
# NOTE: `vanilla-det-*` matches .gitignore (like vanilla-fresh-*).
ROOT=tools/nbt-ref/vanilla-det-$TAG
SRC=tools/nbt-ref/vanilla-fresh-424242
RCON_PORT=25575

mkdir -p "$ROOT"
cd "$ROOT"
# Reuse the pinned jar + libraries (no re-download). Boot via the root
# bundler server.jar exactly like the canonical setup (inner jar alone
# misses manifest Class-Path libraries).
ln -sfn "$(cd ../vanilla-fresh-424242 && pwd)/server.jar" server.jar
ln -sfn "$(cd ../vanilla-fresh-424242 && pwd)/libraries" libraries
ln -sfn "$(cd ../vanilla-fresh-424242 && pwd)/versions" versions

echo 'eula=true' > eula.txt
cat > server.properties <<EOF
level-seed=$SEED
max-tick-time=-1
sync-chunk-writes=false
view-distance=10
pause-when-empty-seconds=0
function-permission-level=2
rcon.port=$RCON_PORT
rcon.password=neutron-det
enable-rcon=true
EOF
# DET_TICKDRIVEN=1: game-time-driven generation. A datapack tick function
# applies gamerules at tick 1 and the forceload square at tick 20 — no RCON
# during generation at all (RCON only for save-all/stop after settling, which
# cannot affect already-decorated blocks). Pre-placed before boot so the
# fresh world picks it up.
if [ -n "${DET_TICKDRIVEN:-}" ]; then
  mkdir -p world/datapacks/det/data/det/function world/datapacks/det/data/minecraft/tags/function
  cat > world/datapacks/det/pack.mcmeta <<'EOF2'
{"pack": {"pack_format": 107, "description": "det tick driver"}}
EOF2
  cat > world/datapacks/det/data/det/function/tick.mcfunction <<'EOF2'
scoreboard objectives add det_t dummy
scoreboard players add $c det_t 1
execute if score $c det_t matches 1 run gamerule random_tick_speed 0
execute if score $c det_t matches 1 run gamerule spawn_mobs false
execute if score $c det_t matches 1 run gamerule advance_weather false
execute if score $c det_t matches 1 run weather clear
execute if score $c det_t matches 20 run forceload add -128 -128 127 127
EOF2
  cat > world/datapacks/det/data/minecraft/tags/function/tick.json <<'EOF2'
{"values": ["det:tick"]}
EOF2
fi

nice -n 10 java -Xmx3G ${DET_JAVA_FLAGS:-} -jar server.jar nogui > server-$TAG.out 2>&1 &
PID=$!
echo "det-$TAG: server pid $PID"

tail -F logs/latest.log 2>/dev/null | grep -q -m1 'Done (' || true
echo "det-$TAG: booted"

rcon() {
  python3 - "$@" <<PYEOF
import socket, struct, sys
def pkt(rid, ptype, payload):
    body = struct.pack('<ii', rid, ptype) + payload.encode() + b'\x00\x00'
    return struct.pack('<i', len(body)) + body
s = socket.create_connection(('127.0.0.1', $RCON_PORT), timeout=20)
s.settimeout(15)
s.sendall(pkt(1, 3, 'neutron-det')); s.recv(4096)
for cmd in sys.argv[1:]:
    s.sendall(pkt(2, 2, cmd))
    try: s.recv(4096)
    except Exception: pass
s.close()
PYEOF
}

# Canonical square only (matches the canonical ref core; no ring — the
# A-vs-B question needs identical procedures, not maximal coverage).
# Freeze post-generation simulation so the diff measures GENERATION order
# only: randomTickSpeed 0 stops vine growth/leaf decay/grass spread/snow
# melt in ticking chunks; doWeatherCycle false + clear stops snowfall.
# DET_NOFORCELOAD=1: skip forceload (spawn area self-generates; removes
# RCON arrival-tick variance from the generation phase entirely — the
# strongest closed-system determinism test).
# (Skipped entirely in DET_TICKDRIVEN mode: the datapack owns gamerules.)
if [ -z "${DET_TICKDRIVEN:-}" ]; then
rcon "gamerule randomTickSpeed 0" "gamerule doWeatherCycle false" "doMobSpawning false" "weather clear"
fi
# DET_PREWAIT: sleep AFTER gamerules, BEFORE forceload — lets boot-time
# spawn generation drain while the system is otherwise idle, so the
# forceload ticket burst lands on a quiescent dispatcher (tests whether
# the race is the forceload-vs-boot-gen macro perturbation).
if [ -n "${DET_PREWAIT:-}" ]; then
  sleep "$DET_PREWAIT"
fi
if [ -z "${DET_NOFORCELOAD:-}${DET_TICKDRIVEN:-}" ]; then
  rcon "forceload add -128 -128 127 127"
fi
sleep "$SETTLE"
rcon "save-all flush"
sleep 10
kill -INT $PID 2>/dev/null || true
sleep 10
kill -9 $PID 2>/dev/null || true

echo "det-$TAG: DONE world=$ROOT/world/dimensions/minecraft/overworld/region"
