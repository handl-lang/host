---
title: String / Char / Unicode Layering
type: adr
date: 2026-09-07
status: accepted
---

# ADR-0093: String / Char / Unicode Layering

## Status

Accepted — retroactively documented after the implementation already landed on
main. The code described here predates this ADR.

## Context

The `str`/`char`/`unicode` surface in handl has to work across runtimes whose
string representations differ (Rust `String`/UTF-8, JS UTF-16, future
backends). `str` is also the most common standard-library surface, so the
implementation must be maintainable and the semantics must not be locked to one
FFI backend.

This ADR records the three-layer design that replaced the former all-FFI
`StrModule` (`plugin-lib-utils`). It is intentionally the same shape used for
`encoding`, `url`, `ws`, and other platform abstractions:

- a backend-neutral `.hnl` contract,
- a pure-handl canonical implementation,
- one or more runtime-specific native backends.

This document is retroactive; the code was merged before the ADR was written.

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

## Consequences

### Positive

- String algorithms are backend-neutral and portable. Swapping the `char`
  backend changes Unicode/ASCII semantics without touching `str-handl`.
- The `str` contract in `str.hnl` carries laws and parseable guarantees for the
  refinement system, not just `True`.
- Unicode tables come from the host's ICU4C — the same library `intl-c`
  already binds — instead of a vendored Rust crate, so the `hnl` binary no
  longer links ICU4X. The contract's exact subset keeps working on hosts
  without ICU.

### Negative / non-obvious runtime enablers

The split forced four runtime changes that are part of this decision, not
incidental bugs. Anyone editing `field_access`, the module chain, or arrow
application must preserve them.

1. **`eval.field-access` is `source: Native`.** The `field_access` op in
   `plugin-engine-exec-tree-walk` operates on `TaggedValue::Obj`/`Arr` in the
   engine. It must not be a `!Foreign` host call: passing a record of closures
   through `serde_json` recursively serialized every `Arrow`'s captured
   environment, causing exponential allocation and OOM at ~20 bindings.

2. **`TreeWalkEngine::eval` saves and restores thread-locals.** It saves and
   restores `CURRENT_AST`, `CURRENT_LANG`, `CURRENT_MODULES`, and
   `CURRENT_GLOBALS` around a nested `Runtime::run`. This lets a `.hnl` module
   function call another `HandlFunction` (e.g. `dedent` → `str.*` →
   `str-handl` → `char.*`) without clobbering the outer runtime's frame.

3. **`HandlFunction` caches a per-thread `Rc<Runtime>`.** The `Rc` is cloned
   out of the `RefCell` before `run` so nested module calls do not hit a
   `RefCell` re-borrow panic.

4. **Per-call globals and the `__home__` frame for arrow values crossing the
   boundary.** `HandlFunction::call` binds arguments as `__arg_N` globals
   instead of serializing them into the applied source text, so closures,
   tagged values, and other reserved-key values cross as real values. Arrows
   that cross are stamped with `__home__` using the tree-walk home-frame
   registry so `apply_app` / `call_value` route the `body` index back to the
   originating AST, Language, and module chain. `apply_app` also curries
   surplus arguments (`x ~> y ~> body` applied to two args applies `x`, then
   applies the result to `y`).

## Update — the `unicode` backend moved from ICU4X to ICU4C

The `unicode` implementation described in §1 was replaced. The contract
(`handl-packages/unicode/unicode.hnl`) is unchanged in *meaning*; the backend,
the module name, and the crate set are not.

**What changed**

- `handl-packages/unicode-c/unicode-c.hnl` is the new backend: a pure-handl
  module that calls the host's ICU4C common library (`libicuuc`) through the
  generic FFI surface (`ffi.call` / `ffi.load` / `ffi.sym`). Its ICU4C bindings
  are sdkgen-generated (`generated/icu-uc.sdkgen.yaml` →
  `generated/icu-uc/` port + `generated/icu-uc-c-linux-x86_64/` adapter), the
  same pipeline `intl-c` uses, so ICU's `urename.h` version suffix
  (`unorm2_normalize_78`) is handled by regeneration rather than hand-written
  symbol tables.
- The runtime module is named **`unicode_c`**, not `unicode`. This mirrors
  `intl`/`intl_c`: the contract package name is not the module name. Importers
  write `from "unicode_c" import * as u`. This is a **breaking surface change**
  for any caller of the old `unicode` module.
- Retired: `crates/plugin-lib-unicode-native` (ICU4X-backed ops),
  `handl-packages/unicode-handl` (the coarse pure-handl fallback), and
  `crates/plugin-lib-unicode` (`UnicodeLibraryPlugin`, the kit
  `LibraryRegistry`'s `unicode` library). Nothing in the module chain links
  ICU4X any more.

**Why**

- One ICU dependency instead of two: `intl-c` already bound ICU4C, so the
  tables now come from the library the host ships rather than a vendored Rust
  crate compiled into `hnl`.
- The coarse fallback was the wrong trade. `unicode-handl` returned
  code-point-level `graphemes`/`width`/`casefold` — silently wrong off ASCII
  (a ZWJ family emoji as five clusters, `width("日本") == 2`). `unicode-c`
  instead keeps the *exact* subset (`code_points`, `from_code_points`,
  `is_ascii`, `code_point_at`, `decode`) working everywhere and raises
  `icu-unavailable` for the table-dependent ops, which matches this ADR's own
  "fails loudly rather than returning wrong data" principle.
- It removes the contract/impl/native trio asymmetry: `unicode` is now the
  same contract + C-backend shape as `intl`, `time`, and the other
  ICU/platform abstractions.

**Consequences for the invariants above**

- §2's "fails loudly" bullet now covers *all* table-dependent ops, not just
  `nfc`/`nfd`.
- §3's registration bullet describes `unicode_c` (see above); the `unicode`
  library entry is gone from the `LibraryRegistry`.
- The `config` kit lists `unicode_c` and declares a `lib.unicode-c` cap, the
  same way it lists `intl_c` and `lib.intl-c`.
