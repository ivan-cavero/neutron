//! Count marker nodes in the density tree.
//! Usage: marker_count [seed]
use neutron_worldgen::density::{DensityEnv, MarkerKind, MarkerState, DFNode};
use neutron_worldgen::ChunkGenerator;

fn count_markers(df: &neutron_worldgen::density::DF, counts: &mut std::collections::HashMap<String, usize>) {
    match &**df {
        DFNode::Marker(kind, wrapped, _slot) => {
            let name = format!("{:?}", kind);
            *counts.entry(name).or_insert(0) += 1;
            count_markers(wrapped, counts);
        }
        DFNode::Mul(a, b) | DFNode::Add(a, b) | DFNode::Min(a, b) | DFNode::Max(a, b) => {
            count_markers(a, counts);
            count_markers(b, counts);
        }
        DFNode::Abs(a) | DFNode::Square(a) | DFNode::Cube(a) | DFNode::HalfNegative(a)
        | DFNode::QuarterNegative(a) | DFNode::Invert(a) | DFNode::Squeeze(a)
        | DFNode::Clamp(a, _, _) => {
            count_markers(a, counts);
        }
        DFNode::RangeChoice(input, _, _, in_range, out_range) => {
            count_markers(input, counts);
            count_markers(in_range, counts);
            count_markers(out_range, counts);
        }
        DFNode::IntervalSelect(input, _, functions) => {
            count_markers(input, counts);
            for f in functions {
                count_markers(f, counts);
            }
        }
        DFNode::Noise(_, _, _) | DFNode::ShiftedNoise(_, _, _, _, _, _)
        | DFNode::ShiftA(_) | DFNode::ShiftB(_) | DFNode::YClampedGradient(_, _, _, _)
        | DFNode::Const(_) | DFNode::BlendAlpha | DFNode::BlendOffset
        | DFNode::Beardifier | DFNode::EndIslands(_) | DFNode::BlendedNoise(_) => {}
        DFNode::FindTopSurface(d, ub, _lb, _ch) => {
            count_markers(d, counts);
            count_markers(ub, counts);
        }
        DFNode::Spline(spline) => {
            count_markers(&spline.coordinate, counts);
            for v in &spline.values {
                match v {
                    neutron_worldgen::density::SplineValue::Const(_) => {}
                    neutron_worldgen::density::SplineValue::Spline(s) => {
                        count_markers(&s.coordinate, counts);
                    }
                }
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);

    let gen = ChunkGenerator::new(seed);
    let st = &gen.state;

    let mut counts = std::collections::HashMap::new();
    count_markers(&st.router.final_density, &mut counts);

    println!("Marker nodes in final_density:");
    for (kind, count) in &counts {
        println!("  {:?}: {}", kind, count);
    }

    // Also count cache slots
    let slot_count = st.reg.cache_slot_count();
    println!("\nCache slot count: {}", slot_count);

    // Test: compute density at (-1, -50, -15) with different counter values
    let wx = -1i32;
    let y = -50i32;
    let wz = -15i32;

    println!("\nDensity at ({wx}, {y}, {wz}) with different counter values:");
    for counter in [0, 100, 1000, 10000, 79645] {
        let mut marker = MarkerState::new(st.cell_width as usize, st.cell_height as usize, slot_count);
        marker.interpolation_counter = counter;
        let mut env = DensityEnv::with_markers(wx, y, wz, st.noises.noises(), &mut marker);
        let density = neutron_worldgen::density::compute(&st.router.final_density, &mut env);
        println!("  counter={counter}: density={density:+.8}");
    }
}
