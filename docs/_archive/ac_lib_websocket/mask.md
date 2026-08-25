**handl_lib_websocket > mask**

# Module: mask

## Contents

**Functions**

- [`apply`](#apply) - Apply a 32-bit masking key to a payload in-place.

---

## handl_lib_websocket::mask::apply

*Function*

Apply a 32-bit masking key to a payload in-place.

This is the XOR masking algorithm described in RFC 6455 Section 5.3.

```rust
fn apply(payload: & mut [u8], key: [u8; 4])
```



