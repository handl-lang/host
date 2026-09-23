# Why strings, chars, and Unicode live in three layers

Working with text looks like one problem. It is three. They have different natures, and conflating them is how string libraries become unmaintainable. This explanation sets out why the handl string stack is split into `char/`, `str/`, and `unicode/`, and why only one of those layers contains Rust.

## The three layers

| Layer                  | What it is                                                                                                                                                                                                                                               | Nature                                                                                                                                                      | Where it lives                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| **1. Algorithms**      | trim, replace, split, levenshtein, word_wrap, pad, strip, slice, reverse                                                                                                                                                                                 | Pure control flow over a sequence of characters. No data required.                                                                                          | `str-handl/` — pure handl.                                                      |
| **2. Unicode tables**  | "is `é` a letter?", "uppercase of `ß`?", "is `\u200b` whitespace?"                                                                                                                                                                                       | Not algorithms. Lookups into the Unicode Character Database (UCD): property tables the Unicode Consortium publishes and revises per version.                 | `crates/plugin-lib-char-native` — Rust over `std::char`. This is the irreducible floor. |
| **3. ICU (full i18n)** | Locale-sensitive collation (German `ä` sorts as `ae` in phonebooks), Turkish `i` → `İ`, NFC/NFD normalisation, grapheme cluster segmentation (`👨‍👩‍👧` is 5 codepoints but 1 user-perceived character), UAX #14 line breaking, number/date/plural formatting | Lookups into CLDR locale data plus UCD, often tens of MB. Rust's `std` does none of this.                                                                   | `unicode-c/`, `intl-c/` — handl over the host's ICU4C via FFI.                   |

The split is not stylistic. Each layer has a different _kind of thing_ in it: an algorithm, a database lookup, a larger database lookup. Writing an algorithm in Rust when it could be handl hides it from the refinement engine and forces an engine edit for every behaviour change. Writing a database lookup in handl embeds tens of kilobytes of data as code, with no way to version it against Unicode releases.

## Why algorithms belong in handl

`trim`, `replace`, `split`, `levenshtein`, `word_wrap` and the rest are _just control flow_ over a character sequence. Given two primitives — "give me character N" and "how many characters are there?" — the algorithm is expressible in any Turing-complete language. There is nothing in `replace` that depends on Rust's `String`, and nothing in `levenshtein` that depends on the UCD.

Keeping them in handl has three concrete payoffs:

- **They become data, not engine code.** A change to `word_wrap`'s break heuristic is a `.hnl` edit, not a Rust recompile (ADR-0044, "language as data").
- **They are visible to the contract engine.** The axioms in `str/str.hnl` (length preservation, `NoOccurrence` after `replace`, `IsPerm` for `reverse`) can be checked against the handl that implements them. A Rust `Function` impl is opaque to refinement.
- **They compose for free.** `slugify` is `lower` + `replace` + `trim_matches` + collapse-dashes. In handl that is literally those calls; in Rust each is a reimplementation.

## Why the Unicode floor cannot be handl

Consider `str.upper("straße")`. The correct result is `"STRASSE"` — `ß` uppercases to `SS`. This is not derivable. There is no rule, no computation; it is a fact recorded in `UnicodeData.txt`. The same is true for `is_whitespace('\u200b')` (zero-width space — yes, it is whitespace), `is_alphabetic('é')` (yes), and every other per-character property.

These facts live in the UCD, which the Unicode Consortium revises for each version of the standard. Rust's `std::char` embeds a generated subset of the UCD at Rust build time. A handl implementation would have to carry those tables as handl literals — a data file masquerading as source, with no mechanism to track Unicode versions, no test against the conformance data, and a runtime cost dominated by interpretation overhead on every lookup.

This is why `char/` is the floor: it is the layer where the only honest implementation is a native call into a curated database. The axioms in `char/` declare the _contract_ (e.g. `char.to_upper` guarantees `IsUpper(output) ∧ LengthGe(output, input)`); the Rust in `plugin-lib-char-native` satisfies it by calling `char::to_uppercase`.

## Why ICU is a separate layer above the std floor

Rust's `std::char` does _root-locale_ Unicode: basic case mapping, basic whitespace, basic category. It does not do:

- **Locale-sensitive case folding.** Turkish `i` uppercases to `İ` (dotted capital), not `I`. Azeri likewise. Root locale gives `I`, which is wrong for those locales.
- **Normalisation.** `é` can be one codepoint (`U+00E9`) or two (`e` + `U+0301` combining acute). `equals` and `find` over un-normalised text silently fail. NFC/NFD/NFKC/NFKD fix this; `std` does not provide them.
- **Grapheme cluster segmentation.** `👨‍👩‍👧` is one user-perceived character, five codepoints (man, ZWJ, woman, ZWJ, girl). `str.length` counting codepoints returns 5; a user expects 1. `word_wrap` breaking mid-cluster is wrong.
- **UAX #14 line breaking.** The current `word_wrap` breaks on ASCII whitespace. Real line breaking is a Unicode algorithm with thousands of break classes.
- **Collation, number/date/plural formatting.** Out of scope for `std` entirely; these are CLDR territory.

That layer is reached through the host's **ICU4C**, not through a Rust crate linked into `hnl`. `unicode-c/` and `intl-c/` are pure-handl modules that call `libicuuc` / `libicui18n` through the generic FFI surface (`ffi.call` / `ffi.load` / `ffi.sym`), with sdkgen-generated bindings. The tables live in the operating system's ICU installation, so they track the platform's Unicode version and cost the binary nothing.

The alternative — linking ICU4X — was tried and retired. It compiled Unicode data into the `hnl` binary, duplicated a dependency the platform already ships for `intl`, and could not reuse the CLDR resources ICU4C exposes.

## Why the ICU layer is swappable

`unicode_c` is one implementation of the `unicode/` contract, not the contract itself. A `unicode-js` backend would satisfy the same axioms with `Intl.Segmenter`, `String.prototype.normalize`, and `\p{ID_Start}` regex classes; `str-handl/` and every caller stay unchanged.

The same holds one layer down: `str-handl/` never calls `std::char` or ICU directly. It calls `char.is_whitespace(c)`, `char.to_upper(c)`, `char.code_point_at(s, i)` — the atoms declared in `char/char.hnl`. Today `plugin-lib-char-native` satisfies them with `std::char`; another backend could satisfy the same axioms differently.

**Upgrading a backend upgrades everything above it for free.** Repoint the `unicode_c` registration at a different FFI backend and, with no change to callers:

- `unicode.graphemes` stays UAX #29-correct under a different segmenter.
- `str.word_wrap` can become UAX #14-correct once a line-break atom is exposed.
- `str.length` can become grapheme-aware (if the `char/` axioms gain a `grapheme_count` atom).

## What this means for "pure handl"

"Pure handl" is a claim about a _package_'s source, not about where its data comes from. `str-handl/` is pure handl — every line is handl. `unicode-c/` is pure handl too: it contains no Rust, and its table-dependent ops are `ffi.call` sites. The data those calls reach (ICU4C's tables) is not handl, and cannot be.

The honest statement is:

> Every string _algorithm_ in handl is handl. The only non-handl data in the string stack is per-character and text-level Unicode data, reached through the generic FFI surface from two backend packages (`unicode-c/`, `intl-c/`).

Two design rules follow from that, and both are enforced by the contract rather than by convention:

1. **No vendored tables.** A backend asks the host; it does not embed a Unicode release. `unicode-c` probes `available`/`icu_version` and reports what it found.
2. **No coarse fallback.** A code-point-level `graphemes` or `width` is silently wrong off ASCII (a ZWJ family emoji becomes five clusters; `width("日本")` becomes 2). `unicode-c` keeps the *exact* subset — `code_points`, `from_code_points`, `is_ascii`, `code_point_at`, `decode` — working without ICU and raises `icu-unavailable` for the rest. A loud failure beats plausible-looking wrong data.

## The deletion consequences

The layering has twice removed Rust that did not need to exist.

**First move.** `utils/str.hnl` and `crates/plugin-lib-utils/src/str/` held all 45 string functions as Rust `Function` impls — algorithms and Unicode lookups interleaved (`c.is_alphabetic()` inlined inside `IsAlpha`, `c.to_uppercase()` inside `Upper`). That code is gone:

- **Deleted:** the 45 `rust_str_*` algorithm impls (trim, replace, levenshtein, ...). They became `str-handl/`.
- **Extracted, not deleted:** the per-character Unicode lookups inlined inside them. They became the `char.*` atoms, the floor.
- **Deleted:** `utils/str.hnl` itself, once `str-handl/` owned the `"str"` module registration.

**Second move.** The Unicode backend was an ICU4X-backed Rust crate (`plugin-lib-unicode-native`) composed with a coarse pure-handl fallback (`unicode-handl`), plus a lightweight `plugin-lib-unicode` library for the kit registry. All three are gone:

- **Deleted:** `crates/plugin-lib-unicode-native` — the `unicode` ops are now `unicode-c`, handl over ICU4C.
- **Deleted:** `handl-packages/unicode-handl` — its exact subset moved into `unicode-c` as pure handl, and its coarse ops were the wrong trade (see above).
- **Deleted:** `crates/plugin-lib-unicode` — superseded by `unicode-c`'s pure-handl ops.

What remains in Rust for text is the smallest possible surface: the `char.*` atoms in one package, the generic FFI plugin that reaches the host's ICU, and the sdkgen-generated bindings.

## Related

- [String, char, and Unicode package reference](../reference/strings-chars-unicode) — the package and module inventory.
- [The arrow calculus kernel is closed](./arrow-calculus-kernel) — the same "minimise the irreducible surface" principle, applied to the calculus.
- [Platform architecture](./platform-architecture) — where `char/`, `str/`, and `unicode/` sit in the crate layers.
- [Plugins, rules, and operations](./plugins-rules-operations) — how `op`/`handl` declarations and `OpRegistry` dispatch work.
