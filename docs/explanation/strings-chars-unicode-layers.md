# Why strings, chars, and Unicode live in three layers

Working with text looks like one problem. It is three. They have different natures, and conflating them is how string libraries become unmaintainable. This explanation sets out why the handl string stack is split into `char/`, `str/`, and `unicode/`, and why only one of those layers is allowed to contain Rust.

## The three layers

| Layer | What it is | Nature | Where it lives |
| ----- | ---------- | ------ | -------------- |
| **1. Algorithms** | trim, replace, split, levenshtein, word_wrap, pad, strip, slice, reverse | Pure control flow over a sequence of characters. No data required. | `str-handl/` — pure handl. |
| **2. Unicode tables** | "is `é` a letter?", "uppercase of `ß`?", "is `\u200b` whitespace?" | Not algorithms. Lookups into the Unicode Character Database (UCD): ~60&nbsp;KB of property tables the Unicode Consortium publishes and revises per version. | `char-unicode-std/` — Rust FFI over `std::char`. This is the irreducible floor. |
| **3. ICU (full i18n)** | Locale-sensitive collation (German `ä` sorts as `ae` in phonebooks), Turkish `i` → `İ`, NFC/NFD normalisation, grapheme cluster segmentation (`👨‍👩‍👧` is 5 codepoints but 1 user-perceived character), UAX #14 line breaking, number/date/plural formatting | Lookups into CLDR locale data plus UCD, often tens of MB. Rust's `std` does none of this. | `char-icu-std/` — Rust FFI over ICU4X. A future, swappable backend. |

The split is not stylistic. Each layer has a different *kind of thing* in it: an algorithm, a database lookup, a larger database lookup. Writing an algorithm in Rust when it could be handl hides it from the refinement engine and forces an engine edit for every behaviour change. Writing a database lookup in handl embeds tens of kilobytes of data as code, with no way to version it against Unicode releases.

## Why algorithms belong in handl

`trim`, `replace`, `split`, `levenshtein`, `word_wrap` and the rest are *just control flow* over a character sequence. Given two primitives — "give me character N" and "how many characters are there?" — the algorithm is expressible in any Turing-complete language. There is nothing in `replace` that depends on Rust's `String`, and nothing in `levenshtein` that depends on the UCD.

Keeping them in handl has three concrete payoffs:

- **They become data, not engine code.** A change to `word_wrap`'s break heuristic is a `.hnl` edit, not a Rust recompile (ADR-0044, "language as data").
- **They are visible to the contract engine.** The axioms in `str/str.hnl` (length preservation, `NoOccurrence` after `replace`, `IsPerm` for `reverse`) can be checked against the handl that implements them. A Rust `Function` impl is opaque to refinement.
- **They compose for free.** `slugify` is `lower` + `replace` + `trim_matches` + collapse-dashes. In handl that is literally those calls; in Rust each is a reimplementation.

## Why the Unicode floor cannot be handl

Consider `str.upper("straße")`. The correct result is `"STRASSE"` — `ß` uppercases to `SS`. This is not derivable. There is no rule, no computation; it is a fact recorded in `UnicodeData.txt`. The same is true for `is_whitespace('\u200b')` (zero-width space — yes, it is whitespace), `is_alphabetic('é')` (yes), and every other per-character property.

These facts live in the UCD, which the Unicode Consortium revises for each version of the standard. Rust's `std::char` embeds a generated subset of the UCD at Rust build time. A handl implementation would have to carry those tables as handl literals — a ~60&nbsp;KB data file masquerading as source, with no mechanism to track Unicode versions, no test against the conformance data, and a runtime cost dominated by interpretation overhead on every lookup.

This is why `char/` is the floor: it is the layer where the only honest implementation is a native call into a curated database. The axioms in `char/` declare the *contract* (e.g. `char.to_upper` guarantees `IsUpper(output) ∧ LengthGe(output, input)`); the Rust in `char-unicode-std/` satisfies it by calling `char::to_uppercase`.

## Why ICU is a separate layer above the std floor

Rust's `std::char` does *root-locale* Unicode: basic case mapping, basic whitespace, basic category. It does not do:

- **Locale-sensitive case folding.** Turkish `i` uppercases to `İ` (dotted capital), not `I`. Azeri likewise. Root locale gives `I`, which is wrong for those locales.
- **Normalisation.** `é` can be one codepoint (`U+00E9`) or two (`e` + `U+0301` combining acute). `equals` and `find` over un-normalised text silently fail. NFC/NFD/NFKC/NFKD fix this; `std` does not provide them.
- **Grapheme cluster segmentation.** `👨‍👩‍👧` is one user-perceived character, five codepoints (man, ZWJ, woman, ZWJ, girl). `str.length` counting codepoints returns 5; a user expects 1. `word_wrap` breaking mid-cluster is wrong.
- **UAX #14 line breaking.** The current `word_wrap` breaks on ASCII whitespace. Real line breaking is a Unicode algorithm with thousands of break classes.
- **Collation, number/date/plural formatting.** Out of scope for `std` entirely; these are CLDR territory.

`char-unicode-std/` (Rust `std::char`) is correct for root-locale, codepoint-level work — which is exactly what the existing Rust `str` impl did. `char-icu-std/` (ICU4X) is the upgrade path for production i18n. The point of making them separate, swappable backends behind the same `char/` contract is below.

## Why the floor is swappable

`str-handl/` never calls `std::char` or ICU directly. It calls `char.is_whitespace(c)`, `char.to_upper(c)`, `char.code_point_at(s, i)`. Those are the atoms declared in `char/str.hnl` (axioms). Today `char-unicode-std/` satisfies them with `std::char`. Tomorrow `char-icu-std/` can satisfy the same axioms with ICU4X.

**Upgrading the floor upgrades every handl algorithm for free.** Swap `char-unicode-std/` for `char-icu-std/` and, with no change to `str-handl/`:

- `str.upper` becomes locale-aware (Turkish `i` → `İ` when a locale is supplied).
- `str.word_wrap` becomes UAX #14-correct.
- `str.length` can become grapheme-aware (if the `char/` axioms gain a `grapheme_count` atom).
- `unicode-handl/`'s `normalize` becomes real instead of a no-op stub.

This is the payoff of the layering. The Rust surface area is minimised to one swappable package; everything above is handl that inherits correctness from the floor.

## What this means for "pure handl"

"Pure handl" is a claim about a *package*, not the whole stack. `str-handl/` is pure handl — every line is handl. `unicode-handl/` is pure handl. The stack as a whole is not, and cannot be, pure handl, because Unicode property data is irreducible. The honest statement is:

> Every string *algorithm* in handl is handl. The only Rust in the string stack is per-character Unicode data lookup, isolated in one swappable package.

That is the strongest defensible position, and it is what the `char/` floor enforces.

## The deletion consequence

Before this layering, `utils/str.hnl` and `crates/plugin-lib-utils/src/str/ops.rs` held all 45 string functions as Rust `Function` impls — algorithms and Unicode lookups interleaved (`c.is_alphabetic()` inlined inside `IsAlpha`, `c.to_uppercase()` inside `Upper`). That code is deleted in the move:

- **Deleted:** the 45 `rust_str_*` algorithm impls (trim, replace, levenshtein, ...). They become `str-handl/`.
- **Extracted, not deleted:** the per-character Unicode lookups inlined inside them. They become the `char.*` atoms in `char-unicode-std/`, the floor.
- **Deleted:** `utils/str.hnl` itself, once `str-handl/` owns the `"str"` module registration.

What remains in Rust is the smallest possible string-related surface: `char.*` atoms backed by `std::char`, in one package, replaceable by ICU4X without touching a line of handl.

## Related

- [The arrow calculus kernel is closed](./arrow-calculus-kernel) — the same "minimise the irreducible surface" principle, applied to the calculus.
- [Platform architecture](./platform-architecture) — where `char/`, `str/`, and `unicode/` sit in the crate layers.
- [Plugins, rules, and operations](./plugins-rules-operations) — how `op`/`handl` declarations and `OpRegistry` dispatch work, which is how `char-unicode-std/`'s Rust atoms are exposed.
