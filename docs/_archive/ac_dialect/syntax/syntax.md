**handl_dialect > syntax > syntax**

# Module: syntax::syntax

## Contents

**Traits**

- [`Syntax`](#syntax) - Syntax tokens for handl expressions.

---

## handl_dialect::syntax::syntax::Syntax

*Trait*

Syntax tokens for handl expressions.

The interpolation markers are string tokens (may be multi-character).
The path separator, call-open, and call-close are single-character tokens.
In the Rust implementation, string tokens are `&str` and single-character
tokens are `char`; in other languages, both may be represented as strings.

These tokens are the minimal syntax surface. Every parser, interpolator,
and type inferencer in the implementation reads them from the dialect
rather than hardcoding strings.

**Methods:**

- `interpolation_prefix`: The interpolation prefix — e.g. `"${"` for the canonical dialect.
- `interpolation_suffix`: The interpolation suffix — e.g. `"}"` for the canonical dialect.
- `path_separator`: The path separator — e.g. `'.'` for the canonical dialect.
- `call_open`: The call-open character — e.g. `'('` for the canonical dialect.
- `call_close`: The call-close character — e.g. `')'` for the canonical dialect.



