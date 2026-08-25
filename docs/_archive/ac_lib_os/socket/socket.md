**handl_lib_os > socket > socket**

# Module: socket::socket

## Contents

**Structs**

- [`CloseFn`](#closefn)
- [`ConnectFn`](#connectfn)
- [`RecvFn`](#recvfn)
- [`SendFn`](#sendfn)
- [`SocketModule`](#socketmodule) - Module adapter for [`Socket`] — exposes `socket.*` functions.

**Functions**

- [`require_number_arg`](#require_number_arg)
- [`require_string_arg`](#require_string_arg)

**Traits**

- [`Socket`](#socket) - Abstract socket service.

---

## handl_lib_os::socket::socket::CloseFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn Socket>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::socket::socket::ConnectFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn Socket>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::socket::socket::RecvFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn Socket>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::socket::socket::SendFn

*Struct*

**Fields:**
- `socket: std::sync::Arc<dyn Socket>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::socket::socket::Socket

*Trait*

Abstract socket service.

Implementations:
- `handl-lib-os-std` — backed by `std::net`
- [`socket_stub::StubSocket`] — in-memory for testing

**Methods:**

- `connect`
- `send`
- `recv`
- `close`



## handl_lib_os::socket::socket::SocketModule

*Struct*

Module adapter for [`Socket`] — exposes `socket.*` functions.

**Fields:**
- `connect: ConnectFn`
- `send: SendFn`
- `recv: RecvFn`
- `close: CloseFn`

**Methods:**

- `fn new<impl Socket + 'static>(socket: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_os::socket::socket::require_number_arg

*Function*

```rust
fn require_number_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<i64, ac_runtime_values::Error>
```



## handl_lib_os::socket::socket::require_string_arg

*Function*

```rust
fn require_string_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



