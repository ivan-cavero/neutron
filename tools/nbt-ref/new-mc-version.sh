#!/usr/bin/env bash
# new-mc-version.sh <mcversion> <seed> [name]
#
# Provisions a fresh vanilla reference world for parity measurement.
# Produces the same layout region_parity / worldgen-probe expect:
#   tools/nbt-ref/<name>/versions/<mcversion>/server-<mcversion>.jar
#   tools/nbt-ref/<name>/libraries/
#   tools/nbt-ref/<name>/world/dimensions/minecraft/overworld/region/
# A Minecraft mega-update = run this once with the new version, then rerun:
#   cargo run -r -p neutron-worldgen --example region_parity -- <seed> 0 0 1 <regiondir>
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
EOF

# Boot; SIGINT at "Done" saves the world cleanly. Fresh spawn area covers the
# center chunks we measure ((0,0) and neighbors).
java -jar server.jar nogui &
PID=$!
tail -F logs/latest.log 2>/dev/null | grep -q -m1 'Done (' && kill -INT $PID || true
wait $PID || true

echo "ref world ready: $ROOT/world/dimensions/minecraft/overworld/region"
