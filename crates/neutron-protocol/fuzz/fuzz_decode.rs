//! Fuzz testing harness for neutron-protocol decode paths.
//!
//! Generates 1M+ random byte inputs and attempts to decode them through every
//! available decode path in the crate. All decode attempts are wrapped in
//! `catch_unwind` so that panics in the library are detected and counted
//! rather than crashing the harness.
//!
//! Goal: zero panics across all decode paths with random input.

use bytes::Bytes;
use rand::Rng;
use std::panic;

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
use neutron_protocol::types::{read_slot, read_string, read_uuid, read_varint, read_varlong, Chat};

/// Count panics detected across all fuzz iterations.
struct PanicCounter {
    total_panics: u64,
    by_path: Vec<(&'static str, u64)>,
}

impl PanicCounter {
    fn new() -> Self {
        Self {
            total_panics: 0,
            by_path: Vec::new(),
        }
    }

    fn record(&mut self, path: &'static str) {
        self.total_panics += 1;
        if let Some(entry) = self.by_path.iter_mut().find(|(p, _)| *p == path) {
            entry.1 += 1;
        } else {
            self.by_path.push((path, 1));
        }
    }
}

/// Run a closure, returning true if it panicked.
fn caught_panic<F: FnOnce() + panic::UnwindSafe>(f: F) -> bool {
    panic::catch_unwind(f).is_err()
}

fn main() {
    // Suppress panic output during fuzzing to keep output clean.
    // The default hook prints every panic message to stderr, which would
    // produce hundreds of megabytes of output with 1M random inputs.
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    println!("Neutron Protocol Fuzz Harness");
    println!("=============================");
    println!("Fuzzing 1,000,000 random inputs across all decode paths...");
    println!();

    let mut rng = rand::thread_rng();
    let mut counter = PanicCounter::new();
    let total: u64 = 1_000_000;

    for i in 0..total {
        // Generate random data of varying sizes (0 to 64KB)
        let size: usize = rng.gen_range(0..=65536);
        let data: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
        let bytes = Bytes::from(data.clone());

        // --- Test 1: VarInt decode ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = read_varint(&mut c);
        })) {
            counter.record("read_varint");
        }

        // --- Test 2: VarLong decode ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = read_varlong(&mut c);
        })) {
            counter.record("read_varlong");
        }

        // --- Test 3: String decode ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = read_string(&mut c);
        })) {
            counter.record("read_string");
        }

        // --- Test 4: UUID decode ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = read_uuid(&mut c);
        })) {
            counter.record("read_uuid");
        }

        // --- Test 5: Slot decode ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = read_slot(&mut c);
        })) {
            counter.record("read_slot");
        }

        // --- Test 6: Chat read_from ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = Chat::read_from(&mut c);
        })) {
            counter.record("Chat::read_from");
        }

        // --- Test 7: BlockPos from_packed ---
        if data.len() >= 8 {
            let val = i64::from_be_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]);
            if caught_panic(panic::AssertUnwindSafe(|| {
                let _ = neutron_protocol::types::BlockPos::from_packed(val);
            })) {
                counter.record("BlockPos::from_packed");
            }
        }

        // --- Test 8: read_raw_packet ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let mut c = bytes.clone();
            let _ = read_raw_packet(&mut c);
        })) {
            counter.record("read_raw_packet");
        }

        // --- Test 9: MinecraftCodec decode (no compression) ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let codec = MinecraftCodec::new();
            let mut c = bytes.clone();
            let _ = codec.decode(&mut c);
        })) {
            counter.record("MinecraftCodec::decode(no_compression)");
        }

        // --- Test 10: MinecraftCodec decode (with compression) ---
        if caught_panic(panic::AssertUnwindSafe(|| {
            let codec = MinecraftCodec::with_compression(256);
            let mut c = bytes.clone();
            let _ = codec.decode(&mut c);
        })) {
            counter.record("MinecraftCodec::decode(compression)");
        }

        // --- Test 11+: Individual packet type decodes ---
        // Each packet type's decode expects raw payload (no framing).
        // We feed random bytes as if they were the payload.

        macro_rules! fuzz_packet {
            ($name:expr, $type:ty) => {
                if caught_panic(panic::AssertUnwindSafe(|| {
                    let mut c = bytes.clone();
                    let _ = <$type>::decode(&mut c);
                })) {
                    counter.record($name);
                }
            };
        }

        fuzz_packet!("Handshake::decode", Handshake);
        fuzz_packet!("LoginStart::decode", LoginStart);
        fuzz_packet!("EncryptionResponse::decode", EncryptionResponse);
        fuzz_packet!("EncryptionRequest::decode", EncryptionRequest);
        fuzz_packet!("SetCompression::decode", SetCompression);
        fuzz_packet!("LoginSuccess::decode", LoginSuccess);
        fuzz_packet!("KeepAlive::decode", KeepAlive);
        fuzz_packet!("JoinGame::decode", JoinGame);
        fuzz_packet!("ServerData::decode", ServerData);
        fuzz_packet!("ChatMessage::decode", ChatMessage);
        fuzz_packet!("SystemChatMessage::decode", SystemChatMessage);
        fuzz_packet!("SetDefaultSpawnPosition::decode", SetDefaultSpawnPosition);
        fuzz_packet!(
            "SynchronizePlayerPosition::decode",
            SynchronizePlayerPosition
        );
        fuzz_packet!("ChunkDataAndUpdateLight::decode", ChunkDataAndUpdateLight);
        fuzz_packet!("BlockUpdate::decode", BlockUpdate);
        fuzz_packet!("KeepAliveResponse::decode", KeepAliveResponse);
        fuzz_packet!("PlayerPosition::decode", PlayerPosition);
        fuzz_packet!(
            "PlayerPositionAndRotation::decode",
            PlayerPositionAndRotation
        );
        fuzz_packet!("PlayerRotation::decode", PlayerRotation);
        fuzz_packet!("SetPlayerAbilities::decode", SetPlayerAbilities);
        fuzz_packet!("ChatCommand::decode", ChatCommand);
        fuzz_packet!("ClientStatus::decode", ClientStatus);

        // Progress reporting
        if (i + 1) % 100_000 == 0 {
            println!(
                "  Fuzzed {:>7} / {} inputs... ({} panics so far)",
                i + 1,
                total,
                counter.total_panics
            );
        }
    }

    // Restore default panic hook for the final assert output
    panic::set_hook(default_hook);

    // Print results
    println!();
    println!("Fuzz complete: {} inputs tested", total);
    println!("Total panics detected: {}", counter.total_panics);

    if !counter.by_path.is_empty() {
        println!();
        println!("Panics by decode path:");
        let mut sorted = counter.by_path.clone();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (path, count) in &sorted {
            println!("  {:>50}: {}", path, count);
        }
    }

    assert_eq!(
        counter.total_panics, 0,
        "Fuzzing found {} panics! See breakdown above.",
        counter.total_panics
    );

    println!();
    println!("PASS: Zero panics across all decode paths.");
}
