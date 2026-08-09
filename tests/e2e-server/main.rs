//! E2E test for the Neutron server.
//!
//! Starts the server, connects a raw TCP client, performs the login handshake,
//! receives chunks, simulates movement, and reports timing metrics.
//!
//! Usage:
//!   cargo run --manifest-path tests/e2e-server/Cargo.toml [-- <server-path>]

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

// ============================================================================
// VarInt helpers (inline, no external dep)
// ============================================================================

fn write_varint(buf: &mut BytesMut, value: i32) {
    let mut val = value as u32;
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if val == 0 {
            break;
        }
    }
}

fn read_varint(buf: &mut Bytes) -> Result<i32, String> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    loop {
        if !buf.has_remaining() {
            return Err("insufficient bytes for VarInt".into());
        }
        let byte = buf.get_u8();
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return Err("VarInt too long".into());
        }
    }
    Ok(result)
}

fn varint_size(value: i32) -> usize {
    let mut val = value as u32;
    let mut size = 0;
    loop {
        size += 1;
        val >>= 7;
        if val == 0 {
            break;
        }
    }
    size
}

fn write_string(buf: &mut BytesMut, s: &str) {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as i32);
    buf.put_slice(bytes);
}

#[allow(dead_code)]
fn read_string(buf: &mut Bytes) -> Result<String, String> {
    let len = read_varint(buf)? as usize;
    if len > 32767 {
        return Err(format!("string too long: {}", len));
    }
    if buf.remaining() < len {
        return Err(format!("insufficient bytes for string: need {}, have {}", len, buf.remaining()));
    }
    let mut out = vec![0u8; len];
    buf.copy_to_slice(&mut out);
    String::from_utf8(out).map_err(|e| e.to_string())
}

// ============================================================================
// Raw packet framing (no compression — server uses compression_threshold=-1)
// ============================================================================

/// A decoded raw packet.
#[derive(Debug)]
struct RawPacket {
    id: u32,
    payload: Bytes,
}

/// Encode a packet with length-delimited framing (uncompressed).
fn encode_packet(packet_id: u32, payload: &[u8]) -> BytesMut {
    let id_size = varint_size(packet_id as i32);
    let total_size = id_size + payload.len();
    let mut buf = BytesMut::with_capacity(total_size + 8);
    write_varint(&mut buf, total_size as i32);
    write_varint(&mut buf, packet_id as i32);
    buf.put_slice(payload);
    buf
}

/// Try to decode one raw packet from the buffer.
/// Returns Ok(Some(packet)) on success, Ok(None) if incomplete.
fn decode_packet(buf: &mut Bytes) -> Result<Option<RawPacket>, String> {
    if !buf.has_remaining() {
        return Ok(None);
    }

    let mut peek = buf.clone();
    let length = match read_varint(&mut peek) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    if length < 0 {
        return Err("negative packet length".into());
    }

    let length = length as usize;
    let length_varint_size = varint_size(length as i32);

    if buf.remaining() < length_varint_size + length {
        return Ok(None);
    }

    // Consume the length VarInt
    let _ = read_varint(buf)?;

    // Read packet ID
    let packet_id = read_varint(buf)? as u32;

    // Read payload
    let overhead = varint_size(packet_id as i32);
    if length < overhead {
        return Err("packet too short for header".into());
    }
    let payload_len = length - overhead;
    if buf.remaining() < payload_len {
        return Err(format!("payload truncated: need {}, have {}", payload_len, buf.remaining()));
    }
    let payload = buf.copy_to_bytes(payload_len);

    Ok(Some(RawPacket {
        id: packet_id,
        payload,
    }))
}

// ============================================================================
// Test metrics
// ============================================================================

#[derive(Debug, Default)]
struct TestMetrics {
    server_started: bool,
    startup_duration: Option<Duration>,
    login_success: bool,
    join_game_received: bool,
    chunks_received: usize,
    first_chunk_time: Option<Duration>,
    first_chunk_size: usize,
    registry_data_count: usize,
    sync_position_received: bool,
    keepalive_responded: usize,
    position_packets_sent: usize,
    errors: Vec<String>,
    total_duration: Option<Duration>,
}

// ============================================================================
// Server process management
// ============================================================================

fn find_server_binary() -> String {
    // Try common build output paths
    let candidates = if cfg!(target_os = "windows") {
        vec![
            "target/debug/neutron-server.exe",
            "target/release/neutron-server.exe",
        ]
    } else {
        vec![
            "target/debug/neutron-server",
            "target/release/neutron-server",
        ]
    };

    // Check from workspace root (tests/e2e-server is relative to it)
    let workspace_root = std::env::current_dir()
        .ok()
        .and_then(|p| {
            // If we're in tests/e2e-server, go up two levels
            let p_str = p.to_string_lossy().to_string();
            if p_str.contains("tests/e2e-server") || p_str.contains("tests\\e2e-server") {
                p.parent()?.parent().map(|s| s.to_path_buf())
            } else {
                Some(p)
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    for candidate in &candidates {
        let path = workspace_root.join(candidate);
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    // Fallback: let the user know
    eprintln!("WARNING: server binary not found, trying default path");
    candidates[0].to_string()
}

fn start_server(server_path: &str) -> Result<Child, String> {
    eprintln!("[e2e] Starting server: {}", server_path);

    let child = Command::new(server_path)
        .args(["--port", "25565", "--view-distance", "3"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start server: {}", e))?;

    Ok(child)
}

fn wait_for_server_ready(child: &mut Child) -> Result<Duration, String> {
    use std::io::BufRead;

    let start = Instant::now();
    let stdout = child.stdout.as_mut().ok_or("no stdout")?;
    let reader = std::io::BufReader::new(stdout);

    for line in reader.lines() {
        let line = line.map_err(|e| format!("read error: {}", e))?;
        eprintln!("[server] {}", line);

        if line.contains("Done") && line.contains('!') {
            let elapsed = start.elapsed();
            eprintln!("[e2e] Server ready in {:.2}s", elapsed.as_secs_f64());
            return Ok(elapsed);
        }

        // Timeout after 30 seconds
        if start.elapsed() > Duration::from_secs(30) {
            return Err("server did not start within 30 seconds".into());
        }
    }

    Err("server process ended without 'Done' message".into())
}

// ============================================================================
// Main test logic
// ============================================================================

fn run_test() -> Result<TestMetrics, String> {
    let mut metrics = TestMetrics::default();
    let test_start = Instant::now();

    // --- Step 1: Start the server ---
    let server_path = find_server_binary();
    let mut server = start_server(&server_path)?;

    let startup_duration = wait_for_server_ready(&mut server)?;
    metrics.server_started = true;
    metrics.startup_duration = Some(startup_duration);

    // --- Step 2: Connect via TCP ---
    eprintln!("[e2e] Connecting to localhost:25565...");
    let connect_start = Instant::now();
    let mut stream = std::net::TcpStream::connect("127.0.0.1:25565")
        .map_err(|e| format!("TCP connect failed: {}", e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    eprintln!(
        "[e2e] Connected in {:.1}ms",
        connect_start.elapsed().as_secs_f64() * 1000.0
    );

    // --- Step 3: Send Handshake ---
    let protocol_version: i32 = 858; // Minecraft 26.2
    let mut hs_payload = BytesMut::new();
    write_varint(&mut hs_payload, protocol_version);
    write_string(&mut hs_payload, "localhost");
    hs_payload.put_u16(25565);
    write_varint(&mut hs_payload, 2); // next_state = Login

    let handshake = encode_packet(0x00, &hs_payload);
    stream
        .write_all(&handshake)
        .map_err(|e| format!("send handshake failed: {}", e))?;
    eprintln!("[e2e] Sent Handshake (protocol={})", protocol_version);

    // --- Step 4: Send LoginStart ---
    let test_uuid = Uuid::new_v4();
    let mut ls_payload = BytesMut::new();
    write_string(&mut ls_payload, "TestBot");
    ls_payload.put_slice(test_uuid.as_bytes());

    let login_start = encode_packet(0x00, &ls_payload);
    stream
        .write_all(&login_start)
        .map_err(|e| format!("send LoginStart failed: {}", e))?;
    eprintln!("[e2e] Sent LoginStart (username=TestBot, uuid={})", test_uuid);

    // --- Step 5: Read packets ---
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut state = "login"; // login -> play
    let login_start_time = Instant::now();

    // Read loop
    loop {
        // Check test timeout
        if test_start.elapsed() > Duration::from_secs(30) {
            metrics.errors.push("test timeout (30s)".into());
            break;
        }

        // Check if we've collected enough data
        if metrics.chunks_received >= 10 && metrics.position_packets_sent >= 10 {
            eprintln!("[e2e] Collected enough data, ending test");
            break;
        }

        // Read from TCP
        let old_len = read_buf.len();
        read_buf.resize(old_len + 8192, 0);
        match stream.read(&mut read_buf[old_len..]) {
            Ok(0) => {
                metrics.errors.push("server closed connection".into());
                break;
            }
            Ok(n) => {
                read_buf.truncate(old_len + n);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // Timeout — send a position packet to keep things moving
                send_position_packet(&mut stream, &mut metrics)?;
                continue;
            }
            Err(e) => {
                metrics.errors.push(format!("read error: {}", e));
                break;
            }
        }

        // Decode packets from buffer
        loop {
            let raw_bytes: Bytes = read_buf.split_to(read_buf.len()).into();
            let mut raw_buf = raw_bytes.clone();

            match decode_packet(&mut raw_buf) {
                Ok(Some(packet)) => {
                    process_packet(
                        &packet,
                        &mut stream,
                        &mut state,
                        &mut metrics,
                        &login_start_time,
                    )?;

                    // Put remaining bytes back
                    if raw_buf.has_remaining() {
                        let remaining = raw_buf.to_vec();
                        read_buf.clear();
                        read_buf.extend_from_slice(&remaining);
                    }
                }
                Ok(None) => {
                    // Incomplete frame — put all bytes back
                    read_buf.clear();
                    read_buf.extend_from_slice(&raw_bytes);
                    break;
                }
                Err(e) => {
                    metrics.errors.push(format!("decode error: {}", e));
                    break;
                }
            }
        }

        // Send position packets periodically (every ~100ms)
        if metrics.position_packets_sent < 20 {
            send_position_packet(&mut stream, &mut metrics)?;
        }
    }

    // --- Step 6: Report ---
    metrics.total_duration = Some(test_start.elapsed());

    // Kill the server
    let _ = server.kill();

    Ok(metrics)
}

fn process_packet(
    packet: &RawPacket,
    stream: &mut std::net::TcpStream,
    state: &mut &str,
    metrics: &mut TestMetrics,
    login_start_time: &Instant,
) -> Result<(), String> {
    match *state {
        "login" => match packet.id {
            0x02 => {
                // LoginSuccess
                eprintln!(
                    "[e2e] Received LoginSuccess (login took {:.1}ms)",
                    login_start_time.elapsed().as_secs_f64() * 1000.0
                );
                metrics.login_success = true;
                *state = "play";
            }
            0x03 => {
                // SetCompression
                let mut payload = packet.payload.clone();
                let threshold = read_varint(&mut payload)?;
                eprintln!("[e2e] Server requested compression (threshold={})", threshold);
                // For this test we don't handle compression — just note it
                if threshold >= 0 {
                    metrics.errors.push(format!(
                        "server enabled compression (threshold={}), test may not work correctly",
                        threshold
                    ));
                }
            }
            _ => {
                eprintln!(
                    "[e2e] Unexpected login packet: 0x{:02X} ({} bytes)",
                    packet.id,
                    packet.payload.len()
                );
            }
        },
        "play" => match packet.id {
            0x2B => {
                // JoinGame
                let mut payload = packet.payload.clone();
                if payload.remaining() >= 4 {
                    let entity_id = payload.get_i32();
                    eprintln!("[e2e] Received JoinGame (entity_id={})", entity_id);
                } else {
                    eprintln!("[e2e] Received JoinGame (truncated)");
                }
                metrics.join_game_received = true;
            }
            0x5D => {
                // RegistryData
                metrics.registry_data_count += 1;
                eprintln!(
                    "[e2e] Received RegistryData #{}",
                    metrics.registry_data_count
                );
            }
            0x54 => {
                // SetDefaultSpawnPosition
                eprintln!("[e2e] Received SetDefaultSpawnPosition");
            }
            0x36 => {
                // PlayerAbilities
                eprintln!("[e2e] Received PlayerAbilities");
            }
            0x50 => {
                // SetCenterChunk
                eprintln!("[e2e] Received SetCenterChunk");
            }
            0x40 => {
                // SynchronizePlayerPosition
                let mut payload = packet.payload.clone();
                if payload.remaining() >= 24 {
                    let x = payload.get_f64();
                    let y = payload.get_f64();
                    let z = payload.get_f64();
                    eprintln!(
                        "[e2e] Received SynchronizePlayerPosition ({:.1}, {:.1}, {:.1})",
                        x, y, z
                    );
                } else {
                    eprintln!("[e2e] Received SynchronizePlayerPosition");
                }
                metrics.sync_position_received = true;
            }
            0x27 => {
                // ChunkDataAndUpdateLight
                let chunk_data_len = packet.payload.len();
                let mut payload = packet.payload.clone();
                if payload.remaining() >= 8 {
                    let chunk_x = payload.get_i32();
                    let chunk_z = payload.get_i32();
                    if metrics.chunks_received == 0 {
                        metrics.first_chunk_time = Some(login_start_time.elapsed());
                        metrics.first_chunk_size = chunk_data_len;
                        eprintln!(
                            "[e2e] Received FIRST chunk at ({}, {}) after {:.1}ms ({} bytes)",
                            chunk_x,
                            chunk_z,
                            login_start_time.elapsed().as_secs_f64() * 1000.0,
                            chunk_data_len,
                        );
                    }
                    metrics.chunks_received += 1;
                    if metrics.chunks_received % 10 == 0 {
                        eprintln!("[e2e]   ... {} chunks received so far (last chunk {} bytes)", metrics.chunks_received, chunk_data_len);
                    }
                }
            }
            0x67 => {
                // SystemChatMessage
                let mut payload = packet.payload.clone();
                if payload.has_remaining() {
                    // Read the length-prefixed JSON string
                    let len = read_varint(&mut payload).unwrap_or(0) as usize;
                    if payload.remaining() >= len {
                        let mut msg_bytes = vec![0u8; len];
                        payload.copy_to_slice(&mut msg_bytes);
                        if let Ok(msg) = String::from_utf8(msg_bytes) {
                            eprintln!("[e2e] Server chat: {}", msg);
                        }
                    }
                }
            }
            0x26 => {
                // KeepAlive — respond immediately
                let mut payload = packet.payload.clone();
                if payload.remaining() >= 8 {
                    let keepalive_id = payload.get_i64();
                    let response = encode_packet(0x18, &keepalive_id.to_be_bytes());
                    stream
                        .write_all(&response)
                        .map_err(|e| format!("send KeepAliveResponse failed: {}", e))?;
                    metrics.keepalive_responded += 1;
                    eprintln!(
                        "[e2e] Responded to KeepAlive #{} (id={})",
                        metrics.keepalive_responded, keepalive_id
                    );
                }
            }
            _ => {
                eprintln!(
                    "[e2e] Play packet: 0x{:02X} ({} bytes)",
                    packet.id,
                    packet.payload.len()
                );
            }
        },
        _ => {}
    }

    Ok(())
}

fn send_position_packet(
    stream: &mut std::net::TcpStream,
    metrics: &mut TestMetrics,
) -> Result<(), String> {
    // Send PlayerPosition (0x17) — simulate walking forward
    let z = metrics.position_packets_sent as f64 * 1.0; // walk along Z axis
    let mut payload = BytesMut::with_capacity(25);
    payload.put_f64(0.0); // x
    payload.put_f64(65.0); // y
    payload.put_f64(z); // z
    payload.put_u8(1); // on_ground = true

    let packet = encode_packet(0x17, &payload);
    stream
        .write_all(&packet)
        .map_err(|e| format!("send PlayerPosition failed: {}", e))?;
    metrics.position_packets_sent += 1;

    // Rate limit: ~80ms between position packets
    std::thread::sleep(Duration::from_millis(80));

    Ok(())
}

// ============================================================================
// Entry point
// ============================================================================

fn main() {
    eprintln!("========================================");
    eprintln!("  Neutron E2E Test");
    eprintln!("========================================");
    eprintln!();

    match run_test() {
        Ok(metrics) => {
            eprintln!();
            eprintln!("========================================");
            eprintln!("  TEST RESULTS");
            eprintln!("========================================");
            eprintln!();
            eprintln!("Server started:      {}", metrics.server_started);
            if let Some(d) = metrics.startup_duration {
                eprintln!("Startup time:        {:.2}s", d.as_secs_f64());
            }
            eprintln!("Login success:       {}", metrics.login_success);
            eprintln!("JoinGame received:   {}", metrics.join_game_received);
            eprintln!("RegistryData packets: {}", metrics.registry_data_count);
            eprintln!("SyncPosition recv:   {}", metrics.sync_position_received);
            eprintln!("Chunks received:     {}", metrics.chunks_received);
            if let Some(d) = metrics.first_chunk_time {
                eprintln!("Time to first chunk: {:.1}ms", d.as_secs_f64() * 1000.0);
            }
            eprintln!(
                "First chunk size:    {} bytes",
                metrics.first_chunk_size
            );
            eprintln!(
                "KeepAlive responses: {}",
                metrics.keepalive_responded
            );
            eprintln!(
                "Position packets:    {}",
                metrics.position_packets_sent
            );
            if let Some(d) = metrics.total_duration {
                eprintln!("Total test time:     {:.2}s", d.as_secs_f64());
            }
            if !metrics.errors.is_empty() {
                eprintln!();
                eprintln!("ERRORS:");
                for err in &metrics.errors {
                    eprintln!("  - {}", err);
                }
            }
            eprintln!();

            // Determine pass/fail
            let pass = metrics.server_started
                && metrics.login_success
                && metrics.join_game_received
                && metrics.chunks_received > 0
                && metrics.sync_position_received;

            if pass {
                eprintln!("RESULT: PASS");
                std::process::exit(0);
            } else {
                eprintln!("RESULT: FAIL");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("FATAL: {}", e);
            std::process::exit(2);
        }
    }
}
