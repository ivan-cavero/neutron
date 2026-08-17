# examples/ — parity & verification probes

Not the public API. These compare neutron-worldgen against vanilla (seed 12345,
often chunk 6,-2) and feed the runs' parity bar.

`Cargo.toml` sets `autoexamples = false` — only the examples listed there
compile. The ones that survive are cited by `runs/` or AGENTS.md; one-off
investigation probes were deleted (recover from git history if needed).

```bash
cargo run --release -p neutron-worldgen --example region_parity
```

## The parity bar (used by runs)

| Example | Purpose |
| --- | --- |
| `region_parity` | Full chunk region parity vs vanilla |
| `clay_overlap` | Clay placement overlap |
| `lush_pale_parity` | Lush caves / pale garden placement |
| `parity_multi` / `block_parity` | Multi-seed / block-name parity |
| `density_shape`, `density_at` | Density function shape checks |
| `gap_blocks`, `marker_stats`, `list_full`, `multi_base` | Gap / marker analysis |
| `carver_many`, `sculk_count`, `sculk_overlap` | Carver / sculk counters |

## Probe support (used by worldgen-probe)

| Example | Purpose |
| --- | --- |
| `sculk_cave`, `sculk_replay`, `sculk_veintrace` (+ `sculk_vanworld_world` helper) | Sculk differential flow: dump ↔ probe |
| `feature_index_probe` | FeatureSorter index ground truth (ProbeFeatureOrder) |

See `tools/README.md` for the tool index and `runs/` for how each was used.
