# lib-fuzzy — Approximate String Matching

## Status

Backlog — not yet implemented.

## Summary

A `fuzzy` module for approximate string matching. Extends beyond the basic
`str.levenshtein` and `str.similarity` already in `lib-utils`. Self-contained
algorithms, no heavy dependencies.

## Crate

`crates/lib-fuzzy` — follows the existing opt-in crate pattern.

## API

```
fuzzy.jaro(a, b) → number
    Jaro-Winkler similarity score (0.0–1.0). Good for names and short strings.
    Prefix-weighted variant of Jaro distance.

fuzzy.ngram(n, a, b) → number
    N-gram Jaccard similarity (0.0–1.0). Set-based comparison using character
    n-grams of size n. Good for longer strings and typo tolerance.

fuzzy.damerau_levenshtein(a, b) → number
    Edit distance with transposition support (adjacent character swaps count
    as 1 operation, not 2). More accurate than plain Levenshtein for typos.

fuzzy.contains_fuzzy(needle, haystack, threshold?) → bool
    True if needle approximately matches any substring of haystack.
    Default threshold: 0.8.

fuzzy.best_match(needle, candidates) → string
    Returns the closest match from candidates. Error if candidates is empty.

fuzzy.filter(needle, candidates, threshold?) → array
    Ranked list of candidates above threshold, sorted by similarity descending.
    Default threshold: 0.0 (all candidates, ranked).
    Returns array of {candidate: string, score: number}.
```

## Design decisions

- **No external dependencies:** All algorithms are pure Rust implementations.
  No need for `strsim` crate — algorithms are short and self-contained.
- **Score convention:** All similarity functions return 0.0–1.0 (1.0 = identical).
  Distance functions (`damerau_levenshtein`) return integer edit distance.
- **Threshold defaults:** `contains_fuzzy` defaults to 0.8 (strict-ish).
  `filter` defaults to 0.0 (return everything ranked). Both are overridable.
- **`best_match` error case:** Empty candidates array → error, not null.
  Consistent with existing module error patterns.

## Open questions

- Should `fuzzy.filter` return objects `{candidate, score}` or just strings?
  (Proposed: objects, since ranking is the whole point.)
- Should we add `fuzzy.soundex` or `fuzzy.metaphone` for phonetic matching?
  Niche but cheap to implement.
- Should `fuzzy.best_match` return null or error when no candidate meets a
  minimum quality threshold?
