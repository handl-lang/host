# ADR-0025 (Remaining): HTTP Client/Server v1.x and v2 Enhancements

## Status

Proposed

## Context

ADR-0025 v1 (HTTP Client/Server Crate Split and v1 Server Runtime) has been implemented and archived, with v1 features complete: client crates renamed, `Module`/`ModuleHost` async/operation dispatch, abstract and concrete server implementations, HTTP/2 and `matchit` routing all in place.

The following work remains for v1.x stability and v2 enhancements.

## Decision

Implement the remaining HTTP server enhancements:

### v1.x Completion

- **handl `FunctionValue` route handler dispatch.** The abstract `route` and `route_function` methods are stubbed with placeholders. Implement full dispatch of handl `FunctionValue`s as route handlers via the VM, with proper error handling and response serialization.

### v2 Features

- **TLS termination.** Add TLS/HTTPS support via `rustls` or native TLS, with certificate loading and SNI support.
- **Middleware hooks.** Provide a composable middleware/layer API for request/response interception (similar to `tower` or `actix-web` middleware).
- **Compression.** Add transparent `gzip`/`brotli` response compression with `Accept-Encoding` negotiation.
- **CORS and rate-limiting.** Provide built-in CORS policy handling and per-IP or per-handler rate-limiting.

## Consequences

- v1.x: handl server applications can use real handl functions as route handlers with full type and error information.
- v2: HTTP/2 + TLS + middleware make handl servers production-ready without a reverse proxy.
- v2: Middleware hooks enable pluggable compression, CORS, rate-limiting, observability, and custom request/response handling.

## Related work

- `project/adrs/archived/0025-http-server.md` — v1 completed features
- `project/adrs/archived/0025-http-server-v1-complete.md` — v1 final status
- Crates: `plugin-lib-http-server`, `plugin-lib-http-server-std`
