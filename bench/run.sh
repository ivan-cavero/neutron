#!/usr/bin/env bash
# run.sh — Neutron benchmark harness (bash / Linux)
#
# Usage:
#   ./bench/run.sh vanilla -n 10 --runs 5
#   ./bench/run.sh paper   -n 10 --runs 5
#   ./bench/run.sh pumpkin -n 10 --runs 5
#   ./bench/run.sh neutron -n 10 --runs 5
#
# Requirements:
#   - Node.js (for join-bench bots)
#   - Java 25 (for vanilla/paper)
#   - Server binary in bench/servers/<type>/
#   - jq (for JSON output — used if available, falls back to printf)

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
SERVER=""
N=10
RUNS=5
SEED_STR="1234567890123456789"
WORLD_DIR=""
RESULTS_DIR=""
LOG_DIR=""
WARMUP_SEC=60
MEM_WATCH_SEC=90  # FIX: was 30, now 90 (60 warmup + 30 post-warmup)

# ── Parse args ───────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        vanilla|paper|pumpkin|neutron)
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
    echo "Usage: $0 <vanilla|paper|pumpkin|neutron> [-n N] [--runs N] [--seed N]"
    exit 1
fi

# ── Paths ─────────────────────────────────────────────────────────────────────
SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "$SCRIPT_ROOT/.." && pwd)"
BENCH_DIR="$SCRIPT_ROOT"
SERVERS_DIR="$BENCH_DIR/servers"
BOTS_DIR="$BENCH_DIR/bots"
BENCH_JOIN="$BOTS_DIR/join-bench"

DEFAULT_RESULTS_DIR="$BENCH_DIR/results"
DEFAULT_LOG_DIR="$BENCH_DIR/logs"

RESULTS_DIR="${RESULTS_DIR:-$DEFAULT_RESULTS_DIR}"
LOG_DIR="${LOG_DIR:-$DEFAULT_LOG_DIR}"

mkdir -p "$RESULTS_DIR" "$LOG_DIR"

# ── Globals ───────────────────────────────────────────────────────────────────
SERVER_PID=""
BOT_PIDS=()
MEM_WATCH_PID=""
SHUTDOWN=0  # flag for signal handler

# ── Helpers ───────────────────────────────────────────────────────────────────
ts() { date '+%Y-%m-%dT%H:%M:%S.%3N' 2>/dev/null || date '+%Y-%m-%dT%H:%M:%S'; }

write_status() {
    echo "[$(ts)] $1" | tee -a "$LOG_DIR/.harness.log" 2>/dev/null || true
}

cmd_exists() { command -v "$1" &>/dev/null; }

# Percentile function: sorted array as args, return percentile value
percentile() {
    local pct="$1"; shift
    local sorted=("$@")
    local n=${#sorted[@]}
    if [[ $n -eq 0 ]]; then echo "0"; return; fi
    local idx=$(echo "scale=0; ($pct / 100.0 * $n + 0.9999) / 1" | bc 2>/dev/null || echo "1")
    idx=$((idx - 1))  # 0-indexed
    if (( idx < 0 )); then idx=0; fi
    if (( idx >= n )); then idx=$((n - 1)); fi
    echo "${sorted[$idx]}"
}

# ── Server config builder ─────────────────────────────────────────────────────
build_server_config() {
    local stype="$1" seed="$2" world_path="$3"

    if [[ "$stype" == "vanilla" || "$stype" == "paper" ]]; then
        local props_path="$BENCH_DIR/server.properties"
        cat > "$props_path" <<PROPS
eula=true
online-mode=false
level-seed=${seed}
view-distance=10
simulation-distance=10
level-name=${world_path}
max-players=${N}
white-list=false
PROPS
    fi
}

# ── Server start helpers ─────────────────────────────────────────────────────
start_server() {
    local stype="$1" sdir="$2" logfile="$3" world_path="$4" seed="$5"
    local start_time
    start_time=$(date +%s%3N)

    if [[ "$stype" == "vanilla" || "$stype" == "paper" ]]; then
        local jar="$sdir/server.jar"
        if [[ ! -f "$jar" ]]; then
            echo "ERROR: Server jar not found: $jar" >&2
            return 1
        fi

        java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar "$jar" nogui >> "$logfile" 2>&1 &
        SERVER_PID=$!
        write_status "Starting ${stype} server (Java)..."

        # Wait for "Done" line
        local timeout=120 elapsed=0 found=0
        while (( elapsed < timeout && found == 0 )); do
            sleep 0.5
            elapsed=$((elapsed + 1))
            if [[ -f "$logfile" ]]; then
                if grep -qE 'Done \([0-9.]+s\)' "$logfile" 2>/dev/null; then
                    found=1
                    local time_str
                    time_str=$(grep -oE 'Done \([0-9.]+s\)' "$logfile" | tail -1 | grep -oP '\K[0-9.]+(?=s\))')
                    write_status "Server started in ${time_str}s"
                fi
            fi
        done
        if (( found == 0 )); then
            echo "ERROR: Server did not start within ${timeout}s (no 'Done' line)" >&2
            return 1
        fi

        local end_time
        end_time=$(date +%s%3N)
        echo $((end_time - start_time))

    elif [[ "$stype" == "pumpkin" ]]; then
        local binary="$sdir/pumpkin"
        [[ -f "$sdir/pumpkin.exe" ]] && binary="$sdir/pumpkin.exe"

        if [[ ! -f "$binary" ]]; then
            echo "ERROR: Pumpkin binary not found in $sdir" >&2
            return 1
        fi

        # Create config.toml
        local config_path="$sdir/config.toml"
        cat > "$config_path" <<TOML
[general]
online_mode = false
seed = ${seed}
view_distance = 10
simulation_distance = 10
level_name = "${world_path}"
max_players = ${N}
[server]
port = 25565
address = "127.0.0.1"
[motd]
single = "Neutron Benchmark Server"
TOML

        write_status "Starting Pumpkin server..."
        (cd "$sdir" && ./pumpkin --config config.toml --world-dir "$world_path") >> "$logfile" 2>&1 &
        SERVER_PID=$!

        local timeout=60 elapsed=0 found=0
        while (( elapsed < timeout && found == 0 )); do
            sleep 0.5
            elapsed=$((elapsed + 1))
            if [[ -f "$logfile" ]]; then
                if grep -qE 'Done \([0-9.]+s\)|started' "$logfile" 2>/dev/null; then
                    found=1
                    write_status "Pumpkin server started"
                fi
            fi
        done
        if (( found == 0 )); then
            echo "ERROR: Pumpkin did not start within ${timeout}s" >&2
            return 1
        fi

        local end_time
        end_time=$(date +%s%3N)
        echo $((end_time - start_time))

    elif [[ "$stype" == "neutron" ]]; then
        if [[ ! -f "$BASE_DIR/Cargo.toml" ]]; then
            echo "ERROR: Cargo.toml not found. Must run from neutron repo root ($BASE_DIR)" >&2
            return 1
        fi

        write_status "Building neutron..."
        if ! cargo build --release -p neutron-cli >> "$LOG_DIR/neutron-build.log" 2>&1; then
            echo "ERROR: Neutron build failed" >&2
            return 1
        fi

        # Create config.toml
        local config_path="$BENCH_DIR/server.toml"
        cat > "$config_path" <<TOML
[general]
online_mode = false
seed = ${seed}
view_distance = 10
simulation_distance = 10
level_name = "${world_path}"
max_players = ${N}
[server]
port = 25565
address = "127.0.0.1"
[motd]
single = "Neutron Benchmark Server"
TOML

        local binary="$BASE_DIR/target/release/neutron"
        [[ -f "$BASE_DIR/target/release/neutron.exe" ]] && binary="$BASE_DIR/target/release/neutron.exe"

        if [[ ! -f "$binary" ]]; then
            echo "ERROR: Neutron binary not found" >&2
            return 1
        fi

        write_status "Starting Neutron server..."
        "$binary" --config "$config_path" >> "$logfile" 2>&1 &
        SERVER_PID=$!

        local timeout=60 elapsed=0 found=0
        while (( elapsed < timeout && found == 0 )); do
            sleep 0.5
            elapsed=$((elapsed + 1))
            if [[ -f "$logfile" ]]; then
                if grep -qE 'Done \([0-9.]+s\)|started' "$logfile" 2>/dev/null; then
                    found=1
                    write_status "Neutron server started"
                fi
            fi
        done
        if (( found == 0 )); then
            echo "ERROR: Neutron did not start within ${timeout}s" >&2
            return 1
        fi

        local end_time
        end_time=$(date +%s%3N)
        echo $((end_time - start_time))
    fi
}

# ── Bot launcher ──────────────────────────────────────────────────────────────
start_bots() {
    local bot_log_dir="$1" output_path="$2" server_type="$3"
    local node_bin

    node_bin=$(command -v node 2>/dev/null || command -v nodejs 2>/dev/null)

    if [[ -z "$node_bin" ]]; then
        echo "ERROR: Node.js not found — required for join-bench bots" >&2
        return 1
    fi

    local bot_script="$BENCH_JOIN/index.js"
    if [[ ! -f "$bot_script" ]]; then
        echo "ERROR: Bot script not found: $bot_script" >&2
        return 1
    fi

    mkdir -p "$bot_log_dir"
    write_status "Launching $N join-bench bots..."

    # FIX: Use --output (not --latency-file). The bot only recognizes --output.
    # Also pass --version so the bot uses the correct Minecraft protocol version.
    "$node_bin" "$bot_script" \
        --host 127.0.0.1 \
        --port 25565 \
        --version 26.2 \
        --count "$N" \
        --output "$output_path" \
        --log-dir "$bot_log_dir" \
        --server-type "$server_type" \
        >> "$bot_log_dir/bot.log" 2>&1 &

    BOT_PIDS+=($!)
    echo "${BOT_PIDS[*]}"
}

# ── Memory watcher (FIX: added equivalent of PS version, DurationSec=90) ──────
start_memory_watcher() {
    local pid="$1" stats_file="$2" duration_sec="${3:-$MEM_WATCH_SEC}"
    # FIX: DurationSec was 30, now 90 (60 warmup + 30 post-warmup)

    (
        local end_time=$(( $(date +%s) + duration_sec ))
        local first=1
        printf '['
        while (( $(date +%s) < end_time )); do
            local rss_bytes="" rss_mb=""
            rss_bytes=$(ps -o rss= -p "$pid" 2>/dev/null) || rss_bytes=""
            rss_bytes=$(echo "$rss_bytes" | tr -d ' ')
            if [[ -n "$rss_bytes" && "$rss_bytes" =~ ^[0-9]+$ ]]; then
                rss_mb=$(echo "scale=2; $rss_bytes / 1024" | bc 2>/dev/null || echo "0")
                local ts
                ts=$(date -Iseconds 2>/dev/null || date '+%Y-%m-%dT%H:%M:%S')
                if [[ $first -eq 1 ]]; then
                    first=0
                else
                    printf ','
                fi
                printf '{"ts":"%s","rss_mb":%s}' "$ts" "$rss_mb"
            fi
            sleep 1
        done
        printf ']'
    ) > "$stats_file" 2>/dev/null &
    MEM_WATCH_PID=$!
}

# ── Get peak RSS from stats file ──────────────────────────────────────────────
get_peak_rss() {
    local stats_file="$1"
    if [[ ! -f "$stats_file" ]]; then echo "0"; return; fi

    if cmd_exists jq; then
        jq -r '[.[].rss_mb] | max // 0' "$stats_file" 2>/dev/null || echo "0"
    else
        grep -oE '"rss_mb":[0-9.]*' "$stats_file" 2>/dev/null | \
            grep -oE '[0-9.]+$' | sort -g | tail -1 2>/dev/null || echo "0"
    fi
}

# ── TPS measurement (Paper: spark plugin) ─────────────────────────────────────
# TBD: Full TPS measurement requires RCON or bot-based command execution.
# Paper ships spark; others need server-specific metrics endpoints.
# TODO: implement when RCON is configured or metrics are available.
measure_tps() {
    local stype="$1" output_dir="$2"
    local tps_file="$output_dir/tps.json"
    local notes="TBD — TPS measurement not yet implemented"

    case "$stype" in
        paper)
            notes="spark TBD — Paper has spark but probe not yet implemented"
            ;;
        vanilla)
            notes="TBD — vanilla TPS requires paper spark or custom metrics"
            ;;
        pumpkin)
            notes="TBD — pumpkin TPS requires server metrics endpoint"
            ;;
        neutron)
            notes="TBD — neutron TPS requires metrics endpoint / bench mode"
            ;;
    esac

    printf '{"tps_p99_ms":null,"notes":"%s"}' "$notes" > "$tps_file"
    echo "null"
}

# ── CPS measurement placeholder ───────────────────────────────────────────────
# cps = chunks generated per second (sustained).
# Vanilla/Paper: Chunky plugin (chunky radius N, chunky start, chunky progress).
# Pumpkin/Neutron: counter from server metrics or equivalent load.
# TODO: implement once Chunky/server metrics are integrated.
measure_cps() {
    local stype="$1" output_dir="$2"
    local cps_file="$output_dir/cps.json"

    local notes="TBD — Chunky for Vanilla/Paper, server counter for Pumpkin/Neutron"
    printf '{"cps":null,"radius":64,"notes":"%s"}' "$notes" > "$cps_file"
    echo "null"
}

# ── Server version ────────────────────────────────────────────────────────────
get_server_version() {
    local stype="$1"
    case "$stype" in
        vanilla)  echo "26.2" ;;
        paper)    echo "paper-latest" ;;
        pumpkin)  echo "pumpkin-nightly" ;;
        neutron)
            if [[ -f "$BASE_DIR/Cargo.toml" ]]; then
                local ver
                ver=$(grep '^version =' "$BASE_DIR/Cargo.toml" | head -1 | grep -oP '"\K[^"]+(?=")' || echo "dev")
                echo "neutron-${ver}"
            else
                echo "neutron-dev"
            fi
            ;;
        *)        echo "unknown" ;;
    esac
}

# ── System info ───────────────────────────────────────────────────────────────
get_system_info() {
    local hw_cpu hw_ram_gb hw_os
    hw_cpu=$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs 2>/dev/null || uname -m 2>/dev/null || echo "unknown")
    hw_ram_gb=$(free -g 2>/dev/null | awk '/Mem:/{print $2}' || echo "$(nproc 2>/dev/null || echo 4)")
    hw_os=$(uname -s 2>/dev/null || echo "unknown")
    echo "${hw_cpu}||${hw_ram_gb}||${hw_os}"
}

# ── Cleanup ───────────────────────────────────────────────────────────────────
cleanup() {
    SHUTDOWN=1
    write_status "=== Cleanup: stopping processes ==="
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        write_status "Stopped server (PID: $SERVER_PID)"
    fi
    for pid in "${BOT_PIDS[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
    if [[ -n "${MEM_WATCH_PID:-}" ]]; then
        kill "$MEM_WATCH_PID" 2>/dev/null || true
        wait "$MEM_WATCH_PID" 2>/dev/null || true
    fi
    BOT_PIDS=()
    SERVER_PID=""
    MEM_WATCH_PID=""
}

trap cleanup SIGINT SIGTERM EXIT

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    write_status "=== Neutron Benchmark Harness (bash) ==="
    write_status "Server: $SERVER | N=$N | Runs=$RUNS | Seed=$SEED_STR | Warmup=${WARMUP_SEC}s"

    local date_stamp
    date_stamp=$(date '+%Y%m%d-%H%M%S')
    local run_id="${SERVER}-${N}j"
    local run_log_dir="$LOG_DIR/$run_id"
    mkdir -p "$run_log_dir"

    local all_startup=()
    local all_flat_file="$run_log_dir/all_flat.txt"
    > "$all_flat_file"  # truncate

    local first_run=1

    for (( run_idx=0; run_idx<RUNS; run_idx++ )); do
        write_status "--- Run $((run_idx+1))/$RUNS ---"

        local run_log="$run_log_dir/run-${run_idx}.log"
        local bot_log="$run_log_dir/bots"
        mkdir -p "$bot_log"
        local output_file="$run_log_dir/latency-${run_idx}.json"
        local stats_file="$run_log_dir/stats-${run_idx}.json"
        local world_path="$run_log_dir/world-run-${run_idx}"

        # Clean world
        rm -rf "$world_path"
        mkdir -p "$world_path"

        # Build server config
        build_server_config "$SERVER" "$SEED_STR" "$world_path"

        # Start server (prints startup_ms to stdout)
        local startup_ms
        startup_ms=$(start_server "$SERVER" "$SERVERS_DIR/$SERVER" "$run_log" "$world_path" "$SEED_STR")

        # Warmup: idle before bots
        write_status "Warmup: ${WARMUP_SEC}s idle..."
        sleep "$WARMUP_SEC"

        # Memory watcher: run during warmup + post-warmup
        local mem_watch_sec=$(( MEM_WATCH_SEC > 0 ? MEM_WATCH_SEC : WARMUP_SEC + 30 ))
        start_memory_watcher "$SERVER_PID" "$stats_file" "$mem_watch_sec"

        # Launch bots (using --output, not --latency-file)
        local bot_pids_str
        bot_pids_str=$(start_bots "$bot_log" "$output_file" "$SERVER")

        # Wait for bots
        write_status "Waiting for bots to connect..."
        for pid in $bot_pids_str; do
            local waited=0
            while kill -0 "$pid" 2>/dev/null && (( waited < 60 )); do
                sleep 0.5
                waited=$((waited + 1))
            done
        done

        # Stop memory watcher
        kill "$MEM_WATCH_PID" 2>/dev/null || true
        wait "$MEM_WATCH_PID" 2>/dev/null || true
        MEM_WATCH_PID=""

        # Read latencies (bot already outputs ms, no conversion needed)
        local latencies_array="[]"
        if [[ -f "$output_file" ]]; then
            if cmd_exists jq; then
                # Bot writes: { results: { joinLatencies: [234.5, ...] } } (ms)
                # Extract joinLatencies as-is — values are already in ms
                latencies_array=$(jq -c '
                    .results.joinLatencies // []
                ' "$output_file" 2>/dev/null || echo '[]')

                # Fallback: if joinLatencies doesn't exist, try top-level array
                if [[ "$latencies_array" == "[]" ]]; then
                    local has_results
                    has_results=$(jq -e '.results.joinLatencies' "$output_file" &>/dev/null && echo "yes" || echo "no")
                    if [[ "$has_results" == "no" && -s "$output_file" ]]; then
                        latencies_array=$(jq -c '.[]' "$output_file" 2>/dev/null | jq -s '.' || echo '[]')
                    fi
                fi
            fi
        fi

        # Parse latencies for percentile calculation
        local lat_values=()
        if cmd_exists jq && [[ "$latencies_array" != "[]" ]]; then
            while IFS= read -r val; do
                val=$(echo "$val" | tr -d ' ')
                [[ -n "$val" ]] && lat_values+=("$val")
            done < <(echo "$latencies_array" | jq -r '.[]')
        else
            # Fallback: grep numbers from raw JSON
            if [[ -s "$output_file" ]]; then
                while IFS= read -r val; do
                    val=$(echo "$val" | grep -oE '[0-9]+\.?[0-9]*' | head -1)
                    [[ -n "$val" ]] && lat_values+=("$val")
                done < <(grep -oE '"[a-z_]+":\s*[0-9]+\.?[0-9]*' "$output_file" 2>/dev/null | \
                    grep -oE '[0-9]+\.?[0-9]*$' || true)
            fi
        fi

        if [[ ${#lat_values[@]} -eq 0 ]]; then
            write_status "WARNING: No latency data for run $((run_idx+1))"
        fi

        # Sort for percentiles
        local sorted_lat=($(printf '%s\n' "${lat_values[@]}" | sort -g 2>/dev/null))
        local n_lat=${#sorted_lat[@]}

        local p50 p95 p99 avg_lat
        if (( n_lat > 0 )); then
            p50=$(percentile 50 "${sorted_lat[@]}")
            p95=$(percentile 95 "${sorted_lat[@]}")
            p99=$(percentile 99 "${sorted_lat[@]}")
            # Average
            local sum=0
            for v in "${sorted_lat[@]}"; do sum=$(echo "$sum + $v" | bc); done
            avg_lat=$(echo "scale=3; $sum / $n_lat" | bc)
        else
            p50=0; p95=0; p99=0; avg_lat=0
        fi

        # Peak RAM from memory watcher
        local peak_ram=0
        if [[ -f "$stats_file" ]]; then
            peak_ram=$(get_peak_rss "$stats_file")
        fi

        # TPS measurement (Paper: spark, others: TBD)
        local tps_val
        tps_val=$(measure_tps "$SERVER" "$run_log_dir")

        # CPS measurement (placeholder — Chunky TBD)
        local cps_val
        cps_val=$(measure_cps "$SERVER" "$run_log_dir")

        write_status "Run $((run_idx+1)): startup=${startup_ms}ms p50=${p50}ms p95=${p95}ms p99=${p99}ms peakRAM=${peak_ram}MB"

        # Accumulate for aggregation
        all_startup+=("$startup_ms")

        # FIX: Write one JSON array per line (no comma separators) for slurp compatibility
        echo "$latencies_array" >> "$all_flat_file"

        # Accumulate individual values for non-jq fallback
        if [[ -n "${latencies_array}" && "${latencies_array}" != "[]" ]]; then
            echo "$latencies_array" | jq -r '.[]' 2>/dev/null >> "$all_flat_file" || true
        fi

        # Store per-run detail in temp file for markdown table
        echo "$((run_idx + 1))|${startup_ms}|${p50}|${p95}|${p99}|${peak_ram}|${tps_val}|${cps_val}" >> "$run_log_dir/per_run.txt"

        # Kill server for next run
        if [[ -n "$SERVER_PID" ]]; then
            kill "$SERVER_PID" 2>/dev/null || true
            wait "$SERVER_PID" 2>/dev/null || true
        fi
        SERVER_PID=""
    done

    # ── Aggregate results ──────────────────────────────────────────────
    write_status "=== Aggregating results ==="

    # Startup median
    local sorted_startup=($(printf '%s\n' "${all_startup[@]}" | sort -g))
    local mid=$(( ${#sorted_startup[@]} / 2 ))
    local startup_median="${sorted_startup[$mid]}"

    # Merge all latencies into one flat array (FIX: use -s slurp, one JSON array per line)
    local all_flat="[]"
    if cmd_exists jq && [[ -s "$all_flat_file" ]]; then
        all_flat=$(jq -s 'flatten' "$all_flat_file" 2>/dev/null || echo '[]')
    else
        # Non-jq fallback: concatenate all numbers from lines
        all_flat="["
        local first=1
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            # Extract numbers from JSON array like [1,2,3]
            while read -r num; do
                num=$(echo "$num" | tr -d ' []')
                [[ -n "$num" ]] || continue
                if [[ $first -eq 1 ]]; then first=0; else all_flat+=","; fi
                all_flat+="$num"
            done < <(echo "$line" | grep -oE '[0-9]+\.?[0-9]*' 2>/dev/null)
        done < "$all_flat_file"
        all_flat="${all_flat%,}]"
    fi

    # Overall percentiles
    local flat_values=()
    if cmd_exists jq && [[ "$all_flat" != "[]" ]]; then
        while IFS= read -r val; do
            val=$(echo "$val" | tr -d ' ')
            [[ -n "$val" ]] && flat_values+=("$val")
        done < <(echo "$all_flat" | jq -r '.[]')
    fi

    local sorted_flat=($(printf '%s\n' "${flat_values[@]}" | sort -g 2>/dev/null))
    local n_flat=${#sorted_flat[@]}
    local overall_p50=0 overall_p95=0 overall_p99=0
    if (( n_flat > 0 )); then
        overall_p50=$(percentile 50 "${sorted_flat[@]}")
        overall_p95=$(percentile 95 "${sorted_flat[@]}")
        overall_p99=$(percentile 99 "${sorted_flat[@]}")
    fi

    # TPS / CPS (use first run's values)
    local tps_aggregated=0 cps_aggregated=0
    if [[ -f "$run_log_dir/per_run.txt" ]]; then
        local first_line
        first_line=$(head -1 "$run_log_dir/per_run.txt")
        tps_aggregated=$(echo "$first_line" | cut -d'|' -f7)
        cps_aggregated=$(echo "$first_line" | cut -d'|' -f8)
    fi

    # RAM idle ≈ average of first 3 samples from first run
    local ram_idle="0"
    local ram_idle_stats="$run_log_dir/stats-0.json"
    if [[ -f "$ram_idle_stats" ]]; then
        if cmd_exists jq; then
            ram_idle=$(jq -r '[.[0:3] | .[].rss_mb] | add / length' "$ram_idle_stats" 2>/dev/null || echo "0")
        else
            ram_idle="0"  # Can't easily compute average without jq
        fi
    fi

    # RAM 100j: TBD (requires 100-concurrent-bots stress test)
    local ram_100j="TBD"
    # CPU idle: TBD (requires OS-level CPU monitoring)
    local cpu_idle="TBD"

    # Server version
    local version
    version=$(get_server_version "$SERVER")

    # Hardware info
    local hw_info
    hw_info=$(get_system_info)
    local hw_cpu hw_ram_gb hw_os
    hw_cpu=$(echo "$hw_info" | cut -d'|' -f1)
    hw_ram_gb=$(echo "$hw_info" | cut -d'|' -f2)
    hw_os=$(echo "$hw_info" | cut -d'|' -f3)

    # ── Write JSON ─────────────────────────────────────────────────────
    local json_out="$RESULTS_DIR/${run_id}-${date_stamp}.json"

    if cmd_exists jq && [[ "$all_flat" != "[]" ]]; then
        # FIX: Use --arg for seed (string) to avoid float precision loss with large seeds
        jq -n \
            --arg test_name "join-bench" \
            --arg server_type "$SERVER" \
            --arg version "$version" \
            --arg date "$date_stamp" \
            --arg seed_str "$SEED_STR" \
            --argjson n_bots "$N" \
            --argjson runs "$RUNS" \
            --arg startup "$startup_median" \
            --argjson p50 "$overall_p50" \
            --argjson p95 "$overall_p95" \
            --argjson p99 "$overall_p99" \
            --arg hw_cpu "$hw_cpu" \
            --arg hw_ram "$hw_ram_gb" \
            --arg hw_os "$hw_os" \
            --arg tps "$tps_aggregated" \
            --arg cps "$cps_aggregated" \
            --arg ram_idle "$ram_idle" \
            '{
                test_name: $test_name,
                server_type: $server_type,
                version: $version,
                date: $date,
                seed: $seed_str,
                n_bots: $n_bots,
                runs: $runs,
                aggregate: {
                    startup_ms: ($startup | tonumber),
                    join_p50_ms: $p50,
                    join_p95_ms: $p95,
                    join_p99_ms: $p99,
                    all_latencies: (input | .),
                    tps_p99_ms: (if $tps == "null" then null else ($tps | tonumber) end),
                    cps: (if $cps == "null" then null else ($cps | tonumber) end),
                    ram_idle_mb: ($ram_idle | tonumber),
                    ram_100j_mb: null,
                    cpu_idle_pct: null
                },
                hardware: {
                    os: $hw_os,
                    cpu: $hw_cpu,
                    ram_gb: ($hw_ram | tonumber)
                }
            }' "$all_flat" > "$json_out"
    else
        # Non-jq fallback: write JSON manually (FIX: include all_latencies and seed as string)
        local all_flat_vals=""
        if [[ -s "$all_flat_file" ]]; then
            all_flat_vals=$(grep -oE '[0-9]+\.?[0-9]*' "$all_flat_file" 2>/dev/null | \
                paste -sd',' - || echo "")
        fi

        local tps_json="null"
        [[ "$tps_aggregated" != "null" ]] && tps_json="$tps_aggregated"
        local cps_json="null"
        [[ "$cps_aggregated" != "null" ]] && cps_json="$cps_aggregated"

        cat > "$json_out" <<JSONEOF
{
  "test_name": "join-bench",
  "server_type": "${SERVER}",
  "version": "${version}",
  "date": "${date_stamp}",
  "seed": "${SEED_STR}",
  "n_bots": ${N},
  "runs": ${RUNS},
  "aggregate": {
    "startup_ms": ${startup_median},
    "join_p50_ms": ${overall_p50},
    "join_p95_ms": ${overall_p95},
    "join_p99_ms": ${overall_p99},
    "all_latencies": [${all_flat_vals}],
    "tps_p99_ms": ${tps_json},
    "cps": ${cps_json},
    "ram_idle_mb": ${ram_idle},
    "ram_100j_mb": null,
    "cpu_idle_pct": null
  },
  "hardware": {
    "os": "${hw_os}",
    "cpu": "${hw_cpu}",
    "ram_gb": ${hw_ram_gb}
  }
}
JSONEOF
    fi

    write_status "JSON written: $json_out"

    # ── Write Markdown ─────────────────────────────────────────────────
    # BENCHMARKS.md §8 template columns:
    # | Server | Version | Startup | RAM idle | RAM 100j | CPU idle | cps | TPS p99 | Join p50 | Join p95 |
    local tps_md="TBD"
    [[ "$tps_aggregated" != "null" ]] && tps_md="${tps_aggregated} ms"
    local cps_md="TBD"
    [[ "$cps_aggregated" != "null" ]] && cps_md="$cps_aggregated"

    local md_out="$RESULTS_DIR/${run_id}-${date_stamp}.md"

    cat > "$md_out" <<MDEOF
# Benchmark ${SERVER} — ${run_id} — ${date_stamp}

OS: ${hw_os} · CPU: ${hw_cpu} · RAM: ${hw_ram_gb}GB · Seed: ${SEED_STR}
View: 10 · Sim: 10 · online-mode: false
Warmup: ${WARMUP_SEC}s · Runs: ${RUNS} (median)

| Metric | Value |
|---|---|
| Server | ${SERVER} |
| Version | ${version} |
| Startup (median) | ${startup_median} ms |
| RAM idle | ${ram_idle} MB |
| RAM 100j | TBD |
| CPU idle | TBD |
| cps | ${cps_md} |
| TPS p99 | ${tps_md} |
| Join p50 | ${overall_p50} ms |
| Join p95 | ${overall_p95} ms |

## Per-Run Detail

| Run | Startup (ms) | p50 (ms) | p95 (ms) | p99 (ms) | Peak RAM (MB) |
|---|---|---|---|---|---|
MDEOF

    # FIX: Add per-run detail table (was missing)
    if [[ -f "$run_log_dir/per_run.txt" ]]; then
        while IFS='|' read -r run_num s_ms p50m p95m p99m pram; do
            echo "| ${run_num} | ${s_ms} | ${p50m} | ${p95m} | ${p99m} | ${pram} |"
        done < "$run_log_dir/per_run.txt" >> "$md_out"
    fi

    write_status "Markdown written: $md_out"
    write_status "=== Done ==="
    write_status "JSON:  $json_out"
    write_status "MD:    $md_out"
    write_status "Logs:  $run_log_dir/"
}

main "$@"