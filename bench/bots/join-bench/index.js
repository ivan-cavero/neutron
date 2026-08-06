#!/usr/bin/env node
'use strict';

/**
 * neutron-join-bench — Join latency benchmark for Neutron Minecraft server.
 *
 * Spawns N simultaneous mineflayer bot connections and measures join latency.
 * Measures t0 (script start), t1 (login event), t2 (spawn event) per bot,
 * then outputs p50/p95/p99 latencies plus per-bot details to JSON.
 *
 * Usage: node index.js [--host HOST] [--port PORT] [--count N] [--version V] [--output FILE]
 *
 * Requirements:
 *   - A running Minecraft server on the target host:port (vanilla/Paper/Pumpkin)
 *   - Node.js ≥ 18
 *
 * Notes:
 *   - Mineflayer 1.20.2+ requires physicsEnabled: false to avoid kicks
 *   - Version string follows Minecraft's version format (e.g. "26.2", "1.21.11")
 *   - Default version is "1.21.11" for broad mineflayer compatibility.
 *     The target Neutron server version is 26.2 — use --version 26.2 if your
 *     mineflayer install supports it (requires matching protocol-data).
 *   - Bots start with ~2ms stagger between each one (near-simultaneous join test)
 *   - All latency fields are in milliseconds (ms), not seconds
 *   - The harness (run.ps1/run.sh) handles pre-bench warmup; this script does NOT warm up
 */

const { createBot } = require('mineflayer');
const { writeFileSync, existsSync, mkdirSync } = require('fs');
const { resolve, dirname } = require('path');

// ── CLI parsing (no dependencies) ────────────────────────────────────────────

function parseArgs(argv) {
  const args = {
    host: 'localhost',
    port: 25565,
    count: 10,
    version: '1.21.11',
    output: null,
    help: false,
  };

  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--help' || a === '-h') {
      args.help = true;
    } else if (a === '--host' && argv[i + 1]) {
      args.host = argv[++i];
    } else if (a === '--port' && argv[i + 1]) {
      args.port = parseInt(argv[++i], 10);
    } else if (a === '--count' && argv[i + 1]) {
      args.count = parseInt(argv[++i], 10);
    } else if (a === '--version' && argv[i + 1]) {
      args.version = argv[++i];
    } else if (a === '--output' && argv[i + 1]) {
      args.output = resolve(argv[++i]);
    }
  }

  return args;
}

function printHelp() {
  console.log(`
neutron-join-bench — Join latency benchmark

Usage: node index.js [options]

Options:
  --host HOST       Server hostname (default: localhost)
  --port PORT       Server port (default: 25565)
  --count N         Number of bots to spawn (default: 10)
  --version VER     Minecraft version (default: 1.21.11)
                    Note: Target Neutron server is 26.2.
                    Use --version 26.2 if your mineflayer supports it.
  --output FILE     Write results to JSON file (default: stdout)
  -h, --help        Show this help

Example:
  node index.js --count 20 --output results/join-bench.json
  node index.js --version 26.2 --count 10 --output results/join-bench.json

Output:
  JSON object with per-bot timestamps, p50Ms/p95Ms/p99Ms latencies (ms), and
  aggregate metrics. All latency fields are in milliseconds.
`);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Compute percentile from a sorted array of numbers.
 * @param {number[]} sorted - Sorted array of numbers
 * @param {number} p - Percentile (0-100)
 * @returns {number}
 */
function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  const idx = (p / 100) * (sorted.length - 1);
  const lo = Math.floor(idx);
  const hi = Math.ceil(idx);
  if (lo === hi) return sorted[lo];
  const frac = idx - lo;
  return sorted[lo] * (1 - frac) + sorted[hi] * frac;
}

/**
 * Ensure a directory exists (recursive).
 */
function ensureDir(filePath) {
  const d = dirname(filePath);
  if (!existsSync(d)) mkdirSync(d, { recursive: true });
}

// ── Bot worker ───────────────────────────────────────────────────────────────

/**
 * Create a single bot, measure its join latency.
 *
 * @param {object} cfg   - { host, port, version, index }
 * @returns {Promise<object>} Bot result
 */
function spawnBot(cfg) {
  return new Promise((resolve) => {
    const { host, port, version, index } = cfg;
    const t0 = Date.now();

    // Mineflayer 1.20.2+ requires physicsEnabled: false to avoid being kicked
    // during the physics tick simulation before spawn.
    const botOpts = {
      host,
      port,
      version,
      username: `bench-bot-${index}-${Date.now()}`,
      // Disable physics to prevent kicks on 1.20.2+ servers
      physicsEnabled: false,
      // Shorten timeout so we fail fast on unreachable servers
      timeout: 10000,
    };

    let loginMs = null;
    let spawnMs = null;
    let errored = false;
    let err = null;
    let bot = null;

    try {
      bot = createBot(botOpts);

      // ── login event: server accepted the handshake ──────────────────────
      bot.once('login', () => {
        loginMs = Date.now() - t0;
      });

      // ── spawn event: world loaded, bot is in-game ──────────────────────
      bot.once('spawn', () => {
        spawnMs = Date.now() - t0;
      });

      // ── error event: connection/protocol failure ───────────────────────
      bot.once('error', (e) => {
        errored = true;
        err = e;
        if (!loginMs) loginMs = Date.now() - t0;
        if (!spawnMs) spawnMs = Date.now() - t0;
      });

      // ── end event: server closed connection ────────────────────────────
      bot.once('end', () => {
        if (!errored && !loginMs) {
          errored = true;
          err = new Error('Bot disconnected before spawn');
          loginMs = Date.now() - t0;
          spawnMs = Date.now() - t0;
        }
      });
    } catch (e) {
      errored = true;
      err = e;
      loginMs = 0;
      spawnMs = 0;
    }

    // ── finish: resolve when spawn, error, end, or timeout fires ─────────
    function finish() {
      resolve({
        index,
        loginMs: loginMs ?? 0,
        spawnMs: spawnMs ?? 0,
        success: !errored && spawnMs !== null,
        error: err ? err.message : null,
      });
    }

    if (bot) {
      bot.once('spawn', finish);
      bot.once('error', finish);
      bot.once('end', finish);
    }

    // Safety timeout: if nothing happens in 15s, give up.
    setTimeout(finish, 15000);
  });
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv);

  if (args.help) {
    printHelp();
    process.exit(0);
  }

  const { host, port, count, version, output } = args;

  // Capture output path in closure for graceful shutdown
  outputPath = output;

  console.log(`neutron-join-bench — starting`);
  console.log(`  host   : ${host}`);
  console.log(`  port   : ${port}`);
  console.log(`  count  : ${count}`);
  console.log(`  version: ${version}`);

  const t0 = Date.now();

  // ── Spawn all bots simultaneously (stagger ~2ms per bot) ────────────────
  const botPromises = [];
  for (let i = 0; i < count; i++) {
    botPromises.push(spawnBot({ host, port, version, index: i }));
    // Stagger by ~2ms between bots (~18ms total spread for 10 bots = (count-1) * 2ms)
    if (i < count - 1) {
      await new Promise((r) => setTimeout(r, 2));
    }
  }

  console.log(`  ${count} bot(s) spawned, waiting for join...`);

  // ── Wait for all bots to complete ───────────────────────────────────────
  const results = await Promise.all(botPromises);

  const totalTimeMs = Date.now() - t0;

  // ── Collect latencies (t0 → spawn, in milliseconds) ────────────────────
  const totalLatencies = results
    .filter((r) => r.success)
    .map((r) => r.spawnMs); // ms, no division

  const sortedTotal = [...totalLatencies].sort((a, b) => a - b);

  const report = {
    test: 'join-bench',
    timestamp: new Date().toISOString(),
    config: {
      host,
      port,
      version,
      count,
      t0Ms: t0,
    },
    results: {
      allConnected: results.every((r) => r.success),
      totalBots: results.length,
      successful: results.filter((r) => r.success).length,
      failed: results.filter((r) => !r.success).length,
      joinLatencies: totalLatencies,
      p50Ms: percentile(sortedTotal, 50),
      p95Ms: percentile(sortedTotal, 95),
      p99Ms: percentile(sortedTotal, 99),
      totalTimeMs,
      perBot: results.map((r) => ({
        index: r.index,
        loginMs: r.loginMs,
        spawnMs: r.spawnMs,
        latencyMs: r.spawnMs - r.loginMs,
        success: r.success,
        error: r.error,
      })),
    },
  };

  // ── Output ──────────────────────────────────────────────────────────────
  const json = JSON.stringify(report, null, 2);

  if (output) {
    ensureDir(output);
    writeFileSync(output, json, 'utf-8');
    console.log(`  results written to ${output}`);
  } else {
    console.log(json);
  }

  // Summary to stdout
  console.log(`\nJoin latency (t0 → spawn, milliseconds):`);
  console.log(`  p50Ms: ${report.results.p50Ms.toFixed(2)}`);
  console.log(`  p95Ms: ${report.results.p95Ms.toFixed(2)}`);
  console.log(`  p99Ms: ${report.results.p99Ms.toFixed(2)}`);
  console.log(`  allConnected: ${report.results.allConnected}`);
  console.log(`  total time: ${totalTimeMs}ms`);

  return report;
}

// ── Closure: store report, output path, and shutdown flag ────────────────────

let report = null;
let outputPath = null;
let shuttingDown = false;

// ── Graceful shutdown ────────────────────────────────────────────────────────

async function gracefulShutdown() {
  if (shuttingDown) return;
  shuttingDown = true;
  console.log('\nShutdown signal received, writing results...');

  // report and outputPath are captured from closure above.
  if (report && outputPath) {
    try {
      ensureDir(outputPath);
      writeFileSync(outputPath, JSON.stringify(report, null, 2), 'utf-8');
      console.log(`  results written to ${outputPath}`);
    } catch (e) {
      console.error('  failed to write results:', e.message);
    }
  } else {
    console.log('  no results to write (report or output path missing)');
  }

  process.exit(0);
}

// ── Run ──────────────────────────────────────────────────────────────────────

main()
  .then((r) => {
    report = r;
    console.log('Benchmark complete.');
  })
  .catch((err) => {
    console.error('Benchmark failed:', err);
    process.exit(1);
  });

// Wire graceful shutdown — captures report and outputPath from closure.
process.on('SIGINT', () => gracefulShutdown().catch(() => process.exit(1)));
process.on('SIGTERM', () => gracefulShutdown().catch(() => process.exit(1)));

// Also catch unhandled rejections / errors and still try to write output.
process.on('unhandledRejection', (reason) => {
  console.error('Unhandled rejection:', reason);
  gracefulShutdown().catch(() => process.exit(1));
});