//! Benchmark for neutron-protocol decode paths.
//!
//! Generates 1M random byte inputs and measures throughput for each decode
//! path. Reports operations per second and average latency per call.

use bytes::Bytes;
use rand::Rng;
use std::time::Instant;

use neutron_protocol::codec::MinecraftCodec;
use neutron_protocol::login::{
    EncryptionRequest, EncryptionResponse, Handshake, LoginStart, LoginSuccess, SetCompression,
};
use neutron_protocol::packet::read_raw_packet;
use neutron_protocol::play::{
    BlockUpdate, ChatCommand, ChatMessage, ChunkDataAndUpdateLight, ClientStatus, JoinGame,
    KeepAlive, KeepAliveResponse, PlayerPosition, PlayerPositionAndRotation, PlayerRotation,
    ServerData, SetDefaultSpawnPosition, SetPlayerAbilities, SynchronizePlayerPosition,
    SystemChatMessage,
};
use neutron_protocol::types::{
    read_slot, read_string, read_uuid, read_varint, read_varlong, Chat,
};

const ITERS: usize = 1_000_000;

fn bench<F: FnMut(&[u8])>(name: &str, data: &Vec<Vec<u8>>, mut f: F) {
    let start = Instant::now();
    for input in data {
        f(input);
    }
    let elapsed = start.elapsed();
    let ops_per_sec = ITERS as f64 / elapsed.as_secs_f64();
    let us_per_op = elapsed.as_micros() as f64 / ITERS as f64;
    println!(
        "  {:>45}: {:>10.0} ops/s  ({:.2} us/op)",
        name, ops_per_sec, us_per_op
    );
}

fn main() {
    println!("Neutron Protocol Decode Benchmark");
    println!("==================================");
    println!("Running {} iterations per decode path...", ITERS);
    println!();

    // Pre-generate random inputs
    let mut rng = rand::thread_rng();
    let data: Vec<Vec<u8>> = (0..ITERS)
        .map(|_| {
            let size: usize = rng.gen_range(0..=8192);
            (0..size).map(|_| rng.gen()).collect()
        })
        .collect();

    println!("--- Primitive type decodes ---");

    bench("read_varint", &data, |input| {
        let _ = read_varint(&mut Bytes::copy_from_slice(input));
    });

    bench("read_varlong", &data, |input| {
        let _ = read_varlong(&mut Bytes::copy_from_slice(input));
    });

    bench("read_string", &data, |input| {
        let _ = read_string(&mut Bytes::copy_from_slice(input));
    });

    bench("read_uuid", &data, |input| {
        let _ = read_uuid(&mut Bytes::copy_from_slice(input));
    });

    bench("read_slot", &data, |input| {
        let _ = read_slot(&mut Bytes::copy_from_slice(input));
    });

    bench("Chat::read_from", &data, |input| {
        let _ = Chat::read_from(&mut Bytes::copy_from_slice(input));
    });

    println!();
    println!("--- Codec decodes ---");

    bench("MinecraftCodec (no compression)", &data, |input| {
        let codec = MinecraftCodec::new();
        let _ = codec.decode(&mut Bytes::copy_from_slice(input));
    });

    bench("MinecraftCodec (compression=256)", &data, |input| {
        let codec = MinecraftCodec::with_compression(256);
        let _ = codec.decode(&mut Bytes::copy_from_slice(input));
    });

    bench("read_raw_packet", &data, |input| {
        let _ = read_raw_packet(&mut Bytes::copy_from_slice(input));
    });

    println!();
    println!("--- Login packet decodes ---");

    macro_rules! bench_packet {
        ($name:expr, $type:ty) => {
            bench($name, &data, |input| {
                let _ = <$type>::decode(&mut Bytes::copy_from_slice(input));
            });
        };
    }

    bench_packet!("Handshake::decode", Handshake);
    bench_packet!("LoginStart::decode", LoginStart);
    bench_packet!("EncryptionResponse::decode", EncryptionResponse);
    bench_packet!("EncryptionRequest::decode", EncryptionRequest);
    bench_packet!("SetCompression::decode", SetCompression);
    bench_packet!("LoginSuccess::decode", LoginSuccess);

    println!();
    println!("--- Play packet decodes ---");

    bench_packet!("KeepAlive::decode", KeepAlive);
    bench_packet!("JoinGame::decode", JoinGame);
    bench_packet!("ServerData::decode", ServerData);
    bench_packet!("ChatMessage::decode", ChatMessage);
    bench_packet!("SystemChatMessage::decode", SystemChatMessage);
    bench_packet!("SetDefaultSpawnPosition::decode", SetDefaultSpawnPosition);
    bench_packet!("SynchronizePlayerPosition::decode", SynchronizePlayerPosition);
    bench_packet!("ChunkDataAndUpdateLight::decode", ChunkDataAndUpdateLight);
    bench_packet!("BlockUpdate::decode", BlockUpdate);
    bench_packet!("KeepAliveResponse::decode", KeepAliveResponse);
    bench_packet!("PlayerPosition::decode", PlayerPosition);
    bench_packet!("PlayerPositionAndRotation::decode", PlayerPositionAndRotation);
    bench_packet!("PlayerRotation::decode", PlayerRotation);
    bench_packet!("SetPlayerAbilities::decode", SetPlayerAbilities);
    bench_packet!("ChatCommand::decode", ChatCommand);
    bench_packet!("ClientStatus::decode", ClientStatus);

    println!();
    println!("Done.");
}
