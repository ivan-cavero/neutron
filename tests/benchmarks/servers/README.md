# tests/benchmarks/servers — server jars for reference extraction

`ref-extract extract` (and `extract-all` for multi-seed batches) boot these jars to
generate reference worlds. The jars are **gitignored** — download them yourself:

| File | Source |
| --- | --- |
| `server-vanilla.jar` | Mojang version manifest (or `mc-decompiler download 26.2`) |
| `server-paper.jar` | <https://papermc.io/downloads> (26.x build) |
| `server-folia.jar` | <https://papermc.io/downloads/folia> (26.x build) |

Only `server-vanilla.jar` is required; paper/folia are optional and are
selected via `ref-extract extract-all --servers vanilla,paper,folia`.

> Note: this directory moved from `bench/servers/` to `tests/benchmarks/servers/`.
> The benchmark harness (`neutron-bench run`) also looks here, for
> `servers/<type>/server.jar` (vanilla/paper/folia) and `servers/pumpkin/pumpkin`.
> `tools/ref-extract` still defaults to the old `bench/servers` path — that
> migration is tracked separately (provisioning piece).
