**handl_lib_http_server_std > http_server**

# Module: http_server

## Contents

**Structs**

- [`ServerConfig`](#serverconfig) - Runtime configuration for [`StdHttpServer`].
- [`ServerRuntime`](#serverruntime)
- [`ServerState`](#serverstate)
- [`StdHttpServer`](#stdhttpserver) - Production-baseline HTTP server backed by `tokio` + `hyper`.

**Functions**

- [`body_string`](#body_string)
- [`convert_response`](#convert_response)
- [`error_response`](#error_response)
- [`extract_headers`](#extract_headers)
- [`factory`](#factory)
- [`handle_connection`](#handle_connection)
- [`handle_request`](#handle_request)
- [`parse_query`](#parse_query)
- [`request_to_value`](#request_to_value)
- [`run_server`](#run_server)
- [`std_http_server_module`](#std_http_server_module) - Creates a new [`StdHttpServerModule`].
- [`value_to_response`](#value_to_response)

**Type Aliases**

- [`StdHttpServerModule`](#stdhttpservermodule) - Preconfigured `HttpServerModule` that builds [`StdHttpServer`] instances.

---

## handl_lib_http_server_std::http_server::ServerConfig

*Struct*

Runtime configuration for [`StdHttpServer`].

**Fields:**
- `body_limit: usize` - Maximum request body size in bytes (default 1 MiB).
- `idle_timeout: std::time::Duration` - Time to wait for a new request on an idle connection (default 30 s).
- `keep_alive: bool` - Whether to enable HTTP/1.1 keep-alive (default `true`).

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ServerConfig`
- **Default**
  - `fn default() -> Self`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_http_server_std::http_server::ServerRuntime

*Struct*

**Fields:**
- `shutdown_tx: Option<oneshot::Sender<()>>`
- `handle: std::thread::JoinHandle<()>`



## handl_lib_http_server_std::http_server::ServerState

*Struct*

**Fields:**
- `routes: std::sync::Mutex<std::collections::HashMap<handl_lib_http_server::HttpMethod, matchit::Router<handl_lib_http_server::Handler>>>`
- `static_routes: std::sync::Mutex<std::collections::HashMap<String, String>>`
- `runtime: std::sync::Mutex<Option<ServerRuntime>>`
- `local_addr: std::sync::Mutex<Option<std::net::SocketAddr>>`
- `config: ServerConfig`
- `eval: std::sync::Arc<ac_runtime_canonical::CanonicalEval<ac_runtime_example_1::DefaultHost>>`

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



## handl_lib_http_server_std::http_server::StdHttpServer

*Struct*

Production-baseline HTTP server backed by `tokio` + `hyper`.

**Fields:**
- `state: std::sync::Arc<ServerState>`

**Methods:**

- `fn local_addr(self: &Self) -> Option<SocketAddr>` - Returns the local address the server is bound to, if running.
- `fn new() -> Self`
- `fn with_config(config: ServerConfig) -> Self`
- `fn new_arc() -> Arc<Self>`
- `fn new_arc_with_config(config: ServerConfig) -> Arc<Self>`

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **HttpServer**
  - `fn start(self: &Self, addr: &str) -> Result<(), HttpServerError>`
  - `fn stop(self: &Self) -> Result<(), HttpServerError>`
  - `fn join(self: &Self) -> Result<(), HttpServerError>`
  - `fn is_running(self: &Self) -> bool`
  - `fn route(self: &Self, method: HttpMethod, path: &str, handler: Handler) -> Result<(), HttpServerError>`
  - `fn route_function(self: &Self, method: HttpMethod, path: &str, function_value: FunctionValue) -> Result<(), HttpServerError>`
  - `fn serve_static(self: &Self, path: &str, dir: &str) -> Result<(), HttpServerError>`



## handl_lib_http_server_std::http_server::StdHttpServerModule

*Type Alias*: `handl_lib_http_server::HttpServerModule<fn(...)>`

Preconfigured `HttpServerModule` that builds [`StdHttpServer`] instances.



## handl_lib_http_server_std::http_server::body_string

*Function*

```rust
fn body_string(body: hyper::body::Incoming, limit: usize) -> Result<String, handl_lib_http_server::HttpServerError>
```



## handl_lib_http_server_std::http_server::convert_response

*Function*

```rust
fn convert_response(response: handl_lib_http_server::HttpServerResponse) -> hyper::Response<http_body_util::Full<bytes::Bytes>>
```



## handl_lib_http_server_std::http_server::error_response

*Function*

```rust
fn error_response(status: u16, body: &str) -> hyper::Response<http_body_util::Full<bytes::Bytes>>
```



## handl_lib_http_server_std::http_server::extract_headers

*Function*

```rust
fn extract_headers(headers: &hyper::HeaderMap) -> std::collections::HashMap<String, String>
```



## handl_lib_http_server_std::http_server::factory

*Function*

```rust
fn factory() -> std::sync::Arc<dyn HttpServer>
```



## handl_lib_http_server_std::http_server::handle_connection

*Function*

```rust
fn handle_connection(stream: tokio::net::TcpStream, state: std::sync::Arc<ServerState>)
```



## handl_lib_http_server_std::http_server::handle_request

*Function*

```rust
fn handle_request(req: hyper::Request<hyper::body::Incoming>, state: std::sync::Arc<ServerState>) -> hyper::Response<http_body_util::Full<bytes::Bytes>>
```



## handl_lib_http_server_std::http_server::parse_query

*Function*

```rust
fn parse_query(query: &str) -> std::collections::HashMap<String, String>
```



## handl_lib_http_server_std::http_server::request_to_value

*Function*

```rust
fn request_to_value(req: &handl_lib_http_server::HttpServerRequest) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_http_server_std::http_server::run_server

*Function*

```rust
fn run_server(listener: std::net::TcpListener, state: std::sync::Arc<ServerState>, shutdown_rx: oneshot::Receiver<()>)
```



## handl_lib_http_server_std::http_server::std_http_server_module

*Function*

Creates a new [`StdHttpServerModule`].

```rust
fn std_http_server_module() -> StdHttpServerModule
```



## handl_lib_http_server_std::http_server::value_to_response

*Function*

```rust
fn value_to_response(value: &ac_runtime_values::NonFunctionValue) -> handl_lib_http_server::HttpServerResponse
```



