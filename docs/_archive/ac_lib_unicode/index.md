# handl_lib_unicode

# handl-lib-unicode

Unicode primitives for the handl language toolchain and runtime.

Provides a focused, compiler-friendly API over Unicode data:

- [`decode`] — validate and decode UTF-8/UTF-16/UTF-32 byte streams into
  `char` iterators, with byte offsets for diagnostic positions.
- [`ident`] — identifier validation using `XID_Start` / `XID_Continue`
  (UAX #31), suitable for lexers and parsers.

Also exposes the `unicode` handl runtime module via [`UnicodeModule`].

Future work: grapheme cluster iteration, normalization, and case folding.

## Modules

### [`handl_lib_unicode`](handl_lib_unicode.md)

*3 modules*

### [`decode`](decode.md)

*1 enum, 1 function, 1 struct*

### [`ident`](ident.md)

*3 functions*

### [`module`](module.md)

*5 functions, 7 structs*

