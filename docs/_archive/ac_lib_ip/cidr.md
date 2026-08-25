**handl_lib_ip > cidr**

# Module: cidr

## Contents

**Structs**

- [`Cidr4`](#cidr4) - An IPv4 CIDR prefix.
- [`Cidr6`](#cidr6) - An IPv6 CIDR prefix.

**Enums**

- [`Cidr`](#cidr) - An IP CIDR that may be IPv4 or IPv6.

**Functions**

- [`cidr_mask_v4`](#cidr_mask_v4)
- [`cidr_mask_v6`](#cidr_mask_v6)

---

## handl_lib_ip::cidr::Cidr

*Enum*

An IP CIDR that may be IPv4 or IPv6.

**Variants:**
- `V4(Cidr4)`
- `V6(Cidr6)`

**Methods:**

- `fn parse(s: &str) -> Result<Self, IpError>` - Parse a string such as `10.0.0.0/8` or `2001:db8::/32`.
- `fn network(self: Self) -> IpAddr` - Network address for this CIDR.
- `fn broadcast(self: Self) -> IpAddr` - Last address for this CIDR.
- `fn contains(self: Self, ip: &IpAddr) -> bool` - Whether this CIDR contains the given address.
- `fn prefix(self: Self) -> u8` - The prefix length.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Cidr) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Cidr`



## handl_lib_ip::cidr::Cidr4

*Struct*

An IPv4 CIDR prefix.

**Fields:**
- `addr: crate::Ipv4`
- `prefix: u8`

**Methods:**

- `fn new(addr: Ipv4, prefix: u8) -> Result<Self, IpError>` - Construct a CIDR from an address and a prefix length.
- `fn parse(s: &str) -> Result<Self, IpError>` - Parse a string such as `192.168.0.0/24`.
- `fn network(self: Self) -> Ipv4` - Network address for this CIDR.
- `fn broadcast(self: Self) -> Ipv4` - Broadcast address for this CIDR.
- `fn address_count(self: Self) -> u64` - Total number of addresses in the CIDR.
- `fn host_count(self: Self) -> u64` - Number of usable host addresses (subtracts network and broadcast).
- `fn first_host(self: Self) -> Ipv4` - First usable host address.
- `fn last_host(self: Self) -> Ipv4` - Last usable host address.
- `fn contains(self: Self, ip: Ipv4) -> bool` - Whether the CIDR contains the given address.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Cidr4) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Cidr4`



## handl_lib_ip::cidr::Cidr6

*Struct*

An IPv6 CIDR prefix.

**Fields:**
- `addr: crate::Ipv6`
- `prefix: u8`

**Methods:**

- `fn new(addr: Ipv6, prefix: u8) -> Result<Self, IpError>` - Construct a CIDR from an address and a prefix length.
- `fn parse(s: &str) -> Result<Self, IpError>` - Parse a string such as `2001:db8::/32`.
- `fn network(self: Self) -> Ipv6` - Network address for this CIDR.
- `fn broadcast(self: Self) -> Ipv6` - Broadcast (last address) for this CIDR.
- `fn contains(self: Self, ip: Ipv6) -> bool` - Whether the CIDR contains the given address.

**Traits:** Copy, Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Cidr6`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Cidr6) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_ip::cidr::cidr_mask_v4

*Function*

```rust
fn cidr_mask_v4(prefix: u8) -> u32
```



## handl_lib_ip::cidr::cidr_mask_v6

*Function*

```rust
fn cidr_mask_v6(prefix: u8) -> u128
```



