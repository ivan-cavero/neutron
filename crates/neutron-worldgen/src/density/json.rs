use super::*;
use std::sync::Arc;
use serde_json::Value;

impl DensityRegistry {
    /// Parse a density function JSON value into a node.
    pub fn parse(&mut self, value: &Value) -> DF {
        match value {
            Value::Number(n) => Arc::new(DFNode::Const(n.as_f64().unwrap())),
            Value::String(s) => {
                // A reference to a named density function (e.g. "minecraft:overworld/offset").
                let key = s.strip_prefix("minecraft:").unwrap_or(s);
                self.function(key)
            }
            Value::Object(obj) => {
                let t = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let t = t.strip_prefix("minecraft:").unwrap_or(t);
                match t {
                    "constant" => Arc::new(DFNode::Const(0.0)),
                    "mul" => Arc::new(DFNode::Mul(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "add" => Arc::new(DFNode::Add(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "min" => Arc::new(DFNode::Min(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "max" => Arc::new(DFNode::Max(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "abs" => Arc::new(DFNode::Abs(self.parse(&obj["argument"]))),
                    "square" => Arc::new(DFNode::Square(self.parse(&obj["argument"]))),
                    "cube" => Arc::new(DFNode::Cube(self.parse(&obj["argument"]))),
                    "half_negative" => Arc::new(DFNode::HalfNegative(self.parse(&obj["argument"]))),
                    "quarter_negative" => {
                        Arc::new(DFNode::QuarterNegative(self.parse(&obj["argument"])))
                    }
                    "invert" => Arc::new(DFNode::Invert(self.parse(&obj["argument"]))),
                    "squeeze" => Arc::new(DFNode::Squeeze(self.parse(&obj["argument"]))),
                    "clamp" => {
                        let inner = self.parse(&obj["input"]);
                        let lo = obj["min"].as_f64().unwrap();
                        let hi = obj["max"].as_f64().unwrap();
                        Arc::new(DFNode::Clamp(inner, lo, hi))
                    }
                    "range_choice" => {
                        let input = self.parse(&obj["input"]);
                        let min = obj["min_inclusive"].as_f64().unwrap();
                        let max = obj["max_exclusive"].as_f64().unwrap();
                        let in_range = self.parse(&obj["when_in_range"]);
                        let out_of_range = self.parse(&obj["when_out_of_range"]);
                        Arc::new(DFNode::RangeChoice(input, min, max, in_range, out_of_range))
                    }
                    "interval_select" => {
                        let input = self.parse(&obj["input"]);
                        let thresholds: Vec<f64> = obj["thresholds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_f64().unwrap())
                            .collect();
                        let functions: Vec<DF> = obj["functions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| self.parse(v))
                            .collect();
                        Arc::new(DFNode::IntervalSelect(input, thresholds, functions))
                    }
                    "noise" => {
                        let key = obj["noise"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        let xz_scale = obj.get("xz_scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
                        let y_scale = obj.get("y_scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
                        Arc::new(DFNode::Noise(key, xz_scale, y_scale))
                    }
                    "shifted_noise" => {
                        let sx = self.parse(&obj["shift_x"]);
                        let sy = self.parse(&obj["shift_y"]);
                        let sz = self.parse(&obj["shift_z"]);
                        let xz_scale = obj["xz_scale"].as_f64().unwrap();
                        let y_scale = obj.get("y_scale").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let key = obj["noise"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        Arc::new(DFNode::ShiftedNoise(sx, sy, sz, xz_scale, y_scale, key))
                    }
                    "shift_a" => {
                        let key = obj["argument"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        Arc::new(DFNode::ShiftA(key))
                    }
                    "shift_b" => {
                        let key = obj["argument"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        Arc::new(DFNode::ShiftB(key))
                    }
                    "y_clamped_gradient" => Arc::new(DFNode::YClampedGradient(
                        obj["from_y"].as_f64().unwrap(),
                        obj["to_y"].as_f64().unwrap(),
                        obj["from_value"].as_f64().unwrap(),
                        obj["to_value"].as_f64().unwrap(),
                    )),
                    "spline" => {
                        let spline = self.parse_spline(&obj["spline"]);
                        Arc::new(DFNode::Spline(spline))
                    }
                    "flat_cache" => {
                        let inner = self.parse(&obj["argument"]);
                        Arc::new(self.marker_node(MarkerKind::FlatCache, inner))
                    }
                    "cache_2d" => {
                        let inner = self.parse(&obj["argument"]);
                        Arc::new(self.marker_node(MarkerKind::Cache2D, inner))
                    }
                    "cache_once" => {
                        let inner = self.parse(&obj["argument"]);
                        Arc::new(self.marker_node(MarkerKind::CacheOnce, inner))
                    }
                    "cache_all_in_cell" => {
                        let inner = self.parse(&obj["argument"]);
                        Arc::new(self.marker_node(MarkerKind::CacheAllInCell, inner))
                    }
                    "interpolated" => {
                        let inner = self.parse(&obj["argument"]);
                        Arc::new(self.marker_node(MarkerKind::Interpolated, inner))
                    }
                    "blend_density" => {
                        let inner = self.parse(&obj["argument"]);
                        Arc::new(self.marker_node(MarkerKind::BlendDensity, inner))
                    }
                    "blend_alpha" => Arc::new(DFNode::BlendAlpha),
                    "blend_offset" => Arc::new(DFNode::BlendOffset),
                    "find_top_surface" => {
                        let density = self.parse(&obj["density"]);
                        let upper_bound = self.parse(&obj["upper_bound"]);
                        let lower_bound = obj["lower_bound"].as_i64().unwrap() as i32;
                        let cell_height = obj["cell_height"].as_i64().unwrap() as i32;
                        Arc::new(DFNode::FindTopSurface(
                            density,
                            upper_bound,
                            lower_bound,
                            cell_height,
                        ))
                    }
                    "beardifier" => Arc::new(DFNode::Beardifier),
                    "old_blended_noise" => {
                        let (tlo, thi) = self
                            .terrain_random
                            .expect("terrain random must be set before parsing");
                        Arc::new(DFNode::BlendedNoise(BlendedNode(
                            crate::noise::BlendedNoise::with_random(
                                crate::rng::Xoroshiro128::from_raw(tlo, thi),
                                obj["xz_scale"].as_f64().unwrap(),
                                obj["y_scale"].as_f64().unwrap(),
                                obj["xz_factor"].as_f64().unwrap(),
                                obj["y_factor"].as_f64().unwrap(),
                                obj["smear_scale_multiplier"].as_f64().unwrap(),
                            ),
                        )))
                    }
                    other => {
                        panic!("unhandled density function type: {other}");
                    }
                }
            }
            other => panic!("unhandled density function value: {other}"),
        }
    }

    fn parse_spline(&mut self, value: &Value) -> SplineDef {
        let coordinate = self.parse(&value["coordinate"]);
        let points = value["points"].as_array().unwrap();
        let mut locations = Vec::with_capacity(points.len());
        let mut derivatives = Vec::with_capacity(points.len());
        let mut values = Vec::with_capacity(points.len());
        for p in points {
            locations.push(p["location"].as_f64().unwrap() as f32);
            derivatives.push(p["derivative"].as_f64().unwrap() as f32);
            let v = &p["value"];
            if v.is_number() {
                values.push(SplineValue::Const(v.as_f64().unwrap() as f32));
            } else if v.get("coordinate").is_some() {
                values.push(SplineValue::Spline(self.parse_spline(v)));
            } else {
                panic!("unhandled spline value: {v}");
            }
        }
        SplineDef {
            coordinate,
            locations,
            derivatives,
            values,
        }
    }

    /// Look up a noise parameter set.
    pub fn noise_params(&self, key: &str) -> &(i32, Vec<f64>) {
        self.noise_params
            .get(key)
            .unwrap_or_else(|| panic!("missing noise params for {key}"))
    }

    /// All noise registry keys.
    pub fn noise_keys(&self) -> impl Iterator<Item = &String> {
        self.noise_params.keys()
    }

    /// Set the terrain random seed pair (derived from the world seed).
    pub fn set_terrain_random(&mut self, lo: u64, hi: u64) {
        self.terrain_random = Some((lo, hi));
    }
}

pub(super) fn parse_noise_json(json: &str) -> Option<(i32, Vec<f64>)> {
    let value: Value = serde_json::from_str(json).ok()?;
    let first_octave = value.get("firstOctave")?.as_i64()? as i32;
    let amplitudes: Vec<f64> = value
        .get("amplitudes")?
        .as_array()?
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    Some((first_octave, amplitudes))
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn empty_noises() -> HashMap<String, NormalNoise> {
        HashMap::new()
    }

    /// Vanilla gives every Cache2D/CacheOnce instance its own state
    /// (NoiseChunk.java L531-553 / L615-644). A shared slot would make the
    /// second sibling return the first one's value.
    #[test]
    fn cache_once_slots_are_per_node() {
        let a: DF = Arc::new(DFNode::Marker(
            MarkerKind::CacheOnce,
            Arc::new(DFNode::Const(41.0)),
            0,
        ));
        let b: DF = Arc::new(DFNode::Marker(
            MarkerKind::CacheOnce,
            Arc::new(DFNode::Const(-42.0)),
            1,
        ));
        let tree: DF = Arc::new(DFNode::Add(a, b));

        let noises = empty_noises();
        let mut ms = MarkerState::new(4, 8, 2);
        ms.interpolation_counter = 7;
        let mut env = DensityEnv::with_markers(0, 0, 0, &noises, &mut ms);

        assert_eq!(compute(&tree, &mut env), -1.0);
    }

    #[test]
    fn cache_2d_slots_are_per_node() {
        // Same column, two Cache2D wrappers around different constants.
        let a: DF = Arc::new(DFNode::Marker(
            MarkerKind::Cache2D,
            Arc::new(DFNode::Const(10.0)),
            0,
        ));
        let b: DF = Arc::new(DFNode::Marker(
            MarkerKind::Cache2D,
            Arc::new(DFNode::Const(20.0)),
            1,
        ));
        let tree: DF = Arc::new(DFNode::Add(a, b));

        let noises = empty_noises();
        let mut ms = MarkerState::new(4, 8, 2);
        let mut env = DensityEnv::with_markers(3, -40, 9, &noises, &mut ms);

        assert_eq!(compute(&tree, &mut env), 30.0);
    }

    /// Cache2D keys on XZ only — same column at another Y is a hit
    /// (ChunkPos.pack(blockX, blockZ), NoiseChunk.java L542-547).
    #[test]
    fn cache_2d_hits_across_y_same_column() {
        let a: DF = Arc::new(DFNode::Marker(
            MarkerKind::Cache2D,
            Arc::new(DFNode::Const(7.0)),
            0,
        ));

        let noises = empty_noises();
        let mut ms = MarkerState::new(4, 8, 1);
        let mut env = DensityEnv::with_markers(0, 10, 0, &noises, &mut ms);
        assert_eq!(compute(&a, &mut env), 7.0);

        env.y = 99; // same column -> hit regardless of Y
        assert_eq!(compute(&a, &mut env), 7.0);

        env.x = 16; // different column -> miss, recomputed value identical here
        assert_eq!(compute(&a, &mut env), 7.0);
    }

    /// CacheOnce re-evaluates when the interpolation counter advances
    /// (lastCounter == NoiseChunk.interpolationComparator check,
    /// NoiseChunk.java L636-643).
    #[test]
    fn cache_once_recomputes_per_block() {
        let a: DF = Arc::new(DFNode::Marker(
            MarkerKind::CacheOnce,
            Arc::new(DFNode::Const(5.0)),
            0,
        ));

        let noises = empty_noises();
        let mut ms = MarkerState::new(4, 8, 1);
        ms.interpolation_counter = 1;
        let mut env = DensityEnv::with_markers(0, 0, 0, &noises, &mut ms);

        assert_eq!(compute(&a, &mut env), 5.0);
        drop(env);
        // Slot key must now be 1, not i64::MIN sentinel.
        assert_eq!(ms.cache_slots[0].key, 1);

        ms.interpolation_counter = 2; // next block -> miss -> store again
        let mut env = DensityEnv::with_markers(0, 0, 0, &noises, &mut ms);
        assert_eq!(compute(&a, &mut env), 5.0);
        drop(env);
        assert_eq!(ms.cache_slots[0].key, 2);
    }

    /// Without marker_state (point contexts: aquifer, biome queries) markers
    /// bypass caching entirely — vanilla's `context != NoiseChunk.this`.
    #[test]
    fn markers_bypass_without_marker_state() {
        let a: DF = Arc::new(DFNode::Marker(
            MarkerKind::CacheOnce,
            Arc::new(DFNode::Const(3.5)),
            0,
        ));
        let noises = empty_noises();
        let mut env = DensityEnv::new(0, 0, 0, &noises);
        assert_eq!(compute(&a, &mut env), 3.5);
    }
}

