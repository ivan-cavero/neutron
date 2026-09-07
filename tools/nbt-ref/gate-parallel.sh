#!/usr/bin/env bash
# gate-parallel.sh <tag> <seed>... — multi-seed parity gate, seeds in parallel.
#
# Same measurements as the gateN.sh one-shot scripts (/tmp/gate<tag>-<seed>.{json,log}),
# but faster two ways:
#   1. Builds target/release/parity ONCE upfront and invokes the binary
#      directly (no `cargo run` fingerprint overhead per seed).
#   2. Runs JOBS seeds concurrently via xargs -P, each with PARITY_WORKERS
#      threads (JOBS x WORKERS <= cores-2 keeps 2 cores free, same policy
#      as the in-process pool).
#
# Resumable: seeds with an existing /tmp/gate<tag>-<seed>.json are skipped.
#
# Usage:
#   bash tools/nbt-ref/gate-parallel.sh 7 424242 12345 777 123 456 789
# Defaults (8 cores): JOBS=2, WORKERS=3. Override: JOBS=3 WORKERS=2 bash ...
set -u

TAG=${1:?usage: gate-parallel.sh <tag> <seed>...}
shift
[ $# -ge 1 ] || { echo "usage: gate-parallel.sh <tag> <seed>..." >&2; exit 64; }

cd "$(dirname "$0")/../.." || exit 1
JOBS=${JOBS:-2}
WORKERS=${WORKERS:-3}

echo "gate$TAG: building parity binary once..."
cargo build --release -p neutron-parity 2>&1 | tail -1
BIN=target/release/parity
[ -x "$BIN" ] || { echo "gate$TAG: build failed" >&2; exit 1; }

run_one() {
    s=$1
    out=/tmp/gate$TAG-$s.json
    if [ -f "$out" ]; then
        echo "$s: already measured, skipping"
        return 0
    fi
    ref=tools/nbt-ref/vanilla-fresh-$s/world/dimensions/minecraft/overworld/region
    echo "=== $s $(date +%H:%M) ==="
    PARITY_WORKERS=$WORKERS "$BIN" --ref "$ref" --seed "$s" --scan 1 \
        --json "$out" --cache /tmp/parity-cache > /tmp/gate$TAG-$s.log 2>&1
    rc=$?
    if [ $rc -eq 0 ]; then
        python3 -c "
import json; d=json.load(open('$out')); b=d['blocks']['all']
print('$s:', round(b['pct'],4), b['mismatch'])"
    else
        echo "$s: FAILED rc=$rc (see /tmp/gate$TAG-$s.log)"
    fi
}
export -f run_one
export TAG BIN WORKERS

printf '%s\n' "$@" | xargs -P "$JOBS" -I{} bash -c 'run_one {}'
echo "GATE${TAG}_DONE"
