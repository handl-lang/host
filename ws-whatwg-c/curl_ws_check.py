#!/usr/bin/env python3
"""Validation harness for the curl WebSocket binding shapes.

Drives libcurl exactly as the generated `CurlWsSend`/`CurlWsRecv`
adapters would (same descriptor expansion), against a minimal RFC 6455
echo server, and prints the frame metadata offsets so `WsFrameDesc` in
`curl_client.hnl` can be checked against the real struct.

Run: python3 handl-packages/ws-whatwg-c/curl_ws_check.py
"""

import base64
import ctypes
import hashlib
import socket
import struct
import threading
import time

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
CURLWS_TEXT = 1
CURLWS_BINARY = 2
CURLWS_CLOSE = 8
CURLWS_PING = 16
CURLOPT_URL = 10002
CURLOPT_CONNECT_ONLY = 141


def ws_server(sock, ready):
    """One RFC 6455 handshake, then echo a single masked text frame."""
    conn, _ = sock.accept()
    req = b""
    while b"\r\n\r\n" not in req:
        req += conn.recv(4096)
    key = ""
    for line in req.split(b"\r\n"):
        if line.lower().startswith(b"sec-websocket-key:"):
            key = line.split(b":", 1)[1].strip().decode()
    accept = base64.b64encode(hashlib.sha1((key + GUID).encode()).digest()).decode()
    conn.sendall(
        (
            "HTTP/1.1 101 Switching Protocols\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
        ).encode()
    )
    ready.set()
    hdr = conn.recv(2)
    opcode = hdr[0] & 0x0F
    length = hdr[1] & 0x7F
    mask = conn.recv(4)
    payload = b""
    while len(payload) < length:
        payload += conn.recv(length - len(payload))
    data = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    # echo back unmasked, same opcode
    conn.sendall(bytes([0x80 | opcode, len(data)]) + data)
    conn.close()


class CurlWsFrame(ctypes.Structure):
    """Must match `WsFrameDesc` in curl_client.hnl."""

    _layout_ = "ms"
    _pack_ = 1
    _fields_ = [
        ("age", ctypes.c_int32),
        ("flags", ctypes.c_int32),
        ("offset", ctypes.c_int64),
        ("bytesleft", ctypes.c_int64),
        ("len", ctypes.c_uint64),
    ]


def main():
    for name in ("age", "flags", "offset", "bytesleft", "len"):
        print(f"curl_ws_frame.{name}.offset = {getattr(CurlWsFrame, name).offset}")
    print(f"sizeof(curl_ws_frame) = {ctypes.sizeof(CurlWsFrame)}")

    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", 0))
    srv.listen(1)
    port = srv.getsockname()[1]
    ready = threading.Event()
    t = threading.Thread(target=ws_server, args=(srv, ready), daemon=True)
    t.start()

    lib = ctypes.CDLL("libcurl.so.4")
    lib.curl_easy_init.restype = ctypes.c_void_p
    lib.curl_easy_perform.argtypes = [ctypes.c_void_p]
    lib.curl_easy_perform.restype = ctypes.c_int
    lib.curl_easy_cleanup.argtypes = [ctypes.c_void_p]
    lib.curl_ws_send.argtypes = [
        ctypes.c_void_p,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.c_int64,
        ctypes.c_uint,
    ]
    lib.curl_ws_send.restype = ctypes.c_int
    lib.curl_ws_recv.argtypes = [
        ctypes.c_void_p,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.POINTER(ctypes.c_void_p),
    ]
    lib.curl_ws_recv.restype = ctypes.c_int
    setopt = lib.curl_easy_setopt

    h = lib.curl_easy_init()
    assert h, "curl_easy_init failed"
    url = f"ws://127.0.0.1:{port}/".encode()
    assert setopt(ctypes.c_void_p(h), ctypes.c_int(CURLOPT_URL), url) == 0
    assert (
        setopt(
            ctypes.c_void_p(h),
            ctypes.c_int(CURLOPT_CONNECT_ONLY),
            ctypes.c_int64(2),
        )
        == 0
    )
    code = lib.curl_easy_perform(ctypes.c_void_p(h))
    print(f"handshake CURLcode = {code}")
    assert code == 0, code
    ready.wait(5)

    sent = ctypes.c_size_t(0)
    code = lib.curl_ws_send(
        ctypes.c_void_p(h), b"hello", 5, ctypes.byref(sent), 0, CURLWS_TEXT
    )
    print(f"ws_send CURLcode = {code} sent = {sent.value}")
    assert code == 0 and sent.value == 5

    buf = ctypes.create_string_buffer(65536)
    recvd = ctypes.c_size_t(0)
    meta = ctypes.c_void_p(0)
    for _ in range(100):
        code = lib.curl_ws_recv(
            ctypes.c_void_p(h), buf, 65536, ctypes.byref(recvd), ctypes.byref(meta)
        )
        if code != 81:  # CURLE_AGAIN
            break
        time.sleep(0.05)
    print(f"ws_recv CURLcode = {code} bytes = {recvd.value} meta = {meta.value}")
    assert code == 0, code
    flags = ctypes.cast(meta, ctypes.POINTER(CurlWsFrame)).contents.flags
    print(f"payload = {buf.raw[:recvd.value]!r} flags = {flags}")
    assert buf.raw[: recvd.value] == b"hello"
    assert flags & CURLWS_TEXT, flags

    lib.curl_easy_cleanup(ctypes.c_void_p(h))
    srv.close()
    t.join(timeout=5)
    print("OK")


if __name__ == "__main__":
    main()
