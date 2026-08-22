//! Chunk data encoding for the 26.2 `level_chunk_with_light` packet.
//!
//! Playable path (`encode_playable_chunk`) remaps internal `BlockId`s to
//! vanilla 26.2 block-state IDs and uses the 26.2 wire layout:
//! heightmap map + section bytes + empty block-entity list + BitSet light.
//!
//! The older flat-world helpers stay for unit tests of palette packing.

use bytes::{BufMut, BytesMut};
use neutron_protocol::types::write_varint;
use neutron_worldgen::{ChunkGenerator, GeneratedChunk};

use crate::protocol_data;

use ussr_nbt::mutf8::MString;
use ussr_nbt::owned::{Compound, Nbt, Tag};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of sections per chunk (1.18+): Y range -64 to 320 = 384 blocks = 24 sections.
const SECTIONS_PER_CHUNK: i32 = 24;

/// Minimum Y coordinate.
const MIN_Y: i32 = -64;

/// Block state IDs (1.21.x / 26.x approximate values).
const BLOCK_AIR: i32 = 0;
const BLOCK_STONE: i32 = 1;
const BLOCK_GRASS_BLOCK: i32 = 8; // grass_block[snowy=false]
const BLOCK_DIRT: i32 = 3;
const BLOCK_BEDROCK: i32 = 33;

/// Biome ID for plains.
const BIOME_PLAINS: i32 = 0;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encode a generated chunk for the 26.2 play packet (everything after X/Z).
pub fn encode_playable_chunk(chunk: &GeneratedChunk) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(16_384);

    write_heightmaps_26(&mut buf, &chunk.heightmap);

    let mut sections = BytesMut::with_capacity(12_288);
    for section_idx in 0..SECTIONS_PER_CHUNK {
        let y_start = MIN_Y + section_idx * 16;
        let mut section_blocks = Vec::with_capacity(4096);
        let mut non_air = 0i16;
        let mut fluids = 0i16;
        for local_y in 0..16i32 {
            let world_y = y_start + local_y;
            let idx_base = (world_y - MIN_Y) as usize * 16 * 16;
            for lz in 0..16usize {
                for lx in 0..16usize {
                    let internal = chunk.blocks[idx_base + lz * 16 + lx];
                    if internal != 0 {
                        non_air += 1;
                    }
                    // Internal 50 = water, 51 = lava (BlockId::is_fluid).
                    if internal == 50 || internal == 51 {
                        fluids += 1;
                    }
                    section_blocks.push(if internal == 50 || internal == 51 {
                        fluid_state_id(chunk, lx, world_y, lz)
                    } else {
                        protocol_data::block_state_id(internal)
                    });
                }
            }
        }

        // Vanilla stores 4×4×4 quarts per section, YZX order (matches worldgen
        // storage layout in GeneratedChunk).
        let biome_base = (section_idx as usize) * 64;
        let mut section_biomes = Vec::with_capacity(64);
        for y4 in 0..4usize {
            for z4 in 0..4usize {
                for x4 in 0..4usize {
                    let internal = chunk.biomes[biome_base + y4 * 16 + z4 * 4 + x4];
                    section_biomes.push(protocol_data::biome_protocol_id(internal));
                }
            }
        }

        sections.put_i16(non_air);
        sections.put_i16(fluids);
        write_paletted_container(&mut sections, &section_blocks, PaletteKind::Block);
        write_paletted_container(&mut sections, &section_biomes, PaletteKind::Biome);
    }

    write_varint(&mut buf, sections.len() as i32).expect("varint");
    buf.put_slice(&sections);

    // Empty block-entity list (Prefixed Array).
    write_varint(&mut buf, 0).expect("varint");

    write_sky_light_from_heightmap(&mut buf, &chunk.heightmap);
    buf.to_vec()
}

/// Pick a 26.2 water/lava state so the client renders slopes and falls.
///
/// Worldgen only stores "this cell is fluid". Vanilla uses `level=` for
/// the mesh: source, flowing into air, or falling into a hole.
fn fluid_state_id(chunk: &GeneratedChunk, x: usize, y: i32, z: usize) -> i32 {
    let here = chunk.blocks[(y - MIN_Y) as usize * 256 + z * 16 + x];
    let is_water = here == 50;
    let below_air = y > MIN_Y
        && chunk.blocks[(y - 1 - MIN_Y) as usize * 256 + z * 16 + x] == 0;
    if below_air {
        return if is_water {
            protocol_data::WATER_FALLING
        } else {
            protocol_data::LAVA_FALLING
        };
    }
    const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    for (dx, dz) in DIRS {
        let nx = x as i32 + dx;
        let nz = z as i32 + dz;
        if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
            continue;
        }
        let n = chunk.blocks[(y - MIN_Y) as usize * 256 + nz as usize * 16 + nx as usize];
        if n == 0 {
            return if is_water {
                protocol_data::WATER_FLOW
            } else {
                protocol_data::LAVA_SOURCE
            };
        }
    }
    if is_water {
        protocol_data::WATER_SOURCE
    } else {
        protocol_data::LAVA_SOURCE
    }
}

/// Generate a chunk using the worldgen crate's terrain generation.
///
/// Used by unit tests. The live server goes through `WorldgenHandle` so the
/// expensive `ChunkGenerator` is built once.
pub fn build_worldgen_chunk(chunk_x: i32, chunk_z: i32, seed: i64) -> Vec<u8> {
    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(chunk_x, chunk_z);
    encode_playable_chunk(&chunk)
}



/// Generate a flat chunk's data payload (heightmaps + sections + biomes).
///
/// Returns the raw bytes that go inside the ChunkDataAndUpdateLight packet
/// after the chunk coordinates and chunk_data_len VarInt.
pub fn build_flat_chunk(chunk_x: i32, chunk_z: i32) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(4096);

    // 1. Heightmaps (NBT compound with MOTION_BLOCKING long array)
    let heightmaps_nbt = build_heightmaps_nbt(chunk_x, chunk_z);
    buf.put_slice(&heightmaps_nbt);

    // 2. Biome data (VarInt: number of biome entries per section = 1024)
    // In modern format, biomes are part of each section, not separate.
    // Actually, the chunk data format in 1.18+ is:
    //   - Heightmaps NBT
    //   - Biomes array (1024 ints, but stored as part of the chunk sections)
    //   - For each section: block_count + block palette + block data + biome palette + biome data
    //
    // We'll encode each section as a block.
    for section_idx in 0..SECTIONS_PER_CHUNK {
        let y_start = MIN_Y + section_idx * 16;
        encode_section(&mut buf, y_start);
    }

    // 3. Block entity count = 0
    write_varint(&mut buf, 0).expect("varint write");

    buf.to_vec()
}

/// Generate full sky light + block light data for all sections.
///
/// Returns the raw bytes for the light data portion of the packet.
pub fn build_full_light() -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(512);

    // Sky light mask: bitmask indicating which sections have sky light data.
    // 24 sections + 1 above + 1 below = 26 bits.
    // All sections have sky light (flat world, no obstructions).
    // Each bit from LSB: section -1 (below), section 0, section 1, ..., section 23, section 24 (above).
    let sky_light_mask: u64 = (1u64 << 26) - 1; // bits 0-25 all set (26 sections)

    // Block light mask: no block light needed (all 0).
    let block_light_mask: u64 = 0;

    // Empty sky light sections mask: sections that are fully empty.
    // For a flat world, sections 7-23 (y=48 to y=320) are all air, so sky light is full.
    // We won't send empty sections data since they're implicitly full (0xFF).
    let empty_sky_light_mask: u64 = 0;

    // Empty block light sections mask.
    let empty_block_light_mask: u64 = 0;

    // Sky light mask
    write_varint(&mut buf, sky_light_mask as i32).expect("varint write");
    // Block light mask
    write_varint(&mut buf, block_light_mask as i32).expect("varint write");
    // Empty sky light sections mask
    write_varint(&mut buf, empty_sky_light_mask as i32).expect("varint write");
    // Empty block light sections mask
    write_varint(&mut buf, empty_block_light_mask as i32).expect("varint write");

    // Sky light data arrays (one per section with data).
    // Each array: 2048 bytes of nibble-packed light values (4 bits per block).
    // For a flat world with no obstructions, sky light = 15 (0xFF) everywhere above ground.
    // Below y=5 (grass), sky light = 0. At and above y=5, sky light = 15.
    let light_array = build_sky_light_array();
    let section_count = sky_light_mask.count_ones() as usize;
    for _ in 0..section_count {
        write_varint(&mut buf, light_array.len() as i32).expect("varint write");
        buf.put_slice(&light_array);
    }

    // Block light data arrays (none needed).
    // (no arrays since block_light_mask = 0)

    buf.to_vec()
}

/// Generate chunks in a spiral pattern around (center_x, center_z).
///
/// Returns a list of (chunk_x, chunk_z) in the order they should be sent.
pub fn spiral_chunks(center_x: i32, center_z: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut chunks = Vec::new();
    let mut x = center_x;
    let mut z = center_z;
    let mut dx = 1i32;
    let mut dz = 0i32;
    let mut segment_length = 1;
    let mut segment_passed = 0;
    let mut segment转弯s = 0;

    // Add center chunk first.
    chunks.push((x, z));

    for _ in 0..(2 * radius + 1).pow(2) - 1 {
        x += dx;
        z += dz;
        chunks.push((x, z));

        segment_passed += 1;
        if segment_passed >= segment_length {
            segment_passed = 0;
            segment转弯s += 1;

            // Turn left: (dx, dz) -> (-dz, dx)
            let new_dx = -dz;
            let new_z = dx;
            dx = new_dx;
            dz = new_z;

            if segment转弯s % 2 == 0 {
                segment_length += 1;
            }
        }

        // Stop when we've covered the radius.
        if (x - center_x).abs() > radius && (z - center_z).abs() > radius {
            break;
        }
    }

    chunks
}

// ---------------------------------------------------------------------------
// 26.2 wire helpers (heightmaps map + BitSet light)
// ---------------------------------------------------------------------------

/// `Heightmap.Types.MOTION_BLOCKING` protocol id (enum declaration order).
const HEIGHTMAP_MOTION_BLOCKING: i32 = 4;

/// 24 world sections + 1 below + 1 above.
const LIGHT_SECTION_COUNT: usize = 26;

fn write_heightmaps_26(buf: &mut BytesMut, heightmap: &[i16]) {
    // Map size = 1 (only MOTION_BLOCKING).
    write_varint(buf, 1).expect("varint");
    write_varint(buf, HEIGHTMAP_MOTION_BLOCKING).expect("varint");

    // 256 entries, 9 bits each, packed 7 per long → 37 longs.
    let bits_per_entry = 9;
    let mask: i64 = (1i64 << bits_per_entry) - 1;
    let entries_per_long = 64 / bits_per_entry;
    let total_longs = (256 + entries_per_long - 1) / entries_per_long;

    write_varint(buf, total_longs as i32).expect("varint");
    for long_idx in 0..total_longs {
        let mut long_val: i64 = 0;
        for entry in 0..entries_per_long {
            let global_entry = long_idx * entries_per_long + entry;
            if global_entry >= 256 {
                break;
            }
            let h = i64::from(heightmap.get(global_entry).copied().unwrap_or(-64)) + 1;
            let clamped = h.clamp(0, 383);
            long_val |= (clamped & mask) << (entry * bits_per_entry);
        }
        buf.put_i64(long_val);
    }
}

fn write_bitset(buf: &mut BytesMut, bits: u64, long_count: i32) {
    write_varint(buf, long_count).expect("varint");
    for i in 0..long_count {
        buf.put_i64(if i == 0 { bits as i64 } else { 0 });
    }
}

/// Sky light from the column heightmap: 15 above the surface, 0 below.
///
/// Sending 15 everywhere made oceans look hollow (the client has no
/// occlusion to shade water). Block light stays empty.
fn write_sky_light_from_heightmap(buf: &mut BytesMut, heightmap: &[i16]) {
    let mut sky_bits: u64 = 0;
    let mut empty_sky_bits: u64 = 0;
    let mut sky_layers: Vec<Vec<u8>> = Vec::new();

    for section in 0..LIGHT_SECTION_COUNT {
        let y0 = MIN_Y - 16 + section as i32 * 16;
        let mut data = vec![0u8; 2048];
        let mut any_light = false;
        let mut all_dark = true;
        for ly in 0..16i32 {
            let y = y0 + ly;
            for z in 0..16usize {
                for x in 0..16usize {
                    let surface = i32::from(heightmap.get(z * 16 + x).copied().unwrap_or(64));
                    let level: u8 = if y > surface { 15 } else { 0 };
                    if level > 0 {
                        any_light = true;
                        all_dark = false;
                    }
                    let idx = ((ly as usize) << 8) | (z << 4) | x;
                    let byte_i = idx >> 1;
                    if idx & 1 == 0 {
                        data[byte_i] |= level;
                    } else {
                        data[byte_i] |= level << 4;
                    }
                }
            }
        }
        if all_dark {
            empty_sky_bits |= 1u64 << section;
        } else if any_light {
            sky_bits |= 1u64 << section;
            sky_layers.push(data);
        }
    }

    write_bitset(buf, sky_bits, 1);
    write_bitset(buf, 0, 1); // block mask
    write_bitset(buf, empty_sky_bits, 1);
    write_bitset(buf, (1u64 << LIGHT_SECTION_COUNT) - 1, 1); // all block-light empty

    write_varint(buf, sky_layers.len() as i32).expect("varint");
    for layer in &sky_layers {
        write_varint(buf, layer.len() as i32).expect("varint");
        buf.put_slice(layer);
    }
    write_varint(buf, 0).expect("varint");
}

// ---------------------------------------------------------------------------
// Worldgen chunk helpers
// ---------------------------------------------------------------------------

/// Build heightmaps NBT from the worldgen heightmap data.
///
/// The worldgen heightmap stores the Y coordinate of the highest non-air block
/// per column. We need to pack these into MOTION_BLOCKING as a LongArray
/// with 9 bits per entry (range 0-384).
fn build_heightmaps_from_chunk(heightmap: &[i16]) -> Vec<u8> {
    let mut compound = Compound { tags: Vec::new() };

    // MOTION_BLOCKING: 256 entries, 9 bits each, packed into 37 longs.
    // Each entry is height + 1 (motion blocking = highest solid + 1).
    let bits_per_entry = 9;
    let mask: i64 = (1i64 << bits_per_entry) - 1;
    let entries_per_long = 64 / bits_per_entry; // 7
    let total_longs = (256 + entries_per_long - 1) / entries_per_long; // 37

    let mut motion_blocking = Vec::with_capacity(total_longs);
    for long_idx in 0..total_longs {
        let mut long_val: i64 = 0;
        for entry in 0..entries_per_long {
            let global_entry = long_idx * entries_per_long + entry;
            if global_entry >= 256 {
                break;
            }
            // heightmap stores the Y of highest non-air block; motion_blocking = Y + 1
            let h = (heightmap[global_entry] as i64) + 1;
            let clamped = h.clamp(0, 383);
            long_val |= (clamped & mask) << (entry * bits_per_entry);
        }
        motion_blocking.push(long_val);
    }

    compound.tags.push((
        MString::from("MOTION_BLOCKING"),
        Tag::LongArray(motion_blocking.into()),
    ));

    let nbt = Nbt {
        name: MString::new(),
        compound,
    };
    let mut buf = Vec::new();
    nbt.write(&mut buf).expect("NBT write should not fail");
    buf
}

/// 26.2 `PalettedContainer` strategy (blocks vs biomes).
#[derive(Clone, Copy)]
enum PaletteKind {
    Block,
    Biome,
}

fn ceil_log2(n: usize) -> i32 {
    if n <= 1 {
        0
    } else {
        (usize::BITS - (n - 1).leading_zeros()) as i32
    }
}

fn bits_for_palette(kind: PaletteKind, palette_len: usize) -> i32 {
    if palette_len <= 1 {
        return 0;
    }
    let needed = ceil_log2(palette_len);
    match kind {
        // Strategy$1: 1–3 bits are remapped to 4-bit linear.
        PaletteKind::Block => needed.max(4).min(8),
        // Strategy$2: 1–3 linear, then global.
        PaletteKind::Biome => needed.min(3),
    }
}

/// Vanilla 26.2 PalettedContainer wire format:
/// `byte bits` + palette + fixed-size long array (no VarInt length).
///
/// bits==0: SingleValuePalette writes only the one VarInt id (no size, no longs).
fn write_paletted_container(buf: &mut BytesMut, values: &[i32], kind: PaletteKind) {
    let mut palette: Vec<i32> = Vec::new();
    for &id in values {
        if !palette.contains(&id) {
            palette.push(id);
        }
    }
    let bits = bits_for_palette(kind, palette.len());
    buf.put_u8(bits as u8);

    if bits == 0 {
        write_varint(buf, palette.first().copied().unwrap_or(0)).expect("varint");
        return;
    }

    write_varint(buf, palette.len() as i32).expect("varint");
    for &id in &palette {
        write_varint(buf, id).expect("varint");
    }

    let indices: Vec<usize> = values
        .iter()
        .map(|id| palette.iter().position(|&p| p == *id).unwrap_or(0))
        .collect();
    for long_val in pack_to_longs(&indices, bits) {
        buf.put_i64(long_val);
    }
}

// ---------------------------------------------------------------------------
// Section encoding (flat world)
// ---------------------------------------------------------------------------

fn encode_section(buf: &mut BytesMut, y_start: i32) {
    // Determine which blocks are in this section.
    let palette = build_section_palette(y_start);
    let blocks = build_section_blocks(y_start, &palette);

    // Block count (number of non-air blocks in this section).
    let non_air_count = blocks.iter().filter(|&&id| id != 0).count() as i16;
    buf.put_i16(non_air_count);

    // Block state palette and data
    encode_palette_and_data(buf, &palette, &blocks);

    // Biome palette and data (single biome: plains)
    encode_biome_section(buf);
}

/// Build the palette for a section at y_start.
fn build_section_palette(y_start: i32) -> Vec<i32> {
    let mut palette = vec![BLOCK_AIR]; // air is always palette[0]

    for local_y in 0..16i32 {
        let global_y = y_start + local_y;
        let block_id = get_block_at_y(global_y);
        if block_id != BLOCK_AIR && !palette.contains(&block_id) {
            palette.push(block_id);
        }
    }

    palette
}

/// Build the block ID array for a section at y_start.
fn build_section_blocks(y_start: i32, palette: &[i32]) -> Vec<usize> {
    let mut blocks = Vec::with_capacity(4096);

    for local_y in 0..16i32 {
        let global_y = y_start + local_y;
        let block_id = get_block_at_y(global_y);
        let palette_idx = palette.iter().position(|&id| id == block_id).unwrap_or(0);
        // All 256 blocks in this y-layer have the same block type.
        for _ in 0..256 {
            blocks.push(palette_idx);
        }
    }

    blocks
}

/// Get the block state ID at a given global Y coordinate.
fn get_block_at_y(y: i32) -> i32 {
    match y {
        -64..=-61 => BLOCK_BEDROCK,
        -60..=3 => BLOCK_STONE,
        4 => BLOCK_DIRT,
        5 => BLOCK_GRASS_BLOCK,
        _ => BLOCK_AIR,
    }
}

/// Encode a palette and compact block data array.
fn encode_palette_and_data(buf: &mut BytesMut, palette: &[i32], blocks: &[usize]) {
    let bits_per_block = calculate_bits_per_block(palette.len());

    // bits_per_block as VarInt
    write_varint(buf, bits_per_block).expect("varint write");

    // Palette
    if bits_per_block <= 8 {
        // Indirect palette: VarInt palette length, then VarInt block state IDs.
        write_varint(buf, palette.len() as i32).expect("varint write");
        for &block_id in palette {
            write_varint(buf, block_id).expect("varint write");
        }
    } else {
        // Direct palette: each block ID is written directly.
        // (palette is implicit, not stored)
    }

    // Data array: compacted longs.
    let longs = pack_to_longs(blocks, bits_per_block);
    write_varint(buf, longs.len() as i32).expect("varint write");
    for &long_val in &longs {
        buf.put_i64(long_val);
    }
}

/// Calculate bits per block for a given palette size.
fn calculate_bits_per_block(palette_size: usize) -> i32 {
    if palette_size <= 1 {
        0
    } else if palette_size <= 2 {
        1
    } else if palette_size <= 4 {
        2
    } else if palette_size <= 8 {
        3
    } else if palette_size <= 16 {
        4
    } else if palette_size <= 32 {
        5
    } else if palette_size <= 64 {
        6
    } else if palette_size <= 128 {
        7
    } else if palette_size <= 256 {
        8
    } else {
        15 // direct palette
    }
}

/// Pack indices into an array of i64 longs with the given bits per entry.
fn pack_to_longs(indices: &[usize], bits_per_entry: i32) -> Vec<i64> {
    if bits_per_entry == 0 {
        return vec![0; 0]; // Empty data array for single-value palette.
    }

    let entries_per_long = 64 / bits_per_entry as usize;
    let mask = (1i64 << bits_per_entry) - 1;
    let total_longs = (indices.len() + entries_per_long - 1) / entries_per_long;

    let mut longs = vec![0i64; total_longs];

    for (i, &value) in indices.iter().enumerate() {
        let long_index = i / entries_per_long;
        let bit_offset = (i % entries_per_long) as i32 * bits_per_entry;
        longs[long_index] |= ((value as i64) & mask) << bit_offset;
    }

    longs
}

/// Encode biome data for a single section.
///
/// Each section has 1024 biome entries (4x4x4 biome resolution for 16x16x16 blocks).
/// For a flat world, all biomes are "plains".
fn encode_biome_section(buf: &mut BytesMut) {
    let bits_per_biome = 1; // 2-entry palette: [plains, ...] but we only need plains.

    // bits_per_biome as VarInt
    write_varint(buf, bits_per_biome).expect("varint write");

    // Palette: just plains (and we need at least 2 entries for bits=1)
    write_varint(buf, 2).expect("varint write"); // palette size = 2
    write_varint(buf, BIOME_PLAINS).expect("varint write"); // plains
    write_varint(buf, BIOME_PLAINS).expect("varint write"); // duplicate (padding for palette[1])

    // Data: 1024 entries, each 1 bit = index 0 (plains).
    // 64 entries per long, 1024/64 = 16 longs, all zeros.
    write_varint(buf, 16).expect("varint write");
    for _ in 0..16 {
        buf.put_i64(0);
    }
}

// ---------------------------------------------------------------------------
// Heightmaps
// ---------------------------------------------------------------------------

fn build_heightmaps_nbt(_chunk_x: i32, _chunk_z: i32) -> Vec<u8> {
    let mut compound = Compound { tags: Vec::new() };

    // MOTION_BLOCKING: long array of 37 longs (256 entries packed into 6-bit fields).
    // Each entry is the Y coordinate of the highest non-air block + 1.
    // For our flat world, the grass is at y=5, so MOTION_BLOCKING = 6 (5+1).
    let mut motion_blocking = Vec::with_capacity(37);
    let motion_blocking_value: i64 = 6; // y=5 + 1
    let mask: i64 = 0x3F; // 6-bit mask
    let entries_per_long = 64 / 6; // 10

    for long_idx in 0..37 {
        let mut long_val: i64 = 0;
        for entry in 0..entries_per_long {
            let global_entry = long_idx * entries_per_long + entry;
            if global_entry >= 256 {
                break;
            }
            long_val |= (motion_blocking_value & mask) << (entry * 6);
        }
        motion_blocking.push(long_val);
    }

    // Root tag is not needed — the NBT writer adds it.
    // Actually, the heightmaps data is a compound tag written directly in the packet.
    // It's a root compound with name "" (empty string).
    compound.tags.push((
        MString::from("MOTION_BLOCKING"),
        Tag::LongArray(motion_blocking.into()),
    ));

    let nbt = Nbt {
        name: MString::new(),
        compound,
    };
    let mut buf = Vec::new();
    nbt.write(&mut buf).expect("NBT write should not fail");
    buf
}

// ---------------------------------------------------------------------------
// Light
// ---------------------------------------------------------------------------

fn build_sky_light_array() -> Vec<u8> {
    // 2048 bytes for a 16x16x16 section (4096 blocks, 2 blocks per byte).
    // For sky light in a flat world:
    // - y >= 6 (above grass): light level = 15 (0xFF per byte pair)
    // - y < 6 (below/ground): light level = 0 (0x00)
    //
    // In our section encoding, local_y=0 is y_start, local_y=15 is y_start+15.
    // For sections that are entirely above y=5 (sections 5+), all light = 15.
    // For section 0 (y=-64 to -49), all light = 0 (below ground).
    // For section 4 (y=0 to 15), light varies: 0 below y=6, 15 at y=6+.

    // For simplicity, return a full-light array (light = 15 everywhere).
    // The client will handle the occlusion.
    vec![0xFF; 2048]
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_block_at_y() {
        assert_eq!(get_block_at_y(-64), BLOCK_BEDROCK);
        assert_eq!(get_block_at_y(-63), BLOCK_BEDROCK);
        assert_eq!(get_block_at_y(-60), BLOCK_STONE);
        assert_eq!(get_block_at_y(0), BLOCK_STONE);
        assert_eq!(get_block_at_y(3), BLOCK_STONE);
        assert_eq!(get_block_at_y(4), BLOCK_DIRT);
        assert_eq!(get_block_at_y(5), BLOCK_GRASS_BLOCK);
        assert_eq!(get_block_at_y(6), BLOCK_AIR);
        assert_eq!(get_block_at_y(319), BLOCK_AIR);
    }

    #[test]
    fn test_calculate_bits_per_block() {
        assert_eq!(calculate_bits_per_block(1), 0);
        assert_eq!(calculate_bits_per_block(2), 1);
        assert_eq!(calculate_bits_per_block(3), 2);
        assert_eq!(calculate_bits_per_block(5), 3);
        assert_eq!(calculate_bits_per_block(9), 4);
        assert_eq!(calculate_bits_per_block(17), 5);
        assert_eq!(calculate_bits_per_block(33), 6);
        assert_eq!(calculate_bits_per_block(129), 8);
    }

    #[test]
    fn test_pack_to_longs() {
        // 4 bits per entry, values [0, 1, 2, 3, 0]
        let indices = vec![0, 1, 2, 3, 0];
        let longs = pack_to_longs(&indices, 4);
        assert_eq!(longs.len(), 1); // 5 entries fit in 1 long (16 per long)
                                    // Entry 0 = 0, Entry 1 = 1 << 4, Entry 2 = 2 << 8, Entry 3 = 3 << 12, Entry 4 = 0 << 16
        assert_eq!(longs[0], (1 << 4) | (2 << 8) | (3 << 12));
    }

    #[test]
    fn test_pack_to_longs_single_bit() {
        let indices = vec![0, 1, 0, 1];
        let longs = pack_to_longs(&indices, 1);
        assert_eq!(longs.len(), 1); // 64 entries per long
                                    // Entry 0 = 0, Entry 1 = 1 << 1, Entry 2 = 0 << 2, Entry 3 = 1 << 3
        assert_eq!(longs[0], (1 << 1) | (1 << 3));
    }

    #[test]
    fn test_build_flat_chunk_produces_data() {
        let data = build_flat_chunk(0, 0);
        assert!(!data.is_empty());
        // Should be at least: heightmaps NBT + 24 sections.
        assert!(data.len() > 1000);
    }

    #[test]
    fn test_build_full_light_produces_data() {
        let data = build_full_light();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_spiral_chunks_center_is_first() {
        let chunks = spiral_chunks(0, 0, 3);
        assert_eq!(chunks[0], (0, 0));
        // Should contain chunks in a spiral pattern.
        assert!(chunks.len() > 10);
    }

    #[test]
    fn test_section_palette_for_grass_level() {
        // Section at y_start = 0 contains y=0..15.
        // Blocks: y=0..3 = stone, y=4 = dirt, y=5 = grass, y=6..15 = air.
        let palette = build_section_palette(0);
        assert!(palette.contains(&BLOCK_AIR));
        assert!(palette.contains(&BLOCK_STONE));
        assert!(palette.contains(&BLOCK_DIRT));
        assert!(palette.contains(&BLOCK_GRASS_BLOCK));
    }

    #[test]
    fn test_section_palette_for_all_air() {
        // Section at y_start = 128 is all air.
        let palette = build_section_palette(128);
        assert_eq!(palette, vec![BLOCK_AIR]);
    }

    #[test]
    fn test_non_air_count_for_air_section() {
        let palette = build_section_palette(128);
        let blocks = build_section_blocks(128, &palette);
        let non_air = blocks.iter().filter(|&&id| id != 0).count();
        assert_eq!(non_air, 0);
    }

    #[test]
    fn test_build_worldgen_chunk_produces_data() {
        let data = build_worldgen_chunk(0, 0, 42);
        assert!(!data.is_empty());
        // Should be at least: heightmaps NBT + 24 sections.
        assert!(data.len() > 1000);
    }

    #[test]
    fn test_build_worldgen_chunk_deterministic() {
        let data1 = build_worldgen_chunk(5, 5, 12345);
        let data2 = build_worldgen_chunk(5, 5, 12345);
        assert_eq!(
            data1, data2,
            "same seed+coords should produce identical output"
        );
    }

    #[test]
    fn test_build_worldgen_chunk_different_chunks_differ() {
        let data1 = build_worldgen_chunk(0, 0, 42);
        let data2 = build_worldgen_chunk(1, 0, 42);
        // Different chunk positions should produce different data (terrain varies).
        assert_ne!(data1, data2);
    }

    #[test]
    fn test_block_palette_uses_min_4_bits() {
        assert_eq!(bits_for_palette(PaletteKind::Block, 1), 0);
        assert_eq!(bits_for_palette(PaletteKind::Block, 2), 4);
        assert_eq!(bits_for_palette(PaletteKind::Block, 16), 4);
        assert_eq!(bits_for_palette(PaletteKind::Block, 17), 5);
        assert_eq!(bits_for_palette(PaletteKind::Biome, 1), 0);
        assert_eq!(bits_for_palette(PaletteKind::Biome, 2), 1);
        assert_eq!(bits_for_palette(PaletteKind::Biome, 8), 3);
    }

    #[test]
    fn test_single_value_palette_has_no_data_longs() {
        let mut buf = BytesMut::new();
        write_paletted_container(&mut buf, &[0; 4096], PaletteKind::Block);
        // byte(0) + varint(air=0)
        assert_eq!(&buf[..], &[0, 0]);
    }

    #[test]
    fn test_two_block_section_writes_256_longs() {
        let mut values = vec![0i32; 4096];
        values[0] = 1;
        let mut buf = BytesMut::new();
        write_paletted_container(&mut buf, &values, PaletteKind::Block);
        assert_eq!(buf[0], 4); // 4-bit linear
                               // palette size varint (2) + two ids + 256 longs, no length prefix
        let longs = pack_to_longs(
            &values
                .iter()
                .map(|&v| if v == 0 { 0 } else { 1 })
                .collect::<Vec<_>>(),
            4,
        );
        assert_eq!(longs.len(), 256);
        assert_eq!(buf.len(), 1 + 1 + 1 + 1 + 256 * 8);
    }

    #[test]
    fn test_playable_chunk_sections_are_26_2_sized() {
        // A worldgen chunk must produce a section buffer the client can drain
        // exactly: 24 × (2 shorts + states + biomes).
        let data = build_worldgen_chunk(0, 0, 12345);
        assert!(data.len() > 100);
        // Heightmaps map starts with varint count; we don't fully parse it here,
        // but the payload must exist and be larger than 24 empty air sections.
        let empty_air_section = 2 + 2 + 2 + 2; // two shorts + two single-value palettes
        assert!(data.len() > 24 * empty_air_section);
    }

    #[test]
    fn test_protocol_block_ids_are_vanilla_26_2() {
        assert_eq!(crate::protocol_data::block_state_id(0), 0); // air
        assert_eq!(crate::protocol_data::block_state_id(1), 1); // stone
        assert_eq!(crate::protocol_data::block_state_id(12), 9); // grass_block[snowy=false]
        assert_eq!(crate::protocol_data::block_state_id(33), 85); // bedrock
        assert_eq!(crate::protocol_data::biome_protocol_id(1), 1); // plains
        assert_eq!(crate::protocol_data::block_state_id(50), protocol_data::WATER_SOURCE);
        assert_eq!(crate::protocol_data::WATER_FALLING, 94);
    }
}
