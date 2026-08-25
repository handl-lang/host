# handl_lib_ip

# handl-lib-ip

IP address, CIDR, and socket utilities for the handl runtime.

Provides ergonomic, zero-allocation-by-default wrappers around IPv4,
IPv6, CIDR prefixes, and socket addresses, with strict validation and
lossless round-trip parsing.

## Usage

```ignore
use handl_lib_ip::{IpAddr, Ipv4, Ipv6, Cidr, SocketAddr};

let v4 = Ipv4::parse("192.168.1.1")?;
let v6 = Ipv6::parse("::1")?;
let cidr = Cidr::parse("10.0.0.0/8")?;
let sock = SocketAddr::parse("[::1]:8080")?;
```

## Modules

### [`handl_lib_ip`](handl_lib_ip.md)

*4 modules*

### [`addr`](addr.md)

*1 enum, 2 functions, 2 structs*

### [`cidr`](cidr.md)

*1 enum, 2 functions, 2 structs*

### [`error`](error.md)

*1 enum*

### [`socket`](socket.md)

*1 struct*

