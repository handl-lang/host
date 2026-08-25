# handl_lib_http_client

# handl-lib-http-client

Abstract HTTP client service trait for the handl runtime.

Defines an abstract HTTP client interface that bridges HTTP request
capabilities into a callable [`Module`] implementation.

## Architecture

```text
handl-runtime-core          handl-lib-http-client            handl-lib-http-client-std
─────────────            ─────────────────────         ───────────────────────────
Module trait             HttpClient trait              StdHttpClient (ureq)
ModuleHost               HttpMethod/HttpRequest/HttpResponse
                         HttpError
                         HttpClientModule adapter
```

- **This crate** — abstract trait + module adapter + test stub.
  No I/O, no platform dependencies.
- **`handl-lib-http-client-std`** — concrete implementation using `ureq`.

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_http_client::{HttpClientModule, http_client_stub::StubHttpClient};

let host = ModuleHost::new()
    .with(HttpClientModule::new(StubHttpClient::default()));
```

Expressions can then call `http.get("https://...")`, `http.post(url, body)`, etc.

## Modules

### [`handl_lib_http_client`](handl_lib_http_client.md)

*2 modules*

### [`error`](error.md)

*1 module*

### [`error::error`](error/error.md)

*1 enum*

### [`http_client`](http_client.md)

*2 modules*

### [`http_client::http_client`](http_client/http_client.md)

*1 enum, 1 trait, 10 structs, 2 macros, 3 functions*

### [`http_client::http_client_stub`](http_client/http_client_stub.md)

*1 struct*

