# chunk-dump

Print the full NBT tree of a single chunk from an Anvil `.mca` region file.
A debugging/exploration tool for inspecting what a vanilla (or neutron) server
actually wrote.

## Usage

```bash
cargo run --release -p chunk-dump -- <region.mca> <local_cx> <local_cz> [--dump-longs] [--verbose]
```

- `<region.mca>` — path to a region file (e.g. a vanilla reference world under
  `tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/`).
- `<local_cx> <local_cz>` — chunk coordinates *inside* the region (0..31).
- `--dump-longs` — also print the raw packed long arrays (block/biome palettes).
- `--verbose` — more detail.

## Related

- `ref-extract` reads the same region files to hash chunks in bulk.
