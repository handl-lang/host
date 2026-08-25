**handl_lib_unicode > decode**

# Module: decode

## Contents

**Structs**

- [`Utf8Iterator`](#utf8iterator) - Iterator that decodes a UTF-8 byte slice into `(usize, char)` pairs,

**Enums**

- [`DecodeError`](#decodeerror) - Error produced when a byte sequence is not valid Unicode.

**Functions**

- [`decode_utf8`](#decode_utf8) - Decode an entire UTF-8 byte slice into a `Vec<char>`.

---

## handl_lib_unicode::decode::DecodeError

*Enum*

Error produced when a byte sequence is not valid Unicode.

**Variants:**
- `InvalidContinuation{ offset: usize }` - The byte at `offset` is not a valid UTF-8 continuation byte.
- `TruncatedSequence{ offset: usize }` - The byte at `offset` begins a multi-byte sequence that is truncated.
- `InvalidCodePoint{ offset: usize }` - The byte at `offset` is an overlong or otherwise invalid code point.
- `InvalidScalar{ offset: usize }` - A 32-bit value is outside the Unicode scalar range or a surrogate.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DecodeError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &DecodeError) -> bool`



## handl_lib_unicode::decode::Utf8Iterator

*Struct*

Iterator that decodes a UTF-8 byte slice into `(usize, char)` pairs,
where the `usize` is the starting byte offset of the scalar value.

**Generic Parameters:**
- 'a

**Fields:**
- `bytes: &'a [u8]`
- `offset: usize`

**Methods:**

- `fn new(bytes: &'a [u8]) -> Self` - Create a new iterator over the provided UTF-8 bytes.
- `fn next_scalar(self: & mut Self) -> Result<Option<(usize, char)>, DecodeError>` - Returns the next valid `(offset, char)` pair or the first decode error.

**Trait Implementations:**

- **Iterator**
  - `fn next(self: & mut Self) -> Option<<Self as >::Item>`
- **Clone**
  - `fn clone(self: &Self) -> Utf8Iterator<'a>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_unicode::decode::decode_utf8

*Function*

Decode an entire UTF-8 byte slice into a `Vec<char>`.

Returns the first error encountered, along with the byte offset where the
error occurred.

```rust
fn decode_utf8(bytes: &[u8]) -> Result<Vec<char>, (usize, DecodeError)>
```



