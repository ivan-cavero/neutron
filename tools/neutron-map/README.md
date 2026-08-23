# neutron-map

See worldgen without launching the game.

## map — render top-down surface maps

```bash
cargo run --release -p neutron-map -- map <seed> <x0,z0> <x1,z1> \
  --out PREFIX [--ref DIR] [--diff]
```

- `PREFIX-neutron.png` — neutron-generated surface (block colors shaded by Y).
- With `--ref DIR` (a vanilla `region/` dir): also `PREFIX-vanilla.png`.
- With `--diff` too: `PREFIX-diff.png` — green = surface match, red = differ,
  dark = vanilla chunk missing/stub. Plus a one-line summary
  (`N match / N differ / N missing`).

Example (whole r.0.-1 area, seed 12345):

```bash
cargo run -p neutron-map -- map 12345 "0,-32" "31,-1" --out parity_r01 \
  --ref tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region \
  --diff
```

Surface sample = column (8,8) of each chunk; status filter = vanilla chunk must
be `full`.

## tree — inspect the embedded worldgen data

```bash
neutron-map biomes                 # list embedded biome JSONs
neutron-map tree pale_garden       # biome -> steps -> placed features (+ global sorter index)
neutron-map feature minecraft:amethyst_geode   # dump placed+configured JSON
```

The tree lives in `crates/neutron-worldgen/src/data/worldgen/**` (embedded at
compile time). Edit those files to change generation — no game needed.

## regen — refresh data from a new jar

Already covered by the mc-decompiler tool:

```bash
cargo run --release -p mc-decompiler -- extract-data <version>
```

Extracts `data/minecraft/worldgen/**` from the server jar into the crate tree
and reports MATCH/CHANGED/JAR-ONLY/CRATE-ONLY per entry.
