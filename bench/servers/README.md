# bench/servers — server jars for reference extraction

`vanilla-hash extract` (and `generate-all.sh`) boot these jars to generate
reference worlds. The jars are **gitignored** — download them yourself:

| File | Source |
| --- | --- |
| `server-vanilla.jar` | Mojang version manifest (or `mc-decompiler download 26.2`) |
| `server-paper.jar` | <https://papermc.io/downloads> (26.x build) |
| `server-folia.jar` | <https://papermc.io/downloads/folia> (26.x build) |

Only `server-vanilla.jar` is required; paper/folia are optional and are
auto-detected by `generate-all.sh` / `generate-all.ps1`.
