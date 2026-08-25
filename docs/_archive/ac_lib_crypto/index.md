# handl_lib_crypto

# handl-lib-crypto

Cryptographic hashing and encoding library for the handl runtime.

Provides functions for computing SHA-256 and MD5 hashes, and for
Base64 and hex encoding/decoding.

## Architecture

```text
handl-runtime-core          handl-lib-crypto
─────────────            ─────────────
Module trait             CryptoModule (Copy, zero-alloc)
ModuleHost               *Function structs
                         CryptoError → VmError
```

All function structs are `Copy` with no internal state.

## Functions

- [`CryptoModule`] — `crypto.sha256`, `crypto.md5`,
  `crypto.base64_encode`, `crypto.base64_decode`,
  `crypto.hex_encode`, `crypto.hex_decode`

## Input conventions

Hashing and encoding functions accept either:
- A **string** — treated as UTF-8 bytes
- An **array of integers** — each value must be in the range 0–255

Decode functions (`base64_decode`, `hex_decode`) return an array of
integers (0–255) since the decoded data may not be valid UTF-8.

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_crypto::CryptoModule;

let host = ModuleHost::new()
    .with(CryptoModule);
```

Expressions can then call `crypto.sha256("hello")`,
`crypto.md5("hello")`, `crypto.base64_encode("hello")`,
`crypto.base64_decode("aGVsbG8=")`, `crypto.hex_encode("hello")`,
`crypto.hex_decode("68656c6c6f")`, etc.

## Modules

### [`handl_lib_crypto`](handl_lib_crypto.md)

*5 modules*

### [`common`](common.md)

*4 functions*

### [`encoding`](encoding.md)

*4 structs*

### [`error`](error.md)

*1 enum*

### [`hash`](hash.md)

*2 structs*

### [`module`](module.md)

*1 struct*

