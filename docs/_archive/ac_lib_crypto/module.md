**handl_lib_crypto > module**

# Module: module

## Contents

**Structs**

- [`CryptoModule`](#cryptomodule) - The `crypto` module — hashing and encoding functions.

---

## handl_lib_crypto::module::CryptoModule

*Struct*

The `crypto` module — hashing and encoding functions.

Provides functions callable as `crypto.sha256(data)`,
`crypto.md5(data)`, `crypto.base64_encode(data)`,
`crypto.base64_decode(encoded)`, `crypto.hex_encode(data)`,
`crypto.hex_decode(encoded)` in handl expressions.

Hashing functions accept a string (UTF-8 bytes) or an array of
integers (0–255) and return a lowercase hex-encoded digest.

Encoding functions accept the same input types for encode operations
and return a string. Decode operations accept an encoded string and
return an array of integers (0–255).

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> CryptoModule`



