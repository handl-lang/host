# HTTP + WebSocket example

A runnable Handl server: HTTP routing and a WebSocket echo endpoint on the
epoll scheduler. The listener, the HTTP/1.1 codec, the upgrade handshake, the
frame loop and the scheduler are all Handl; Rust supplies only the syscalls
they call through the generated libc bindings.

```bash
target/debug/hnl run examples/http/echo.hnl     # blocks; the accept loop IS main
```

From another shell:

```bash
curl http://127.0.0.1:8080/hello                # -> hello
curl -X POST -d 'ping' http://127.0.0.1:8080/echo   # -> ping
websocat ws://127.0.0.1:8080/ws                 # -> ws echo: <your message>
```

`echo.hnl` is the whole app: two ordinary arrows (`handler` for HTTP,
`ws_echo` for WebSocket) and one `serve` call. Handlers are plain arrows, so
they are unit-testable without a socket — call `handler(fake_req)`.

TLS is `serve(handler, { port, tls: { cert, key }, on_ws })`; see
`handl-packages/serve-epoll/serve-epoll.hnl` for the full option set and
`handl-packages/serve-epoll/serve-epoll.test.hnl` for the tests that cover
GET/POST/keep-alive/chunked/TLS/WebSocket end to end.
