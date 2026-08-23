# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 23 Aug 2026.**

## Now

Worldgen 1:1 vs vanilla **26.2**. HEAD: steps 1/2/10 wired + geode dead-branch fix
+ `BlockId::CaveAir` + biome-climate freeze gate.

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **97.69%** r=1 (-0.01: honest mineshaft labels expose piece-layout desync) | was 97.70 with masked Air |
| 12345 | **98.28%** center (6,-2) r=1 (+0.05; was 98.23) | geodes now match |
| 777 | 99.38% chunk (0,0) only full chunk on disk | |

- Geodes: wired step 2 (LOCAL_MODIFICATIONS). Vanilla's last layer branch is DEAD
  CODE (`else if >= outerCrust` after negated guard) — port kept verbatim; cells
  below inner_crust stay terrain. Both ref geodes match (~161 residual air->stone).
- Lakes (step 1) + freeze_top_layer (step 10) wired. Freeze/lake-ice gate =
  biome base `temperature < 0.15` from JSON (`feature_catalog::biome_climate`),
  NOT router temperature noise. ponytail: FROZEN modifier + >y80 noise term need
  PerlinSimplexNoise(1234L/3456L) ports.
- Step 3 (monster_room/fossil) **stays OFF**: rooms place where vanilla's 10
  attempts reject (evidence 424242 (1,-1): 250 wrong cave_air at y=4). Suspect
  uniform-height draw order vs `Mth.randomBetweenInclusive`. Next: ProbeYAnchor.
- Mineshaft pieces write `CAVE_AIR` (vanilla). Piece LAYOUT diverges from vanilla:
  424242 (1,-1) has a vanilla corridor y≈-14 we don't build; 12345 corridors match.
  Fixing layout converts 424242 -0.01 into gain (proven by 12345 +0.05 same code).
- lush/pale recall ~59.8% · trees per-pipeline exact, cascade over non-100% base.

## Perf (this machine, release)

- Decoration biome union memoized per origin (`origin_biome_union_memo`) +
  quart grids stored in `RegionBuf::put_chunk_biomes` / `stored_noise_biome`.
  Was ~750 ms per apply_step_origin call → 0-60 ms first, then free.
- block_parity single-chunk gen: 50 s → 11.5 s. Remaining hotspots: noise+surface
+carvers for the 5×5 buffer (~9 s), veg step ~125 ms ×9 origins.
- `NEUTRON_STEP_TIMING=1` prints per-origin per-step ms. `[geode]` trace via
  `NEUTRON_GEODE_TRACE`.

## Next (one question)

Mineshaft piece-layout parity vs vanilla (salt/spacing/probability + piece RNG):
closes 424242 -0.01 AND unlocks step 3 re-enable path. Then ProbeYAnchor for
uniform-height draw order (monster_room count=10).

## Dead (do not reopen without a new two-sided dump)

Chunk decoration order · cave-biome stored-grid vs voronoi · `vegetation_patch`
HashSet · noodle sign · T4 feature ports as recall lever · carvers write cave_air
(overworld carvers write plain AIR; refs' cave_air comes from structures) ·
geode outer-layer branch reachable (it is dead code in 26.2 jar) · freeze gate as
router-temperature (biome attribute is correct) · worm start-Y desync ·
`placeGround` already-ground joining surface set.

## This machine

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
12345   tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region
777     tools/nbt-ref/vanilla-fresh-777/world/dimensions/minecraft/overworld/region
jar     tools/mc-decompiler/jars/server-26.2.jar
java    tools/mc-decompiler/output/26.2/src
```
