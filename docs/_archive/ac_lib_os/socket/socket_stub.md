**handl_lib_os > socket > socket_stub**

# Module: socket::socket_stub

## Contents

**Structs**

- [`StubConnection`](#stubconnection)
- [`StubSocket`](#stubsocket) - A simple in-memory socket service for tests.

---

## handl_lib_os::socket::socket_stub::StubConnection

*Struct*

**Fields:**
- `input: Vec<u8>`
- `output: Vec<u8>`

**Trait Implementations:**

- **Default**
  - `fn default() -> StubConnection`



## handl_lib_os::socket::socket_stub::StubSocket

*Struct*

A simple in-memory socket service for tests.

**Fields:**
- `next_handle: std::sync::Mutex<i64>`
- `sockets: std::sync::Mutex<std::collections::HashMap<i64, StubConnection>>`
- `bindings: std::collections::HashMap<String, Vec<u8>>`

**Methods:**

- `fn new() -> Self`
- `fn with_binding<impl Into<String>, impl Into<Vec<u8>>>(address: impl Trait, data: impl Trait) -> Self`

**Trait Implementations:**

- **Socket**
  - `fn connect(self: &Self, address: &str) -> Result<i64, OsError>`
  - `fn send(self: &Self, handle: i64, data: &str) -> Result<usize, OsError>`
  - `fn recv(self: &Self, handle: i64, length: usize) -> Result<String, OsError>`
  - `fn close(self: &Self, handle: i64) -> Result<(), OsError>`
- **Default**
  - `fn default() -> StubSocket`



