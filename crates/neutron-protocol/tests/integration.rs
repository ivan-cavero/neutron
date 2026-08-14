//! Integration tests verifying that neutron-protocol and neutron-world work
//! together correctly as building blocks for the Neutron server.

use bytes::{Buf, Bytes, BytesMut};
use neutron_protocol::codec::MinecraftCodec;
use neutron_protocol::login::{
    EncryptionRequest, EncryptionResponse, Handshake, LoginStart, LoginSuccess, SetCompression,
};
use neutron_protocol::packet::PacketId;
use neutron_protocol::play::{
    BlockUpdate, ChatCommand, ChatMessage, ChunkDataAndUpdateLight, ClientStatus, JoinGame,
    KeepAlive, KeepAliveResponse, PlayerPosition, PlayerPositionAndRotation, PlayerRotation,
    ServerData, SetDefaultSpawnPosition, SetPlayerAbilities, SynchronizePlayerPosition,
    SystemChatMessage,
};
use neutron_protocol::types::{Angle, BlockPos, Chat, GameMode, Slot, SlotData, Vec3d, Vec3f};
use neutron_protocol::{Direction, ProtocolState};
use neutron_world::{Dimension, Region, World};

// =========================================================================
// Test 1: World persistence — create, write chunk, save, reopen, verify
// =========================================================================

#[test]
fn test_world_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let world_path = dir.path().join("test_world");

    // Create a new world with a specific seed.
    let mut world = World::create(&world_path, 12345).unwrap();
    assert_eq!(world.name(), "test_world");
    assert_eq!(world.level().seed, 12345);

    // Write chunk data into region (0, 0) at local coords (10, 15).
    let chunk_data = b"Hello from integration test chunk payload!";
    {
        let region = world.get_region(Dimension::Overworld, 0, 0).unwrap();
        region.write_chunk(10, 15, chunk_data).unwrap();
    }

    // Save the world to disk.
    world.save().unwrap();
    assert!(world_path.join("level.dat").exists());
    assert!(world_path.join("region").exists());

    // Drop the world (releases session.lock).
    drop(world);

    // Re-open the world.
    let mut world2 = World::open(&world_path).unwrap();
    assert_eq!(world2.level().seed, 12345);

    // Verify the chunk data survived the round-trip.
    let region = world2.get_region(Dimension::Overworld, 0, 0).unwrap();
    let persisted = region.get_chunk(10, 15).unwrap();
    assert_eq!(persisted.as_deref(), Some(chunk_data.as_slice()));

    // Verify an empty slot returns None.
    let empty = region.get_chunk(0, 0).unwrap();
    assert!(empty.is_none());
}

// =========================================================================
// Test 2: Login flow simulation using protocol types
// =========================================================================

#[test]
fn test_login_flow_simulation() {
    // --- Step 1: Client sends Handshake ---
    let handshake = Handshake {
        protocol_version: 858, // Minecraft 26.2
        server_address: "localhost".to_string(),
        server_port: 25565,
        next_state: 2, // Login
    };

    // Encode the handshake.
    let mut buf = BytesMut::new();
    handshake.encode(&mut buf).unwrap();

    // Decode it back.
    let decoded = Handshake::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
    assert_eq!(decoded.protocol_version, 858);
    assert_eq!(decoded.server_address, "localhost");
    assert_eq!(decoded.server_port, 25565);
    assert_eq!(decoded.next_state, 2);

    // Verify PacketId metadata.
    assert_eq!(Handshake::STATE, ProtocolState::Handshake);
    assert_eq!(Handshake::DIRECTION, Direction::Serverbound);
    assert_eq!(Handshake::ID, 0x00);

    // --- Step 2: Client sends LoginStart ---
    let player_uuid = uuid::Uuid::parse_str("d4735e3a-6a5c-4f8c-a5a0-33a27765f29d").unwrap();
    let login_start = LoginStart {
        name: "TestPlayer".to_string(),
        uuid: Some(player_uuid),
    };

    let mut buf = BytesMut::new();
    login_start.encode(&mut buf).unwrap();
    let decoded = LoginStart::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
    assert_eq!(decoded.name, "TestPlayer");
    assert_eq!(decoded.uuid, Some(player_uuid));

    assert_eq!(LoginStart::STATE, ProtocolState::Login);
    assert_eq!(LoginStart::DIRECTION, Direction::Serverbound);

    // --- Step 3: Server would send LoginSuccess (offline mode) ---
    let login_success = LoginSuccess {
        uuid: player_uuid,
        username: "TestPlayer".to_string(),
        num_properties: 0,
    };

    let mut buf = BytesMut::new();
    login_success.encode(&mut buf).unwrap();
    let decoded = LoginSuccess::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
    assert_eq!(decoded.uuid, player_uuid);
    assert_eq!(decoded.username, "TestPlayer");
    assert_eq!(decoded.num_properties, 0);

    assert_eq!(LoginSuccess::STATE, ProtocolState::Login);
    assert_eq!(LoginSuccess::DIRECTION, Direction::Clientbound);
    assert_eq!(LoginSuccess::ID, 0x02);

    // --- Verify full login flow through the codec (framed) ---
    let codec = MinecraftCodec::new();
    let mut wire = BytesMut::new();

    // Client -> Server: Handshake
    let mut payload = BytesMut::new();
    handshake.encode(&mut payload).unwrap();
    codec.encode(Handshake::ID, &payload, &mut wire).unwrap();

    // Client -> Server: LoginStart
    payload.clear();
    login_start.encode(&mut payload).unwrap();
    codec.encode(LoginStart::ID, &payload, &mut wire).unwrap();

    // Server -> Client: LoginSuccess
    payload.clear();
    login_success.encode(&mut payload).unwrap();
    codec.encode(LoginSuccess::ID, &payload, &mut wire).unwrap();

    // Decode all three from the wire.
    let mut read_buf: Bytes = wire.freeze();

    let pkt = codec.decode(&mut read_buf).unwrap().unwrap();
    assert_eq!(pkt.id, Handshake::ID);
    let mut p = pkt.payload;
    let hs = Handshake::decode(&mut p).unwrap();
    assert_eq!(hs.protocol_version, 858);

    let pkt = codec.decode(&mut read_buf).unwrap().unwrap();
    assert_eq!(pkt.id, LoginStart::ID);
    let mut p = pkt.payload;
    let ls = LoginStart::decode(&mut p).unwrap();
    assert_eq!(ls.name, "TestPlayer");

    let pkt = codec.decode(&mut read_buf).unwrap().unwrap();
    assert_eq!(pkt.id, LoginSuccess::ID);
    let mut p = pkt.payload;
    let sg = LoginSuccess::decode(&mut p).unwrap();
    assert_eq!(sg.username, "TestPlayer");

    assert!(!read_buf.has_remaining());
}

// =========================================================================
// Test 3: Packet roundtrip for all packet types
// =========================================================================

#[test]
fn test_packet_roundtrip_all_types() {
    // Helper macro: encode, decode, assert_eq.
    // We test every packet type that has symmetric encode/decode.
    macro_rules! roundtrip {
        ($original:expr, $decode_fn:path) => {{
            let packet = $original;
            let mut buf = BytesMut::new();
            packet.encode(&mut buf).unwrap();
            let decoded = $decode_fn(&mut Bytes::copy_from_slice(&buf)).unwrap();
            assert_eq!(packet, decoded);
        }};
    }

    // --- Login packets ---

    roundtrip!(
        Handshake {
            protocol_version: 858,
            server_address: "mc.example.com".to_string(),
            server_port: 25565,
            next_state: 2,
        },
        Handshake::decode
    );

    roundtrip!(
        LoginStart {
            name: "Alice".to_string(),
            uuid: Some(uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()),
        },
        LoginStart::decode
    );

    roundtrip!(
        LoginStart {
            name: "Bob".to_string(),
            uuid: None,
        },
        LoginStart::decode
    );

    roundtrip!(SetCompression { threshold: 512 }, SetCompression::decode);

    roundtrip!(
        LoginSuccess {
            uuid: uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap(),
            username: "Player1".to_string(),
            num_properties: 3,
        },
        LoginSuccess::decode
    );

    // --- Play packets (Clientbound) ---

    roundtrip!(KeepAlive { id: 0xCAFEBABE }, KeepAlive::decode);

    roundtrip!(
        JoinGame {
            entity_id: 42,
            is_hardcore: true,
            dimension_count: 3,
            max_players: 100,
            view_distance: 12,
            simulation_distance: 10,
            reduced_debug_info: false,
            enable_respawn_screen: true,
            is_lan: false,
            game_mode: GameMode::Creative,
            prev_game_mode: 1,
            dimension_type: "minecraft:overworld".to_string(),
            dimension_name: "minecraft:overworld".to_string(),
            hashed_seed: 99999,
            is_flat: false,
            has_death_location: true,
        },
        JoinGame::decode
    );

    roundtrip!(
        BlockUpdate {
            location: BlockPos::new(100, 64, -200),
            block_state_id: 33,
        },
        BlockUpdate::decode
    );

    roundtrip!(
        SetDefaultSpawnPosition {
            dimension: "minecraft:overworld".into(),
            location: BlockPos::new(0, 64, 0),
            yaw: 90.0,
            pitch: 0.0,
        },
        SetDefaultSpawnPosition::decode
    );

    roundtrip!(
        SynchronizePlayerPosition {
            teleport_id: 7,
            x: 100.5,
            y: 65.0,
            z: -200.3,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            yaw: 45.0,
            pitch: -30.0,
            relatives: 0,
        },
        SynchronizePlayerPosition::decode
    );

    roundtrip!(
        SystemChatMessage {
            content: Chat::Json(r#"{"text":"Welcome!"}"#.to_string()),
            overlay: false,
        },
        SystemChatMessage::decode
    );

    // --- Play packets (Serverbound) ---

    roundtrip!(KeepAliveResponse { id: 42 }, KeepAliveResponse::decode);

    roundtrip!(
        PlayerPosition {
            x: 10.5,
            y: 64.0,
            z: -10.5,
            on_ground: true,
        },
        PlayerPosition::decode
    );

    roundtrip!(
        PlayerPositionAndRotation {
            x: 10.5,
            y: 64.0,
            z: -10.5,
            yaw: 90.0,
            pitch: 0.0,
            on_ground: true,
        },
        PlayerPositionAndRotation::decode
    );

    roundtrip!(
        PlayerRotation {
            yaw: 180.0,
            pitch: -45.0,
            on_ground: false,
        },
        PlayerRotation::decode
    );

    roundtrip!(
        SetPlayerAbilities {
            flags: 0x06, // invulnerable + flying
            flying_speed: 0.05,
            fov_modifier: 0.1,
        },
        SetPlayerAbilities::decode
    );

    roundtrip!(
        ChatCommand {
            command: "tp @s 0 64 0".to_string(),
        },
        ChatCommand::decode
    );

    roundtrip!(ClientStatus { action_id: 0 }, ClientStatus::decode);
    roundtrip!(ClientStatus { action_id: 1 }, ClientStatus::decode);

    // --- Play packets with asymmetric encode/decode (test encode then decode) ---

    // ChatMessage: encode uses write_to (VarInt length + JSON bytes), decode uses read_from
    // These produce different internal representations so we test encode/decode
    // produces a valid decode, not necessarily PartialEq equal.
    {
        let msg = ChatMessage {
            message: Chat::Json(r#"{"text":"Hello!"}"#.to_string()),
            position: 0,
            sender: uuid::Uuid::nil(),
        };
        let mut buf = BytesMut::new();
        msg.encode(&mut buf).unwrap();
        let decoded = ChatMessage::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.position, 0);
        assert_eq!(decoded.sender, uuid::Uuid::nil());
        assert!(decoded.message.to_json_string().contains("Hello!"));
    }

    // ServerData: encode writes icon/enforce first, then description;
    // decode reads icon/enforce first, then description — same order.
    {
        let data = ServerData {
            description: Chat::Json(r#"{"text":"A Neutron server"}"#.to_string()),
            has_icon: false,
            icon: None,
            enforces_secure_chat: false,
        };
        let mut buf = BytesMut::new();
        data.encode(&mut buf).unwrap();
        let decoded = ServerData::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert!(!decoded.has_icon);
        assert!(!decoded.enforces_secure_chat);
        assert!(decoded
            .description
            .to_json_string()
            .contains("A Neutron server"));
    }

    // ChunkDataAndUpdateLight: uses raw Bytes for chunk/light data.
    {
        let chunk = ChunkDataAndUpdateLight {
            chunk_x: 5,
            chunk_z: -3,
            chunk_data: Bytes::from_static(&[0xAA, 0xBB, 0xCC]),
            light_data: Bytes::from_static(&[0x11, 0x22]),
        };
        let mut buf = BytesMut::new();
        chunk.encode(&mut buf).unwrap();
        let decoded = ChunkDataAndUpdateLight::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.chunk_x, 5);
        assert_eq!(decoded.chunk_z, -3);
        assert_eq!(&decoded.chunk_data[..], &[0xAA, 0xBB, 0xCC]);
        assert_eq!(&decoded.light_data[..], &[0x11, 0x22]);
    }

    // EncryptionRequest roundtrip
    roundtrip!(
        EncryptionRequest {
            server_id: String::new(),
            public_key_length: 128,
            public_key: Bytes::from(vec![0xAA; 128]),
            verify_token_length: 16,
            verify_token: Bytes::from(vec![0xBB; 16]),
        },
        EncryptionRequest::decode
    );

    // EncryptionResponse roundtrip
    roundtrip!(
        EncryptionResponse {
            shared_secret_length: 256,
            shared_secret: Bytes::from(vec![0xCC; 256]),
            verify_token_length: 128,
            verify_token: Bytes::from(vec![0xDD; 128]),
        },
        EncryptionResponse::decode
    );
}

// =========================================================================
// Test 4: World directory structure — vanilla-compatible layout
// =========================================================================

#[test]
fn test_world_directory_structure() {
    let dir = tempfile::tempdir().unwrap();
    let world_path = dir.path().join("my_world");

    // Create the world.
    let _world = World::create(&world_path, 42).unwrap();
    let parent = dir.path();

    // Verify overworld structure: <name>/level.dat, <name>/session.lock, <name>/region/
    assert!(world_path.join("level.dat").exists(), "level.dat missing");
    assert!(
        world_path.join("session.lock").exists(),
        "session.lock missing"
    );
    assert!(
        world_path.join("region").is_dir(),
        "overworld region/ directory missing"
    );

    // Verify nether structure: <name>_nether/DIM-1/region/
    let nether_dir = parent.join("my_world_nether");
    assert!(nether_dir.is_dir(), "nether directory missing");
    assert!(
        nether_dir.join("region").is_dir(),
        "nether region/ directory missing"
    );

    // Verify the end structure: <name>_the_end/DIM1/region/
    let the_end_dir = parent.join("my_world_the_end");
    assert!(the_end_dir.is_dir(), "the_end directory missing");
    assert!(
        the_end_dir.join("region").is_dir(),
        "the_end region/ directory missing"
    );

    // Verify no extra directories exist.
    let entries: Vec<String> = std::fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        entries.len(),
        3,
        "expected exactly 3 dirs: my_world, my_world_nether, my_world_the_end"
    );

    // Verify level.dat is readable.
    let level = neutron_world::LevelDat::read(&world_path.join("level.dat")).unwrap();
    assert_eq!(level.seed, 42);

    // Verify dimension directory helpers agree with the filesystem.
    let world = World::open(&world_path).unwrap();
    assert_eq!(world.dimension_dir(Dimension::Overworld), world_path);
    assert_eq!(world.dimension_dir(Dimension::Nether), nether_dir);
    assert_eq!(world.dimension_dir(Dimension::TheEnd), the_end_dir);

    // Verify region directories are empty initially.
    let regions = world.list_regions(Dimension::Overworld).unwrap();
    assert!(regions.is_empty());
}

// =========================================================================
// Test 5: Codec stress test — 10,000 random-ish packet frames
// =========================================================================

#[test]
fn test_codec_stress() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Test both compressed and uncompressed codecs.
    for compression in [None, Some(128)] {
        let codec = match compression {
            None => MinecraftCodec::new(),
            Some(threshold) => MinecraftCodec::with_compression(threshold),
        };

        let mut wire = BytesMut::new();
        let mut expected: Vec<(u32, Vec<u8>)> = Vec::new();

        // Generate 10,000 packets with deterministic pseudo-random payloads.
        for i in 0u32..10_000 {
            // Deterministic packet ID from a small set.
            let packet_id = match i % 7 {
                0 => 0x00u32, // Handshake-like
                1 => 0x01u32, // LoginStart-like
                2 => 0x02u32, // LoginSuccess-like
                3 => 0x09u32, // BlockUpdate-like
                4 => 0x17u32, // PlayerPosition-like
                5 => 0x26u32, // KeepAlive-like
                _ => 0x67u32, // SystemChatMessage-like
            };

            // Deterministic payload size (0 to 512 bytes).
            let mut h = DefaultHasher::new();
            i.hash(&mut h);
            let size = (h.finish() % 513) as usize;

            // Deterministic payload content.
            let payload: Vec<u8> = (0..size).map(|j| ((i + j as u32) & 0xFF) as u8).collect();

            codec.encode(packet_id, &payload, &mut wire).unwrap();
            expected.push((packet_id, payload));
        }

        // Verify total encoded size is reasonable (10k packets should fit in < 10 MB).
        assert!(
            wire.len() < 10 * 1024 * 1024,
            "encoded wire too large: {} bytes",
            wire.len()
        );

        // Decode all packets back.
        let mut read_buf: Bytes = wire.freeze();
        for (idx, (expected_id, expected_payload)) in expected.iter().enumerate() {
            let result = codec.decode(&mut read_buf);
            match result {
                Ok(Some(packet)) => {
                    assert_eq!(
                        packet.id, *expected_id,
                        "packet {} id mismatch: expected 0x{:02X}, got 0x{:02X}",
                        idx, expected_id, packet.id
                    );
                    assert_eq!(
                        &packet.payload[..],
                        expected_payload.as_slice(),
                        "packet {} payload mismatch",
                        idx
                    );
                }
                Ok(None) => {
                    panic!(
                        "packet {}: got None (incomplete frame) with {} bytes remaining",
                        idx,
                        read_buf.remaining()
                    );
                }
                Err(e) => {
                    panic!("packet {}: decode error: {}", idx, e);
                }
            }
        }

        // Buffer should be fully consumed.
        assert!(
            !read_buf.has_remaining(),
            "buffer not fully consumed: {} bytes remaining",
            read_buf.remaining()
        );
    }
}

// =========================================================================
// Bonus: VarInt/varlong types roundtrip through codec frames
// =========================================================================

#[test]
fn test_types_roundtrip_through_codec() {
    // BlockPos packed encoding roundtrip.
    let pos = BlockPos::new(100, 64, -200);
    let packed = pos.to_packed();
    let decoded = BlockPos::from_packed(packed).unwrap();
    assert_eq!(pos, decoded);

    // Angle roundtrip.
    let angle = Angle::from_degrees(90.0);
    assert!((angle.to_degrees() - 90.0).abs() < 2.0);

    // GameMode mapping.
    assert_eq!(GameMode::from_id(0), Some(GameMode::Survival));
    assert_eq!(GameMode::from_id(1), Some(GameMode::Creative));
    assert_eq!(GameMode::from_id(2), Some(GameMode::Adventure));
    assert_eq!(GameMode::from_id(3), Some(GameMode::Spectator));
    assert_eq!(GameMode::from_id(4), None);

    // Chat JSON roundtrip.
    let chat = Chat::Plain("Test message".to_string());
    let json = chat.to_json_string();
    assert!(json.contains("Test message"));
    assert!(json.starts_with('{'));

    // Slot roundtrip through codec.
    let slot = Some(SlotData {
        item_id: 1,
        item_count: 64,
        nbt: None,
    });
    let mut buf = BytesMut::new();
    neutron_protocol::types::write_slot(&mut buf, &slot).unwrap();
    let decoded_slot =
        neutron_protocol::types::read_slot(&mut Bytes::copy_from_slice(&buf)).unwrap();
    assert_eq!(decoded_slot, slot);

    // Empty slot.
    let empty: Slot = None;
    let mut buf = BytesMut::new();
    neutron_protocol::types::write_slot(&mut buf, &empty).unwrap();
    let decoded_empty =
        neutron_protocol::types::read_slot(&mut Bytes::copy_from_slice(&buf)).unwrap();
    assert_eq!(decoded_empty, None);

    // Vec3d / Vec3f basic construction.
    let v3d = Vec3d::new(1.0, 2.0, 3.0);
    assert_eq!(v3d.x, 1.0);
    let v3f = Vec3f::new(4.0, 5.0, 6.0);
    assert_eq!(v3f.z, 6.0);
}

// =========================================================================
// Bonus: World region file vanilla format verification
// =========================================================================

#[test]
fn test_region_vanilla_format_structure() {
    let dir = tempfile::tempdir().unwrap();
    let region_dir = dir.path().join("region");
    std::fs::create_dir_all(&region_dir).unwrap();

    // Write a region with a single chunk.
    let mut region = Region::new(0, 0);
    let chunk_data = b"vanilla format test data";
    region.write_chunk(5, 10, chunk_data).unwrap();

    let region_path = region_dir.join("r.0.0.mca");
    region.save(&region_path).unwrap();

    // Read the raw bytes and verify the Anvil header structure.
    let bytes = std::fs::read(&region_path).unwrap();

    // Minimum file size: 2 header tables (offset + timestamp) = 8192 bytes.
    assert!(
        bytes.len() >= 8192,
        "region file too small: {} bytes",
        bytes.len()
    );

    // Verify offset table entry for chunk (5, 10):
    // index = (5 & 31) + (10 & 31) * 32 = 5 + 320 = 325
    let idx = 325;
    let base = idx * 4;
    let sector_offset = u32::from_be_bytes([0, bytes[base], bytes[base + 1], bytes[base + 2]]);
    let sector_count = bytes[base + 3];
    assert!(sector_offset > 0, "chunk offset should be non-zero");
    assert!(sector_count > 0, "chunk sector count should be non-zero");

    // Verify chunk at (0, 0) is empty.
    let base0 = 0 * 4;
    let so0 = u32::from_be_bytes([0, bytes[base0], bytes[base0 + 1], bytes[base0 + 2]]);
    assert_eq!(so0, 0, "empty chunk should have zero offset");

    // Verify the region file can be parsed back.
    let loaded = Region::open(&region_path).unwrap();
    let data = loaded.get_chunk(5, 10).unwrap();
    assert_eq!(data.as_deref(), Some(chunk_data.as_slice()));

    // Verify region coordinates are retrievable from filename.
    let parsed = neutron_world::parse_region_filename("r.0.0.mca");
    assert_eq!(parsed, Some((0, 0)));

    // Verify negative coordinates work.
    let parsed_neg = neutron_world::parse_region_filename("r.-3.12.mca");
    assert_eq!(parsed_neg, Some((-3, 12)));
}
