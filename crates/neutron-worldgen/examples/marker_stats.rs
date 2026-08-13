use neutron_worldgen::density::{compute, DensityEnv};
use neutron_worldgen::WorldgenState;

fn squeeze(v: f64) -> f64 {
    let c = v.clamp(-1.0, 1.0);
    c / 2.0 - c * c * c / 24.0
}

fn main() {
    let st = WorldgenState::overworld(12345);

    let (a_part, noodle_part) = match &*st.router.final_density {
        neutron_worldgen::density::DFNode::Min(a, b) => (a.clone(), b.clone()),
        _ => panic!("final_density must be a min node"),
    };

    println!("=== Generator pipeline at chunk (0,0) ===");
    for y in (0..=100).step_by(5) {
        let mut ms = neutron_worldgen::density::MarkerState::new(4, 8);
        let mut env = DensityEnv::with_markers(8, y, 8, st.noises.noises(), &mut ms);

        let inner_val = compute(&a_part, &mut env);
        let squeezed = squeeze(inner_val);
        let noodle_val = compute(&noodle_part, &mut env);
        let final_val = squeezed.min(noodle_val);

        println!(
            "  Y={:3}: inner={:.4} squeeze={:.4} noodle={:.4} final={:.4} stone={}",
            y,
            inner_val,
            squeezed,
            noodle_val,
            final_val,
            if final_val > 0.0 { "STONE" } else { "aquifer" }
        );
    }
}
