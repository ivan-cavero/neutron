// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// RandomState equivalent for Minecraft 26.2: per-seed noise instantiation and
// the overworld NoiseRouter built from the datapack JSON.
//
// In 26.2 every noise is seeded independently as
// `MD5("minecraft:" + key) XOR mainPositionalFactory` (see
// `Noises.instantiate`), so the instantiation order is irrelevant and the only
// main-RNG consumption is the initial `forkPositional()`.

use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;

use crate::density::{DF, DensityEnv, DensityRegistry, MarkerState};
use crate::noise::{BlendedNoise, NormalNoise};
use crate::rng::Xoroshiro128;

/// Per-seed NormalNoise instances, keyed by noise registry name.
pub struct NoiseSet {
    noises: HashMap<String, NormalNoise>,
}

impl NoiseSet {
    /// Instantiate all noises for a world seed (RandomState equivalent).
    pub fn for_seed(seed: i64, reg: &DensityRegistry) -> Self {
        let mut seed_rng = Xoroshiro128::new(seed);
        let (lo, hi) = seed_rng.fork_positional();
        let mut noises = HashMap::new();
        for key in reg.noise_keys() {
            let (first_octave, amps) = reg.noise_params(key);
            let s = Xoroshiro128::from_raw(lo, hi)
                .from_hash_of(&format!("minecraft:{key}"))
                .seed();
            let nn = NormalNoise::create(s.0, s.1, *first_octave, amps);
            noises.insert(key.to_string(), nn);
        }
        Self { noises }
    }

    pub fn get(&self, key: &str) -> &NormalNoise {
        &self.noises[key]
    }

    pub fn noises(&self) -> &HashMap<String, NormalNoise> {
        &self.noises
    }
}

/// The overworld NoiseRouter: 15 density functions.
pub struct Router {
    pub barrier: DF,
    pub fluid_level_floodedness: DF,
    pub fluid_level_spread: DF,
    pub lava: DF,
    pub temperature: DF,
    pub vegetation: DF,
    pub continents: DF,
    pub erosion: DF,
    pub depth: DF,
    pub ridges: DF,
    pub preliminary_surface_level: DF,
    pub final_density: DF,
    pub vein_toggle: DF,
    pub vein_ridged: DF,
    pub vein_gap: DF,
}

impl Router {
    /// Build the overworld router from the embedded noise_settings JSON.
    pub fn overworld(reg: &mut DensityRegistry) -> Self {
        let json = crate::datapack_data::datapack_json("noise_settings_overworld.json")
            .expect("overworld noise_settings JSON");
        let value: Value = serde_json::from_str(json).expect("invalid noise_settings JSON");
        let router = &value["noise_router"];
        let mut f = |name: &str| -> DF { reg.parse(&router[name]) };
        Self {
            barrier: f("barrier"),
            fluid_level_floodedness: f("fluid_level_floodedness"),
            fluid_level_spread: f("fluid_level_spread"),
            lava: f("lava"),
            temperature: f("temperature"),
            vegetation: f("vegetation"),
            continents: f("continents"),
            erosion: f("erosion"),
            depth: f("depth"),
            ridges: f("ridges"),
            preliminary_surface_level: f("preliminary_surface_level"),
            final_density: f("final_density"),
            vein_toggle: f("vein_toggle"),
            vein_ridged: f("vein_ridged"),
            vein_gap: f("vein_gap"),
        }
    }
}

/// The worldgen context: settings + per-seed noises + router.
pub struct WorldgenState {
    pub reg: DensityRegistry,
    pub noises: NoiseSet,
    pub router: Router,
    pub sea_level: i32,
    pub min_y: i32,
    pub height: i32,
    pub cell_width: i32,
    pub cell_height: i32,
    /// World seed used for feature decoration RNG.
    pub seed: i64,
    /// Main positional factory (`Xoroshiro(seed).forkPositional()`).
    pub main_lo: u64,
    pub main_hi: u64,
    /// Aquifer positional factory seed pair
    /// (`mainRandom.fromHashOf("aquifer").forkPositional()`).
    pub aquifer_lo: u64,
    pub aquifer_hi: u64,
    /// Ore-vein positional factory
    /// (`mainRandom.fromHashOf("ore").forkPositional()`).
    pub ore_lo: u64,
    pub ore_hi: u64,
}

impl WorldgenState {
    /// Create the full overworld state for a seed.
    pub fn overworld(seed: i64) -> Self {
        let mut reg = DensityRegistry::build();
        // RandomState: main random = Xoroshiro(seed).forkPositional(); terrain
        // random = main.fromHashOf("minecraft:terrain").
        let mut seed_rng = Xoroshiro128::new(seed);
        let (lo, hi) = seed_rng.fork_positional();
        let terrain = Xoroshiro128::from_raw(lo, hi).from_hash_of("minecraft:terrain");
        reg.set_terrain_random(terrain.seed().0, terrain.seed().1);
        // aquiferRandom = main.fromHashOf("aquifer").forkPositional()
        let mut aquifer = Xoroshiro128::from_raw(lo, hi).from_hash_of("minecraft:aquifer");
        let (aquifer_lo, aquifer_hi) = aquifer.fork_positional();
        // oreRandom = main.fromHashOf("ore").forkPositional()
        let mut ore = Xoroshiro128::from_raw(lo, hi).from_hash_of("minecraft:ore");
        let (ore_lo, ore_hi) = ore.fork_positional();
        let noises = NoiseSet::for_seed(seed, &reg);
        let router = Router::overworld(&mut reg);
        Self {
            reg,
            noises,
            router,
            sea_level: 63,
            min_y: -64,
            height: 384,
            cell_width: 4,
            cell_height: 8,
            seed,
            main_lo: lo,
            main_hi: hi,
            aquifer_lo,
            aquifer_hi,
            ore_lo,
            ore_hi,
        }
    }

    /// Evaluate a density function at a block coordinate.
    pub fn eval(&self, df: &DF, x: i32, y: i32, z: i32) -> f64 {
        let mut env = DensityEnv::new(x, y, z, self.noises.noises());
        crate::density::compute(df, &mut env)
    }

    /// The unseeded overworld BlendedNoise (base_3d_noise).
    pub fn blended_noise(&self) -> BlendedNoise {
        BlendedNoise::create_unseeded(0.25, 0.125, 80.0, 160.0, 8.0)
    }
}
