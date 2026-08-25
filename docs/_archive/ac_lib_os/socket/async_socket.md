**handl_lib_os > socket > async_socket**

# Module: socket::async_socket

## Contents

**Structs**

- [`AsyncCloseFn`](#asyncclosefn)
- [`AsyncConnectFn`](#asyncconnectfn)
- [`AsyncRecvFn`](#asyncrecvfn)
- [`AsyncSendFn`](#asyncsendfn)
- [`AsyncSocketModule`](#asyncsocketmodule) - Module adapter for [`AsyncSocket`] — exposes `socket.*` async functions.

**Functions**

- [`require_number_arg`](#require_number_arg)
- [`require_string_arg`](#require_string_arg)

**Traits**

- [`AsyncSocket`](#asyncsocket) - Abstract async socket service.

**Type Aliases**

- [`AsyncSocketFuture`](#asyncsocketfuture) - A boxed, sendable future for async socket operations.

---

## handl_lib_os::socket::async_socket::AsyncCloseFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn AsyncSocket>`

**Trait Implementations:**

- **AsyncFunction**
  - `fn call<'a>(self: &'a Self, args: &'a [NonFunctionValue], _ctx: &'a  mut dyn Context) -> Pin<Box<dyn Future>>`



## handl_lib_os::socket::async_socket::AsyncConnectFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn AsyncSocket>`

**Trait Implementations:**

- **AsyncFunction**
  - `fn call<'a>(self: &'a Self, args: &'a [NonFunctionValue], _ctx: &'a  mut dyn Context) -> Pin<Box<dyn Future>>`



## handl_lib_os::socket::async_socket::AsyncRecvFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn AsyncSocket>`

**Trait Implementations:**

- **AsyncFunction**
  - `fn call<'a>(self: &'a Self, args: &'a [NonFunctionValue], _ctx: &'a  mut dyn Context) -> Pin<Box<dyn Future>>`



## handl_lib_os::socket::async_socket::AsyncSendFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn AsyncSocket>`

**Trait Implementations:**

- **AsyncFunction**
  - `fn call<'a>(self: &'a Self, args: &'a [NonFunctionValue], _ctx: &'a  mut dyn Context) -> Pin<Box<dyn Future>>`



## handl_lib_os::socket::async_socket::AsyncSocket

*Trait*

Abstract async socket service.

Implementations:
- `handl-lib-os-std` — `MioSocket` backed by `mio` (Unix only).

**Methods:**

- `connect`: Asynchronously connect to `address` and return a handle.
- `send`: Asynchronously send `data` on `handle` and return the bytes written.
- `recv`: Asynchronously receive up to `length` bytes from `handle`.
- `close`: Asynchronously close `handle`.



## handl_lib_os::socket::async_socket::AsyncSocketFuture

*Type Alias*: `std::pin::Pin<Box<dyn Future>>`

A boxed, sendable future for async socket operations.



## handl_lib_os::socket::async_socket::AsyncSocketModule

*Struct*

Module adapter for [`AsyncSocket`] — exposes `socket.*` async functions.

**Fields:**
- `connect: AsyncConnectFn`
- `send: AsyncSendFn`
- `recv: AsyncRecvFn`
- `close: AsyncCloseFn`

**Methods:**

- `fn new<impl AsyncSocket + 'static>(socket: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, _name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`



## handl_lib_os::socket::async_socket::require_number_arg

*Function*

```rust
fn require_number_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<i64, ac_runtime_values::Error>
```



## handl_lib_os::socket::async_socket::require_string_arg

*Function*

```rust
fn require_string_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



