**handl_lib_crypto > encoding**

# Module: encoding

## Contents

**Structs**

- [`Base64Decode`](#base64decode) - `crypto.base64_decode(encoded: string) → array<number>`
- [`Base64Encode`](#base64encode) - `crypto.base64_encode(data: string | array<number>) → string`
- [`HexDecode`](#hexdecode) - `crypto.hex_decode(encoded: string) → array<number>`
- [`HexEncode`](#hexencode) - `crypto.hex_encode(data: string | array<number>) → string`

---

## handl_lib_crypto::encoding::Base64Decode

*Struct*

`crypto.base64_decode(encoded: string) → array<number>`

Decodes a Base64 string and returns the raw bytes as an array of
integers in the range 0–255.

Returns an error if the input is not valid Base64.

Examples:
- `crypto.base64_decode("aGVsbG8=")` → `[104, 101, 108, 108, 111]`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Base64Decode`



## handl_lib_crypto::encoding::Base64Encode

*Struct*

`crypto.base64_encode(data: string | array<number>) → string`

Encodes the input to a Base64 string using standard padding.

The input may be a string (UTF-8 bytes) or an array of integers
in the range 0–255.

Examples:
- `crypto.base64_encode("hello")` → `"aGVsbG8="`
- `crypto.base64_encode([0, 1, 2])` → `"AAEC"`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Base64Encode`



## handl_lib_crypto::encoding::HexDecode

*Struct*

`crypto.hex_decode(encoded: string) → array<number>`

Decodes a hex-encoded string and returns the raw bytes as an array
of integers in the range 0–255.

Returns an error if the input contains non-hex characters or has
an odd length.

Examples:
- `crypto.hex_decode("68656c6c6f")` → `[104, 101, 108, 108, 111]`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> HexDecode`



## handl_lib_crypto::encoding::HexEncode

*Struct*

`crypto.hex_encode(data: string | array<number>) → string`

Encodes the input to a lowercase hex-encoded string.

The input may be a string (UTF-8 bytes) or an array of integers
in the range 0–255.

Examples:
- `crypto.hex_encode("hello")` → `"68656c6c6f"`
- `crypto.hex_encode([0, 255, 16])` → `"00ff10"`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> HexEncode`



