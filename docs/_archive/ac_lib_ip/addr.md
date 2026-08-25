**handl_lib_ip > addr**

# Module: addr

## Contents

**Structs**

- [`Ipv4`](#ipv4) - An IPv4 address.
- [`Ipv6`](#ipv6) - An IPv6 address.

**Enums**

- [`IpAddr`](#ipaddr) - An IP address that may be IPv4 or IPv6.

**Functions**

- [`expand_ipv6`](#expand_ipv6)
- [`longest_zero_run`](#longest_zero_run)

---

## handl_lib_ip::addr::IpAddr

*Enum*

An IP address that may be IPv4 or IPv6.

**Variants:**
- `V4(Ipv4)`
- `V6(Ipv6)`

**Methods:**

- `fn parse(s: &str) -> Result<Self, IpError>` - Parse an IPv4 or IPv6 address string.
- `fn is_loopback(self: Self) -> bool` - `127.0.0.1` or `::1`.
- `fn is_multicast(self: Self) -> bool` - Whether the address is in a multicast range.
- `fn is_unspecified(self: Self) -> bool` - `0.0.0.0` or `::`.
- `fn is_private(self: Self) -> bool` - Whether the address is in a private IPv4 range.
- `fn is_link_local(self: Self) -> bool` - Whether the address is an IPv4 link-local address.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &IpAddr) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> IpAddr`



## handl_lib_ip::addr::Ipv4

*Struct*

An IPv4 address.

**Tuple Struct**: `(u32)`

**Methods:**

- `fn new(a: u8, b: u8, c: u8, d: u8) -> Self` - Construct from four octets.
- `fn parse(s: &str) -> Result<Self, IpError>` - Parse a dotted-decimal address.
- `fn octets(self: Self) -> [u8; 4]` - Return the four octets.
- `fn to_u32(self: Self) -> u32` - Pack the address into a `u32` in network byte order.
- `fn from_u32(value: u32) -> Self` - Construct from a `u32` in network byte order.
- `fn is_loopback(self: Self) -> bool` - `127.0.0.0/8`.
- `fn is_private(self: Self) -> bool` - `10.0.0.0/8`, `172.16.0.0/12`, or `192.168.0.0/16`.
- `fn is_link_local(self: Self) -> bool` - `169.254.0.0/16`.
- `fn is_multicast(self: Self) -> bool` - `224.0.0.0/4`.
- `fn is_unspecified(self: Self) -> bool` - `0.0.0.0`.
- `fn is_broadcast(self: Self) -> bool` - `255.255.255.255`.

**Traits:** Copy, Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Ipv4`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Ipv4) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_ip::addr::Ipv6

*Struct*

An IPv6 address.

**Tuple Struct**: `(u128)`

**Methods:**

- `fn new(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> Self` - Construct from eight 16-bit segments.
- `fn parse(s: &str) -> Result<Self, IpError>` - Parse an IPv6 address in the usual textual notation.
- `fn from_segments(segments: [u16; 8]) -> Self` - Construct from eight 16-bit segments.
- `fn segments(self: Self) -> [u16; 8]` - Return the eight 16-bit segments.
- `fn to_u128(self: Self) -> u128` - Pack the address into a `u128` in network byte order.
- `fn from_u128(value: u128) -> Self` - Construct from a `u128` in network byte order.
- `fn is_loopback(self: Self) -> bool` - `::1`.
- `fn is_multicast(self: Self) -> bool` - `ff00::/8`.
- `fn is_unspecified(self: Self) -> bool` - `::`.

**Traits:** Copy, Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Ipv6`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Ipv6) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_ip::addr::expand_ipv6

*Function*

```rust
fn expand_ipv6(s: &str) -> Result<String, crate::IpError>
```



## handl_lib_ip::addr::longest_zero_run

*Function*

```rust
fn longest_zero_run(segments: &[u16; 8]) -> (usize, usize)
```



