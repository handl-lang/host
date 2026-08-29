# ADR-0025 (Remaining): HTTP Client/Server v1.x and v2 Work

## Status

Proposed

## Context

ADR-0025 (HTTP Client/Server Crate Split and v1 Server Runtime) has been implemented and archived. v1 is complete: the client crates were renamed, the `Module`/`ModuleHost` async/operation dispatch landed, `handl-lib-http-server` and `handl-lib-http-server-std` were created, and the workspace integration is passing tests.

The following work remains for v1.x and v2.

## Decision

Implement the remaining HTTP server capabilities:

- handl `FunctionValue` route handler dispatch.
- HTTP/2 support.
- `matchit` path parameter routing.
- TLS termination.
- Middleware hooks.
- Compression.
- CORS and rate-limiting.

## Consequences

- handl server applications can use path parameters and real handl functions as route handlers.
- HTTP/2 and TLS make the server suitable for production without a reverse proxy.
- Middleware hooks enable pluggable compression, CORS, rate-limiting, and observability.

## Related work

- `project/adrs/archived/0025-http-server.md`
