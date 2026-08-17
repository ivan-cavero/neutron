# ref-extract

Extract chunk checksums from a Minecraft server — the **vanilla/reference side**
of the parity pipeline. Formerly `vanilla-hash`.

## What it does

- **`extract`** — boots a vanilla/paper/folia server with a seed, lets it generate
  chunks around spawn, reads the `.mca` region files, and writes one xxHash64 per
  chunk to a JSON file ("reference data").
- **`extract-all`** — batch version for multiple seeds × server types.
- **`compare`** — diffs two reference data JSON files (matching / different /
  missing chunks). This is the single comparator for parity checks.

## Usage

```bash
# Extract one reference (deterministic blocks mode)
cargo run --release -p ref-extract -- extract \
  --seed 12345 --server vanilla --hash-mode blocks --output hashes/vanilla-12345-blocks.json

# Batch: all seeds × available servers
cargo run --release -p ref-extract -- extract-all \
  --seeds 12345 67890 --servers vanilla paper

# Compare a vanilla reference against a neutron one
cargo run --release -p ref-extract -- compare \
  --left hashes/vanilla-12345-blocks.json --right hashes/neutron-12345-blocks.json
```

Hash modes:
- `full` — xxHash64 of the whole decompressed chunk NBT (includes lighting,
  timestamps; NOT deterministic across runs).
- `blocks` — hashes only the `sections` array (block states + biomes). Deterministic.
  This is the mode used for parity.

## Output naming

`<server>-<seed>-<mode>.json`, e.g. `vanilla-12345-blocks.json`, `neutron-12345-blocks.json`.

## Prerequisites

- Java (server jars) + cargo.
- Server jars in `bench/servers/` (gitignored): `server-vanilla.jar`,
  `server-paper.jar`, `server-folia.jar` — see `bench/servers/README.md`.

## Related

- `neutron-hash` generates the neutron side of the comparison.
- `chunk-dump` prints the raw NBT tree of a single chunk for debugging.
