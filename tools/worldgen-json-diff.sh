#!/usr/bin/env bash
# worldgen-json-diff.sh <old-server.jar> <new-server.jar> [outdir]
#
# UPDATE TRIAGE — extract every worldgen datapack JSON from two vanilla
# server jars and print a unified diff. This answers "what did Mojang change"
# at the DATA level in one command: placed/configured features, noise
# settings, density functions, biome params and block tags are all plain
# JSON inside the jar (data/minecraft/worldgen, data/minecraft/tags/block).
#
# Typical update day:
#   1. new-mc-version.sh <newver> <seed>          # canonical ref world
#   2. worldgen-json-diff.sh old.jar new.jar      # THIS script — what changed
#   3. For every changed JSON: update the embedded copy under
#      crates/neutron-worldgen/src/data/ and re-derive the Rust port if the
#      referenced Java class changed (see docs/PARITY.md component map).
#   4. PARITY_SCAN=1 region_parity -> ledger -> fix by gap ranking.
set -euo pipefail

OLD=${1:?usage: worldgen-json-diff.sh <old.jar> <new.jar> [outdir]}
NEW=${2:?usage: worldgen-json-diff.sh <old.jar> <new.jar> [outdir]}
OUT=${3:-/tmp/opencode/worldgen-json-diff}

rm -rf "$OUT"
mkdir -p "$OUT/old" "$OUT/new"

for side in old new; do
  jar=${side/old/$OLD}; jar=${side/new/$NEW}
  unzip -qq -o "$jar" 'data/minecraft/worldgen/*' 'data/minecraft/tags/block/*' -d "$OUT/$side"
done

echo "== changed / added / removed files =="
diff -qr "$OUT/old/data" "$OUT/new/data" || true

echo
echo "== content diffs =="
diff -ru "$OUT/old/data" "$OUT/new/data" || true
