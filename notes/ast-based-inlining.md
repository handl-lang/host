# AST-based inlining for `substitute_param`

## Status

Backlog — deferred architectural improvement. Current implementation is correct for all tested cases.

## Summary

`substitute_param` in `lib-utils/src/fn/combinators.rs` performs parameter
substitution for function inlining (e.g., `pipe`, `compose`) using byte-level
string scanning rather than AST-based substitution.

## Current approach

The function walks the function body as raw bytes, tracking whether it is
inside a double-quoted string with a simple `in_string` flag. When it finds
a param name outside a string, bounded by non-identifier characters on both
sides, it replaces it with the wrapped replacement expression.

Key mechanics:
- `in_string` toggles on every unescaped `"` byte.
- `\\` escape handling: skips the next byte (correctly handles `\"`).
- `is_ident_char` checks word boundaries (alphanumeric + `_`).
- `next_char_boundary` advances over multi-byte UTF-8 sequences.

## Limitation

String-boundary detection is a byte-level heuristic, not an AST traversal:

- It handles `\"` escapes correctly — the only escape that affects boundary
  detection.
- Other escapes (`\n`, `\t`, etc.) are passed through verbatim and do not
  interfere with the `"` toggle, so they are fine.
- **Theoretical risk:** if a future string syntax introduced an escape that
  produces a literal `"` (e.g., `\x22` or `\u0022`), the scanner would
  misidentify the string boundary and either skip or incorrectly substitute
  a param.

## Why it's fine today

1. Param substitution only happens outside strings.
2. The escape handling correctly skips `\"`, which is the only escape that
   affects boundary detection.
3. Other escapes don't contain `"` and don't affect the toggle.

## Deferred fix: AST-based substitution

Parse the body into an expression tree, walk only identifier nodes, and
substitute there — making string boundary detection a non-issue entirely.

### What this requires

- A parser for the handl expression grammar.
- A tree-walker that identifies identifier nodes matching the param name.
- Re-serialization back to a string.

### Why it was deferred

The byte-scanner is ~45 lines with zero parser dependencies. The AST approach
would require parse → walk → serialize round-tripping, which is significantly
more wiring and risks altering formatting or introducing its own bugs. The
current approach is correct for all real-world inputs.

### When to revisit

When the language introduces string escapes that can produce a literal `"`
character, or when the combinator functions need to handle more complex
expression forms that the byte-scanner cannot reliably parse.

## Location

- `packages/handl/crates/lib-utils/src/fn/combinators.rs` — `substitute_param` (lines 9–55)
- Called by `inline_chain` (line 90), used by `pipe` and `compose`.
