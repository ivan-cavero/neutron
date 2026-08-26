# examples/ — parity & verification probes

Not the public API. These compare neutron-worldgen against vanilla (seed 424242)
and feed the runs' parity bar.

**The canonical meter is now `crates/neutron-parity`** (`cargo run --release
-p neutron-parity -- --ref <dir>`); see docs/PARITY.md. The examples below are
the specialized probes that remain useful around it.

`Cargo.toml` sets `autoexamples = false` — only the examples listed there
compile. The ones that survive are cited by `runs/` or AGENTS.md; one-off
investigation probes were deleted (recover from git history if needed).

```bash
cargo run --release -p neutron-worldgen --example region_parity
```

## The parity bar (used by runs)

| Example | Purpose |
| --- | --- |
| `region_parity` | Full chunk region parity vs vanilla (the meter; PARITY_SCAN/PARITY_LEDGER) |
| `biome_grid_parity` | Stored quart biomes vs climate sampler (the only biome decoder) |
| `clay_overlap` | Clay placement overlap |
| `lush_pale_parity` | Lush caves / pale garden placement |
| `density_at`, `dens9`, `raw_density`, `noodle_check`, `multi_base` | Density function shape checks |
| `marker_stats`, `list_full`, `wd_check` | Gap / marker analysis, ref inspection |
| `carver_many`, `sculk_count` | Carver / sculk counters |

## Probe support (used by worldgen-probe)

| Example | Purpose |
| --- | --- |
| `sculk_veintrace` (+ `sculk_vanworld_world` helper) | Sculk differential flow: dump ↔ probe |
| `decorate_oracle` | NDEC1 export for ProbeDecorate + last-writer-wins compare |
| `feature_index_probe`, `sorter6`, `step_scan` | FeatureSorter index ground truth (ProbeFeatureOrder/ProbeSorter6) |
| `region_random_dump`, `rng_echo`, `hash_echo` | RNG/hash stream echoes for draw-for-draw JVM diffs |

Deleted one-off probes (recover from git history): `block_parity`,
`parity_multi`, `sculk_cave`, `sculk_overlap`, `sculk_replay`, `gap_blocks`,
`density_shape`, `ref_biome*`, `seed_scan`, `ground_check`, `neigh`.

See `tools/README.md` for the tool index and `runs/` for how each was used.
