# tests/benchmarks/servers — managed server jars

The benchmark harness (`neutron-bench`) provisions and boots server jars from
this directory. Jars are **gitignored** — download them with the harness itself:

```bash
cd tests/benchmarks
cargo run --release -p neutron-bench -- servers download vanilla 26.2
cargo run --release -p neutron-bench -- servers download paper 26.2
cargo run --release -p neutron-bench -- servers download folia 26.2
cargo run --release -p neutron-bench -- servers download pumpkin <version>
```

## Layout (multi-version)

```
servers/
├── vanilla/26.2/server.jar      # Mojang version manifest
├── paper/26.2/server.jar        # PaperMC downloads service (fill.papermc.io/v3)
├── folia/26.2/server.jar        # PaperMC downloads service (fill.papermc.io/v3)
└── pumpkin/<version>/pumpkin    # GitHub releases (Pumpkin-MC/Pumpkin); .exe on Windows
```

The legacy single-jar layout (`servers/<type>/server.jar`) still works for
`neutron-bench run` and for `tools/ref-extract` (see note below).

## Offline / local fallback

When the network is unavailable (pass `--offline`, or the host is unreachable),
`servers download` copies the jar from a local cache dir instead of failing:

```bash
# cache layout mirrors the managed layout:
#   <dir>/vanilla/26.2/server.jar  <dir>/paper/26.2/server.jar  ...
export NEUTRON_BENCH_SERVERS_FALLBACK=/path/to/jar-cache
cargo run --release -p neutron-bench -- servers download vanilla 26.2 --offline
```

Downloads use bounded timeouts — they never hang on a dead network.

## Inspecting what is present

```bash
cargo run --release -p neutron-bench -- servers list      # paths only
cargo run --release -p neutron-bench -- servers status    # OK / MISSING + validity
```

## Note for the human: tools/ref-extract

`tools/ref-extract` (outside this workspace) still defaults to the old
`bench`-prefixed servers path from before the move. It was **not** migrated
here (tools/ is human-owned and out of bounds for the benchmark refactor).
Its path default still points at the pre-move location, and it reads the
single-jar layout (`server-vanilla.jar` etc.) rather than the multi-version
layout above. Migrating that tool is tracked separately.