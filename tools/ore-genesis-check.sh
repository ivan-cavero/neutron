#!/usr/bin/env bash
# Ore genesis two-sided check: is a step-6 ore family a RNG/port bug or is it
# terrain-driven?
#
# Method (AGENTS.md: no hypothesis without a two-sided dump):
#   voor each side the FIRST blob genesis per origin is world-independent
#   (in_square x/z + trapezoid y + size nextDoubles). If the exact same
#   (x,y,z) comes out of the live jar and from Neutron for the SAME origin
#   and feature index, the port is draw-exact and any gap is terrain-driven
#   (discard_on_air_exposure responds to caves the two sides carve
#   differently). If genesis diverges, you have an RNG/order bug.
#
# Usage: tools/ore-genesis-check.sh <seed> <origin_block_x> <origin_block_z> <feature_index>
#   feature index = position in vanilla step-6 FeatureSorter list, e.g.
#   ore_dirt 0, ore_coal_upper 9, ore_coal_lower 10 (verify with:
#   java ... ProbeFeatureOrder 424242 | sed -n '/step 6/,/step 7/p').
#   NOTE: ProbeOreFlow's hand-written table is a SUBSET and its row order is
#   NOT the global sorter order for every row. Only indices that line up
#   (verified so far: 0 ore_dirt, 10 ore_coal_lower) give a meaningful
#   verdict; for others you must first confirm the probe row == global order.
set -eu
SEED=${1:?seed}
OX=${2:?origin block x}
OZ=${3:?origin block z}
IDX=${4:?feature index}
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
JAR="$ROOT/tools/nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar"
LIBS="$(find "$ROOT/tools/nbt-ref/vanilla-fresh-424242/libraries" -name '*.jar' | tr '\n' ':')"
OUT=/tmp/opencode/ore-genesis
mkdir -p "$OUT" "$OUT/probe-bin"

# probe side
javac -cp "$JAR:$LIBS" -d "$OUT/probe-bin" \
  "$ROOT/tools/worldgen-probe/src/ProbeOreFlow.java" 2>/dev/null
java -cp "$OUT/probe-bin:$JAR:$LIBS" ProbeOreFlow "$SEED" "$OX" "$OZ" "$IDX" 2>/dev/null \
  | head -8 > "$OUT/java.txt"

# neutron side: generates chunk at (ox/16, oz/16) with the trace on
cargo build --release -p neutron-parity --bin parity -q 2>/dev/null
( cd "$ROOT" && NEUTRON_ORE_TRACE=1 \
  NEUTRON_SCULK_ORIGIN_ORDER=canonical_pregen \
  ./target/release/parity --seed "$SEED" \
  --center "$((OX/16)),$((OZ/16))" --radius 0 2>&1 ) \
  | grep -E "^$IDX .* @\($OX,$OZ\)" | head -8 > "$OUT/rust.txt"

echo "=== JAVA (ProbeOreFlow) ==="
cat "$OUT/java.txt"
echo "=== RUST (our trace) ==="
cat "$OUT/rust.txt"
echo "---"
# Compare only the resolved genesis coords (formats differ: java has the
# feature name, rust carries the @origin suffix).
jd=$(grep -oE '^[0-9]+ [^ ]+ \([-0-9]+,[-0-9]+,[-0-9]+\)' "$OUT/java.txt" | head -1)
rd=$(grep -oE '^[0-9]+ \([-0-9]+,[-0-9]+,[-0-9]+\) @' "$OUT/rust.txt" | head -1)
echo "JAVA coords: $jd"
echo "RUST coords: $rd"
if [ -n "$jd" ] && [ -n "$rd" ]; then
  # strip "idx name " and parens
  jz=$(echo "$jd" | sed -E 's/^[0-9]+ [^ ]+ //' | tr -d '()')
  rz=$(echo "$rd" | sed -E 's/^[0-9]+ //' | grep -o '[-0-9]*,[-0-9]*,[-0-9]*')
  if [ "$jz" = "$rz" ]; then
    echo "VERDICT: FIRST GENESIS MATCHES ($jz) -> ore port draw-exact;"
    echo "         residual gap is TERRAIN-DRIVEN."
    echo "         (discard_on_air_exposure reacts to caves the two sides carve"
    echo "         differently = carver/BASE divergence. Fix terrain, not ore.)"
    exit 0
  fi
fi
echo "VERDICT: first genesis DIVERGES -> RNG/order bug in the ore path."
echo "         Dump: $OUT/java.txt vs $OUT/rust.txt"
exit 1