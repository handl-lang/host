# lib-diff — String Diffing

## Status

Backlog — not yet implemented.

## Summary

A `diff` module for character, word, and line-level string diffing, plus
patch application. Useful for changelogs, text comparison, and versioning
workflows.

## Crate

`crates/lib-diff` — follows the existing opt-in crate pattern.
Can use the `similar` crate (Rust diff library) or implement LCS-based diff
internally. `similar` is lightweight and well-maintained.

## API

```
diff.chars(a, b) → array
    Character-level diff. Returns array of operations:
    {op: "equal", text: "abc"} | {op: "insert", text: "xyz"} | {op: "delete", text: "def"}

diff.words(a, b) → array
    Word-level diff. Same operation format as diff.chars, but text segments
    are words (whitespace-separated tokens).

diff.lines(a, b) → array
    Line-level diff. Same operation format, text segments are lines.

diff.patch(original, ops) → string
    Apply a sequence of diff operations to original string.
    ops is the array returned by diff.chars/words/lines.
    Returns the resulting string.
```

## Design decisions

- **Operation format:** Unified across all granularities. Each op is an object
  with `op` ("equal" | "insert" | "delete") and `text` (the affected segment).
  This makes `diff.patch` work with any granularity's output.
- **Algorithm:** LCS (Longest Common Subsequence) based diff. O(n*m) time/space.
  Acceptable for typical text sizes. For very large inputs, consider Myers'
  algorithm (O(ND) where D is edit distance).
- **Crate dependency:** `similar` crate provides optimized diff algorithms
  (Myers, Patience, LCS). Lightweight, no transitive deps. Recommended over
  hand-rolling.
- **No unified diff format:** This module produces structured ops, not
  unified-diff text. A `diff.to_unified(ops)` convenience could be added later
  if needed, but the structured format is more useful programmatically.

## Open questions

- Should `diff.patch` validate that ops are consistent with original (e.g.,
  "delete" text matches), or apply blindly? (Proposed: validate, error on
  mismatch — prevents silent data corruption.)
- Should we add `diff.to_unified(ops, a_label, b_label) → string` for
  human-readable diff output?
- Should word-level diff preserve whitespace in "equal" ops, or normalize?
- Should we support move operations (not just insert/delete) for a richer
  diff? (Adds complexity; probably out of scope for v1.)
