**handl_lib_ip > socket**

# Module: socket

## Contents

**Structs**

- [`SocketAddr`](#socketaddr) - A socket address (IP + port).

---

## handl_lib_ip::socket::SocketAddr

*Struct*

A socket address (IP + port).

**Fields:**
- `ip: crate::IpAddr`
- `port: u16`

**Methods:**

- `fn parse(s: &str) -> Result<Self, IpError>` - Parse a string such as `127.0.0.1:8080` or `[::1]:443`.
- `fn ip(self: Self) -> IpAddr` - The IP address.
- `fn port(self: Self) -> u16` - The port number.
- `fn is_loopback(self: Self) -> bool` - Whether the IP address is a loopback address.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &SocketAddr) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> SocketAddr`



