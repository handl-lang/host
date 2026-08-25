**handl_lib_compress_gzip > gzip**

# Module: gzip

## Contents

**Structs**

- [`Gzip`](#gzip) - Gzip compression/decompression codec.

---

## handl_lib_compress_gzip::gzip::Gzip

*Struct*

Gzip compression/decompression codec.

**Unit Struct**

**Methods:**

- `fn new() -> Self`

**Traits:** Copy

**Trait Implementations:**

- **Compressor**
  - `fn compress(self: &Self, input: &[u8]) -> Result<Vec<u8>, CompressError>`
- **Decompressor**
  - `fn decompress(self: &Self, input: &[u8]) -> Result<Vec<u8>, CompressError>`
- **Clone**
  - `fn clone(self: &Self) -> Gzip`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> Gzip`



