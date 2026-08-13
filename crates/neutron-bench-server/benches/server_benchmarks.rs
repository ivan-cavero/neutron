//! Benchmarks for Neutron server core components.
//!
//! Measures performance of:
//! - Protocol encoding/decoding (VarInt, packets, codec)
//! - World storage (region files, level.dat, NBT)
//! - World generation (chunk generation, noise, RNG)
//! - Integration (startup time, chunk throughput)

use bytes::{Bytes, BytesMut};
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

// ============================================================
// Protocol benchmarks
// ============================================================

fn bench_decode_varint(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol/decode_varint");

    // Generate 10K VarInts encoded in a buffer
    let values: Vec<i32> = (0..10_000).map(|i| (i * 7) % 5000 - 2500).collect();
    let mut encoded_buf = BytesMut::new();
    for &v in &values {
        neutron_protocol::types::write_varint(&mut encoded_buf, v).unwrap();
    }
    let encoded_bytes = encoded_buf.freeze();

    group.throughput(Throughput::Elements(10_000));
    group.bench_function("decode_10k_varints", |b| {
        b.iter(|| {
            let mut buf = encoded_bytes.clone();
            for _ in 0..10_000 {
                let _ = black_box(neutron_protocol::types::read_varint(&mut buf)).unwrap();
            }
        });
    });
    group.finish();
}

fn bench_encode_varint(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol/encode_varint");

    let values: Vec<i32> = (0..10_000).map(|i| (i * 7) % 5000 - 2500).collect();

    group.throughput(Throughput::Elements(10_000));
    group.bench_function("encode_10k_varints", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(50_000);
            for &v in &values {
                black_box(neutron_protocol::types::write_varint(&mut buf, v)).unwrap();
            }
        });
    });
    group.finish();
}

fn bench_decode_login_success(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol/decode_packet");

    let packet = neutron_protocol::login::LoginSuccess {
        uuid: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        username: "TestPlayer123".to_string(),
        num_properties: 0,
    };
    let mut buf = BytesMut::new();
    packet.encode(&mut buf).unwrap();
    let encoded = buf.freeze();

    group.bench_function("decode_login_success", |b| {
        b.iter(|| {
            let mut data = encoded.clone();
            black_box(neutron_protocol::login::LoginSuccess::decode(&mut data)).unwrap();
        });
    });
    group.finish();
}

fn bench_encode_chunk_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol/encode_packet");

    // Simulate a realistic chunk data payload (~200KB for a full chunk section)
    let chunk_data = vec![0x01u8; 196_608]; // ~192KB
    let light_data = vec![0x02u8; 2048]; // ~2KB

    let packet = neutron_protocol::play::ChunkDataAndUpdateLight {
        chunk_x: 5,
        chunk_z: -3,
        chunk_data: Bytes::from(chunk_data),
        light_data: Bytes::from(light_data),
    };

    group.bench_function("encode_chunk_data_and_update_light", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(200_000);
            black_box(packet.encode(&mut buf)).unwrap();
        });
    });
    group.finish();
}

fn bench_codec_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol/codec_roundtrip");

    let codec_no_compress = neutron_protocol::MinecraftCodec::new();
    let codec_compress = neutron_protocol::MinecraftCodec::with_compression(256);

    // Small packet (below compression threshold)
    let small_payload = vec![0x42u8; 64];
    // Large packet (above compression threshold)
    let large_payload = vec![0x42u8; 4096];

    group.bench_function("roundtrip_uncompressed_small", |b| {
        b.iter(|| {
            let mut wire = BytesMut::new();
            codec_no_compress
                .encode(0x26, &small_payload, &mut wire)
                .unwrap();
            let mut data = wire.freeze();
            black_box(codec_no_compress.decode(&mut data)).unwrap();
        });
    });

    group.bench_function("roundtrip_compressed_large", |b| {
        b.iter(|| {
            let mut wire = BytesMut::new();
            codec_compress
                .encode(0x27, &large_payload, &mut wire)
                .unwrap();
            let mut data = wire.freeze();
            black_box(codec_compress.decode(&mut data)).unwrap();
        });
    });

    group.finish();
}

// ============================================================
// World storage benchmarks
// ============================================================

fn bench_region_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("world_storage/region");

    // Create a region with some chunks for realistic benchmarks
    let mut region = neutron_world::Region::new(0, 0);
    let chunk_data = vec![0xABu8; 8192]; // 8KB chunk payload
    for cx in 0..32 {
        for cz in 0..32 {
            region.write_chunk(cx, cz, &chunk_data).unwrap();
        }
    }

    group.throughput(Throughput::Elements(1)); // Single file operation

    group.bench_function("region_write_file", |b| {
        let dir = tempfile::tempdir().unwrap();
        let path = neutron_world::region_path(dir.path(), 0, 0);
        b.iter(|| {
            region.save(black_box(&path)).unwrap();
        });
    });

    group.bench_function("region_read_file", |b| {
        let dir = tempfile::tempdir().unwrap();
        let path = neutron_world::region_path(dir.path(), 0, 0);
        region.save(&path).unwrap();
        b.iter(|| {
            black_box(neutron_world::Region::open(&path)).unwrap();
        });
    });

    group.bench_function("region_serialize_to_bytes", |b| {
        b.iter(|| {
            black_box(region.to_bytes()).unwrap();
        });
    });

    group.bench_function("region_parse_from_bytes", |b| {
        let bytes = region.to_bytes().unwrap();
        b.iter(|| {
            black_box(neutron_world::Region::from_bytes(&bytes)).unwrap();
        });
    });

    group.finish();
}

fn bench_level_dat(c: &mut Criterion) {
    let mut group = c.benchmark_group("world_storage/level_dat");

    let ld = neutron_world::LevelDat::new(12345, "bench_world");
    let nbt_data = ld.to_nbt();
    let compressed = neutron_world::nbt::write_gzip_nbt(&nbt_data).unwrap();

    group.bench_function("level_dat_serialize", |b| {
        b.iter(|| {
            black_box(ld.to_nbt());
        });
    });

    group.bench_function("level_dat_compress", |b| {
        b.iter(|| {
            black_box(neutron_world::nbt::write_gzip_nbt(&nbt_data)).unwrap();
        });
    });

    group.bench_function("level_dat_decompress_and_parse", |b| {
        b.iter(|| {
            black_box(neutron_world::LevelDat::from_bytes(&compressed)).unwrap();
        });
    });

    group.finish();
}

fn bench_nbt_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("world_storage/nbt");

    // Create a moderately complex NBT structure
    let mut compound = neutron_world::nbt::new_compound();
    for i in 0..50 {
        let key = format!("key_{}", i);
        neutron_world::nbt::compound_insert(&mut compound, &key, neutron_world::nbt::tag_int(i));
        let nested_key = format!("nested_{}", i);
        let mut inner = neutron_world::nbt::new_compound();
        neutron_world::nbt::compound_insert(
            &mut inner,
            "value",
            neutron_world::nbt::tag_long((i as i64) * 100),
        );
        neutron_world::nbt::compound_insert(
            &mut inner,
            "name",
            neutron_world::nbt::tag_string(&format!("item_{}", i)),
        );
        neutron_world::nbt::compound_insert(
            &mut compound,
            &nested_key,
            neutron_world::nbt::tag_compound(inner),
        );
    }
    let nbt = neutron_world::nbt::root_nbt(compound);
    let raw_bytes = neutron_world::nbt::write_nbt(&nbt);

    group.throughput(Throughput::Elements(1));
    group.bench_function("parse_compound_50_keys", |b| {
        b.iter(|| {
            black_box(neutron_world::nbt::read_nbt(&raw_bytes)).unwrap();
        });
    });

    group.finish();
}

// ============================================================
// Worldgen benchmarks
// ============================================================

fn bench_generate_chunk(c: &mut Criterion) {
    let mut group = c.benchmark_group("worldgen/generate_chunk");

    let generator = neutron_worldgen::ChunkGenerator::new(42);

    group.throughput(Throughput::Elements(1));
    group.bench_function("generate_single_chunk", |b| {
        b.iter(|| {
            black_box(generator.generate_chunk(black_box(0), black_box(0)));
        });
    });

    group.finish();
}

fn bench_generate_16x16(c: &mut Criterion) {
    let mut group = c.benchmark_group("worldgen/generate_16x16");

    let generator = neutron_worldgen::ChunkGenerator::new(42);

    group.throughput(Throughput::Elements(256));
    group.bench_function("generate_256_chunks", |b| {
        b.iter(|| {
            for cx in 0..16i32 {
                for cz in 0..16i32 {
                    black_box(generator.generate_chunk(cx, cz));
                }
            }
        });
    });

    group.finish();
}

fn bench_noise_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("worldgen/noise_eval");

    let mut rng = neutron_worldgen::Xoroshiro128::new(42);
    let noise = neutron_worldgen::OctavePerlinNoise::new(&mut rng, 6, 1.0 / 1500.0, 1.0);

    group.throughput(Throughput::Elements(10_000));
    group.bench_function("sample_10k_points", |b| {
        b.iter(|| {
            for i in 0..10_000u32 {
                let x = (i as f64) * 0.1;
                let z = (i as f64) * 0.13;
                black_box(noise.sample(x, 0.0, z, true));
            }
        });
    });

    group.finish();
}

fn bench_xoroshiro128_next(c: &mut Criterion) {
    let mut group = c.benchmark_group("worldgen/xoroshiro128_next");

    group.throughput(Throughput::Elements(10_000_000));
    group.bench_function("generate_10m_random_numbers", |b| {
        b.iter(|| {
            let mut rng = neutron_worldgen::Xoroshiro128::new(42);
            for _ in 0..10_000_000 {
                black_box(rng.next_i64());
            }
        });
    });

    group.bench_function("next_i32_throughput", |b| {
        b.iter(|| {
            let mut rng = neutron_worldgen::Xoroshiro128::new(99);
            for _ in 0..1_000_000 {
                black_box(rng.next_i32());
            }
        });
    });

    group.finish();
}

// ============================================================
// Integration benchmarks
// ============================================================

fn bench_startup_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration/startup_time");

    group.bench_function("create_chunk_generator", |b| {
        b.iter(|| {
            black_box(neutron_worldgen::ChunkGenerator::new(black_box(42)));
        });
    });

    group.bench_function("create_and_generate_first_chunk", |b| {
        b.iter(|| {
            let gen = neutron_worldgen::ChunkGenerator::new(42);
            black_box(gen.generate_chunk(0, 0));
        });
    });

    group.finish();
}

fn bench_chunk_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration/chunk_throughput");

    let generator = neutron_worldgen::ChunkGenerator::new(42);
    let _codec = neutron_protocol::MinecraftCodec::with_compression(256);

    // Generate a chunk and encode it as a ChunkDataAndUpdateLight packet
    group.throughput(Throughput::Elements(1));
    group.bench_function("generate_encode_send_1_chunk", |b| {
        b.iter(|| {
            let chunk = generator.generate_chunk(0, 0);

            // Encode blocks as a simple payload (in real code this would be the full chunk encoding)
            let chunk_payload: Vec<u8> =
                chunk.blocks.iter().flat_map(|b| b.to_le_bytes()).collect();
            let light_payload = vec![0xFFu8; 2048]; // All light set

            let packet = neutron_protocol::play::ChunkDataAndUpdateLight {
                chunk_x: 0,
                chunk_z: 0,
                chunk_data: Bytes::from(chunk_payload),
                light_data: Bytes::from(light_payload),
            };

            let mut wire = BytesMut::new();
            black_box(packet.encode(&mut wire)).unwrap();

            // Decode it back
            let mut data = wire.freeze();
            black_box(neutron_protocol::play::ChunkDataAndUpdateLight::decode(
                &mut data,
            ))
            .unwrap();
        });
    });

    // Throughput test: generate + encode N chunks
    group.throughput(Throughput::Elements(16));
    group.bench_function("generate_encode_16_chunks", |b| {
        b.iter(|| {
            for i in 0..16i32 {
                let chunk = generator.generate_chunk(i, 0);
                let chunk_payload: Vec<u8> =
                    chunk.blocks.iter().flat_map(|b| b.to_le_bytes()).collect();
                let light_payload = vec![0xFFu8; 2048];

                let packet = neutron_protocol::play::ChunkDataAndUpdateLight {
                    chunk_x: i,
                    chunk_z: 0,
                    chunk_data: Bytes::from(chunk_payload),
                    light_data: Bytes::from(light_payload),
                };

                let mut wire = BytesMut::new();
                black_box(packet.encode(&mut wire)).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================
// Benchmark groups
// ============================================================

criterion_group!(
    protocol_benches,
    bench_decode_varint,
    bench_encode_varint,
    bench_decode_login_success,
    bench_encode_chunk_data,
    bench_codec_roundtrip,
);

criterion_group!(
    world_storage_benches,
    bench_region_read_write,
    bench_level_dat,
    bench_nbt_parse,
);

criterion_group!(
    worldgen_benches,
    bench_generate_chunk,
    bench_generate_16x16,
    bench_noise_eval,
    bench_xoroshiro128_next,
);

criterion_group!(
    integration_benches,
    bench_startup_time,
    bench_chunk_throughput,
);

criterion_main!(
    protocol_benches,
    world_storage_benches,
    worldgen_benches,
    integration_benches,
);
