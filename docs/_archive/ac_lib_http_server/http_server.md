**handl_lib_http_server > http_server**

# Module: http_server

## Contents

**Structs**

- [`HttpServerModule`](#httpservermodule) - Module adapter for [`HttpServer`] — exposes `http_server.*` functions.
- [`Inner`](#inner)
- [`JoinFn`](#joinfn)
- [`NewFn`](#newfn)
- [`RouteFn`](#routefn)
- [`ServeStaticFn`](#servestaticfn)
- [`StartFn`](#startfn)
- [`StopFn`](#stopfn)
- [`StubHttpServer`](#stubhttpserver)
- [`StubState`](#stubstate)

**Functions**

- [`require_id_at`](#require_id_at)
- [`require_string_at`](#require_string_at)

**Traits**

- [`HttpServer`](#httpserver) - Abstract HTTP server service.

**Type Aliases**

- [`Handler`](#handler)

---

## handl_lib_http_server::http_server::Handler

*Type Alias*: `Box<dyn Fn>`



## handl_lib_http_server::http_server::HttpServer

*Trait*

Abstract HTTP server service.

**Methods:**

- `start`: Start accepting connections on `addr`.
- `stop`: Signal the server to stop accepting new connections.
- `join`: Block until the server has fully shut down.
- `is_running`: Returns `true` if the server is currently listening.
- `route`: Register a handler for a method/path pattern.
- `route_function`: Register an handl `FunctionValue` as a handler for a method/path pattern.
- `serve_static`: Serve static files from `dir` under `path`.



## handl_lib_http_server::http_server::HttpServerModule

*Struct*

Module adapter for [`HttpServer`] — exposes `http_server.*` functions.

| Function | Args | Description |
|----------|------|-------------|
| `http_server.new()` | `()` | Creates a new server and returns its id. |
| `http_server.start(server, addr)` | `(server, addr)` | Starts the server. |
| `http_server.stop(server)` | `(server)` | Stops the server. |
| `http_server.join(server)` | `(server)` | Blocks until shutdown. |
| `http_server.route(server, method, path, body)` | `(server, method, path, body)` | Registers a static response route. |
| `http_server.serve_static(server, path, dir)` | `(server, path, dir)` | Serves static files. |

**Generic Parameters:**
- F

**Fields:**
- `new_fn: NewFn<F>`
- `start_fn: StartFn<F>`
- `stop_fn: StopFn<F>`
- `join_fn: JoinFn<F>`
- `route_fn: RouteFn<F>`
- `serve_static_fn: ServeStaticFn<F>`

**Methods:**

- `fn new(factory: F) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_http_server::http_server::Inner

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `factory: F`
- `servers: std::sync::Mutex<std::collections::HashMap<usize, std::sync::Arc<dyn HttpServer>>>`
- `next_id: std::sync::atomic::AtomicUsize`
- `_phantom: std::marker::PhantomData<F>`

**Methods:**

- `fn new(factory: F) -> Self`
- `fn create_server(self: &Self) -> usize`
- `fn with_server<T, impl FnOnce(&Arc<dyn HttpServer>) -> Result<T, HttpServerError>>(self: &Self, id: usize, f: impl Trait) -> Result<T, HttpServerError>`



## handl_lib_http_server::http_server::JoinFn

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `inner: std::sync::Arc<Inner<F>>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_server::http_server::NewFn

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `inner: std::sync::Arc<Inner<F>>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_server::http_server::RouteFn

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `inner: std::sync::Arc<Inner<F>>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_server::http_server::ServeStaticFn

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `inner: std::sync::Arc<Inner<F>>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_server::http_server::StartFn

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `inner: std::sync::Arc<Inner<F>>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_server::http_server::StopFn

*Struct*

**Generic Parameters:**
- F

**Fields:**
- `inner: std::sync::Arc<Inner<F>>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_server::http_server::StubHttpServer

*Struct*

**Fields:**
- `state: std::sync::Mutex<StubState>`

**Methods:**

- `fn new() -> Self`
- `fn response_for(self: &Self, req: HttpServerRequest) -> Option<HttpServerResponse>`

**Trait Implementations:**

- **HttpServer**
  - `fn start(self: &Self, _addr: &str) -> Result<(), HttpServerError>`
  - `fn stop(self: &Self) -> Result<(), HttpServerError>`
  - `fn join(self: &Self) -> Result<(), HttpServerError>`
  - `fn is_running(self: &Self) -> bool`
  - `fn route(self: &Self, method: HttpMethod, path: &str, handler: Handler) -> Result<(), HttpServerError>`
  - `fn route_function(self: &Self, method: HttpMethod, path: &str, _handler: FunctionValue) -> Result<(), HttpServerError>`
  - `fn serve_static(self: &Self, path: &str, dir: &str) -> Result<(), HttpServerError>`
- **Default**
  - `fn default() -> StubHttpServer`



## handl_lib_http_server::http_server::StubState

*Struct*

**Fields:**
- `running: bool`
- `routes: std::collections::HashMap<(crate::http_method::HttpMethod, String), Handler>`
- `static_routes: std::collections::HashMap<String, String>`

**Trait Implementations:**

- **Default**
  - `fn default() -> StubState`



## handl_lib_http_server::http_server::require_id_at

*Function*

```rust
fn require_id_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<usize, ac_runtime_values::Error>
```



## handl_lib_http_server::http_server::require_string_at

*Function*

```rust
fn require_string_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&str, ac_runtime_values::Error>
```



