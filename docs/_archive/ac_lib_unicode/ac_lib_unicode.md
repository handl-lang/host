**handl_lib_unicode**

# Module: handl_lib_unicode

## Contents

**Modules**

- [`decode`](#decode) - Source-text decoding for handl language input.
- [`ident`](#ident) - Identifier validation for the handl language.
- [`module`](#module) - The `unicode` handl runtime module.

---

## Module: decode

Source-text decoding for handl language input.

Exposes low-level helpers for converting raw byte streams into a sequence
of Unicode scalar values, reporting the exact byte offset where an
invalid encoding is detected.



## Module: ident

Identifier validation for the handl language.

Uses the Unicode `XID_Start` and `XID_Continue` properties defined in
[UAX #31](https://unicode.org/reports/tr31/) to decide whether a scalar
value may appear in a language identifier.



## Module: module

The `unicode` handl runtime module.

Exposes Unicode helpers callable from handl expressions:

- `unicode.is_valid_identifier(string)` — `bool`
- `unicode.is_xid_start(string)` — `bool`
- `unicode.is_xid_continue(string)` — `bool`
- `unicode.code_points(string)` — array of numeric code points
- `unicode.code_point_at(string, index)` — number or `null`
- `unicode.decode(string)` — array of `[byte_offset, code_point]` pairs



