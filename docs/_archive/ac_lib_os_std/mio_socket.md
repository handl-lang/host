**handl_lib_os_std > mio_socket**

# Module: mio_socket

## Contents

**Structs**

- [`MioSocket`](#miosocket) - Async socket implementation backed by `mio` on Unix.
- [`WaitForReady`](#waitforready) - Waits for a `mio` readiness event for a single connection, waking the

**Functions**

- [`io_error_to_os_error`](#io_error_to_os_error)

---

## handl_lib_os_std::mio_socket::MioSocket

*Struct*

Async socket implementation backed by `mio` on Unix.

**Fields:**
- `next_handle: std::sync::Mutex<i64>`
- `sockets: std::sync::Mutex<std::collections::HashMap<i64, std::sync::Arc<std::sync::Mutex<mio::net::TcpStream>>>>`

**Trait Implementations:**

- **AsyncSocket**
  - `fn connect<'a>(self: &'a Self, address: &'a str) -> Pin<Box<dyn Future>>`
  - `fn send<'a>(self: &'a Self, handle: i64, data: &'a str) -> Pin<Box<dyn Future>>`
  - `fn recv<'a>(self: &'a Self, handle: i64, length: usize) -> Pin<Box<dyn Future>>`
  - `fn close<'a>(self: &'a Self, handle: i64) -> Pin<Box<dyn Future>>`
- **Default**
  - `fn default() -> MioSocket`



## handl_lib_os_std::mio_socket::WaitForReady

*Struct*

Waits for a `mio` readiness event for a single connection, waking the
`std::task::Waker` from a background `mio::Poll` loop.

**Fields:**
- `conn: std::sync::Arc<std::sync::Mutex<mio::net::TcpStream>>`
- `interest: mio::Interest`
- `ready: std::sync::Arc<std::sync::atomic::AtomicBool>`
- `error: std::sync::Arc<std::sync::Mutex<Option<io::Error>>>`
- `started: bool`

**Methods:**

- `fn new(conn: Arc<Mutex<TcpStream>>, interest: Interest) -> Self`

**Trait Implementations:**

- **Future**
  - `fn poll(self: Pin<& mut Self>, cx: & mut Context) -> Poll<<Self as >::Output>`



## handl_lib_os_std::mio_socket::io_error_to_os_error

*Function*

```rust
fn io_error_to_os_error(error: io::Error) -> handl_lib_os::OsError
```



