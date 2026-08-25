#!/usr/bin/env bash
# new-mc-version.sh <mcversion> <seed> [name]
#
# Provisions a fresh vanilla reference world for parity measurement.
# Produces the same layout region_parity / worldgen-probe expect:
#   tools/nbt-ref/<name>/versions/<mcversion>/server-<mcversion>.jar
#   tools/nbt-ref/<name>/libraries/
#   tools/nbt-ref/<name>/world/dimensions/minecraft/overworld/region/
#
# PREGNATION PROCEDURE (canonical — the decoration order embedded in the ref
# depends on HOW chunks were loaded, and neutron's origin order approximates
# the vanilla ticket wavefront of THIS procedure; do not change it casually):
#   1. Boot headless with RCON enabled, pause-when-empty-seconds=0.
#   2. `forceload add` ONE centered square (<=256 chunks — vanilla caps the
#      command area) around spawn. Tickets propagate outward from its center,
#      so chunks decorate in a center-first wavefront.
#   3. Wait for generation to settle, then add one surrounding RING via four
#      side strips (each <=256 chunks). Ring chunks see the inner square as
#      their decoration history — concentric squares preserve the wavefront.
#   4. save-all flush + stop.
set -euo pipefail

V=${1:?usage: new-mc-version.sh <mcversion> <seed> [name]}
SEED=${2:?usage: new-mc-version.sh <mcversion> <seed> [name]}
NAME=${3:-vanilla-fresh-$SEED}
ROOT=tools/nbt-ref/$NAME

mkdir -p "$ROOT"
cd "$ROOT"

META=$(curl -fsSL https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)
VURL=$(echo "$META" | python3 -c "import json,sys;print(next(e['url'] for e in json.load(sys.stdin)['versions'] if e['id']=='$V'))")
JURL=$(curl -fsSL "$VURL" | python3 -c "import json,sys;print(json.load(sys.stdin)['downloads']['server']['url'])")

curl -fSL -o server.jar "$JURL"
sha256sum server.jar   # pin this in STATE.md as the ref baseline

echo 'eula=true' > eula.txt
cat > server.properties <<EOF
level-seed=$SEED
max-tick-time=-1
sync-chunk-writes=false
view-distance=10
pause-when-empty-seconds=0
rcon.port=25575
rcon.password=neutron-ref
enable-rcon=true
EOF

java -Xmx6G -jar server.jar nogui &
PID=$!
tail -F logs/latest.log 2>/dev/null | grep -q -m1 'Done (' || true

rcon() {
  python3 - "$@" <<'PYEOF'
import socket, struct, sys
def pkt(rid, ptype, payload):
    body = struct.pack('<ii', rid, ptype) + payload.encode() + b'\x00\x00'
    return struct.pack('<i', len(body)) + body
s = socket.create_connection(('127.0.0.1', 25575), timeout=20)
s.settimeout(15)
s.sendall(pkt(1, 3, 'neutron-ref')); s.recv(4096)
for cmd in sys.argv[1:]:
    s.sendall(pkt(2, 2, cmd))
    try: s.recv(4096)
    except Exception: pass
s.close()
PYEOF
}

# Concentric centered squares: inner 16x16-chunk square (block area exactly
# 256 chunks, at the command cap), wait, then one outer ring in 4 strips
# (each <=256 chunks). Full coverage: chunks x,z in [-12, +11].
rcon "forceload add -128 -128 127 127"
sleep 150
rcon "forceload add -192 -192 -161 191" \
     "forceload add 160 -192 191 191" \
     "forceload add -160 -192 159 -161" \
     "forceload add -160 160 159 191"
sleep 240
rcon "save-all flush"
sleep 10
kill -INT $PID 2>/dev/null || true
sleep 10
kill -9 $PID 2>/dev/null || true

echo "ref world ready: $ROOT/world/dimensions/minecraft/overworld/region"
