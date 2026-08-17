#!/usr/bin/env python3
"""Minecraft 26.2 (protocol 776) server-list status ping.

Evidence capture for the neutron-server review: prints every raw byte
exchanged and the decoded JSON status response.

Usage: python3 status_ping.py [host] [port]
"""
import json
import socket
import struct
import sys
import time

HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 25565
PROTOCOL = 776  # 26.2


def write_varint(val):
    out = b""
    while True:
        b = val & 0x7F
        val >>= 7
        if val:
            b |= 0x80
        out += bytes([b])
        if not val:
            return out


def read_varint(buf):
    result = 0
    shift = 0
    for i, byte in enumerate(buf):
        result |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return result, buf[i + 1 :]
        shift += 7
    raise ValueError("truncated varint")


def frame(packet_id, payload):
    body = write_varint(packet_id) + payload
    return write_varint(len(body)) + body


def recv_frame(sock):
    # read length varint byte by byte
    length = 0
    shift = 0
    while True:
        byte = sock.recv(1)
        if not byte:
            raise ConnectionError("EOF while reading frame length")
        length |= (byte[0] & 0x7F) << shift
        if not byte[0] & 0x80:
            break
        shift += 7
    data = b""
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            raise ConnectionError("EOF while reading frame body")
        data += chunk
    pid, rest = read_varint(data)
    return pid, rest


def hexdump(b):
    return " ".join(f"{x:02x}" for x in b)


def main():
    sock = socket.create_connection((HOST, PORT), timeout=10)
    sock.settimeout(10)
    print(f"connected to {HOST}:{PORT} at {time.strftime('%Y-%m-%dT%H:%M:%S')}")

    # 1. Handshake, next_state = 1 (status)
    addr = HOST.encode()
    hs = write_varint(PROTOCOL) + write_varint(len(addr)) + addr + struct.pack(">H", PORT) + write_varint(1)
    hs_frame = frame(0x00, hs)
    print(f"\n[tx] handshake (next_state=1)  raw={hexdump(hs_frame)}")
    sock.sendall(hs_frame)

    # 2. StatusRequest (0x00, empty payload)
    req = frame(0x00, b"")
    print(f"[tx] status request             raw={hexdump(req)}")
    sock.sendall(req)

    # 3. StatusResponse (0x00): length-prefixed JSON string
    pid, payload = recv_frame(sock)
    print(f"[rx] status response            id=0x{pid:02x} raw={hexdump(payload)}")
    assert pid == 0x00, f"expected 0x00, got 0x{pid:02x}"
    json_len, rest = read_varint(payload)
    raw_json = rest[:json_len].decode("utf-8")
    print(f"     json len={json_len}")
    print(f"     decoded JSON: {json.dumps(json.loads(raw_json), indent=2)}")

    # 4. StatusPing (0x01, i64 payload)
    ping_payload = struct.pack(">q", 0x2A_5E_DEAD_BEEF)
    ping = frame(0x01, ping_payload)
    print(f"\n[tx] status ping                raw={hexdump(ping)}")
    sock.sendall(ping)

    # 5. Pong (0x01, same i64)
    pid, payload = recv_frame(sock)
    print(f"[rx] pong                       id=0x{pid:02x} raw={hexdump(payload)}")
    assert pid == 0x01, f"expected 0x01, got 0x{pid:02x}"
    assert payload == ping_payload, "pong payload mismatch"
    print(f"     payload match: 0x{struct.unpack('>q', payload)[0]:016x}")

    sock.close()
    print("\nSTATUS PING OK")


if __name__ == "__main__":
    main()