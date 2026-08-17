//! RCON client for sending commands to Minecraft servers.
//!
//! Used to automatically enable spark HTTP after server startup.
//! Protocol: https://developer.valvesoftware.com/wiki/Source_RCON_Protocol

use eyre::Result;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// RCON packet types
const SERVERDATA_AUTH: i32 = 3;
const SERVERDATA_EXECCOMMAND: i32 = 2;
const SERVERDATA_RESPONSE_VALUE: i32 = 0;

/// RCON client for Minecraft servers.
pub struct RconClient {
    stream: TcpStream,
    request_id: i32,
}

impl RconClient {
    /// Connect to an RCON server.
    pub fn connect(host: &str, port: u16, password: &str) -> Result<Self> {
        let mut stream = TcpStream::connect(format!("{}:{}", host, port))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let mut client = Self {
            stream,
            request_id: 0,
        };

        // Authenticate
        client.authenticate(password)?;

        Ok(client)
    }

    /// Send an RCON command and return the response.
    pub fn send_command(&mut self, command: &str) -> Result<String> {
        self.request_id += 1;
        let id = self.request_id;

        // Send command packet
        self.send_packet(SERVERDATA_EXECCOMMAND, id, command)?;

        // Read response
        let (resp_id, body) = self.read_packet()?;

        if resp_id == -1 {
            eyre::bail!("RCON authentication failed");
        }

        Ok(body)
    }

    /// Authenticate with the RCON server.
    fn authenticate(&mut self, password: &str) -> Result<()> {
        self.request_id += 1;
        let id = self.request_id;

        // Send auth packet
        self.send_packet(SERVERDATA_AUTH, id, password)?;

        // Read auth response
        let (resp_id, _) = self.read_packet()?;

        if resp_id == -1 {
            eyre::bail!("RCON authentication failed");
        }

        Ok(())
    }

    /// Send an RCON packet.
    fn send_packet(&mut self, packet_type: i32, id: i32, body: &str) -> Result<()> {
        let body_bytes = body.as_bytes();
        let size = 4 + 4 + body_bytes.len() + 1 + 1; // id + type + body + null + null

        let mut packet = Vec::with_capacity(4 + size);
        packet.extend_from_slice(&(size as i32).to_le_bytes());
        packet.extend_from_slice(&id.to_le_bytes());
        packet.extend_from_slice(&packet_type.to_le_bytes());
        packet.extend_from_slice(body_bytes);
        packet.push(0); // null terminator
        packet.push(0); // null terminator

        self.stream.write_all(&packet)?;
        self.stream.flush()?;

        Ok(())
    }

    /// Read an RCON packet.
    fn read_packet(&mut self) -> Result<(i32, String)> {
        // Read size (4 bytes)
        let mut size_buf = [0u8; 4];
        self.stream.read_exact(&mut size_buf)?;
        let size = i32::from_le_bytes(size_buf) as usize;

        // Read rest of packet
        let mut packet = vec![0u8; size];
        self.stream.read_exact(&mut packet)?;

        // Parse: id (4 bytes) + type (4 bytes) + body
        let id = i32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]);
        // packet[4..8] is type, but we don't need it for responses
        let body = String::from_utf8_lossy(&packet[8..size - 2]).to_string();

        Ok((id, body))
    }
}

/// Send a command to a Minecraft server via RCON.
pub fn send_command(host: &str, port: u16, password: &str, command: &str) -> Result<String> {
    let mut client = RconClient::connect(host, port, password)?;
    let response = client.execute(command)?;
    Ok(response)
}

impl RconClient {
    /// Execute a command and return the response.
    pub fn execute(&mut self, command: &str) -> Result<String> {
        self.request_id += 1;
        let id = self.request_id;

        self.send_packet(SERVERDATA_EXECCOMMAND, id, command)?;

        // Read response (may have multiple packets for long responses)
        let mut response = String::new();
        loop {
            let (resp_id, body) = self.read_packet()?;
            if resp_id == id {
                response = body;
                break;
            }
            // Other IDs are ignored (broadcast packets)
        }

        Ok(response)
    }
}
