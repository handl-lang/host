**handl_lib_os_std > socket**

# Module: socket

## Contents

**Structs**

- [`StdSocket`](#stdsocket) - Socket implementation backed by [`std::net::TcpStream`].

**Functions**

- [`io_error_to_os_error`](#io_error_to_os_error)

**Constants**

- [`DEFAULT_TIMEOUT`](#default_timeout)

---

## handl_lib_os_std::socket::DEFAULT_TIMEOUT

*Constant*: `std::time::Duration`



## handl_lib_os_std::socket::StdSocket

*Struct*

Socket implementation backed by [`std::net::TcpStream`].

**Fields:**
- `next_handle: std::sync::Mutex<i64>`
- `sockets: std::sync::Mutex<std::collections::HashMap<i64, std::net::TcpStream>>`

**Trait Implementations:**

- **Socket**
  - `fn connect(self: &Self, address: &str) -> Result<i64, OsError>`
  - `fn send(self: &Self, handle: i64, data: &str) -> Result<usize, OsError>`
  - `fn recv(self: &Self, handle: i64, length: usize) -> Result<String, OsError>`
  - `fn close(self: &Self, handle: i64) -> Result<(), OsError>`
- **Default**
  - `fn default() -> StdSocket`



## handl_lib_os_std::socket::io_error_to_os_error

*Function*

```rust
fn io_error_to_os_error(error: io::Error) -> handl_lib_os::OsError
```



