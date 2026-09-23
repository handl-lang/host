#!/usr/bin/env python3
"""Canned RFC 6455 echo server for the ws-c tests (stdlib only).

One connection at a time: complete the upgrade, then echo every text and
binary frame back (unmasked, as a server must), answer pings with pongs,
and close on a close frame or when the peer goes away. Serves until it
is killed, so a single fixture backs several tests.

Writes /tmp/ws-c-test-ready once it is listening; the handl test polls
for it instead of sleeping.
"""

import base64
import hashlib
import socket
import struct
import sys

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
PORT = 18360
READY = "/tmp/ws-c-test-ready"

OP_CONT = 0x0
OP_TEXT = 0x1
OP_BINARY = 0x2
OP_CLOSE = 0x8
OP_PING = 0x9
OP_PONG = 0xA


def read_exact(conn, n):
    buf = b""
    while len(buf) < n:
        chunk = conn.recv(n - len(buf))
        if not chunk:
            return None
        buf += chunk
    return buf


def read_frame(conn):
    """One frame, unmasked by the server. Returns (op, payload) or None."""
    hdr = read_exact(conn, 2)
    if hdr is None:
        return None
    op = hdr[0] & 0x0F
    length = hdr[1] & 0x7F
    if length == 126:
        length = struct.unpack(">H", read_exact(conn, 2))[0]
    elif length == 127:
        length = struct.unpack(">Q", read_exact(conn, 8))[0]
    mask = read_exact(conn, 4) if hdr[1] & 0x80 else b"\x00\x00\x00\x00"
    payload = read_exact(conn, length) if length else b""
    if payload is None:
        return None
    return op, bytes(b ^ mask[i % 4] for i, b in enumerate(payload))


def send_frame(conn, op, payload):
    conn.sendall(bytes([0x80 | op]) + bytes([len(payload)]) + payload)


def handshake(conn):
    req = b""
    while b"\r\n\r\n" not in req:
        chunk = conn.recv(4096)
        if not chunk:
            return False
        req += chunk
    key = ""
    for line in req.split(b"\r\n"):
        if line.lower().startswith(b"sec-websocket-key:"):
            key = line.split(b":", 1)[1].strip().decode()
    if not key:
        return False
    accept = base64.b64encode(hashlib.sha1((key + GUID).encode()).digest()).decode()
    conn.sendall(
        (
            "HTTP/1.1 101 Switching Protocols\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
        ).encode()
    )
    return True


def serve_conn(conn):
    if not handshake(conn):
        return
    while True:
        got = read_frame(conn)
        if got is None:
            return
        op, payload = got
        if op == OP_CLOSE:
            send_frame(conn, OP_CLOSE, b"")
            return
        if op == OP_PING:
            send_frame(conn, OP_PONG, payload)
        elif op in (OP_TEXT, OP_BINARY, OP_CONT):
            send_frame(conn, op, payload)


def main():
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", PORT))
    srv.listen(4)
    with open(READY, "w") as f:
        f.write("ready")
    while True:
        conn, _ = srv.accept()
        try:
            serve_conn(conn)
        except (OSError, ConnectionError):
            pass
        finally:
            conn.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(0)
