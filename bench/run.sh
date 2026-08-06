#!/usr/bin/env bash
# run.sh — Neutron benchmark harness (bash / Linux / macOS)
#
# Usage:
#   ./bench/run.sh vanilla -n 10 --runs 5
#   ./bench/run.sh paper   -n 10 --runs 5
#   ./bench/run.sh folia   -n 10 --runs 5
#   ./bench/run.sh pumpkin -n 10 --runs 5
#   ./bench/run.sh neutron -n 10 --runs 5
#
# Requirements:
#   - Node.js (for join-bench bots, up to 1.21.11)
#   - Rust (for azalea bots, 26.x)
#   - Java 25 (for vanilla/paper/folia servers)
#   - Server binary in bench/servers/<type>/
#   - jq (optional, for JSON output)

set -euo pipefail

# Defaults
SERVER=""
N=10
RUNS=5
SEED_STR="1234567890123456789"
WORLD_DIR=""
RESULTS_DIR=""
LOG_DIR=""
WARMUP_SEC=60
MEM_WATCH_SEC=90

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        vanilla|paper|folia|pumpkin|neutron)
            SERVER="$1"; shift ;;
        -n|--bots) N="$2"; shift 2 ;;
        --runs)    RUNS="$2"; shift 2 ;;
        --seed)    SEED_STR="$2"; shift 2 ;;
        --world)   WORLD_DIR="$2"; shift 2 ;;
        --results-dir) RESULTS_DIR="$2"; shift 2 ;;
        --log-dir) LOG_DIR="$2"; shift 2 ;;
        --warmup)  WARMUP_SEC="$2"; shift 2 ;;
        --mem-watch) MEM_WATCH_SEC="$2"; shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [[ -z "$SERVER" ]]; then
    echo "Usage: $0 <vanilla|paper|folia|pumpkin|neutron> [-n N] [--runs N] [--seed N]"
    exit 1
fi

# Paths
SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_DIR="$SCRIPT_ROOT"
SERVERS_DIR="$BENCH_DIR/servers"
BOTS_DIR="$BENCH_DIR/bots"
BENCH_JOIN="$BOTS_DIR/join-bench"
AZALEA_BIN="$BOTS_DIR/azalea-join-bench/target/release/azalea-join-bench"

DEFAULT_RESULTS_DIR="$BENCH_DIR/results"
DEFAULT_LOG_DIR="$BENCH_DIR/logs"
RESULTS_DIR="${RESULTS_DIR:-$DEFAULT_RESULTS_DIR}"
LOG_DIR="${LOG_DIR:-$DEFAULT_LOG_DIR}"

mkdir -p "$RESULTS_DIR" "$LOG_DIR"

# Status logger
log() { echo "[$(date '+%Y-%m-%dT%H:%M:%S.%3N')] $*"; }

# Server version
get_server_version() {
    case "$1" in
        vanilla|paper|folia|pumpkin) echo "26.2" ;;
        neutron)
            local ver
            ver=$(grep '^version =' "$BASE_DIR/Cargo.toml" 2>/dev/null | head -1 | grep -oP '"\K[^"]+(?=")' || echo "dev")
            echo "$ver"
            ;;
    esac
}

# Bot path
get_bot_path() {
    if [[ -x "$AZALEA_BIN" ]]; then
        echo "$AZALEA_BIN"
    elif [[ -f "$BENCH_JOIN/index.js" ]]; then
        echo "node $BENCH_JOIN/index.js"
    else
        echo "ERROR: No bot found. Build azalea: cd bench/bots/azalea-join-bench && cargo build --release" >&2
        exit 1
    fi
}

# Percentile
percentile() {
    local sorted=("$@")
    local count=${#sorted[@]}
    local p="${sorted[$count-1]}"
    local idx
    idx=$(echo "scale=10; ($p / 100.0) * ($count - 1)" | bc -l 2>/dev/null || echo "0")
    local lo=${idx%.*}
    local hi=$((lo + 1))
    if [[ $lo -ge $count ]]; then lo=$((count-1)); fi
    if [[ $hi -ge $count ]]; then hi=$((count-1)); fi
    if [[ $lo -eq $hi ]]; then echo "${sorted[$lo]}"; return; fi
    local frac=$(echo "$idx - $lo" | bc -l)
    echo "$(echo "${sorted[$lo]} * (1.0 - $frac) + ${sorted[$hi]} * $frac" | bc -l)"
}

# Hardware info
get_hardware() {
    local os cpu ram
    os=$(uname -srm 2>/dev/null || echo "Unknown")
    cpu=$(grep 'model name' /proc/cpuinfo 2>/dev/null | head -1 | sed 's/.*: //' || echo "Unknown")
    ram=$(grep MemTotal /proc/meminfo 2>/dev/null | awk '{printf "%.1f", $2/1024/1024}' || echo "?")
    echo "{\"os\":\"$os\",\"cpu\":\"$cpu\",\"ram_gb\":$ram}"
}

# Memory watcher (background)
start_memory_watcher() {
    local pid=$1 out=$2 duration=$3 interval=${4:-1}
    (
        local end=$(( $(date +%s) + duration ))
        echo "[" > "$out"
        local first=true
        while [[ $(date +%s) -lt $end ]]; do
            if kill -0 "$pid" 2>/dev/null; then
                if [[ "$first" == false ]]; then echo "," >> "$out"; fi
                first=false
                local rss
                rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || echo "0")
                echo "{\"ts\":\"$(date -Iseconds)\",\"rss_kb\":$rss}" >> "$out"
            else
                break
            fi
            sleep "$interval"
        done
        echo "]" >> "$out"
    ) &
    echo $!
}

# Start server
start_server() {
    local server_type=$1 run_dir=$2 seed=$3
    local log_file="$run_dir/server.log"
    local err_file="$run_dir/server.err"

    echo "eula=true" > "$run_dir/eula.txt"
    mkdir -p "$run_dir/world"

    cat > "$run_dir/server.properties" <<- EOF
eula=true
online-mode=false
level-seed=$seed
view-distance=10
simulation-distance=10
server-port=25565
max-players=20
gamemode=survival
difficulty=peaceful
spawn-animals=false
spawn-monsters=false
spawn-npcs=false
level-type=minecraft:normal
allow-nether=false
allow-end=false
sync-chunk-writes=true
enforce-secure-profile=false
level-name=$run_dir/world
EOF

    case "$server_type" in
        vanilla|paper|folia)
            local jar="$SERVERS_DIR/$server_type/server.jar"
            if [[ ! -f "$jar" ]]; then echo "Not found: $jar" >&2; exit 1; fi
            java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar "$jar" nogui --working-directory "$run_dir" \
                > "$log_file" 2> "$err_file" &
            echo $!
            ;;
        pumpkin)
            local exe="$SERVERS_DIR/pumpkin/pumpkin"
            [[ "$(uname)" == "MINGW"* || "$(uname)" == "CYGWIN"* ]] && exe="$SERVERS_DIR/pumpkin/pumpkin.exe"
            if [[ ! -f "$exe" ]]; then echo "Not found: $exe" >&2; exit 1; fi
            cat > "$run_dir/config.toml" <<- EOF
online_mode = false
max_players = 20
seed = $seed
EOF
            "$exe" --working-directory "$run_dir" > "$log_file" 2> "$err_file" &
            echo $!
            ;;
        neutron)
            cargo run --release -p neutron-cli --working-directory "$BENCH_DIR/.." \
                > "$log_file" 2> "$err_file" &
            echo $!
            ;;
    esac
}

# Wait for Done
wait_for_done() {
    local log_file=$1 timeout=${2:-120}
    local start=$(date +%s)
    while [[ $(date +%s) -lt $((start + timeout)) ]]; do
        sleep 0.5
        if [[ -f "$log_file" ]]; then
            local content
            content=$(cat "$log_file" 2>/dev/null || echo "")
            if [[ "$content" =~ Done\ \(([0-9.]+)s\)! ]]; then
                local seconds="${BASH_REMATCH[1]}"
                echo "$(echo "$seconds * 1000" | bc -l | xargs printf '%.1f')"
                return 0
            fi
        fi
    done
    return 1
}

# Read latency JSON
read_latencies() {
    local file=$1
    if [[ ! -f "$file" ]]; then echo "null"; return 1; fi
    if command -v jq &>/dev/null; then
        jq '{successful: .results.successful, failed: .results.failed, totalBots: .results.totalBots, p50Ms: .results.p50Ms, p95Ms: .results.p95Ms, p99Ms: .results.p99Ms}' "$file" 2>/dev/null || echo "null"
    else
        echo "null"
    fi
}

# Main
log "=== Neutron Benchmark Harness (bash) ==="
log "Server: $SERVER | N=$N | Runs=$RUNS | Seed=$SEED_STR | Warmup=${WARMUP_SEC}s"

DATE_STAMP=$(date +%Y%m%d-%H%M%S)
RUN_ID="${SERVER}-${N}j"
VER_STR=$(get_server_version "$SERVER")
HW_INFO=$(get_hardware)

declare -a RUN_DETAILS=()
declare -a ALL_STARTUP=()

for ((run_idx=0; run_idx<RUNS; run_idx++)); do
    RUN_LOG_DIR="${LOG_DIR}/${RUN_ID}-${DATE_STAMP}-run$((run_idx+1))"
    mkdir -p "$RUN_LOG_DIR"

    log "--- Run $((run_idx+1))/$RUNS ---"

    # Memory watcher
    MEM_WATCH=$(( MEM_WATCH_SEC > 0 ? MEM_WATCH_SEC : WARMUP_SEC + 30 ))
    STATS_FILE="$RUN_LOG_DIR/memory.json"
    MEM_JOB=$(start_memory_watcher $$ "$STATS_FILE" "$MEM_WATCH")

    # Start server
    SERVER_PID=$(start_server "$SERVER" "$RUN_LOG_DIR" "$SEED_STR")
    log "Server PID: $SERVER_PID"

    STARTUP_MS=$(wait_for_done "$RUN_LOG_DIR/server.log" 120) || {
        log "Server did not start. Log tail:"
        tail -5 "$RUN_LOG_DIR/server.log" 2>/dev/null || true
        exit 1
    }
    log "Server ready! Startup: ${STARTUP_MS}ms"
    ALL_STARTUP+=("$STARTUP_MS")

    # Warmup
    log "Warmup ${WARMUP_SEC}s..."
    sleep "$WARMUP_SEC"

    # Bot command
    OUTPUT_PATH="$RUN_LOG_DIR/latency.json"
    BOT_CMD=$(get_bot_path "$SERVER")

    if echo "$BOT_CMD" | grep -q "azalea"; then
        # Azalea bot
        "$BOT_CMD" --host 127.0.0.1 --port 25565 --count "$N" --version "$VER_STR" --output "$OUTPUT_PATH" \
            > "$RUN_LOG_DIR/bot_out.log" 2> "$RUN_LOG_DIR/bot_err.log" &
    else
        # Mineflayer bot (Node.js)
        node "$BENCH_JOIN/index.js" --host 127.0.0.1 --port 25565 --count "$N" --version "$VER_STR" --output "$OUTPUT_PATH" \
            > "$RUN_LOG_DIR/bot_out.log" 2> "$RUN_LOG_DIR/bot_err.log" &
    fi
    BOT_PID=$!
    log "Bot PID: $BOT_PID"

    # Wait for bots
    sleep 5
    local bot_wait=0
    while kill -0 $BOT_PID 2>/dev/null && [[ $bot_wait -lt 30 ]]; do
        sleep 1; bot_wait=$((bot_wait+1))
    done
    kill $BOT_PID 2>/dev/null || true

    # Read results
    LAT_DATA=$(read_latencies "$OUTPUT_PATH")
    if [[ "$LAT_DATA" != "null" ]]; then
        local p50=$(echo "$LAT_DATA" | jq -r '.p50Ms // 0')
        local p95=$(echo "$LAT_DATA" | jq -r '.p95Ms // 0')
        local p99=$(echo "$LAT_DATA" | jq -r '.p99Ms // 0')
        local suc=$(echo "$LAT_DATA" | jq -r '.successful // 0')
        local tot=$(echo "$LAT_DATA" | jq -r '.totalBots // 0')
        log "Bots: $suc/$tot connected | p50=${p50}ms p95=${p95}ms p99=${p99}ms"
    else
        log "No bot latency data"
    fi

    # Stop server
    log "Stopping server..."
    kill $SERVER_PID 2>/dev/null || true
    pkill -f java 2>/dev/null || true
    sleep 1

    # Run detail
    RUN_DETAILS+=("{\"run\":$((run_idx+1)),\"startup_ms\":$STARTUP_MS,\"p50_ms\":$p50,\"p95_ms\":$p95,\"p99_ms\":$p99,\"bot_success\":$suc,\"bot_failed\":$((tot - suc))}")
done

# Aggregate
IFS=$'\n' sorted=($(sort <<<"${ALL_STARTUP[*]}"))
unset IFS
STARTUP_MEDIAN=$(printf '%s\n' "${sorted[@]}" | head -$(( (${#sorted[@]} + 1) / 2 )) | tail -1)

# Write JSON
JSON_OUT="$RESULTS_DIR/${RUN_ID}-${DATE_STAMP}.json"
jq -n \
    --arg id "$RUN_ID" \
    --arg date "$DATE_STAMP" \
    --arg server "$SERVER" \
    --arg ver "$VER_STR" \
    --arg seed "$SEED_STR" \
    --argjson n "$N" \
    --argjson runs "$RUNS" \
    --argjson warmup "$WARMUP_SEC" \
    --argjson hw "$HW_INFO" \
    --argjson startup "$STARTUP_MEDIAN" \
    --argjson details "$(printf '%s\n' "${RUN_DETAILS[@]}" | jq -s .)" \
    '{
        benchmarkId: $id, date: $date, server: $server, version: $ver,
        seed: $seed, botCount: $n, runs: $runs, warmupSec: $warmup,
        hardware: $hw,
        aggregate: { startup_ms: $startup },
        runs_detail: $details,
        notes: ("Baseline B0 - " + $server + " 26.2 - " + (if env.OS | test("Windows") then "Windows" else "Linux" end) + " - " + ($runs | tostring) + " runs")
    }' > "$JSON_OUT"

log "JSON written: $JSON_OUT"

# Write Markdown
MD_OUT="$RESULTS_DIR/${RUN_ID}-${DATE_STAMP}.md"
{
    echo "# Benchmark ${SERVER} - ${RUN_ID} - ${DATE_STAMP}"
    echo ""
    echo "OS: $(echo $HW_INFO | jq -r .os) - CPU: $(echo $HW_INFO | jq -r .cpu) - RAM: $(echo $HW_INFO | jq -r .ram_gb)GB"
    echo "Seed: ${SEED_STR} - View: 10 - Sim: 10 - Warmup: ${WARMUP_SEC}s - Runs: ${RUNS}"
    echo ""
    echo "| Metric | Value |"
    echo "|---|---|"
    echo "| Server | ${SERVER} |"
    echo "| Version | ${VER_STR} |"
    echo "| Startup (median) | ${STARTUP_MEDIAN} ms |"
    echo "| Join p50 | $(echo $LAT_DATA | jq -r '.p50Ms // "TBD"') ms |"
    echo "| Join p95 | $(echo $LAT_DATA | jq -r '.p95Ms // "TBD"') ms |"
    echo "| Join p99 | $(echo $LAT_DATA | jq -r '.p99Ms // "TBD"') ms |"
    echo ""
    echo "## Per-Run Detail"
    echo ""
    echo "| Run | Startup (ms) | p50 (ms) | p95 (ms) | p99 (ms) | Bot success | Bot failed |"
    echo "|---|---|---|---|---|---|---|"
    for d in "${RUN_DETAILS[@]}"; do
        local r=$(echo "$d" | jq -r '.run')
        local s=$(echo "$d" | jq -r '.startup_ms')
        local p50=$(echo "$d" | jq -r '.p50_ms // "TBD"')
        local p95=$(echo "$d" | jq -r '.p95_ms // "TBD"')
        local p99=$(echo "$d" | jq -r '.p99_ms // "TBD"')
        local bs=$(echo "$d" | jq -r '.bot_success')
        local bf=$(echo "$d" | jq -r '.bot_failed')
        echo "| $r | $s | $p50 | $p95 | $p99 | $bs | $bf |"
    done
} > "$MD_OUT"

log "Markdown written: $MD_OUT"
log "=== Done ==="
log "JSON: $JSON_OUT"
log "MD:   $MD_OUT"
log "Logs: $LOG_DIR"