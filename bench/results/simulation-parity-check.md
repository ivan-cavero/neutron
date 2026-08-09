# Simulation Parity Check: Neutron vs Vanilla Minecraft 26.2

> Date: 2026-08-09
> Scope: `crates/neutron-sim/src/` — redstone, light, fluid, spawn
> Method: Code audit of Neutron implementations against vanilla mechanics documented on minecraft.wiki and community references.

---

## 1. Redstone (`crates/neutron-sim/src/redstone.rs`)

### 1.1 PP (Post-Placement) Update Order

| | Vanilla | Neutron | Match |
|---|---|---|---|
| PP order | W, E, N, S, D, U | W, E, N, S, D, U | YES |

Source: minecraft.wiki — Redstone wire post-placement notification order in Java Edition 1.21+ is West, East, North, South, Down, Up.

### 1.2 NC (Neighbor-Change) Update Order

| | Vanilla | Neutron | Match |
|---|---|---|---|
| NC order | N, S, W, E, D, U | W, E, D, U, N, S | **NO** |

**GAP**: Neutron uses `WIRE_NC_ORDER = [W, E, D, U, N, S]` but vanilla Java Edition uses `N, S, W, E, D, U`. This affects update propagation priority and can cause different redstone behavior in edge-case circuits where order matters.

Source: minecraft.wiki redstone wire neighbor-change notification order.

### 1.3 Torch Burnout

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Threshold | "forced to turn off **more than 8** times" = 9+ | `>= 8` changes | **NO** |

**GAP**: Vanilla torch burns out at **9 or more** state changes in 60 ticks. Neutron triggers burnout at **8 or more**. The difference of 1 change means some valid torch clocks in vanilla will burn out prematurely in Neutron.

Additionally, vanilla burned-out torches:
- Stay off until the count drops below 8 in the 60-tick window
- May automatically try to relight after 160 ticks (8 seconds) in Java Edition

Neutron's behavior:
- Burns out at >= 8 (should be > 8, i.e., >= 9)
- Removes `torch_changes` history on burnout (vanilla does NOT do this — it keeps the history and only resets when the window passes)
- No relight attempt after 160 ticks

Source: minecraft.wiki/Redstone_torch — "forced to turn off more than eight times in 60 game ticks."

### 1.4 Quasi-Connectivity

| | Vanilla | Neutron | Match |
|---|---|---|---|
| QC | Pistons, dispensers, droppers activated by power 1 block above | Not implemented | **NO** |

**GAP**: Quasi-connectivity (QC) is a fundamental Java Edition mechanic. Pistons, dispensers, and droppers check for power 1 block above their position (like doors check 2 blocks tall). Neutron has no QC support. This means any redstone contraption relying on QC (BUD switches, bud-powered pistons, etc.) will not work.

Source: minecraft.wiki — "In Java Edition, dispensers, droppers, and pistons can be activated by a signal supplied to the block above them."

### 1.5 Wire Power Propagation

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Max range | 15 blocks from source | 15 blocks (saturating_sub) | YES |
| Power decrease | -1 per wire | -1 per wire | YES |
| Source power | 15 (lever, torch) | 15 | YES |

### 1.6 Double Doors

Neutron's double door logic toggles adjacent doors in all 4 horizontal directions. Vanilla double doors are determined by the `direction` and `hinge` block state properties — only doors sharing the same X/Z and differing only in hinge side count as a double door pair. Neutron's approach is a simplification that may cause false positives (opening a non-paired adjacent door).

### 1.7 Vanilla Bugs to Replicate

- **Torch burnout relight**: After 160 ticks, vanilla torches automatically attempt to turn back on. Neutron does not implement this.
- **Quasi-connectivity**: This is intentional in vanilla Java Edition and used by countless contraptions. Must be replicated for parity.

---

## 2. Lighting (`crates/neutron-sim/src/light.rs`)

### 2.1 Sky Light Initialization

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Top of world | 15 | 15 (MAX_LIGHT) | YES |
| Through air | Inherits from above | Inherits from above | YES |
| Through opaque | Blocked (0) | Set to 0 | YES |
| Through transparent | Reduces by 1 | Reduces by 1 | YES |

### 2.2 Sky Light Propagation Model

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Downward through air | No reduction | No reduction | YES |
| Horizontal/upward | Reduces by 1 per block | **Not implemented** | **NO** |

**GAP**: Neutron's `init_sky_light` only does a **vertical column pass** (top-down per-column). It does NOT propagate sky light horizontally. In vanilla, sky light at level 15 spreads 15 blocks horizontally from any position with sky-exposed light, reducing by 1 per block. Neutron only handles vertical inheritance — sky light does not spread sideways at all.

This means caves near the surface will have incorrect sky light levels. A cave entrance that should let light 15 blocks deep will only show correct light at positions directly above opaque blocks.

### 2.3 Block Light Propagation

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Algorithm | BFS flood fill | BFS flood fill | YES |
| Decrease rate | -1 per block | -1 per block | YES |
| 6-direction | Yes | Yes | YES |
| Opaque blocking | Stops propagation | Stops propagation | YES |

### 2.4 Light Emission Values

| Block | Vanilla | Neutron | Match |
|---|---|---|---|
| Glowstone | 15 | 15 | YES |
| Torch | 14 | 14 (id=50) | YES |
| Lantern | 15 | 14 (id=76) | **NO** |
| Sea lantern | 15 | 15 | YES |
| Redstone lamp | 15 | 15 | YES |
| Fire | 15 | 13 (id=51) | **NO** |
| Redstone torch | 7 | 0 (not in emission map) | **NO** |
| Soul torch | 10 | 0 (not in emission map) | **NO** |
| Brewing stand | 1 | 14 | **NO** |
| Enchanting table | 7 | 0 (not in emission map) | **NO** |
| Brown mushroom | 1 | 0 (not in emission map) | **NO** |
| End rod | 14 | 0 (not in emission map) | **NO** |
| Campfire (lit) | 15 | 0 (not in emission map) | **NO** |
| Shroomlight | 15 | 0 (not in emission map) | **NO** |
| Conduit | 15 | 0 (not in emission map) | **NO** |

**GAP**: The `light_emission()` function in `block.rs` is missing many light sources. Key mis-matches:
- Lantern emits 15 in vanilla, 14 in Neutron
- Fire emits 15 in vanilla, 13 in Neutron
- Brewing stand emits 1 in vanilla, 14 in Neutron
- Many common sources (soul torch, end rod, campfire, shroomlight, conduit, redstone torch, enchanting table) are not mapped at all (return 0)

### 2.5 Light-Filtering Blocks

Vanilla distinguishes between:
- **Fully transparent** (glass, air): light passes through without reduction
- **Light-filtering** (water, leaves, ice): reduce sky light by 1 per block, but block light passes through normally

Neutron treats all non-opaque blocks as "transparent" with a flat `is_transparent()` check. It does not distinguish light-filtering from fully transparent. This means:
- Glass reduces block light by 1 (should not reduce it at all)
- Water reduces block light by 1 (should not reduce block light)
- The vertical sky light pass does correctly reduce through "transparent" blocks, which partially matches, but horizontal propagation (if it existed) would be wrong

### 2.6 Incremental Updates

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Block change | Recomputes affected sections | `remove_light_around` + re-propagate | PARTIAL |

Neutron's `on_block_change` clears all block light within a Manhattan-radius of 15, then re-propagates from the new source. This is a brute-force approach that works but is O(radius^3) per block change. Vanilla (and Starlight) use a more surgical approach that only recalculates affected light values.

Additionally, Neutron's `on_block_change` only re-propagates block light for the new block's emission. It does NOT re-propagate sky light after a block change (it marks sections dirty but never re-computes them).

### 2.7 Vanilla Bugs to Replicate

- Sky light propagation through light-filtering blocks (water reduces sky light by 1, but NOT block light)
- Sky light level 15 propagates downward through transparent blocks without reduction (water column mechanics)

---

## 3. Fluids (`crates/neutron-sim/src/fluid.rs`)

### 3.1 Water Spread Rate

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Horizontal spread speed | 1 block per **5 ticks** | 1 block per **1 tick** | **NO** |

**GAP**: Neutron water spreads 5x faster than vanilla. `spread_delay` for water is 1 tick, but vanilla water spreads 1 block every 5 ticks.

Source: minecraft.wiki — "Water spreads at a rate of 1 block every 5 game ticks."

### 3.2 Lava Spread Rate (Overworld)

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Horizontal spread speed | 1 block per **30 ticks** | 1 block per **5 ticks** | **NO** |

**GAP**: Neutron lava spreads 6x faster than vanilla overworld lava. `spread_delay` for lava is 5 ticks, but vanilla overworld lava spreads 1 block every 30 ticks.

Source: minecraft.wiki — "Lava flows at a rate of 30 ticks per block in the Overworld."

### 3.3 Spread Distance Limits

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Water max horizontal | 7 blocks | **No limit** | **NO** |
| Lava max horizontal (overworld) | 4 blocks | **No limit** | **NO** |
| Lava max horizontal (nether) | 8 blocks | **No limit** | **NO** |

**GAP**: Neutron has no maximum spread distance. Water will flow indefinitely, and lava will spread much further than vanilla. The `spread_fluid` and `spread_from_source` methods do not check the current flow level against a maximum.

Actually, the level DOES decrease by 1 per horizontal block (from the source level of 8 down to 1), so water naturally stops at 7 blocks. But lava starts at level 8 and decreases by 1, so it would also stop at 7 horizontal blocks. This is actually correct for horizontal spread in the overworld (lava max = 4 blocks, but the level system limits it to 7... wait, let me re-check).

Actually, re-reading the code: the level starts at 8 (source) and horizontal flow creates blocks with `level - 1`. So a source creates flowing blocks at level 7, those create at level 6, etc. The last flowing block has level 1, which is at distance 7 from the source. This matches water's vanilla max of 7 blocks.

For lava, vanilla limits horizontal spread to 4 blocks, but Neutron's level system allows 7 blocks (level 8 -> 1). The level system does not enforce the vanilla 4-block limit for lava.

### 3.4 Downward Flow (Waterfall)

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Downward flow keeps level | Yes (level 8 downward = waterfall) | Yes (level 8 for source-down, level for flowing-down) | YES |

### 3.5 Bubble Columns

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Soul sand | Upward push | Upward (velocity +0.7) | YES |
| Magma block | Downward pull | Downward (velocity -0.25) | YES |
| Detection | Water above soul sand/magma | Checks below block | YES |

### 3.6 Waterlogging

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Waterloggable blocks | ~60+ block types | 11 block types | **NO** |

**GAP**: Neutron's `WATERLOGGABLE_BLOCKS` list contains only 11 block IDs. Vanilla waterlogs many more blocks including: all stairs, slabs, fences, walls, glass panes, iron bars, buttons, signs, trapdoors, ladders, rails, carpets, skulls, banners, beds, end rods, chains, lanterns, bells, flower pots, cauldrons, brewing stands, scaffolding, Amethyst clusters, copper grate, copper bulb, etc.

Key missing categories:
- Trapdoors (all types)
- Ladders
- Iron bars
- Trapdoors
- Chains
- Scaffolding
- Coral fans and blocks
- Amethyst clusters

### 3.7 Flow Direction Calculation

Neutron's `get_flow_direction` checks for the steepest descent. In vanilla, flow direction is calculated by checking the fluid level at the 4 horizontal neighbors and averaging them to produce a weighted direction vector. Neutron picks the single steepest neighbor, which may produce different entity-pushing behavior.

### 3.8 Vanilla Bugs to Replicate

- Waterlogging: in vanilla, waterlogging a block with waterlogged=true requires the block's waterlogged property to be set. Neutron stores this in the fluid engine but doesn't modify the block's blockstate.
- Lava spread in the Nether is faster (10 ticks/block) and travels 8 blocks. Neutron does not differentiate dimensions.

---

## 4. Mob Spawning (`crates/neutron-sim/src/spawn.rs`)

### 4.1 Hostile Light Level Check

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Overworld | sky_light <= 7 **AND** block_light == 0 | max(sky, block) < 7 | **NO** |

**GAP**: Neutron combines sky and block light using `max(sky_light, block_light) < 7`. Vanilla uses TWO separate conditions:
1. Internal sky light level <= 7
2. Block light level == 0

This means in Neutron, a monster could spawn in a position with sky_light=6 and block_light=6 (max=6 < 7), but vanilla would NOT allow this spawn because block_light must be exactly 0. Conversely, Neutron would block a spawn at sky_light=0, block_light=6 (max=6 < 7... wait, 6 < 7 is true, so it WOULD allow it). Actually, let me re-check: vanilla requires sky_light <= 7 AND block_light == 0. Neutron requires max(sky, block) < 7.

The difference: vanilla requires block_light == 0, Neutron allows block_light up to 6 (as long as max < 7). This means Neutron allows hostile spawns in areas with block light 1-6 that vanilla would not allow.

Source: minecraft.wiki — "In the Overworld and the End, hostile mobs spawn only if the internal sky light level is 7 or less and the block light level is 0."

### 4.2 Passive Light Level Check

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Creature spawn | Sky light > 7 | max(sky, block) > 7 | **NO** |

**GAP**: Similar issue. Vanilla checks sky light for passive mobs. Neutron checks max(sky, block). A creature could spawn in a dark cave with a single torch (block_light=14) in Neutron, but vanilla requires sky light > 7 (meaning outdoor/daylight).

### 4.3 Spawn Distance

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Minimum | 24 blocks | 24 blocks | YES |
| Maximum | 128 blocks | 128 blocks | YES |
| Measurement | 3D Euclidean | 3D Euclidean | YES |

### 4.4 Spawn Cap

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Monster | 70 | 70 | YES |
| Creature | 10 per player | 10 per player | YES |
| Ambient | 15 | 15 | YES |
| Water creature | 5 per player | 5 per player | YES |
| Water ambient | **20** per player | **5** per player | **NO** |
| Underground creature | **5** per player | **10** per player | **NO** |
| Axolotl | 5 per player | 5 per player | YES |

**GAP**: Water ambient cap is 20 in vanilla, 5 in Neutron. Underground creature cap is 5 in vanilla, 10 in Neutron.

Source: minecraft.wiki/Mob_spawning — Java Edition category caps.

### 4.5 Despawn Mechanics

| | Vanilla | Neutron | Match |
|---|---|---|---|
| > 128 blocks | Instant despawn | Instant despawn | YES |
| 32-128 blocks | 1/800 chance per tick (after 30s no-player-nearby) | Linear distance-based probability | **NO** |
| < 32 blocks | Never despawn | Never despawn | YES |

**GAP**: Neutron uses a deterministic linear probability based on distance: `chance = (d - 32) / 96`. Vanilla uses a flat 1/800 chance per tick, with an additional requirement that no player has been within 32 blocks for more than 30 seconds. The vanilla mechanic is time-dependent, not distance-dependent.

### 4.6 Pack Spawning Range

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Pack offset | +-5 on X/Z (triangular distribution) | +-1 on X/Z (grid pattern) | **NO** |

**GAP**: Vanilla packs have a center point with individual mobs offset by +-5 blocks on X and Z using a triangular distribution (so most are close to center, fewer at edges). Neutron places mobs on a small grid pattern `(i % 3) - 1` which only gives offsets of -1, 0, +1.

Source: minecraft.wiki — "Individual spawn locations are offset by plus or minus 5 blocks on the X and Z axes from the pack center using a triangular distribution."

### 4.7 Spawn Cycle Frequency

| | Vanilla | Neutron | Match |
|---|---|---|---|
| Hostile spawn cycle | Every tick (1/20s) | Every tick | YES |
| Passive spawn cycle | Every 400 ticks (20s) | Every 400 ticks | YES |
| Max attempts per cycle | 3 pack attempts per category per eligible chunk | 1 attempt per player per category | **NO** |

**GAP**: Vanilla attempts up to 3 pack spawns per mob category per eligible chunk per cycle. Neutron only makes 1 attempt per player per category per tick.

### 4.8 Biome Mob Lists

Neutron's biome-specific mob lists are incomplete:

**Missing hostile mobs:**
- Pillager (spawns in patrols, not regular spawning)
- Vindicator (woodland mansions)
- Evoker (woodland mansions)
- Ravager (raids)
- Phantom (spawns above players who haven't slept)
- Blaze (nether)
- Ghast (nether)
- Magma Cube (nether)
- Wither Skeleton (nether fortress)
- Piglin (nether)
- Hoglin (nether)
- Strider (nether lava)
- Slime (swamps and specific Y levels)

**Missing passive mobs:**
- Wolf (forest, taiga)
- Cat (various biomes)
- Parrot (jungle)
- Llama (savanna, windswept hills)
- Turtle (beach)
- Goat (stony peaks, jagged peaks)
- Frog (swamp, mangrove swamp)
- Allay (pillager outpost, woodland mansion)

**Missing water mobs:**
- Glow squid (underground water)

### 4.9 Vanilla Bugs to Replicate

- **30-second despawn timer**: Vanilla despawn only starts the 1/800 chance after no player has been within 32 blocks for 30 seconds. Neutron has no time-based component.
- **Persistent mobs**: Named mobs, tamed mobs, and mobs that have been interacted with never despawn. Neutron does not check for persistence.
- **Pack spawning triangular distribution**: The +-5 offset uses a triangular distribution, not uniform.

---

## 5. Summary of Critical Gaps

### High Priority (affects core gameplay)

| # | System | Gap | Impact |
|---|---|---|---|
| 1 | **Fluid** | Water spreads 5x too fast (1 tick vs 5 ticks) | All water mechanics wrong — redstone farms, water elevators, channels |
| 2 | **Fluid** | Lava spreads 6x too fast (5 ticks vs 30 ticks) | Lava barriers, cobblestone generators broken |
| 3 | **Redstone** | NC update order wrong (W,E,D,U,N,S vs N,S,W,E,D,U) | Redstone circuits behave differently in edge cases |
| 4 | **Redstone** | No quasi-connectivity | Many Java Edition contraptions impossible |
| 5 | **Spawn** | Light check uses max(sky,block) instead of sky<=7 AND block==0 | Hostiles spawn in lit caves, passives spawn underground |
| 6 | **Light** | No horizontal sky light propagation | Caves near surface have wrong light levels |
| 7 | **Spawn** | Despawn is distance-based, not time-based | Mobs despawn too predictably, no 30s grace period |

### Medium Priority (affects specific mechanics)

| # | System | Gap | Impact |
|---|---|---|---|
| 8 | **Redstone** | Torch burnout threshold off by 1 (8 vs 9) | Some torch clocks burn out prematurely |
| 9 | **Light** | Many light emission values wrong/missing | Blocks emit wrong light levels |
| 10 | **Fluid** | Waterloggable blocks list incomplete (11 vs 60+) | Many blocks cannot be waterlogged |
| 11 | **Spawn** | Water ambient cap wrong (5 vs 20) | Too few fish |
| 12 | **Spawn** | Underground creature cap wrong (10 vs 5) | Too many glow squid |
| 13 | **Spawn** | Pack spawning offset too small (+-1 vs +-5) | Mobs spawn in too-tight clusters |
| 14 | **Spawn** | Missing many biome-specific mobs | Empty biomes |
| 15 | **Light** | Light-filtering vs fully-transparent not distinguished | Water/leaves reduce block light when they shouldn't |

### Low Priority (nice to have)

| # | System | Gap | Impact |
|---|---|---|---|
| 16 | **Fluid** | Flow direction uses steepest-neighbor, not weighted average | Entity push direction slightly off |
| 17 | **Redstone** | Double door detection oversimplified | Opens wrong adjacent doors |
| 18 | **Redstone** | No torch burnout relight after 160 ticks | Burned-out torches never recover |
| 19 | **Light** | No sky light re-propagation on block change | Sky light stale after breaking blocks |
| 20 | **Spawn** | No persistent mob check | Named/tamed mobs can despawn |

---

## 6. Recommended Fixes (Priority Order)

### Fix 1: Fluid spread rates (CRITICAL)
```rust
// fluid.rs spread_delay
fn spread_delay(fluid_type: FluidType) -> u32 {
    match fluid_type {
        FluidType::Water => 5,   // 1 block per 5 ticks (vanilla)
        FluidType::Lava => 30,   // 1 block per 30 ticks (overworld)
    }
}
```

### Fix 2: Hostile light check (CRITICAL)
```rust
// spawn.rs meets_light_requirements
MobCategory::Monster => {
    let sky = world.get_sky_light(pos.0, pos.1, pos.2);
    let block = world.get_block_light(pos.0, pos.1, pos.2);
    sky <= 7 && block == 0
}
```

### Fix 3: NC update order (HIGH)
```rust
// redstone.rs WIRE_NC_ORDER
pub const WIRE_NC_ORDER: [Direction; 6] = [
    Direction::N,
    Direction::S,
    Direction::W,
    Direction::E,
    Direction::D,
    Direction::U,
];
```

### Fix 4: Torch burnout threshold (HIGH)
```rust
// redstone.rs check_torch_burnout
// Change threshold from 8 to 9 (vanilla: "more than 8")
const TORCH_BURNOUT_THRESHOLD: u8 = 9;
// Also: do NOT remove torch_changes on burnout — vanilla keeps history
```

### Fix 5: Spawn caps (MEDIUM)
```rust
// spawn.rs new()
caps.insert(MobCategory::WaterAmbient, 20);    // was 5
caps.insert(MobCategory::UndergroundCreature, 5); // was 10
```

### Fix 6: Light emission values (MEDIUM)
Add missing sources to `block::light_emission()` and fix incorrect values (lantern 15, fire 15, brewing stand 1, redstone torch 7, soul torch 10, end rod 14, campfire 15, shroomlight 15, enchanting table 7).

### Fix 7: Waterloggable blocks list (MEDIUM)
Expand `WATERLOGGABLE_BLOCKS` to include all vanilla waterloggable block types (~60+ IDs).

### Fix 8: Pack spawning offset (LOW)
Replace the `(i % 3) - 1` grid with a triangular distribution offset in range [-5, +5].

### Fix 9: Quasi-connectivity (MEDIUM-HIGH)
Implement QC check for pistons, dispensers, and droppers — check power at `(x, y+1, z)` in addition to `(x, y, z)`.

### Fix 10: Sky light horizontal propagation (MEDIUM)
Add BFS propagation for sky light, not just vertical column inheritance. Sky light at level 15 should spread horizontally from exposed positions, reducing by 1 per block through light-filtering blocks.

---

## 7. Files to Modify

| File | Changes |
|---|---|
| `crates/neutron-sim/src/fluid.rs` | Fix spread_delay (5 and 30), add lava max distance (4), expand waterloggable list |
| `crates/neutron-sim/src/spawn.rs` | Fix light check (sky<=7 AND block==0), fix caps, add 30s despawn timer, expand biome mobs |
| `crates/neutron-sim/src/redstone.rs` | Fix NC order, fix burnout threshold (9), add QC, add torch relight |
| `crates/neutron-sim/src/light.rs` | Add horizontal sky light BFS, fix on_block_change for sky light |
| `crates/neutron-sim/src/block.rs` | Fix light_emission values, add missing sources |
