# vanilla-hash

Extract chunk checksums from a Minecraft server and compare reference data files
for 1:1 worldgen parity verification.

## What it does

- **`extract`** — boots a vanilla/paper/folia server with a seed, lets it generate
  chunks around spawn, reads the `.mca` region files, and writes one xxHash64 per
  chunk to a JSON file ("reference data").
- **`extract-all`** — batch version of `extract` for multiple seeds × server types
  (Rust replacement for the old `generate-all.sh`/`.ps1`).
- **`compare`** — diffs two reference data JSON files (matching / different /
  missing chunks). This is the single comparator for parity checks.

## Usage

```bash
# Extract one reference (vanilla server, blocks mode, deterministic)
cargo run --release -p vanilla-hash -- extract \
  --seed 12345 --server vanilla --hash-mode blocks --output hashes/vanilla-12345-blocks.json

# Batch: all seeds × available servers
cargo run --release -p vanilla-hash -- extract-all \
  --seeds 12345 67890 --servers vanilla paper

# Compare a vanilla reference against a neutron one
cargo run --release -p vanilla-hash -- compare \
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

- `vanilla-parity` generates the neutron side of the comparison.
- `chunk-dump` prints the raw NBT tree of a single chunk for debugging.
