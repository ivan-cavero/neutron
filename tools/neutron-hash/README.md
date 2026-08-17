# neutron-hash

Generate Neutron chunks and emit their hashes in the same format as `ref-extract`
— the **neutron side** of the parity pipeline. Formerly `vanilla-parity`.

## What it does

- Generates chunks around spawn with `neutron-worldgen` for a given seed.
- Prints chunk statistics: block distribution, heightmap, surface block, raw hash,
  NBT hash (vanilla-compatible section serialization).
- `--generate-neutron` writes a `neutron-<seed>-blocks.json` file compatible with
  `ref-extract compare`.

## Usage

```bash
# Stats for seed 12345 (radius 8)
cargo run --release -p neutron-hash -- --seed 12345

# Emit neutron reference data for later comparison
cargo run --release -p neutron-hash -- --seed 12345 \
  --generate-neutron tools/ref-extract/hashes/neutron-12345-blocks.json

# Compare against the vanilla reference (the actual parity gate)
cargo run --release -p ref-extract -- compare \
  --left tools/ref-extract/hashes/vanilla-12345-blocks.json \
  --right tools/ref-extract/hashes/neutron-12345-blocks.json
```

The old `--golden` flag (an in-tool comparator) was removed — `ref-extract compare`
is the single comparator.

## Related

- `ref-extract` produces the vanilla reference side and the comparator.
- Worldgen examples `region_parity`, `clay_overlap`, `lush_pale_parity` measure
  block-level parity against real vanilla region files (see `runs/`).
