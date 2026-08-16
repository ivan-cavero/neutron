# examples/ — F2d parity probes

Not the public API. Each was launched in a run to isolate a gap against vanilla
(seed 12345, often chunk 6,-2).

`Cargo.toml` sets `autoexamples = false`. Only the examples listed there compile
(`block_parity`, `dump_ms`, `sculk_cave`, `parity_diag`, `chunk_check`,
`gap_diag`). The rest stay on disk as evidence.

```bash
cargo run --release -p neutron-worldgen --example block_parity
```

## Still useful

| Example | Purpose |
| --- | --- |
| `block_parity` | Block-name match vs vanilla NBT |
| `dump_ms` | Mineshaft piece tree |
| `sculk_cave` | ChargeCursor overlay on real cave |
| `parity_diag` / `gap_diag` | Extra/miss by block type |
| `chunk_check` | Sanity of a loose chunk |

## Historical (do not delete: run evidence)

- **Noise / density:** `noise_check`, `density_*`, `debug_final*`, `blended_*`, `shift_check`, `router_check`
- **Carvers:** `carver_*`
- **Biomes:** `biome_*`, `van_biome_at`
- **Ores / andesite:** `andesite_*`, `r12_counts`
- **Sculk:** `sculk_*` (except `sculk_cave`)
- **Vegetation:** `veg_pos`, `tree_gap`
- **Chunk structure:** `chunk_blocks`, `chunk_struct*`, `nbt_dump`

If an example doesn't compile, fix it or leave a note in the run that used it.
They are not "cleaned up" so the `runs/run-0xx.md` history stays reproducible.
