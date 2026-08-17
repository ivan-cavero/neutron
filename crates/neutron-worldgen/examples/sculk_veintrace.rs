// Vein-feature differential: run sculk_vein for origin (96,-32) on the
// vanilla-stripped 3x3 world, recording gate decisions + per-attempt events.
// Writes tools/worldgen-probe/vein-gate-96--32.txt for the Java probe.
// cargo run -p neutron-worldgen --example sculk_veintrace --release

use neutron_worldgen::sculk;
use neutron_worldgen::ChunkGenerator;

include!("sculk_vanworld_world.rs");

fn main() {
    let seed: i64 = 12345;
    let cx = 6;
    let cz = -2;
    let ox0 = 96;
    let oz0 = -32;

    let g = ChunkGenerator::new(seed);
    let mut region = g.generate_ores_region(cx, cz);
    overlay_vanilla_stripped(&mut region, cx, cz, seed);

    // Write the overlaid world so the Java probe sees the identical input
    // (non-air cells only; name = vanilla_name without the minecraft: prefix).
    {
        use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
        let mut s = String::new();
        for y in WORLD_BOTTOM..WORLD_TOP {
            for lz in 0..region.side {
                for lx in 0..region.side {
                    let x = region.origin_x + lx;
                    let z = region.origin_z + lz;
                    let b = region.get(x, y, z);
                    if b == neutron_worldgen::surface::BlockId::Air {
                        continue;
                    }
                    let name =
                        neutron_worldgen::surface::vanilla_name(b).trim_start_matches("minecraft:");
                    s.push_str(&format!("{x} {y} {z} {name}\n"));
                }
            }
        }
        std::fs::write("tools/worldgen-probe/cave-overlay-3x3.txt", s).expect("write world dump");
    }

    // Gate decisions from neutron's biome source (or vanilla 3D biomes).
    if std::env::var_os("VAN_BIOMES").is_some() {
        let path = std::path::PathBuf::from(
            "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
        );
        let vregion = Region::open(&path).unwrap().with_coords(0, -1);
        let mut quarts = std::collections::HashSet::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                quarts.extend(load_van_deep_dark_quarts(&vregion, cx + dx, cz + dz));
            }
        }
        println!("vanilla deep_dark quarts={}", quarts.len());
        sculk::set_biome_gate_override(Some(std::sync::Arc::new(move |x, y, z| {
            quarts.contains(&(x.div_euclid(4), y.div_euclid(4), z.div_euclid(4)))
        })));
    }
    let gate = sculk::probe_vein_gate_origin(ox0, oz0, seed, 0, &g.state);
    let accepted = gate.iter().filter(|e| e.3 == 1).count();
    println!(
        "gate entries={} accepted={} (deep_dark)",
        gate.len(),
        accepted
    );
    let mut s = String::new();
    for &(x, y, z, ok) in &gate {
        s.push_str(&format!("{x} {y} {z} {ok}\n"));
    }
    std::fs::write("tools/worldgen-probe/vein-gate-96--32.txt", s).expect("write gate file");

    // Also export the patch-feature gate for the Java flow probe.
    let pgate = sculk::probe_patch_gate_origin(ox0, oz0, seed, 1, &g.state);
    let mut ps = String::new();
    for &(x, y, z, ok) in &pgate {
        ps.push_str(&format!(
            "{x} {y} {z} {ok}
"
        ));
    }
    std::fs::write("tools/worldgen-probe/patch-gate-96--32.txt", ps).expect("write patch gate");

    let (events, faces) = sculk::probe_vein_origin_traced(&mut region, ox0, oz0, seed, 0, &gate);
    let placed = events.iter().filter(|e| e.starts_with("PLACED")).count();
    let solid = events.iter().filter(|e| e.starts_with("SOLID")).count();
    println!(
        "events: PLACED={placed} SOLID={solid} FAIL={}",
        events.len() - placed - solid
    );
    for e in events.iter().take(80) {
        println!("{e}");
    }
    // Final vein map (sorted) with face masks.
    let mut cells: Vec<_> = faces.iter().collect();
    cells.sort();
    println!("final vein cells={}", cells.len());
    for ((x, y, z), m) in cells.iter() {
        if region.get(*x, *y, *z) == neutron_worldgen::surface::BlockId::SculkVein {
            println!("VEIN {x},{y},{z}#{m}");
        }
    }
}
