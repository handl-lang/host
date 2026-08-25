# HTTP Client/Server Crate Split and Server Runtime

## Context

The handl runtime currently has a single HTTP capability: `handl-lib-http` / `handl-lib-http-std`. These names are client-only, which is now too narrow. The language needs to run **servers** as well — fast, production-grade HTTP servers written in handl and backed by an efficient Rust implementation.

The VM already supports `AsyncFunction` and `Operation` at the `Host` level, but the module system (`Module` trait and `ModuleHost`) only dispatches synchronous `Function` calls. Exposing an async server module requires the module layer to wire through `Host::async_function` and `Host::operation`.

This ADR scopes the rename, the new server crates, and the staged work needed to make handl a server-authoring language.

## Decision

### 1. Rename the existing HTTP client crates

- `crates/lib-http` → `crates/lib-http-client` (package `handl-lib-http` → `handl-lib-http-client`)
- `crates/lib-http-std` → `crates/lib-http-client-std` (package `handl-lib-http-std` → `handl-lib-http-client-std`)
- Update all `Cargo.toml` workspace members, dependencies, feature flags, doc comments, and re-exports.
- Preserve all public API names (`HttpClient`, `HttpClientModule`, `HttpMethod`, etc.) and existing `ureq` implementation.

### 2. Extend `Module` and `ModuleHost` to support async/operation dispatch

Before the server module can expose async functions, the module layer must forward them:

- Add `Module::async_function` and `Module::operation` default methods returning `None`.
- Update `ModuleHost` to implement `Host::async_function` and `Host::operation` using the same `.` split-and-dispatch logic as `Host::function`.
- Add unit tests in `crates/runtime-core` proving a module can export an `AsyncFunction` and an `Operation`.

### 3. Introduce `handl-lib-http-server` (abstract server)

The abstract crate defines a platform-agnostic HTTP server interface and an handl module adapter. It has **no I/O** and no platform dependencies, mirroring the client crate pattern.

Public types:

- `HttpServer` trait — `start`, `stop`, `join`, `route`, `serve_static`, `is_running`.
- `HttpServerRequest` — method, path, query map, headers, body.
- `HttpServerResponse` — status, headers, body.
- `HttpMethod` (re-exported/aligned with the client crate, or shared in a future `lib-http-core`).
- `HttpServerError` — network, invalid argument, already running, not running, handler error, with `From` conversion to `ac_runtime::Error`.
- `HttpServerModule` — exposes `http_server.*` functions.
- `StubHttpServer` — in-memory fake for tests.

`HttpServer` v1 surface:

| Method | Purpose |
|--------|---------|
| `route(method, path, handler)` | Register a handler for a method/path pattern. |
| `serve_static(path, dir)` | Register a directory of static files. |
| `start(addr)` | Start accepting connections on `addr`. Returns immediately; server runs on its own threads. |
| `stop()` | Signal graceful shutdown. |
| `join()` | Block until the server has shut down. |
| `is_running()` | Whether the server is currently listening. |

`HttpServerModule` v1 function surface (all sync `Function`s; the server trait hides its internal async I/O):

- `http_server.new()` — create a server value.
- `http_server.route(server, method, path, handler)` — register an handl `FunctionValue` as the route handler.
- `http_server.serve_static(server, path, dir)` — serve static files from `dir` under `path`.
- `http_server.start(server, addr)` — start the server.
- `http_server.stop(server)` — request shutdown.
- `http_server.join(server)` — block until shutdown.

### 4. Introduce `handl-lib-http-server-std` (concrete server)

`StdHttpServer` is a high-performance, production-baseline implementation:

- Built on `tokio` + `hyper` 1.x.
- HTTP/1.1 and HTTP/2 keep-alive.
- Multi-threaded Tokio runtime.
- Trie-based routing with `matchit` (static and named parameter segments).
- Graceful shutdown with `tokio::sync::watch` or `oneshot`.
- Per-request body size limit and timeout.
- Static file serving with path-sanitization and MIME guessing.
- Handler dispatch to handl `FunctionValue`s via the VM (`Context::eval_expr` / `Context::eval_expr_async`) using a per-request fresh `Context` and the server-owned `VirtualMachine`.

Proposed dependencies (stable, well-established versions):

- `tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "time"] }`
- `hyper = { version = "1", features = ["server", "http1", "http2"] }`
- `hyper-util = { version = "0.1", features = ["tokio"] }`
- `http-body-util = "0.1"`
- `matchit = "0.8"`
- `mime_guess = "2"`

### 5. Update the workspace `Cargo.toml`

- Replace `crates/lib-http` and `crates/lib-http-std` with the four new members.
- Add `handl-lib-http-client` and `handl-lib-http-client-std` workspace dependency entries.
- Add `handl-lib-http-server` and `handl-lib-http-server-std` workspace dependency entries.
- Rename `lib-http` and `lib-http-std` feature flags to `lib-http-client` and `lib-http-client-std`, and add `lib-http-server` and `lib-http-server-std`.

## Feature List (v1)

### Client (preserved)

- All existing `HttpClient` trait methods and `HttpClientModule` functions.
- `ureq` implementation in `lib-http-client-std`.
- Stub client for tests.

### Server core

- Abstract `HttpServer` trait with lifecycle and routing.
- Request/response value types.
- `HttpServerError` with VM error conversion.
- `HttpServerModule` exposing `http_server.*` functions to handl.
- `StubHttpServer` for unit tests.

### Server standard implementation

- Tokio/Hyper HTTP/1.1 + HTTP/2 server.
- Graceful start, stop, and join.
- Route matching with `{name}` path parameters and query strings.
- Middleware/interceptor hook (request/response transform) on the `HttpServer` trait.
- Static file serving with directory traversal protection.
- Body size and idle timeout limits.
- handl handler dispatch: an handl `FunctionValue` receives `request` and returns a response object.

### Out of scope for v1

- HTTPS/TLS termination (can be added behind a reverse proxy or in v2).
- WebSockets.
- Request/response streaming bodies (bodies are buffered to strings for v1).
- Compression (`gzip`, `brotli`).
- Built-in CORS/rate-limit middleware (pluggable via the middleware hook).

## Requirements

1. **No breaking changes** to existing handl programs that use `http.get`, `http.post`, etc. after the rename.
2. **No `throw` in library code** — all errors are returned as `Result` and converted to `ac_runtime::Error` at the module boundary.
3. **Async capability** — the server I/O is async, but the module functions remain sync so they work with the current `Function` trait. The implementation runs the server on a background Tokio runtime and communicates via channels.
4. **Thread safety** — `HttpServer` and `HttpServerModule` must be `Send + Sync`.
5. **Testability** — every public function has a unit test; `StdHttpServer` has an integration test that starts a server and makes real HTTP requests.
6. **Documentation** — each crate has an updated top-level module doc and `README.md`.

## Test Plan

### `crates/lib-http-client`

- Keep/rewrite existing `http_client_tests.rs` and `error_tests.rs` under the new path.
- Add a rename regression test that imports `handl_lib_http_client` and calls `HttpClientModule::new(StubHttpClient)`.

### `crates/lib-http-client-std`

- Keep/rewrite existing `ureq` implementation tests.
- Optionally add a tiny real network smoke test behind a feature flag (`network_tests`).

### `crates/lib-http-server`

- `src/http_server/http_server_stub.rs` — stub implementation returning canned responses.
- `src/http_server/http_server_tests.rs` — module tests for `http_server.new`, `route`, `start`, `stop`, `join` with the stub.
- `src/route/route_tests.rs` — route matching (static, parameters, 404, method mismatch).
- `src/request/request_tests.rs` — query parsing, header extraction.
- `src/error/error_tests.rs` — `HttpServerError` display and VM conversion.

### `crates/lib-http-server-std`

- `src/http_server/std_tests.rs` — unit tests for `StdHttpServer` using the stub.
- `tests/integration_tests.rs` — start `StdHttpServer` on `127.0.0.1:0`, make a request with a small test client, assert status/body/headers, then `stop` and `join`.

### `crates/runtime-core`

- `src/module/module_tests.rs` — add tests that an `AsyncFunction` and an `Operation` registered through a `Module` are dispatched by `ModuleHost`.

## Implementation Phases

1. **P0: Module async/operation dispatch** — unblock server module from exposing async functions and operations.
2. **P1: Rename client crates** — `lib-http` → `lib-http-client`, `lib-http-std` → `lib-http-client-std`; update workspace and feature flags; run `cargo check` and tests.
3. **P2: Abstract server crate** — create `lib-http-server` with trait, types, error, module adapter, stub, and tests.
4. **P3: Standard server crate** — create `lib-http-server-std` with `StdHttpServer` (tokio/hyper), routing, static files, handl handler dispatch, and integration tests.
5. **P4: Workspace integration** — add server feature flags to the root `handl` crate, update `README.md`, and run `cargo test -p handl-lib-http-client -p handl-lib-http-client-std -p handl-lib-http-server -p handl-lib-http-server-std`.

## Consequences

- The HTTP surface is split into client and server, making the capability names honest and future-proof.
- handl programs can author HTTP servers by registering routes and handlers, then calling `http_server.start`.
- New runtime dependencies (`tokio`, `hyper`, `matchit`, `mime_guess`) are isolated in `lib-http-server-std`.
- The `Module`/`ModuleHost` changes benefit all future async modules (`fs`, `kv`, etc.).
- handl server performance is bounded by the synchronous, per-request VM evaluation; v1 is production-capable for I/O, and handler throughput can be optimized later with VM caching or pre-compiled continuations.

## v1 Completion Notes

The HTTP client/server split and production-baseline server are implemented and tested.

- P0: `Module` and `ModuleHost` now dispatch `AsyncFunction` and `Operation`; tests added.
- P1: Renamed `lib-http` → `lib-http-client` and `lib-http-std` → `lib-http-client-std`; updated workspace members, dependencies, and feature flags.
- P2: Created `handl-lib-http-server` with the `HttpServer` trait, `HttpServerModule`, `HttpServerRequest`/`HttpServerResponse`, `HttpServerError`, and `StubHttpServer` tests.
- P3: Created `handl-lib-http-server-std` with `StdHttpServer` (tokio + hyper HTTP/1.1), `ServerConfig` (body limit, idle timeout, keep-alive), request body size enforcement, idle timeout, `JoinSet`-based graceful shutdown, `mime_guess` for static files, and default `Content-Type` for handler responses.
- `cargo test` passes for `handl-lib-http-client`, `handl-lib-http-client-std`, `handl-lib-http-server`, `handl-lib-http-server-std`, and `handl-runtime-modules`.

Remaining v1.x / v2 work: handl `FunctionValue` route handler dispatch, HTTP/2, `matchit` path parameter routing, TLS, middleware hooks, compression, and CORS/rate-limiting.

## Status

Accepted
