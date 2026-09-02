# lib-text — Text Processing

## Status

Backlog — not yet implemented.

## Summary

A `text` module for higher-level text operations that don't belong in `str`
(too high-level) or `intl` (not locale-aware). NLP-adjacent utilities for
word counting, tokenization, n-grams, and whitespace normalization.

## Crate

`crates/lib-text` — follows the existing opt-in crate pattern.

## API

```
text.word_count(input) → number
    Count words (whitespace-separated tokens).

text.sentence_count(input) → number
    Count sentences (split on . ! ? followed by whitespace or end-of-string).

text.extract(input, pattern) → array
    Extract matches for predefined patterns: "email", "url", "date", "number".
    Returns array of matched strings. Uses regex under the hood.
    Example: text.extract("Contact a@b.com or c@d.com", "email") → ["a@b.com", "c@d.com"]

text.tokenize(input) → array
    Word tokenization. Splits on whitespace and punctuation, returns array of
    tokens. Strips punctuation from token boundaries.

text.ngrams(input, n) → array
    Character or word n-grams of size n. Returns array of strings.
    Example: text.ngrams("hello", 2) → ["he", "el", "ll", "lo"]

text.truncate_words(input, max_words) → string
    Truncate to at most max_words words, appending "…" if truncated.

text.dedent(input) → string
    Remove common leading whitespace from all lines (like Python's textwrap.dedent).

text.indent(input, prefix) → string
    Prepend prefix to each line of input.
```

## Design decisions

- **`extract` predefined patterns:** Hardcoded regex patterns for common
  extractions (email, url, date, number). Not user-supplied regex — that's
  `lib-re`'s job. Keeps `text` dependency-free for v1.
  - If `lib-re` is available, `extract` could accept raw regex patterns too.
  - But coupling to `lib-re` is optional; predefined patterns use internal regex.
- **`tokenize` simplicity:** Whitespace + punctuation split. No ML, no
  language-aware tokenization. That's a different scope entirely.
- **`ngrams` granularity:** Character n-grams by default. Could add a
  `text.word_ngrams(input, n)` variant if needed.
- **No locale awareness:** All functions are locale-agnostic. Locale-aware
  text processing belongs in `intl` (e.g., `intl.segment`).

## Open questions

- Should `extract` support custom regex patterns (coupling to `lib-re`) or
  stay predefined-only?
- Should `tokenize` return objects with position info, or just strings?
- Should `dedent` handle tabs, or only spaces? (Proposed: treat tabs and
  spaces equivalently, expand tabs to 4 spaces first.)
- Should we add `text.wrap(input, width)` here, or is `str.word_wrap`
  sufficient? (Currently `str.word_wrap` exists — may be enough.)
