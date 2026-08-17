//! E2E test for the Neutron server — Minecraft 26.2 (protocol 776).
//!
//! Two modes:
//!   e2e-test join [--port N] [--duration SECS] [--spawn <server-binary>]
//!       Full protocol-level login: handshake -> login -> configuration ->
//!       play. Reaches the Play state and receives real level-chunk data.
//!       Reports timing metrics and a keepalive-cadence TPS estimate.
//!   e2e-test status [--port N]
//!       Server-list status ping (26.2), prints raw bytes + decoded JSON.
//!
//! Uses the `neutron-protocol` crate for framing and the typed Handshake /
//! LoginStart packets; packet IDs are the 26.2 values from the server's
//! `protocol_ids` module.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use neutron_protocol::codec::MinecraftCodec;
use neutron_protocol::login::{Handshake, LoginStart};
use neutron_protocol::types::{read_varint, write_varint};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

// 26.2 protocol version and packet IDs (mirror crates/neutron-server/src/protocol_ids.rs).
const PROTOCOL_VERSION: i32 = 776;

const CFG_SB_SELECT_KNOWN_PACKS: u32 = 0x07;
const CFG_SB_FINISH: u32 = 0x03;

const PLAY_LOGIN: u32 = 0x31;
const PLAY_KEEP_ALIVE: u32 = 0x2C;
const PLAY_LEVEL_CHUNK: u32 = 0x2D;
const PLAY_POSITION: u32 = 0x48;
const PLAY_CENTER_CHUNK: u32 = 0x5E;
const PLAY_CHUNK_BATCH_START: u32 = 0x0C;
const PLAY_CHUNK_BATCH_FINISHED: u32 = 0x0B;
const PLAY_SYSTEM_CHAT: u32 = 0x79;

const SB_KEEP_ALIVE: u32 = 0x1C;
const SB_MOVE_POS: u32 = 0x1E;
const SB_ACCEPT_TELEPORT: u32 = 0x00;

// ============================================================================
// Raw framing helpers (uncompressed; the server runs compression_threshold=-1)
// ============================================================================

fn write_string(buf: &mut BytesMut, s: &str) {
    write_varint(buf, s.len() as i32).unwrap();
    buf.put_slice(s.as_bytes());
}

fn hexdump(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

// ============================================================================
// Metrics
// ============================================================================

#[derive(Debug, Default)]
struct Metrics {
    login_success: bool,
    join_game_received: bool,
    registry_data_count: usize,
    sync_position_received: bool,
    center_chunk: Option<(i32, i32)>,
    chunks_received: usize,
    first_chunk: Option<(Duration, usize)>,
    keepalive_responded: usize,
    keepalive_intervals: Vec<Duration>,
    keepalive_last: Option<Instant>,
    position_packets_sent: usize,
    packets_by_window: Vec<usize>,
    packets_in_window: usize,
    errors: Vec<String>,
    chat_messages: Vec<String>,
    unknown_packets: std::collections::BTreeMap<u32, usize>,
}

impl Metrics {
    fn tps_from_keepalives(&self) -> Option<f64> {
        if self.keepalive_intervals.is_empty() {
            return None;
        }
        // Server sends a keepalive every 600 ticks.
        let sum: f64 = self
            .keepalive_intervals
            .iter()
            .map(|d| d.as_secs_f64())
            .sum();
        let n = self.keepalive_intervals.len() as f64;
        Some(600.0 / (sum / n))
    }
}

// ============================================================================
// Client session
// ============================================================================

fn run_join_session(stream: &mut TcpStream, duration: Duration) -> Result<Metrics, String> {
    let mut metrics = Metrics::default();
    let codec = MinecraftCodec::new();
    let session_start = Instant::now();
    let mut window_start = Instant::now();

    // --- Handshake (next_state = 2, login) ---
    let hs = Handshake {
        protocol_version: PROTOCOL_VERSION,
        server_address: "127.0.0.1".to_string(),
        server_port: 25565,
        next_state: 2,
    };
    let mut hs_payload = BytesMut::new();
    hs.encode(&mut hs_payload).map_err(|e| e.to_string())?;
    send_frame(stream, &codec, 0x00, &hs_payload)?;
    println!("[tx] Handshake protocol={PROTOCOL_VERSION} next_state=2");

    // --- LoginStart ---
    let uuid = Uuid::new_v4();
    let ls = LoginStart {
        name: "SmokeBot".to_string(),
        uuid: Some(uuid),
    };
    let mut ls_payload = BytesMut::new();
    ls.encode(&mut ls_payload).map_err(|e| e.to_string())?;
    send_frame(stream, &codec, 0x00, &ls_payload)?;
    println!("[tx] LoginStart username=SmokeBot uuid={uuid}");

    let login_start = Instant::now();
    let mut phase = "login";
    let mut input_buf = BytesMut::with_capacity(1 << 20);

    loop {
        if session_start.elapsed() > duration {
            println!("[e2e] session duration reached, ending test");
            break;
        }

        // Read available bytes (non-blocking with short timeout so position
        // packets keep flowing even when the server is quiet).
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
        let old_len = input_buf.len();
        input_buf.resize(old_len + 64 * 1024, 0);
        match stream.read(&mut input_buf[old_len..]) {
            Ok(0) => {
                metrics.errors.push("server closed connection".into());
                break;
            }
            Ok(n) => {
                input_buf.truncate(old_len + n);
                metrics.packets_in_window += 1;
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {
                // read() timed out: discard the zero-fill that resize() added so
                // the decode loop never sees fake bytes (they decoded as a
                // zero-length frame -> spurious "packet too short for header").
                input_buf.truncate(old_len);
            }
            Err(e) => {
                metrics.errors.push(format!("read error: {e}"));
                break;
            }
        }

        // Decode as many complete packets as are buffered.
        loop {
            let raw: Bytes = input_buf.split_to(input_buf.len()).into();
            let mut rest = raw.clone();
            match codec.decode(&mut rest) {
                Ok(Some(pkt)) => {
                    process_packet(
                        &pkt,
                        stream,
                        &codec,
                        &mut phase,
                        &mut metrics,
                        &login_start,
                    )?;
                    if rest.has_remaining() {
                        input_buf.clear();
                        input_buf.extend_from_slice(&rest);
                    }
                }
                Ok(None) => {
                    input_buf.clear();
                    input_buf.extend_from_slice(&raw);
                    break;
                }
                Err(e) => {
                    metrics.errors.push(format!("decode error: {e}"));
                    if metrics.errors.len() <= 5 {
                        let dump: Vec<String> = raw
                            .iter()
                            .take(80)
                            .map(|b| format!("{b:02x}"))
                            .collect();
                        println!(
                            "[e2e] !!! decode error #{} phase={phase} elapsed={:.1}s buf_len={} raw={}",
                            metrics.errors.len(),
                            session_start.elapsed().as_secs_f64(),
                            raw.len(),
                            dump.join(" ")
                        );
                    }
                    break;
                }
            }
        }

        // Report per-10s packet window.
        if window_start.elapsed() >= Duration::from_secs(10) {
            metrics.packets_by_window.push(metrics.packets_in_window);
            metrics.packets_in_window = 0;
            window_start = Instant::now();
            println!(
                "[e2e] 10s window packet count: {}",
                metrics.packets_by_window.last().copied().unwrap_or(0)
            );
        }

        // Send a position packet every ~200ms.
        send_position_packet(stream, &codec, &mut metrics)?;
        std::thread::sleep(Duration::from_millis(200));
    }

    Ok(metrics)
}

fn send_frame(
    stream: &mut TcpStream,
    codec: &MinecraftCodec,
    id: u32,
    payload: &[u8],
) -> Result<(), String> {
    let mut buf = BytesMut::with_capacity(payload.len() + 8);
    codec.encode(id, payload, &mut buf).map_err(|e| e.to_string())?;
    stream
        .write_all(&buf)
        .map_err(|e| format!("send packet 0x{id:02x} failed: {e}"))
}

fn process_packet(
    pkt: &neutron_protocol::packet::RawPacket,
    stream: &mut TcpStream,
    codec: &MinecraftCodec,
    phase: &mut &str,
    metrics: &mut Metrics,
    login_start: &Instant,
) -> Result<(), String> {
    match *phase {
        "login" => match pkt.id {
            0x02 => {
                // LoginFinished -> send LoginAcknowledged (0x03, empty).
                println!(
                    "[rx] LoginFinished (login took {:.1}ms)",
                    login_start.elapsed().as_secs_f64() * 1000.0
                );
                metrics.login_success = true;
                send_frame(stream, codec, 0x03, &[])?;
                println!("[tx] LoginAcknowledged -> configuration");
                *phase = "configuration";
            }
            _ => unknown(metrics, pkt.id, pkt.payload.len()),
        },
        "configuration" => match pkt.id {
            0x0E => {
                // SelectKnownPacks -> reply with count 0.
                println!("[rx] SelectKnownPacks");
                let mut buf = BytesMut::new();
                write_varint(&mut buf, 0).map_err(|e| e.to_string())?;
                send_frame(stream, codec, CFG_SB_SELECT_KNOWN_PACKS, &buf)?;
                println!("[tx] SelectKnownPacks (none)");
            }
            0x07 => {
                metrics.registry_data_count += 1;
                println!("[rx] RegistryData #{}", metrics.registry_data_count);
            }
            0x03 => {
                // FinishConfiguration -> reply, enter Play.
                println!("[rx] FinishConfiguration");
                send_frame(stream, codec, CFG_SB_FINISH, &[])?;
                println!("[tx] FinishConfiguration -> play");
                *phase = "play";
            }
            0x0C => println!("[rx] UpdateFeatures"),
            0x0D => println!("[rx] UpdateTags"),
            _ => unknown(metrics, pkt.id, pkt.payload.len()),
        },
        "play" => match pkt.id {
            PLAY_LOGIN => {
                let mut payload = pkt.payload.clone();
                let entity_id = payload.get_i32();
                println!("[rx] PlayLogin (entity_id={entity_id})");
                metrics.join_game_received = true;
            }
            PLAY_KEEP_ALIVE => {
                let mut payload = pkt.payload.clone();
                let keepalive_id = payload.get_i64();
                let now = Instant::now();
                if let Some(last) = metrics.keepalive_last {
                    metrics.keepalive_intervals.push(now.duration_since(last));
                }
                metrics.keepalive_last = Some(now);
                let mut resp = BytesMut::with_capacity(8);
                resp.put_i64(keepalive_id);
                send_frame(stream, codec, SB_KEEP_ALIVE, &resp)?;
                metrics.keepalive_responded += 1;
                println!(
                    "[rx] KeepAlive id={keepalive_id} -> responded (n={})",
                    metrics.keepalive_responded
                );
            }
            PLAY_LEVEL_CHUNK => {
                let mut payload = pkt.payload.clone();
                let cx = payload.get_i32();
                let cz = payload.get_i32();
                let size = pkt.payload.len();
                if metrics.chunks_received == 0 {
                    metrics.first_chunk = Some((login_start.elapsed(), size));
                    println!(
                        "[rx] FIRST LevelChunk ({cx},{cz}) {} bytes after {:.1}ms",
                        size,
                        login_start.elapsed().as_secs_f64() * 1000.0
                    );
                }
                metrics.chunks_received += 1;
                if metrics.chunks_received % 20 == 0 {
                    println!("[rx] ... {size} bytes LevelChunk ({cx},{cz}) (total {})", metrics.chunks_received);
                }
            }
            PLAY_POSITION => {
                let mut payload = pkt.payload.clone();
                let x = payload.get_f64();
                let y = payload.get_f64();
                let z = payload.get_f64();
                println!("[rx] SynchronizePlayerPosition ({x:.1},{y:.1},{z:.1})");
                metrics.sync_position_received = true;
                // Acknowledge the teleport.
                let mut buf = BytesMut::new();
                write_varint(&mut buf, 1).map_err(|e| e.to_string())?;
                send_frame(stream, codec, SB_ACCEPT_TELEPORT, &buf)?;
            }
            PLAY_CENTER_CHUNK => {
                let mut payload = pkt.payload.clone();
                let cx = read_varint(&mut payload).map_err(|e| e.to_string())?;
                let cz = read_varint(&mut payload).map_err(|e| e.to_string())?;
                metrics.center_chunk = Some((cx, cz));
                println!("[rx] SetCenterChunk ({cx},{cz})");
            }
            PLAY_CHUNK_BATCH_START => println!("[rx] ChunkBatchStart"),
            PLAY_CHUNK_BATCH_FINISHED => {
                let mut payload = pkt.payload.clone();
                let count = read_varint(&mut payload).unwrap_or(0);
                println!("[rx] ChunkBatchFinished count={count}");
            }
            PLAY_SYSTEM_CHAT => {
                // Network NBT: TAG_String (0x08) + u16 len + utf8 + u8 overlay.
                let mut payload = pkt.payload.clone();
                if payload.has_remaining() {
                    let tag = payload.get_u8();
                    let len = payload.get_u16() as usize;
                    if payload.remaining() >= len {
                        let bytes = payload.copy_to_bytes(len);
                        if let Ok(msg) = String::from_utf8(bytes.to_vec()) {
                            println!("[rx] SystemChat: {msg}");
                            metrics.chat_messages.push(msg);
                        } else {
                            println!("[rx] SystemChat (tag={tag}, non-utf8)");
                        }
                    }
                }
            }
            _ => unknown(metrics, pkt.id, pkt.payload.len()),
        },
        _ => {}
    }
    Ok(())
}

fn unknown(metrics: &mut Metrics, id: u32, size: usize) {
    *metrics.unknown_packets.entry(id).or_insert(0) += 1;
    if size < 4096 {
        println!("[rx] packet 0x{id:02X} ({size} bytes)");
    }
}

fn send_position_packet(
    stream: &mut TcpStream,
    codec: &MinecraftCodec,
    metrics: &mut Metrics,
) -> Result<(), String> {
    let z = metrics.position_packets_sent as f64 * 1.0;
    let mut payload = BytesMut::with_capacity(25);
    payload.put_f64(0.5);
    payload.put_f64(65.0);
    payload.put_f64(z);
    payload.put_u8(1); // on ground
    send_frame(stream, codec, SB_MOVE_POS, &payload)?;
    metrics.position_packets_sent += 1;
    Ok(())
}

// ============================================================================
// Server process management (spawn mode)
// ============================================================================

fn start_server(path: &str, port: u16) -> Result<Child, String> {
    Command::new(path)
        .args(["--port", &port.to_string(), "--view-distance", "3"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start server: {e}"))
}

fn wait_for_server_ready(child: &mut Child) -> Result<Duration, String> {
    use std::io::BufRead;
    let start = Instant::now();
    let stdout = child.stdout.as_mut().ok_or("no stdout")?;
    let reader = std::io::BufReader::new(stdout);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("read error: {e}"))?;
        if line.contains("Done") && line.contains('!') {
            return Ok(start.elapsed());
        }
        if start.elapsed() > Duration::from_secs(30) {
            return Err("server did not print Done within 30s".into());
        }
    }
    Err("server process ended without 'Done' message".into())
}

// ============================================================================
// Status ping mode
// ============================================================================

fn run_status_ping(port: u16) -> Result<(), String> {
    let codec = MinecraftCodec::new();
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;

    let mut hs_payload = BytesMut::new();
    write_varint(&mut hs_payload, PROTOCOL_VERSION).map_err(|e| e.to_string())?;
    write_string(&mut hs_payload, "127.0.0.1");
    hs_payload.put_u16(port);
    write_varint(&mut hs_payload, 1).map_err(|e| e.to_string())?; // status
    let mut frame = BytesMut::new();
    codec
        .encode(0x00, &hs_payload, &mut frame)
        .map_err(|e| e.to_string())?;
    println!("[tx] handshake (status) raw={}", hexdump(&frame));
    stream.write_all(&frame).map_err(|e| e.to_string())?;

    let mut req = BytesMut::new();
    codec.encode(0x00, &[], &mut req).map_err(|e| e.to_string())?;
    println!("[tx] status request     raw={}", hexdump(&req));
    stream.write_all(&req).map_err(|e| e.to_string())?;

    let resp = read_frame(&mut stream, &codec)?;
    println!("[rx] status response    id=0x{:02x} raw={}", resp.0, hexdump(&resp.1));
    let mut payload = resp.1.clone();
    let len = read_varint(&mut payload).map_err(|e| e.to_string())? as usize;
    let raw_json = payload.copy_to_bytes(len);
    let json = String::from_utf8(raw_json.to_vec()).map_err(|e| e.to_string())?;
    println!("     decoded JSON: {json}");

    let ping_payload = 0x2A5E_DEAD_BEEFi64.to_be_bytes();
    let mut ping = BytesMut::new();
    codec
        .encode(0x01, &ping_payload, &mut ping)
        .map_err(|e| e.to_string())?;
    println!("[tx] status ping        raw={}", hexdump(&ping));
    stream.write_all(&ping).map_err(|e| e.to_string())?;

    let pong = read_frame(&mut stream, &codec)?;
    println!("[rx] pong               id=0x{:02x} raw={}", pong.0, hexdump(&pong.1));
    if pong.0 != 0x01 || &pong.1[..] != &ping_payload[..] {
        return Err("pong mismatch".into());
    }
    println!("     payload match: 0x2a5edeadbeef");
    println!("STATUS PING OK");
    Ok(())
}

fn read_frame(stream: &mut TcpStream, codec: &MinecraftCodec) -> Result<(u32, Bytes), String> {
    let mut buf = BytesMut::new();
    loop {
        let mut chunk = [0u8; 8192];
        match stream.read(&mut chunk) {
            Ok(0) => return Err("EOF".into()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
        let mut rest = buf.clone().into();
        if let Ok(Some(pkt)) = codec.decode(&mut rest) {
            return Ok((pkt.id, pkt.payload));
        }
    }
}

// ============================================================================
// Entry point
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("join");
    let mut port: u16 = 25565;
    let mut duration = Duration::from_secs(45);
    let mut spawn: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                port = args[i].parse().unwrap_or(25565);
            }
            "--duration" => {
                i += 1;
                duration = Duration::from_secs(args[i].parse().unwrap_or(45));
            }
            "--spawn" => {
                i += 1;
                spawn = Some(args[i].clone());
            }
            _ => {}
        }
        i += 1;
    }

    if mode == "status" {
        match run_status_ping(port) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("FATAL: {e}");
                std::process::exit(2);
            }
        }
    }

    // Join mode.
    let mut server: Option<Child> = None;
    let connect_result = match spawn {
        Some(path) => {
            eprintln!("[e2e] spawning server: {path}");
            let mut child = start_server(&path, port).expect("start server");
            match wait_for_server_ready(&mut child) {
                Ok(d) => eprintln!("[e2e] server Done in {:.2}s", d.as_secs_f64()),
                Err(e) => {
                    let _ = child.kill();
                    eprintln!("FATAL: {e}");
                    std::process::exit(2);
                }
            }
            server = Some(child);
            TcpStream::connect(("127.0.0.1", port))
        }
        None => {
            eprintln!("[e2e] connecting to existing server on 127.0.0.1:{port}");
            TcpStream::connect(("127.0.0.1", port))
        }
    };

    let mut stream = match connect_result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FATAL: connect: {e}");
            std::process::exit(2);
        }
    };
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .ok();

    eprintln!("[e2e] session duration: {:?}", duration);
    match run_join_session(&mut stream, duration) {
        Ok(m) => {
            report(&m);
            if let Some(mut child) = server {
                let _ = child.kill();
            }
            let pass = m.login_success
                && m.join_game_received
                && m.chunks_received > 0
                && m.sync_position_received
                && m.errors.is_empty();
            if pass {
                println!("RESULT: PASS");
                std::process::exit(0);
            }
            println!("RESULT: FAIL");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("FATAL: {e}");
            if let Some(mut child) = server {
                let _ = child.kill();
            }
            std::process::exit(2);
        }
    }
}

fn report(m: &Metrics) {
    println!("\n========================================");
    println!("  E2E TEST RESULTS (26.2)");
    println!("========================================");
    println!("Login success:          {}", m.login_success);
    println!("PlayLogin received:     {}", m.join_game_received);
    println!("RegistryData packets:   {}", m.registry_data_count);
    println!("SyncPosition received:  {}", m.sync_position_received);
    println!("Center chunk:           {:?}", m.center_chunk);
    println!("Chunks received:        {}", m.chunks_received);
    if let Some((t, size)) = m.first_chunk {
        println!("Time to first chunk:    {:.1}ms", t.as_secs_f64() * 1000.0);
        println!("First chunk size:       {size} bytes");
    }
    println!("KeepAlive responses:    {}", m.keepalive_responded);
    if let Some(tps) = m.tps_from_keepalives() {
        println!("TPS (keepalive cadence): {tps:.2}");
    }
    println!("Position packets sent:  {}", m.position_packets_sent);
    println!("Packets per 10s window: {:?}", m.packets_by_window);
    if !m.chat_messages.is_empty() {
        println!("Server chat:            {:?}", m.chat_messages);
    }
    if !m.unknown_packets.is_empty() {
        println!("Unknown/unhandled pkts: {:#x?}", m.unknown_packets);
    }
    if !m.errors.is_empty() {
        println!("ERRORS:");
        for e in &m.errors {
            println!("  - {e}");
        }
    }
}
