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
| `unicode` handl impl     | `handl-packages/unicode-handl/unicode-handl.hnl` | Code-point-level fallbacks over `char.*`; `nfc`/`nfd` intentionally omitted.                        |
| `unicode` native backend | `crates/plugin-lib-unicode-native`               | ICU4X-backed native `unicode.*` ops, composed with `unicode-handl` as fallback.                     |

There is also `crates/plugin-lib-unicode` (no `-native`): a lightweight
`UnicodeLibraryPlugin` used by the kit `LibraryRegistry` for non-canonical
setups. It exposes `unicode.decode` / `unicode.code_points` /
`unicode.is_valid_identifier` and friends without the ICU4X table. The
canonical runtime module, however, is `plugin-lib-unicode-native::UnicodeModule`.

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
- `unicode-handl` provides only the subset computable from `char.*`. Table-
  dependent ops (`nfc`, `nfd`) are left out of the record so a pure-handl
  backend fails loudly rather than returning un-normalized text.

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
- `"unicode"` resolves to `plugin-lib-unicode-native::UnicodeModule`; native
  `unicode.*` ops take precedence, and any name not implemented natively falls
  through to the `unicode-handl` record.

## Consequences

### Positive

- String algorithms are backend-neutral and portable. Swapping the `char`
  backend changes Unicode/ASCII semantics without touching `str-handl`.
- The `str` contract in `str.hnl` carries laws and parseable guarantees for the
  refinement system, not just `True`.
- Kits that do not need the ICU4X dependency can use the lighter
  `plugin-lib-unicode` library, while the canonical runtime gets full
  `unicode-native`.

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
