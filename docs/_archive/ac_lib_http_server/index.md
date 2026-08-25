# handl_lib_http_server

# handl-lib-http-server

Abstract HTTP server service trait and module adapter for the handl runtime.

Defines an abstract HTTP server interface and exposes `http_server.*`
functions to handl programs. No I/O and no platform dependencies in this
crate — concrete servers live in `handl-lib-http-server-std`.

## Architecture

```text
handl-runtime-core          handl-lib-http-server            handl-lib-http-server-std
─────────────            ─────────────────────         ───────────────────────────
Module trait             HttpServer trait              StdHttpServer
ModuleHost               HttpServerRequest/Response
                         HttpServerError
                         HttpServerModule adapter
```

- **This crate** — abstract trait + module adapter + test stub.
- **`handl-lib-http-server-std`** — concrete implementation.

## Modules

### [`handl_lib_http_server`](handl_lib_http_server.md)

*5 modules*

### [`error`](error.md)

*1 enum*

### [`http_method`](http_method.md)

*1 enum*

### [`http_server`](http_server.md)

*1 trait, 1 type alias, 10 structs, 2 functions*

### [`request`](request.md)

*1 struct*

### [`response`](response.md)

*1 struct*

