// params-packer/builder.rs: faithful transcription of vanilla 26.2
// `OverworldBiomeBuilder.addBiomes` emission order (first-strict-minimum
// linear search in `Climate.ParameterList` makes the order load-bearing).
//
// Source of truth:
//   tools/mc-decompiler/output/26.2/src/net/minecraft/world/level/biome/
//     OverworldBiomeBuilder.java  (constants L24-71, tables L72-110,
//     emission tree L137-145, methods L200-1022, pickers L902-985)
//     Climate.java                (quantizeCoord L77-79, Parameter.span L111-129)
//   MultiNoiseBiomeSourceParameterList.java (overworld preset wiring)
//
// Quantization: `Climate.quantizeCoord(f) = (long)(f * 10000.0F)` — literals
// are parsed as f32 and multiplied as f32, then truncated toward zero. Do NOT
// do this math in f64; the double rounding differs for values like -0.11.
//
// `span(Parameter, Parameter)` joins RAW longs: `(a.min, b.max)`, never
// requantized (e.g. swamp continentalness = (-1100, 10000)).
//
// Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::format::Record;

/// Quantized interval `[min, max]` for one climate dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param(pub i64, pub i64);

/// `Climate.quantizeCoord`: `(long)(f * 10000.0F)`, all arithmetic in f32.
const fn q(f: f32) -> i64 {
    (f * 10000.0f32) as i64
}

/// `Climate.Parameter.point`.
const fn point(f: f32) -> Param {
    let v = q(f);
    Param(v, v)
}

/// `Climate.Parameter.span(f32, f32)`: quantize both endpoints independently.
const fn span(min: f32, max: f32) -> Param {
    Param(q(min), q(max))
}

/// `Climate.Parameter.span(Parameter, Parameter)`: raw-long union `(a.min, b.max)`.
const fn span_join(a: Param, b: Param) -> Param {
    assert!(a.0 <= b.1, "span join min > max");
    Param(a.0, b.1)
}

impl Param {
    fn push(self, intervals: &mut [i64; 12], dim: usize) {
        intervals[dim * 2] = self.0;
        intervals[dim * 2 + 1] = self.1;
    }
}

// ---------------------------------------------------------------------------
// Biome ids: Neutron-internal ids from crates/neutron-worldgen/src/biome/source.rs
mod biome {
    pub const OCEAN: u8 = 0;
    pub const DESERT: u8 = 2;
    pub const PLAINS: u8 = 1;
    pub const FOREST: u8 = 3;
    pub const TAIGA: u8 = 4;
    pub const SWAMP: u8 = 5;
    pub const RIVER: u8 = 6;
    pub const BEACH: u8 = 7;
    pub const DEEP_OCEAN: u8 = 8;
    pub const SNOWY_PLAINS: u8 = 9;
    pub const STONY_SHORE: u8 = 13;
    pub const SAVANNA: u8 = 11;
    pub const JUNGLE: u8 = 10;
    pub const DARK_FOREST: u8 = 12;
    pub const MEADOW: u8 = 14;
    pub const FROZEN_RIVER: u8 = 16;
    pub const FROZEN_OCEAN: u8 = 15;
    pub const ICE_SPIKES: u8 = 17;
    pub const OLD_GROWTH_BIRCH_FOREST: u8 = 18;
    pub const OLD_GROWTH_PINE_FOREST: u8 = 19; // vanilla OLD_GROWTH_PINE_TAIGA
    pub const WINDSWEPT_HILLS: u8 = 20;
    pub const GROVE: u8 = 21;
    pub const SNOWY_SLOPES: u8 = 22;
    pub const JAGGED_PEAKS: u8 = 23;
    pub const FROZEN_PEAKS: u8 = 24;
    pub const STONY_PEAKS: u8 = 25;
    pub const BADLANDS: u8 = 26;
    pub const ERODED_BADLANDS: u8 = 27;
    pub const WOODED_BADLANDS: u8 = 28;
    pub const MUSHROOM_FIELDS: u8 = 29;
    pub const CHERRY_GROVE: u8 = 30;
    pub const DEEP_DARK: u8 = 31;
    pub const MANGROVE_SWAMP: u8 = 32;
    pub const BIRCH_FOREST: u8 = 33;
    pub const LUSH_CAVES: u8 = 34;
    pub const DRIPSTONE_CAVES: u8 = 35;
    pub const SULFUR_CAVES: u8 = 36;
    pub const DEEP_FROZEN_OCEAN: u8 = 37;
    pub const DEEP_COLD_OCEAN: u8 = 38;
    pub const COLD_OCEAN: u8 = 39;
    pub const DEEP_LUKEWARM_OCEAN: u8 = 40;
    pub const LUKEWARM_OCEAN: u8 = 41;
    pub const WARM_OCEAN: u8 = 42;
    pub const SNOWY_BEACH: u8 = 43;
    pub const WINDSWEPT_FOREST: u8 = 44;
    pub const WINDSWEPT_GRAVELLY_HILLS: u8 = 45;
    pub const WINDSWEPT_SAVANNA: u8 = 46;
    pub const SAVANNA_PLATEAU: u8 = 47;
    pub const SPARSE_JUNGLE: u8 = 48;
    pub const BAMBOO_JUNGLE: u8 = 49;
    pub const SUNFLOWER_PLAINS: u8 = 50;
    pub const FLOWER_FOREST: u8 = 51;
    pub const OLD_GROWTH_SPRUCE_TAIGA: u8 = 52;
    pub const SNOWY_TAIGA: u8 = 53;
    pub const PALE_GARDEN: u8 = 54;
}

use biome::*;

// ---------------------------------------------------------------------------
// Climate.Parameter constants (OverworldBiomeBuilder L38-71).

const FULL_RANGE: Param = Param(-10_000, 10_000); // span(-1.0F, 1.0F)

const TEMPERATURES: [Param; 5] = [
    span(-1.0, -0.45),
    span(-0.45, -0.15),
    span(-0.15, 0.2),
    span(0.2, 0.55),
    span(0.55, 1.0),
];

const HUMIDITIES: [Param; 5] = [
    span(-1.0, -0.35),
    span(-0.35, -0.1),
    span(-0.1, 0.1),
    span(0.1, 0.3),
    span(0.3, 1.0),
];

const EROSIONS: [Param; 7] = [
    span(-1.0, -0.78),
    span(-0.78, -0.375),
    span(-0.375, -0.2225),
    span(-0.2225, 0.05),
    span(0.05, 0.45),
    span(0.45, 0.55),
    span(0.55, 1.0),
];

const FROZEN_RANGE: Param = TEMPERATURES[0];
const UNFROZEN_RANGE: Param = span_join(TEMPERATURES[1], TEMPERATURES[4]);

const MUSHROOM_FIELDS_CONTINENTALNESS: Param = span(-1.2, -1.05);
const DEEP_OCEAN_CONTINENTALNESS: Param = span(-1.05, -0.455);
const OCEAN_CONTINENTALNESS: Param = span(-0.455, -0.19);
const COAST_CONTINENTALNESS: Param = span(-0.19, -0.11);
const INLAND_CONTINENTALNESS: Param = span(-0.11, 0.55);
const NEAR_INLAND_CONTINENTALNESS: Param = span(-0.11, 0.03);
const MID_INLAND_CONTINENTALNESS: Param = span(0.03, 0.3);
const FAR_INLAND_CONTINENTALNESS: Param = span(0.3, 1.0);

// ---------------------------------------------------------------------------
// Biome tables (OverworldBiomeBuilder L72-110). Row = temperature index,
// column = humidity index. `None` is Java null.

#[rustfmt::skip]
const OCEANS_DEEP: [u8; 5] = [
    DEEP_FROZEN_OCEAN, DEEP_COLD_OCEAN, DEEP_OCEAN, DEEP_LUKEWARM_OCEAN, WARM_OCEAN,
];
#[rustfmt::skip]
const OCEANS: [u8; 5] = [
    FROZEN_OCEAN, COLD_OCEAN, OCEAN, LUKEWARM_OCEAN, WARM_OCEAN,
];

type Table = [[u8; 5]; 5];
type OptTable = [[Option<u8>; 5]; 5];

#[rustfmt::skip]
const MIDDLE_BIOMES: Table = [
    /* t0 */ [SNOWY_PLAINS, SNOWY_PLAINS, SNOWY_PLAINS, SNOWY_TAIGA, TAIGA],
    /* t1 */ [PLAINS, PLAINS, FOREST, TAIGA, OLD_GROWTH_SPRUCE_TAIGA],
    /* t2 */ [FLOWER_FOREST, PLAINS, FOREST, BIRCH_FOREST, DARK_FOREST],
    /* t3 */ [SAVANNA, SAVANNA, FOREST, JUNGLE, JUNGLE],
    /* t4 */ [DESERT, DESERT, DESERT, DESERT, DESERT],
];

#[rustfmt::skip]
const MIDDLE_BIOMES_VARIANT: OptTable = [
    /* t0 */ [Some(ICE_SPIKES), None, Some(SNOWY_TAIGA), None, None],
    /* t1 */ [None, None, None, None, Some(OLD_GROWTH_PINE_FOREST)],
    /* t2 */ [Some(SUNFLOWER_PLAINS), None, None, Some(OLD_GROWTH_BIRCH_FOREST), None],
    /* t3 */ [None, None, Some(PLAINS), Some(SPARSE_JUNGLE), Some(BAMBOO_JUNGLE)],
    /* t4 */ [None, None, None, None, None],
];

#[rustfmt::skip]
const PLATEAU_BIOMES: Table = [
    /* t0 */ [SNOWY_PLAINS, SNOWY_PLAINS, SNOWY_PLAINS, SNOWY_TAIGA, SNOWY_TAIGA],
    /* t1 */ [MEADOW, MEADOW, FOREST, TAIGA, OLD_GROWTH_SPRUCE_TAIGA],
    /* t2 */ [MEADOW, MEADOW, MEADOW, MEADOW, PALE_GARDEN],
    /* t3 */ [SAVANNA_PLATEAU, SAVANNA_PLATEAU, FOREST, FOREST, JUNGLE],
    /* t4 */ [BADLANDS, BADLANDS, BADLANDS, WOODED_BADLANDS, WOODED_BADLANDS],
];

#[rustfmt::skip]
const PLATEAU_BIOMES_VARIANT: OptTable = [
    /* t0 */ [Some(ICE_SPIKES), None, None, None, None],
    /* t1 */ [Some(CHERRY_GROVE), None, Some(MEADOW), Some(MEADOW), Some(OLD_GROWTH_PINE_FOREST)],
    /* t2 */ [Some(CHERRY_GROVE), Some(CHERRY_GROVE), Some(FOREST), Some(BIRCH_FOREST), None],
    /* t3 */ [None, None, None, None, None],
    /* t4 */ [Some(ERODED_BADLANDS), Some(ERODED_BADLANDS), None, None, None],
];

#[rustfmt::skip]
const SHATTERED_BIOMES: OptTable = [
    /* t0 */ [Some(WINDSWEPT_GRAVELLY_HILLS), Some(WINDSWEPT_GRAVELLY_HILLS), Some(WINDSWEPT_HILLS), Some(WINDSWEPT_FOREST), Some(WINDSWEPT_FOREST)],
    /* t1 */ [Some(WINDSWEPT_GRAVELLY_HILLS), Some(WINDSWEPT_GRAVELLY_HILLS), Some(WINDSWEPT_HILLS), Some(WINDSWEPT_FOREST), Some(WINDSWEPT_FOREST)],
    /* t2 */ [Some(WINDSWEPT_HILLS), Some(WINDSWEPT_HILLS), Some(WINDSWEPT_HILLS), Some(WINDSWEPT_FOREST), Some(WINDSWEPT_FOREST)],
    /* t3 */ [None, None, None, None, None],
    /* t4 */ [None, None, None, None, None],
];

// Depth spans shared by the emitters.
const DEPTH_SURFACE_POINTS: [i64; 2] = [0, 10_000]; // point(0.0F), point(1.0F)
const DEPTH_UNDERGROUND: Param = span(0.2, 0.9); // (2000, 9000)
const DEPTH_BOTTOM: Param = point(1.1); // (11000, 11000)

// ---------------------------------------------------------------------------

// -- pickers (OverworldBiomeBuilder L902-985). --------------------------
// All weirdness sign checks are on the QUANTIZED max long.

fn pick_middle(ti: usize, hi: usize, w_max: i64) -> u8 {
    if w_max < 0 {
        return MIDDLE_BIOMES[ti][hi];
    }
    MIDDLE_BIOMES_VARIANT[ti][hi].unwrap_or(MIDDLE_BIOMES[ti][hi])
}

fn pick_middle_or_badlands_if_hot(ti: usize, hi: usize, w_max: i64) -> u8 {
    if ti == 4 {
        pick_badlands(hi, w_max)
    } else {
        pick_middle(ti, hi, w_max)
    }
}

fn pick_middle_or_badlands_if_hot_or_slope_if_cold(ti: usize, hi: usize, w_max: i64) -> u8 {
    if ti == 0 {
        pick_slope(ti, hi, w_max)
    } else {
        pick_middle_or_badlands_if_hot(ti, hi, w_max)
    }
}

fn maybe_pick_windswept_savanna(ti: usize, hi: usize, w_max: i64, underlying: u8) -> u8 {
    if ti > 1 && hi < 4 && w_max >= 0 {
        WINDSWEPT_SAVANNA
    } else {
        underlying
    }
}

fn pick_shattered_coast(ti: usize, hi: usize, w_max: i64) -> u8 {
    let beach_or_middle = if w_max >= 0 {
        pick_middle(ti, hi, w_max)
    } else {
        pick_beach(ti)
    };
    maybe_pick_windswept_savanna(ti, hi, w_max, beach_or_middle)
}

fn pick_beach(ti: usize) -> u8 {
    if ti == 0 {
        SNOWY_BEACH
    } else if ti == 4 {
        DESERT
    } else {
        BEACH
    }
}

fn pick_badlands(hi: usize, w_max: i64) -> u8 {
    if hi < 2 {
        if w_max < 0 { BADLANDS } else { ERODED_BADLANDS }
    } else if hi < 3 {
        BADLANDS
    } else {
        WOODED_BADLANDS
    }
}

fn pick_plateau(ti: usize, hi: usize, w_max: i64) -> u8 {
    if w_max >= 0 {
        if let Some(v) = PLATEAU_BIOMES_VARIANT[ti][hi] {
            return v;
        }
    }
    PLATEAU_BIOMES[ti][hi]
}

fn pick_peak(ti: usize, hi: usize, w_max: i64) -> u8 {
    if ti <= 2 {
        if w_max < 0 { JAGGED_PEAKS } else { FROZEN_PEAKS }
    } else if ti == 3 {
        STONY_PEAKS
    } else {
        pick_badlands(hi, w_max)
    }
}

fn pick_slope(ti: usize, hi: usize, w_max: i64) -> u8 {
    if ti >= 3 {
        pick_plateau(ti, hi, w_max)
    } else if hi <= 1 {
        SNOWY_SLOPES
    } else {
        GROVE
    }
}

fn pick_shattered(ti: usize, hi: usize, w_max: i64) -> u8 {
    SHATTERED_BIOMES[ti][hi].unwrap_or_else(|| pick_middle(ti, hi, w_max))
}


#[derive(Default)]
pub struct Builder {
    records: Vec<Record>,
}

impl Builder {
    fn surface(&mut self, t: Param, h: Param, c: Param, e: Param, w: Param, biome: u8) {
        // addSurfaceBiome: TWO points per call — depth point(0.0) then point(1.0).
        for d in DEPTH_SURFACE_POINTS {
            let mut iv = [0i64; 12];
            t.push(&mut iv, 0);
            h.push(&mut iv, 1);
            c.push(&mut iv, 2);
            e.push(&mut iv, 3);
            iv[8] = d;
            iv[9] = d;
            w.push(&mut iv, 5);
            self.records.push(Record { biome, intervals: iv });
        }
    }

    fn underground(&mut self, t: Param, h: Param, c: Param, e: Param, w: Param, biome: u8) {
        // addUndergroundBiome: ONE point, depth span(0.2, 0.9).
        let mut iv = [0i64; 12];
        t.push(&mut iv, 0);
        h.push(&mut iv, 1);
        c.push(&mut iv, 2);
        e.push(&mut iv, 3);
        DEPTH_UNDERGROUND.push(&mut iv, 4);
        w.push(&mut iv, 5);
        self.records.push(Record { biome, intervals: iv });
    }

    fn bottom(&mut self, t: Param, h: Param, c: Param, e: Param, w: Param, biome: u8) {
        // addBottomBiome: ONE point, depth point(1.1).
        let mut iv = [0i64; 12];
        t.push(&mut iv, 0);
        h.push(&mut iv, 1);
        c.push(&mut iv, 2);
        e.push(&mut iv, 3);
        DEPTH_BOTTOM.push(&mut iv, 4);
        w.push(&mut iv, 5);
        self.records.push(Record { biome, intervals: iv });
    }

    // -- emission stages (OverworldBiomeBuilder L200-900). -------------------

    /// addOffCoastBiomes: mushroom fields + 5x(deep ocean, ocean) = 11 calls x 2 pts.
    fn add_off_coast_biomes(&mut self) {
        self.surface(
            FULL_RANGE,
            FULL_RANGE,
            MUSHROOM_FIELDS_CONTINENTALNESS,
            FULL_RANGE,
            FULL_RANGE,
            MUSHROOM_FIELDS,
        );
        for ti in 0..TEMPERATURES.len() {
            let t = TEMPERATURES[ti];
            self.surface(t, FULL_RANGE, DEEP_OCEAN_CONTINENTALNESS, FULL_RANGE, FULL_RANGE, OCEANS_DEEP[ti]);
            self.surface(t, FULL_RANGE, OCEAN_CONTINENTALNESS, FULL_RANGE, FULL_RANGE, OCEANS[ti]);
        }
    }

    // addInlandBiomes (L216-230) is driven slice-by-slice from `build()` so
    // each weirdness slice gets its own hard count assertion; the 13 slices
    // and their spans are mirrored EXACTLY there in emission order.

    /// addPeaks: 25 (t,h) combos x 11 calls.
    fn add_peaks(&mut self, w: Param) {
        for ti in 0..TEMPERATURES.len() {
            let t = TEMPERATURES[ti];
            for hi in 0..HUMIDITIES.len() {
                let h = HUMIDITIES[hi];
                let wm = w.1;
                let middle = pick_middle(ti, hi, wm);
                let middle_hot = pick_middle_or_badlands_if_hot(ti, hi, wm);
                let middle_hot_slope = pick_middle_or_badlands_if_hot_or_slope_if_cold(ti, hi, wm);
                let plateau = pick_plateau(ti, hi, wm);
                let shattered = pick_shattered(ti, hi, wm);
                let shattered_ws = maybe_pick_windswept_savanna(ti, hi, wm, shattered);
                let peak = pick_peak(ti, hi, wm);

                self.surface(t, h, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[0], w, peak);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, NEAR_INLAND_CONTINENTALNESS), EROSIONS[1], w, middle_hot_slope);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[1], w, peak);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, NEAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[2], EROSIONS[3]), w, middle);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[2], w, plateau);
                self.surface(t, h, MID_INLAND_CONTINENTALNESS, EROSIONS[3], w, middle_hot);
                self.surface(t, h, FAR_INLAND_CONTINENTALNESS, EROSIONS[3], w, plateau);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[4], w, middle);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, NEAR_INLAND_CONTINENTALNESS), EROSIONS[5], w, shattered_ws);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[5], w, shattered);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[6], w, middle);
            }
        }
    }

    /// addHighSlice: 25 (t,h) combos x 13 calls.
    fn add_high_slice(&mut self, w: Param) {
        for ti in 0..TEMPERATURES.len() {
            let t = TEMPERATURES[ti];
            for hi in 0..HUMIDITIES.len() {
                let h = HUMIDITIES[hi];
                let wm = w.1;
                let middle = pick_middle(ti, hi, wm);
                let middle_hot = pick_middle_or_badlands_if_hot(ti, hi, wm);
                let middle_hot_slope = pick_middle_or_badlands_if_hot_or_slope_if_cold(ti, hi, wm);
                let plateau = pick_plateau(ti, hi, wm);
                let shattered = pick_shattered(ti, hi, wm);
                let middle_ws = maybe_pick_windswept_savanna(ti, hi, wm, middle);
                let slope = pick_slope(ti, hi, wm);
                let peak = pick_peak(ti, hi, wm);

                self.surface(t, h, COAST_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[1]), w, middle);
                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, EROSIONS[0], w, slope);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[0], w, peak);
                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, EROSIONS[1], w, middle_hot_slope);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[1], w, slope);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, NEAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[2], EROSIONS[3]), w, middle);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[2], w, plateau);
                self.surface(t, h, MID_INLAND_CONTINENTALNESS, EROSIONS[3], w, middle_hot);
                self.surface(t, h, FAR_INLAND_CONTINENTALNESS, EROSIONS[3], w, plateau);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[4], w, middle);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, NEAR_INLAND_CONTINENTALNESS), EROSIONS[5], w, middle_ws);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[5], w, shattered);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[6], w, middle);
            }
        }
    }

    /// addMidSlice: 3 fixed calls + 25 x (12 base + 1 if w.max < 0 + 1 if ti == 0).
    fn add_mid_slice(&mut self, w: Param) {
        self.surface(FULL_RANGE, FULL_RANGE, COAST_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[2]), w, STONY_SHORE);
        self.surface(
            span_join(TEMPERATURES[1], TEMPERATURES[2]),
            FULL_RANGE,
            span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS),
            EROSIONS[6],
            w,
            SWAMP,
        );
        self.surface(
            span_join(TEMPERATURES[3], TEMPERATURES[4]),
            FULL_RANGE,
            span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS),
            EROSIONS[6],
            w,
            MANGROVE_SWAMP,
        );

        for ti in 0..TEMPERATURES.len() {
            let t = TEMPERATURES[ti];
            for hi in 0..HUMIDITIES.len() {
                let h = HUMIDITIES[hi];
                let wm = w.1;
                let middle = pick_middle(ti, hi, wm);
                let middle_hot = pick_middle_or_badlands_if_hot(ti, hi, wm);
                let middle_hot_slope = pick_middle_or_badlands_if_hot_or_slope_if_cold(ti, hi, wm);
                let shattered = pick_shattered(ti, hi, wm);
                let plateau = pick_plateau(ti, hi, wm);
                let beach = pick_beach(ti);
                let middle_ws = maybe_pick_windswept_savanna(ti, hi, wm, middle);
                let shattered_coast = pick_shattered_coast(ti, hi, wm);
                let slope = pick_slope(ti, hi, wm);

                self.surface(t, h, span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[0], w, slope);
                self.surface(t, h, span_join(NEAR_INLAND_CONTINENTALNESS, MID_INLAND_CONTINENTALNESS), EROSIONS[1], w, middle_hot_slope);
                self.surface(t, h, FAR_INLAND_CONTINENTALNESS, EROSIONS[1], w, if ti == 0 { slope } else { plateau });
                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, EROSIONS[2], w, middle);
                self.surface(t, h, MID_INLAND_CONTINENTALNESS, EROSIONS[2], w, middle_hot);
                self.surface(t, h, FAR_INLAND_CONTINENTALNESS, EROSIONS[2], w, plateau);
                self.surface(t, h, span_join(COAST_CONTINENTALNESS, NEAR_INLAND_CONTINENTALNESS), EROSIONS[3], w, middle);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[3], w, middle_hot);
                if wm < 0 {
                    self.surface(t, h, COAST_CONTINENTALNESS, EROSIONS[4], w, beach);
                    self.surface(t, h, span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[4], w, middle);
                } else {
                    self.surface(t, h, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[4], w, middle);
                }
                self.surface(t, h, COAST_CONTINENTALNESS, EROSIONS[5], w, shattered_coast);
                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, EROSIONS[5], w, middle_ws);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[5], w, shattered);
                if wm < 0 {
                    self.surface(t, h, COAST_CONTINENTALNESS, EROSIONS[6], w, beach);
                } else {
                    self.surface(t, h, COAST_CONTINENTALNESS, EROSIONS[6], w, middle);
                }
                if ti == 0 {
                    self.surface(t, h, span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[6], w, middle);
                }
            }
        }
    }

    /// addLowSlice: 3 fixed calls + 25 x (10 base + 1 if ti == 0).
    fn add_low_slice(&mut self, w: Param) {
        self.surface(FULL_RANGE, FULL_RANGE, COAST_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[2]), w, STONY_SHORE);
        self.surface(
            span_join(TEMPERATURES[1], TEMPERATURES[2]),
            FULL_RANGE,
            span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS),
            EROSIONS[6],
            w,
            SWAMP,
        );
        self.surface(
            span_join(TEMPERATURES[3], TEMPERATURES[4]),
            FULL_RANGE,
            span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS),
            EROSIONS[6],
            w,
            MANGROVE_SWAMP,
        );

        for ti in 0..TEMPERATURES.len() {
            let t = TEMPERATURES[ti];
            for hi in 0..HUMIDITIES.len() {
                let h = HUMIDITIES[hi];
                let wm = w.1;
                let middle = pick_middle(ti, hi, wm);
                let middle_hot = pick_middle_or_badlands_if_hot(ti, hi, wm);
                let middle_hot_slope = pick_middle_or_badlands_if_hot_or_slope_if_cold(ti, hi, wm);
                let beach = pick_beach(ti);
                let middle_ws = maybe_pick_windswept_savanna(ti, hi, wm, middle);
                let shattered_coast = pick_shattered_coast(ti, hi, wm);

                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[1]), w, middle_hot);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[0], EROSIONS[1]), w, middle_hot_slope);
                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, span_join(EROSIONS[2], EROSIONS[3]), w, middle);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[2], EROSIONS[3]), w, middle_hot);
                self.surface(t, h, COAST_CONTINENTALNESS, span_join(EROSIONS[3], EROSIONS[4]), w, beach);
                self.surface(t, h, span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[4], w, middle);
                self.surface(t, h, COAST_CONTINENTALNESS, EROSIONS[5], w, shattered_coast);
                self.surface(t, h, NEAR_INLAND_CONTINENTALNESS, EROSIONS[5], w, middle_ws);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[5], w, middle);
                self.surface(t, h, COAST_CONTINENTALNESS, EROSIONS[6], w, beach);
                if ti == 0 {
                    self.surface(t, h, span_join(NEAR_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[6], w, middle);
                }
            }
        }
    }

    /// addValleys: 11 fixed calls + 25 loop calls.
    fn add_valleys(&mut self, w: Param) {
        let wm = w.1;
        self.surface(FROZEN_RANGE, FULL_RANGE, COAST_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[1]), w, if wm < 0 { STONY_SHORE } else { FROZEN_RIVER });
        self.surface(UNFROZEN_RANGE, FULL_RANGE, COAST_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[1]), w, if wm < 0 { STONY_SHORE } else { RIVER });
        self.surface(FROZEN_RANGE, FULL_RANGE, NEAR_INLAND_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[1]), w, FROZEN_RIVER);
        self.surface(UNFROZEN_RANGE, FULL_RANGE, NEAR_INLAND_CONTINENTALNESS, span_join(EROSIONS[0], EROSIONS[1]), w, RIVER);
        self.surface(FROZEN_RANGE, FULL_RANGE, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[2], EROSIONS[5]), w, FROZEN_RIVER);
        self.surface(UNFROZEN_RANGE, FULL_RANGE, span_join(COAST_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[2], EROSIONS[5]), w, RIVER);
        self.surface(FROZEN_RANGE, FULL_RANGE, COAST_CONTINENTALNESS, EROSIONS[6], w, FROZEN_RIVER);
        self.surface(UNFROZEN_RANGE, FULL_RANGE, COAST_CONTINENTALNESS, EROSIONS[6], w, RIVER);
        self.surface(
            span_join(TEMPERATURES[1], TEMPERATURES[2]),
            FULL_RANGE,
            span_join(INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS),
            EROSIONS[6],
            w,
            SWAMP,
        );
        self.surface(
            span_join(TEMPERATURES[3], TEMPERATURES[4]),
            FULL_RANGE,
            span_join(INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS),
            EROSIONS[6],
            w,
            MANGROVE_SWAMP,
        );
        self.surface(FROZEN_RANGE, FULL_RANGE, span_join(INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), EROSIONS[6], w, FROZEN_RIVER);

        for ti in 0..TEMPERATURES.len() {
            let t = TEMPERATURES[ti];
            for hi in 0..HUMIDITIES.len() {
                let h = HUMIDITIES[hi];
                let middle_hot = pick_middle_or_badlands_if_hot(ti, hi, wm);
                self.surface(t, h, span_join(MID_INLAND_CONTINENTALNESS, FAR_INLAND_CONTINENTALNESS), span_join(EROSIONS[0], EROSIONS[1]), w, middle_hot);
            }
        }
    }

    /// addUndergroundBiomes: dripstone, lush, sulfur, deep dark (in order).
    fn add_underground_biomes(&mut self) {
        self.underground(FULL_RANGE, FULL_RANGE, span(0.8, 1.0), FULL_RANGE, FULL_RANGE, DRIPSTONE_CAVES);
        self.underground(FULL_RANGE, span(0.7, 1.0), FULL_RANGE, FULL_RANGE, FULL_RANGE, LUSH_CAVES);
        self.underground(
            FULL_RANGE,
            FULL_RANGE,
            span_join(COAST_CONTINENTALNESS, INLAND_CONTINENTALNESS),
            span_join(EROSIONS[5], EROSIONS[6]),
            span(-1.1, -0.85),
            SULFUR_CAVES,
        );
        self.bottom(FULL_RANGE, FULL_RANGE, FULL_RANGE, span_join(EROSIONS[0], EROSIONS[1]), FULL_RANGE, DEEP_DARK);
    }
}

/// Per-slice expected point counts, mirroring `addInlandBiomes` order.
const SLICE_POINTS: [usize; 13] = [
    716, 650, 550, 650, 716, 516, 72, 516, 666, 650, 550, 650, 666,
];

/// Regenerate every record in vanilla emission order, with hard assertions.
pub fn build() -> Vec<Record> {
    let mut b = Builder::default();

    b.add_off_coast_biomes();
    assert_len(&b, 22, "addOffCoastBiomes");

    let slices: [fn(&mut Builder, Param); 13] = [
        Builder::add_mid_slice,
        Builder::add_high_slice,
        Builder::add_peaks,
        Builder::add_high_slice,
        Builder::add_mid_slice,
        Builder::add_low_slice,
        Builder::add_valleys,
        Builder::add_low_slice,
        Builder::add_mid_slice,
        Builder::add_high_slice,
        Builder::add_peaks,
        Builder::add_high_slice,
        Builder::add_mid_slice,
    ];
    let weirdness = [
        span(-1.0, -0.93333334),
        span(-0.93333334, -0.7666667),
        span(-0.7666667, -0.56666666),
        span(-0.56666666, -0.4),
        span(-0.4, -0.26666668),
        span(-0.26666668, -0.05),
        span(-0.05, 0.05),
        span(0.05, 0.26666668),
        span(0.26666668, 0.4),
        span(0.4, 0.56666666),
        span(0.56666666, 0.7666667),
        span(0.7666667, 0.93333334),
        span(0.93333334, 1.0),
    ];
    for (i, (&emit, &w)) in slices.iter().zip(weirdness.iter()).enumerate() {
        let start = b.records.len();
        emit(&mut b, w);
        assert_len(&b, start + SLICE_POINTS[i], &format!("inland slice {i}"));
    }

    b.add_underground_biomes();
    assert_len(&b, 7594, "total (with addUndergroundBiomes = 4)");

    // Every interval must satisfy min <= max (span joins are raw unions).
    for (i, r) in b.records.iter().enumerate() {
        for pair in r.intervals.chunks(2) {
            assert!(
                pair[0] <= pair[1],
                "record {i}: interval min {} > max {}",
                pair[0],
                pair[1]
            );
        }
    }

    b.records
}

fn assert_len(b: &Builder, expected: usize, what: &str) {
    assert_eq!(b.records.len(), expected, "point count mismatch after {what}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::RECORD_SIZE;

    #[test]
    fn regenerates_7594_records() {
        assert_eq!(build().len(), 7594);
        assert_eq!(build().len() * RECORD_SIZE, 7594 * RECORD_SIZE);
    }

    #[test]
    fn first_record_matches_packed_blob_pins() {
        let first = build()[0];
        assert_eq!(first.biome, 29); // mushroom_fields
        assert_eq!(
            first.intervals,
            [
                -10000, 10000, // temperature full range
                -10000, 10000, // humidity full range
                -12000, -10500, // mushroom continentalness span(-1.2, -1.05)
                -10000, 10000, // erosion full range
                0, 0,          // depth point(0.0)
                -10000, 10000, // weirdness full range
            ]
        );
    }

    #[test]
    fn last_record_matches_packed_blob_pins() {
        let last = build()[7593];
        assert_eq!(last.biome, 31); // deep_dark
        assert_eq!(
            last.intervals,
            [
                -10000, 10000, // temperature full range
                -10000, 10000, // humidity full range
                -10000, 10000, // continentalness full range
                -10000, -3750, // erosion span(e0, e1) raw join
                11000, 11000,  // depth point(1.1)
                -10000, 10000, // weirdness full range
            ]
        );
    }

    #[test]
    fn quantization_matches_java_semantics() {
        // Spot pins proven against the packed blob / vanilla javadoc values.
        assert_eq!(q(-0.11), -1100); // f32 multiply rounds back to exactly 1100.0
        assert_eq!(q(0.03), 300);
        assert_eq!(q(1.05), 10500);
        assert_eq!(q(0.2225), 2225);
        assert_eq!(q(1.1), 11000);
        assert_eq!(q(-0.05), -500);
        assert_eq!(span(0.2, 0.9), Param(2000, 9000));
        assert_eq!(span(0.7, 1.0), Param(7000, 10000)); // lush caves humidity pin
    }
}
