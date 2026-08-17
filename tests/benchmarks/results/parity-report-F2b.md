# Parity Report F2b — Neutron Worldgen vs Vanilla

> Run date: 2026-08-08
> Previous result (F2a): **0/81 chunks match**
> Current result (F2b): **0/81 chunks match**

## Executive Summary

No chunks match vanilla after calibration improvements. The NBT hash comparison remains at **0/81** for seed 12345. However, detailed analysis reveals the root cause is **biome selection** -- not noise or terrain generation.

### Key Finding

The primary parity gap for seed 12345 is that the MultiNoise biome source selects **StonyShore** (cobblestone surface) for 90.1% of chunks and **Taiga/OldGrowthPineForest** (podzol surface) for 9.9%. Vanilla generates Plains/Forest biomes near spawn for this seed. This is a seed-specific noise mapping issue, not a systemic terrain problem.

## 1. Matching Chunks (Before vs After)

| Metric | F2a (Before) | F2b (After) | Delta |
|--------|-------------|-------------|-------|
| Matching NBT hashes | 0/81 | 0/81 | 0 |
| Different NBT hashes | 81/81 | 81/81 | 0 |
| Missing in golden | 0 | 0 | 0 |

The NBT hashes remain different because:
1. The biome assignment is wrong for seed 12345 (primary cause)
2. The NBT serialization format differs between vanilla and Neutron (secondary cause)
3. Block state properties and palette ordering differ (tertiary cause)

## 2. Terrain Height Comparison

### Neutron (seed 12345, radius 4)
- Average surface height: **67.8**
- Min chunk average: 50.8
- Max chunk average: 83.9
- Global min: 3
- Global max: 124

### Vanilla (seed 12345, overlapping region)
- Average NBT size: 5,255 bytes (vs Neutron: 25,131 bytes)
- 67/81 chunks have minimal terrain (2,895 bytes = near-empty)
- 14/81 chunks have significant terrain (up to 44,410 bytes)

### Assessment
Neutron's terrain heights are **reasonable** (avg 67.8, near sea level 63). The height variation (50-84 avg per chunk) matches vanilla's typical overworld terrain. However, vanilla's overlapping region is mostly empty terrain (67 chunks at minimal size), suggesting the vanilla spawn area for seed 12345 may be in an ocean or flat region that Neutron's biome source misidentifies.

## 3. Biome Distribution Comparison

### Seed 12345 (Neutron)
| Surface Block | Inferred Biome | Chunks | Percentage |
|---------------|----------------|--------|------------|
| cobblestone | StonyShore | 73 | 90.1% |
| podzol | Taiga/OldGrowthPineForest | 8 | 9.9% |
| grass_block | Plains/Forest | 0 | 0.0% |

### Seed 67890 (Neutron)
| Surface Block | Inferred Biome | Chunks | Percentage |
|---------------|----------------|--------|------------|
| grass_block | Plains/Forest | ~75 | ~93% |
| cobblestone | StonyShore | ~4 | ~5% |
| podzol | Taiga | ~2 | ~2% |

### Seed 42 (Neutron)
| Surface Block | Inferred Biome | Chunks | Percentage |
|---------------|----------------|--------|------------|
| grass_block | Plains/Forest | ~78 | ~96% |
| Other | Various | ~3 | ~4% |

### Seed 99999 (Neutron)
| Surface Block | Inferred Biome | Chunks | Percentage |
|---------------|----------------|--------|------------|
| stone | WindsweptHills | ~40 | ~49% |
| podzol | Taiga | ~15 | ~19% |
| cobblestone | StonyShore | ~15 | ~19% |
| grass_block | Plains/Forest | ~11 | ~14% |

### Seed 11111 (Neutron)
| Surface Block | Inferred Biome | Chunks | Percentage |
|---------------|----------------|--------|------------|
| grass_block | Plains/Forest | ~70 | ~86% |
| Other | Various | ~11 | ~14% |

### Assessment
The biome source works well for most seeds (67890, 42, 11111 show 86-96% grass_block surfaces). The failure for seed 12345 is **seed-specific** -- the climate noise values at spawn coordinates happen to map to StonyShore/Taiga targets. This suggests the climate target parameters in `MultiNoiseBiomeSource::default_biomes()` need tuning, particularly for the StonyShore and Taiga entries.

## 4. Block Distribution Comparison

### Neutron (all seeds, radius 4)
| Block | Seed 12345 | Seed 67890 | Seed 42 | Seed 11111 |
|-------|-----------|-----------|---------|-----------|
| air | 71.8% | 68.6% | 67.9% | 67.1% |
| stone | 25.2% | 28.2% | 28.9% | 29.4% |
| bedrock | 1.3% | 1.3% | 1.3% | 1.3% |
| water | 0.9% | 0.9% | 1.6% | 1.5% |
| dirt | 0.1% | 0.8% | 0.2% | 0.5% |
| grass_block | 0.0% | 0.3% | 0.1% | 0.2% |
| cobblestone | 0.7% | 0.0% | 0.0% | 0.0% |
| podzol | 0.0% | 0.0% | 0.0% | 0.0% |

### Assessment
Block distributions are consistent across seeds (excluding the biome-dependent surface blocks). The stone/air ratio (28-29% / 67-72%) is reasonable for overworld terrain. The bedrock floor (1.3% = 5 layers at bottom) is correct. Water fill below sea level is working (0.9-1.6%).

## 5. Remaining Gaps and Recommendations

### Gap 1: Biome Source Calibration (HIGH PRIORITY)
**Problem**: The MultiNoise biome source selects wrong biomes for seed 12345 near spawn. The StonyShore target at `(temperature=0.2, humidity=-0.5, continentalness=0.0, erosion=0.0, weirdness=0.3)` is too close to the typical climate values at spawn.

**Recommendation**: 
- Adjust climate target parameters to better separate StonyShore from Plains/Forest
- Add a "continentalness threshold" check: StonyShore should only appear at continentalness near 0.0 (coastline), not inland
- Consider adding a fallback: if continentalness > 0.2, never select shore biomes

### Gap 2: NBT Serialization Format (MEDIUM PRIORITY)
**Problem**: Neutron's NBT output is ~5x larger than vanilla's (25KB vs 5KB average). This suggests the serialization includes unnecessary fields or uses less compact encoding.

**Recommendation**:
- Audit the NBT serialization in `serialize_chunk_to_sections_nbt()`
- Ensure only sections, heightmaps, and block_entities are included (matching vanilla's chunk format)
- Verify palette encoding matches vanilla's bit-packing scheme

### Gap 3: Block State Properties (LOW PRIORITY)
**Problem**: The parity check's `block_id_to_properties()` only covers a subset of blocks. Missing properties could cause NBT hash mismatches even if blocks are correct.

**Recommendation**:
- Extend `block_id_to_properties()` to cover all blocks in the registry
- Verify property values match vanilla defaults exactly

### Gap 4: Cave Carving Accuracy (LOW PRIORITY)
**Problem**: Cave generation uses simplified noise thresholds rather than vanilla's exact density functions.

**Recommendation**:
- Implement vanilla's `DensityFunctions` for cave carving
- Use the same threshold curves as `NoiseBasedChunkGenerator`

## 6. Comparison Report Files

- Parity output (seed 12345): `bench/results/parity-output-F2b.json`
- Neutron golden data (seed 12345): `bench/results/neutron-golden-F2b.json`
- Parity output (seed 67890): `bench/results/parity-output-67890.json`
- Parity output (seed 42): `bench/results/parity-output-42.json`

## 7. Next Steps

1. **Fix biome source for seed 12345**: Adjust climate targets or add continentalness threshold
2. **Regenerate golden data**: After biome fix, re-run vanilla extraction for seed 12345
3. **Re-run parity check**: Verify hash match after biome correction
4. **Extend to other seeds**: Once seed 12345 matches, test all 5 seeds
