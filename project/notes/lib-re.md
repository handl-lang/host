# lib-re — Regular Expressions

## Status

Backlog — not yet implemented.

## Summary

A `re` module for pattern matching, backed by Rust's `regex` crate.
Too specialized for `str` but universally needed. Keeps `str` simple
and puts pattern complexity behind an opt-in module.

## Crate

`crates/lib-re` — follows the existing pattern of `lib-intl`, `lib-crypto`,
`lib-rand`, etc. Opt-in dependency; `regex` crate adds binary size so it
should not be forced into `lib-utils`.

## API

```
re.match(pattern, input) → bool
    True if the pattern matches anywhere in input.

re.test(pattern, input) → bool
    Alias for re.match. (JS convention: test is on the regex, match on string.
    Here both are module-level, so test is redundant but familiar.)

re.find(pattern, input) → array
    All matches as objects: {match: string, index: number, groups: array}.
    Empty array if no matches.

re.replace(pattern, input, replacement) → string
    Replace all matches. Supports $1, $2, ${name} in replacement.

re.split(pattern, input) → array
    Split input on pattern matches.

re.groups(pattern, input) → array
    Capture groups from the first match: [full, group1, group2, ...].
    Null if no match.
```

## Design decisions

- **Crate choice:** Rust `regex` crate (linear-time, no backtracking).
  Avoids ReDoS. Does not support lookbehind/lookahead — acceptable trade-off.
- **Pattern compilation:** Compile on each call (simplest). If performance
  matters, consider caching compiled patterns in a thread-local or accepting
  pre-compiled pattern objects. Premature for v1.
- **Error handling:** Invalid regex pattern → VmError with descriptive message.
  No silent fallback.
- **Replacement syntax:** `$1`, `$2`, … for numbered groups. `${name}` for
  named groups. `$$` for literal `$`. Matches Rust `regex` crate conventions.

## Open questions

- Should `re.find` return all matches or just the first? (Proposed: all, with
  `re.find_first` as a convenience if needed.)
- Should we support `regex-lite` for smaller binaries, similar to the
  `lib-intl` blob provider pattern?
- Named groups: return as array or as object? (Array is simpler and consistent
  with the "groups" name; object would need a separate `re.named_groups` fn.)
