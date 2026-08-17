# bench/servers — server jars for reference extraction

`ref-extract extract` (and `extract-all` for multi-seed batches) boot these jars to
generate reference worlds. The jars are **gitignored** — download them yourself:

| File | Source |
| --- | --- |
| `server-vanilla.jar` | Mojang version manifest (or `mc-decompiler download 26.2`) |
| `server-paper.jar` | <https://papermc.io/downloads> (26.x build) |
| `server-folia.jar` | <https://papermc.io/downloads/folia> (26.x build) |

Only `server-vanilla.jar` is required; paper/folia are optional and are
selected via `ref-extract extract-all --servers vanilla,paper,folia`.
