---
title: String / Char / Unicode Layering
type: adr
date: 2026-09-07
status: accepted
status_note: |-
  Accepted — retroactively documented after the implementation already landed on
  main. The code described here predates this ADR.
---


# ADR-0093: String / Char / Unicode Layering

## Decision


### 1. Three layers: contract, pure-handl implementation, native backend

| Layer                    | File / crate                                     | Role                                                                                                |
| ------------------------ | ------------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| `str` contract           | `handl-packages/str/str.hnl`                     | 45 axioms + laws; no implementation; `String`/`Float` are treated as type variables.                |
| `char` contract          | `handl-packages/char/char.hnl`                   | 18 axioms; semantic names (`is_whitespace`, `is_alpha`, …) with no encoding or byte-offset leakage. |
| `str` canonical impl     | `handl-packages/str-handl/str-handl.hnl`         | Pure handl over `char.*`; `kit-canonical` registers it as the module named `"str"`.                 |
| `char` native backend    | `crates/plugin-lib-char-native`                  | Rust `std::char` Unicode scalar-value backend for `char.*` ops.                                     |
| `unicode` contract       | `handl-packages/unicode/unicode.hnl`             | Graphemes / normalization / casefold / width (UAX #29/#15/#11).                                     |
| `unicode` backend        | `handl-packages/unicode-c/unicode-c.hnl`         | Pure-handl module over the host's ICU4C (`libicuuc`) via the generic FFI surface.                    |

`unicode-c` is the `unicode` analogue of `intl-c` — a *contract + C backend*
pair rather than a contract + pure-handl + native trio. It splits by need:
`code_points` / `from_code_points` / `is_ascii` / `code_point_at` / `decode`
are pure handl and exact on any host, while `graphemes` / `grapheme_count`
(`ubrk_*`, UAX #29), `nfc` / `nfd` (`unorm2_*`, UAX #15), `casefold`
(`u_strFoldCase`), the XID predicates (`u_hasBinaryProperty`, UAX #31), and
`width` (`u_getIntPropertyValue` East Asian Width + `u_charType`, UAX #11)
call ICU4C. `available` / `icu_version` report probe results, and the
table-dependent ops raise `icu-unavailable` on a host without ICU.

**Superseded (see "Update" below):** the ICU4X-backed native
`crates/plugin-lib-unicode-native`, the coarse pure-handl
`handl-packages/unicode-handl` fallback, and the lightweight Rust
`crates/plugin-lib-unicode` (`UnicodeLibraryPlugin`) that the kit
`LibraryRegistry` exposed as the `unicode` library.

### 2. Invariants that the split preserves

- `str-handl` calls only `char.*` semantic atoms. It never indexes bytes,
  manipulates UTF-8, or assumes a particular encoding. This lets the same
  source run over `char-native`, `char-ascii`, `char-js`, or a future
  `char-icu` backend.
- `char` names are _semantic_: `is_whitespace`, `is_alpha`, `to_upper`, etc.
  The contract deliberately never mentions `is_ascii_*`, `utf8_at`, or byte
  offsets.
- Indices in `char` and `str` are character (scalar) indices, not byte offsets.
- `char.chars` and `char.from_chars` are the bulk conversion atoms. They make
  string iteration `O(n)` total instead of `O(n^2)` through repeated `char.at`.
- `unicode-c` provides the subset computable from `char.*` alone exactly, and
  delegates every table-dependent op to ICU4C. There is deliberately **no
  coarse fallback**: a code-point-level `graphemes` or `width` is silently
  wrong off ASCII, so on a host without ICU the table-dependent ops fail
  loudly instead of returning plausible-looking wrong data.

### 3. Canonical registration and fallback ordering

- `kit-canonical` resolves the module name `"str"` to `str-handl` through its
  `StdlibResolver` before the all-FFI `str` table row in `plugin-lib-stdlib`
  (`ops::strs`) is reached. `str.*` and `from "str" import` therefore resolve
  to the pure-handl implementation in canonical kits.
- Non-canonical kits that do not wire the `char` resolver still get the FFI
  `str` ops through the `utils` package table, keeping the old behavior as a
  fallback.
- `"char"` resolves to `CharModule` (`plugin-lib-char-native`) directly in
  `kit-canonical`.
- `"unicode_c"` resolves to the `unicode_c` table module (`unicode-c.hnl`),
  registered by `kit-canonical`'s module chain like `intl_c`. There is no
  fallback record: every name in the contract is implemented by `unicode-c`,
  and the table-dependent ones raise when the host has no ICU.
- The module name is `unicode_c`, **not** `unicode` (a deliberate surface
  change mirroring `intl`/`intl_c` — the contract package name is not the
  runtime module name). `unicode_c`'s own bare-name imports (`result`,
  `bytes`) and its `../generated/icu-uc-c-linux-x86_64/*` adapter imports
  resolve through that same chain.


## Why

The `str`/`char`/`unicode` surface in handl has to work across runtimes whose string representations differ (Rust `String`/UTF-8, JS UTF-16, future backends). `str` is also the most common standard-library surface, so the implementation must be maintainable and the semantics must not be locked to one FFI backend.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- String algorithms are backend-neutral and portable. Swapping the `char`
- The `str` contract in `str.hnl` carries laws and parseable guarantees for the
- Unicode tables come from the host's ICU4C — the same library `intl-c`

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0093-string-char-unicode-layering.md`](../design/0093-string-char-unicode-layering.md).
