//! Packed overworld climate parameter points (vanilla `OverworldBiomeBuilder`).
//!
//! The 7 498 points used to live in a 45 k-line Rust array. They are now a
//! little-endian blob so rust-analyzer / grep / incremental builds stay usable.
//!
//! Each record is 97 bytes:
//! - `u8` biome id
//! - `[i64; 12]` min/max for temperature, humidity, continentalness,
//!   erosion, depth, weirdness
//!
//! Source: `BIOME-SPEC.md` via the previous `biome_params.rs` dump.
//! Re-pack after a Mojang climate change; do not hand-edit the `.bin`.

use std::sync::OnceLock;

/// Bytes per climate point (`u8` + 12 × `i64`).
const RECORD_SIZE: usize = 1 + 12 * 8;

/// Embedded parameter table (little-endian records).
const RAW: &[u8] = include_bytes!("../data/biome_params.bin");

/// One climate parameter point: biome id + 6 `[min, max]` intervals.
#[derive(Clone, Copy, Debug)]
pub struct BiomeParam {
    /// Vanilla biome numeric id used by worldgen (not the protocol registry).
    pub biome: u8,
    /// `[t_min, t_max, h_min, h_max, c_min, c_max, e_min, e_max, d_min, d_max, w_min, w_max]`.
    pub intervals: [i64; 12],
}

/// Number of parameter points in the embedded table.
pub const POINT_COUNT: usize = RAW.len() / RECORD_SIZE;

fn decode_at(offset: usize) -> BiomeParam {
    let rec = &RAW[offset..offset + RECORD_SIZE];
    let biome = rec[0];
    let mut intervals = [0i64; 12];
    for (i, slot) in intervals.iter_mut().enumerate() {
        let start = 1 + i * 8;
        *slot = i64::from_le_bytes(rec[start..start + 8].try_into().expect("8-byte i64"));
    }
    BiomeParam { biome, intervals }
}

fn decoded_table() -> &'static [BiomeParam] {
    static TABLE: OnceLock<Vec<BiomeParam>> = OnceLock::new();
    TABLE.get_or_init(|| {
        assert_eq!(
            RAW.len() % RECORD_SIZE,
            0,
            "biome_params.bin is not a whole number of records"
        );
        (0..POINT_COUNT).map(|i| decode_at(i * RECORD_SIZE)).collect()
    })
}

/// Iterate every climate parameter point (decoded once, then borrowed).
pub fn iter() -> impl Iterator<Item = BiomeParam> + 'static {
    decoded_table().iter().copied()
}

/// Parameter point at `index`, if in range.
pub fn get(index: usize) -> Option<BiomeParam> {
    decoded_table().get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_table_has_expected_count() {
        assert_eq!(POINT_COUNT, 7594);
        assert_eq!(RAW.len(), 7594 * RECORD_SIZE);
        assert_eq!(decoded_table().len(), 7594);
    }

    #[test]
    fn distinct_biome_count_and_pale_garden_points() {
        let mut ids = std::collections::BTreeSet::new();
        let mut pale = 0usize;
        for p in iter() {
            ids.insert(p.biome);
            if p.biome == 54 {
                pale += 1;
            }
        }
        // 55 distinct biomes (vanilla 26.2 OverworldBiomeBuilder), 40 pale_garden points
        assert_eq!(ids.len(), 55);
        assert_eq!(pale, 40);
    }

    #[test]
    fn first_and_last_points_match_legacy_dump() {
        let first = get(0).expect("first point");
        assert_eq!(first.biome, 29);
        assert_eq!(
            first.intervals,
            [
                -10000, 10000, -10000, 10000, -12000, -10500, -10000, 10000, 0, 0, -10000, 10000,
            ]
        );

        let last = get(POINT_COUNT - 1).expect("last point");
        assert_eq!(last.biome, 31);
        assert_eq!(
            last.intervals,
            [
                -10000, 10000, -10000, 10000, -10000, 10000, -10000, -3750, 11000, 11000, -10000,
                10000,
            ]
        );
    }

    #[test]
    fn lush_and_sulfur_keep_unique_ids() {
        let lush = decoded_table()
            .iter()
            .find(|p| p.biome == 34)
            .expect("lush_caves point");
        assert_eq!(
            lush.intervals,
            [
                -10000, 10000, 7000, 10000, -10000, 10000, -10000, 10000, 2000, 9000, -10000, 10000,
            ]
        );
        let sulfur = decoded_table()
            .iter()
            .find(|p| p.biome == 36)
            .expect("sulfur_caves point");
        assert_eq!(
            sulfur.intervals,
            [
                -10000, 10000, -10000, 10000, -1900, 5500, 4500, 10000, 2000, 9000, -11000, -8500,
            ]
        );
    }
}
