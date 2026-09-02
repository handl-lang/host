# ADR-0025 (Archived): HTTP Client/Server v1 — Complete

## Status

Accepted

## Context

The handl runtime needed HTTP capabilities for both clients and servers. The initial implementation had only a client, with a unified name that didn't reflect server capabilities. The async module system also needed to be extended to support `AsyncFunction` and `Operation` dispatch.

## Decision

Completed v1 of HTTP client/server support:

### 1. Renamed HTTP client crates
- `lib-http` → `lib-http-client`
- `lib-http-std` → `lib-http-client-std`
- Preserved all public API names and `ureq` implementation

### 2. Extended module async/operation dispatch
- Added `Module::async_function` and `Module::operation` default methods
- Updated `ModuleHost` to forward async and operation calls
- Tested with unit tests in `runtime-core`

### 3. Introduced `handl-lib-http-server` (abstract)
- Defined `HttpServer` trait with `start`, `stop`, `join`, `route`, `serve_static`, `is_running`
- Defined `HttpServerRequest`, `HttpServerResponse`, `HttpServerError` types
- Created `HttpServerModule` exposing `http_server.*` functions
- Added `StubHttpServer` for testing

### 4. Introduced `handl-lib-http-server-std` (concrete)
- Implemented `StdHttpServer` using `tokio` + `hyper` 1.x
- HTTP/1.1 and HTTP/2 keep-alive support (via hyper features)
- Multi-threaded Tokio runtime
- Trie-based routing with `matchit` for static and named parameter segments
- Graceful shutdown
- Per-request body size limit and timeout
- Static file serving with path-sanitization and MIME guessing

### 5. Updated workspace
- Added four new crate members to `Cargo.toml`
- Added workspace dependency entries and feature flags

## Implementation

- ✅ Client crates renamed and working
- ✅ Module async/operation dispatch implemented
- ✅ Abstract server interface defined and tested
- ✅ Concrete server implementation with HTTP/1.1 + HTTP/2, routing, static files
- ✅ Workspace integration passing tests

## Consequences

- handl has both HTTP client and server capabilities as first-class features
- The module system supports async and operation patterns for extensibility
- HTTP/2 support is available (via hyper) for high-performance connections
- Path parameter routing is available (via matchit) for dynamic routes
- Production-grade baseline server ready for further enhancements

## See also

- ADR-0025 (Remaining) for v1.x and v2 enhancements
- Crates: `plugin-lib-http-client`, `plugin-lib-http-client-std`, `plugin-lib-http-server`, `plugin-lib-http-server-std`
