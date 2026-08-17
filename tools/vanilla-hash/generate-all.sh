#!/usr/bin/env bash
# generate-all.sh — Generate reference data for multiple seeds and server types.
#
# Usage: ./generate-all.sh [seed1 seed2 ...]
# If no seeds are given, uses default set: 12345 67890 11111 99999 42
#
# Requires: cargo, java, and server jars in bench/servers/ (gitignored):
#   server-vanilla.jar, server-paper.jar, server-folia.jar

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/hashes"
SERVERS_DIR="$REPO_ROOT/bench/servers"

# Default seeds if none provided
if [ $# -eq 0 ]; then
    SEEDS=(12345 67890 11111 99999 42)
else
    SEEDS=("$@")
fi

SERVERS=(vanilla)
# Only include paper/folia if their jars exist
[ -f "$SERVERS_DIR/server-paper.jar" ] && SERVERS+=(paper)
[ -f "$SERVERS_DIR/server-folia.jar" ] && SERVERS+=(folia)

# Check java
if ! command -v java &>/dev/null; then
    echo "ERROR: java not found in PATH" >&2
    exit 1
fi

# Check cargo
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found in PATH" >&2
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "=== Reference Data Generation ==="
echo "Seeds: ${SEEDS[*]}"
echo "Servers: ${SERVERS[*]}"
echo "Output: $OUTPUT_DIR"
echo ""

cd "$REPO_ROOT"

# Build once
echo "--- Building vanilla-hash ---"
cargo build -p vanilla-hash --release
echo ""

FAILED=0
for server in "${SERVERS[@]}"; do
    for seed in "${SEEDS[@]}"; do
        OUTPUT_FILE="$OUTPUT_DIR/${server}-${seed}-blocks.json"
        echo "--- Generating: server=$server seed=$seed ---"

        if cargo run -p vanilla-hash --release -- \
            --seed "$seed" \
            --server "$server" \
            --servers-dir "$SERVERS_DIR" \
            --hash-mode blocks \
            --output "$OUTPUT_FILE"; then
            echo "  OK: $OUTPUT_FILE"
        else
            echo "  FAILED: server=$server seed=$seed" >&2
            FAILED=$((FAILED + 1))
        fi
        echo ""
    done
done

echo "=== Done ==="
echo "Generated files in $OUTPUT_DIR:"
ls -la "$OUTPUT_DIR"/*.json 2>/dev/null || echo "  (none)"

if [ "$FAILED" -gt 0 ]; then
    echo "WARNING: $FAILED extraction(s) failed" >&2
    exit 1
fi