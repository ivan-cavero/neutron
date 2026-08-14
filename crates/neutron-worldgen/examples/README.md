# examples/ — sondas de parity F2d

No son la API pública. Cada uno se lanzó en un run para aislar un gap
contra vanilla (seed 12345, a menudo chunk 6,-2).

`Cargo.toml` pone `autoexamples = false`. Solo se compilán los examples
listados ahí (`block_parity`, `dump_ms`, `sculk_cave`, `parity_diag`,
`chunk_check`, `gap_diag`). El resto queda en disco como evidencia.

```bash
cargo run --release -p neutron-worldgen --example block_parity
```

## Los que siguen siendo útiles

| Example | Para qué |
|---|---|
| `block_parity` | Match de nombre de bloque vs NBT vanilla |
| `dump_ms` | Árbol de piezas mineshaft |
| `sculk_cave` | Overlay ChargeCursor en cueva real |
| `parity_diag` / `gap_diag` | Extra/miss por tipo de bloque |
| `chunk_check` | Sanity de un chunk suelto |

## Históricos (no borrar: evidencia de runs)

- **Noise / density:** `noise_check`, `density_*`, `debug_final*`, `blended_*`, `shift_check`, `router_check`
- **Carvers:** `carver_*`
- **Biomas:** `biome_*`, `van_biome_at`
- **Ores / andesite:** `andesite_*`, `r12_counts`
- **Sculk:** `sculk_*` (excepto `sculk_cave`)
- **Vegetación:** `veg_pos`, `tree_gap`
- **Estructura del chunk:** `chunk_blocks`, `chunk_struct*`, `nbt_dump`

Si un example no compila, se repara o se deja como nota en el run que lo usó.
No se “limpian” para que el historial de `runs/run-0xx.md` siga siendo
reproducible.
