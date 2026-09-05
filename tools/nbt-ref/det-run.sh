#!/usr/bin/env bash
# det-run.sh <tag> — determinism experiment: ONE vanilla fresh generation.
#
# Fixed procedure, identical for every tag: seed 424242, boot headless,
# forceload the canonical 16x16 square, settle 150 s, save-all flush, stop.
# Run twice (`det-run.sh a`, then `det-run.sh b`) and diff the two worlds
# with: parity vanilladiff --a <A>/world/.../region --b <B>/world/.../region
# If A == B cell-for-cell, vanilla decoration is deterministic in practice
# and 100% parity is achievable. Any diff = true scheduler nondeterminism.
set -euo pipefail

TAG=${1:?usage: det-run.sh <a|b>}
SEED=424242
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
rcon.port=$RCON_PORT
rcon.password=neutron-det
enable-rcon=true
EOF

nice -n 10 java -Xmx3G -jar server.jar nogui > server-$TAG.out 2>&1 &
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
rcon "forceload add -128 -128 127 127"
sleep 150
rcon "save-all flush"
sleep 10
kill -INT $PID 2>/dev/null || true
sleep 10
kill -9 $PID 2>/dev/null || true

echo "det-$TAG: DONE world=$ROOT/world/dimensions/minecraft/overworld/region"
