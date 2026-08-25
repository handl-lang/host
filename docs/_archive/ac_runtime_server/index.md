# ac_runtime_server

# handl-runtime-server

A [`Host`] for running handl expressions with effects enabled.

Includes everything from [`handl-runtime-sandbox`] plus:

- **OS stdlib** (`fs.*`, `env.*`, `path.*`, `time.*`, `cmd.*`, `process.*`, `tmp.*`, `socket.*`, `sig.*`)
  from [`handl-lib-os-std`].
- **HTTP server** (`http_server.*`) from [`handl-lib-http-server-std`].
- **HTTP client** (`http_client.*`) from [`handl-lib-http-client-std`].
- **Random** (`rand.*`) from [`handl-lib-rand`].

This is the host you want when handl needs to open ports, read files,
spawn processes, or make HTTP requests.

## Modules

### [`ac_runtime_server`](ac_runtime_server.md)

*2 structs*

