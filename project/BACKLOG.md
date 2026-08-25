# Backlog

## lib-utils

- Add non-crypto hashes — hash.fnv1a(input), hash.murmur3(input, seed?), hash.xxhash(input, seed?). Fast non-cryptographic hashes returning numbers (not hex digests). For sharding, bucket keys, dedup. NOT in lib-crypto (not security-related). Low priority — build only if needed.

- Add matrix — 2D operations (transpose, multiply, determinant, identity, inverse). Useful but niche; only if handl users actually need linear algebra.
- Add encode.* — encode.base64(data), encode.hex(data), decode.base64(str). Common in automation for handling binary data, tokens. Pure compute, lib-utils.
- Add log.* — log.info(msg), log.warn(msg), log.error(msg), log.debug(msg). Structured output to stderr. Platform-dependent (writes to stdout/stderr), fits in lib-platform + lib-platform-std.
- serialize.* — JSON/YAML/TOML parse/stringify (but serde_json may suffice inline)
- stream.* — streaming/chunked data processing
- format / template — string interpolation. Two designs: (A) `str.format(template, values)` as a pure function in lib-utils (fits existing pattern), or (B) scope-resolving interpolation in the runtime layer. See notes/format-template.md for details.

## lib-graph

- Add graph theory production-ready bindings/lib. feature complete.

## lib-utils (combinators)

- AST-based inlining — `substitute_param` uses byte-level string scanning, not AST-based substitution. Correct for all tested cases but theoretically fragile with exotic string escapes. See notes/ast-based-inlining.md.

## lib-re — Regular Expressions

- New crate `lib-re` for pattern matching, backed by Rust's `regex` crate.
- Keeps `str` simple; puts pattern complexity behind an opt-in module.
- See notes/lib-re.md for API details.

## lib-fuzzy — Approximate String Matching

- New crate `lib-fuzzy` for fuzzy string matching beyond basic `str.levenshtein`/`str.similarity`.
- Jaro-Winkler, n-gram, Damerau-Levenshtein, fuzzy search/filter.
- See notes/lib-fuzzy.md for API details.

## lib-text — Text Processing

- New crate `lib-text` for higher-level text operations (word count, tokenization, n-grams, dedent, indent).
- Does not belong in `str` (too high-level) or `intl` (not locale-aware).
- See notes/lib-text.md for API details.

## lib-diff — String Diffing

- New crate `lib-diff` for character/word/line-level diffing and patch application.
- See notes/lib-diff.md for API details.

## Module system: async + operation dispatch

The VM (`handl-vm`) supports three dispatch channels via `Host`: sync `Function`, `AsyncFunction`, and `Operation`. But `Module` trait and `ModuleHost` only wire sync `Function`. This blocks all lib-* crates from exposing async or operation-based capabilities.

- **P0: Extend `Module` + `ModuleHost`** — add `async_function()` and `operation()` to `Module` (default `None`), implement `Host::async_function` and `Host::operation` on `ModuleHost`. Unblocks everything below.
- **P1: `http`/`fs`/`cmd` async + operations** — add async variants to platform traits; expose I/O operations as `Operation` for handler interception, permission control, cross-machine resumability.
- **P2: `fp.map`/`filter`/`reduce` async** — async higher-order functions using `ctx.eval_expr_async` (`AsyncEval` callback).
- **P3: `kv` async** — async `KeyValueStore` trait for network backends (Redis, SQLite).
- **P3: `DefaultHost` expansion** — more built-ins beyond `add`/`mul`/`concat`/`len`.

See notes/module-async-operations.md for details.

## Tree-shaking / Dead code elimination

- **Crate-level: DONE** — `handl` facade is tree-shakeable via Cargo features (22 feature flags, `optional = true` deps, `default` = everything for backwards compat).
- **Language-level: NOT DONE** — no DCE pass exists. All declared fields are computed; no reachability analysis. A `DeadFieldElimPass` could slot into the compiler pipeline via the `Pass` trait. See notes/tree-shaking.md for design and challenges.

## ecma402 libs

- Add remaining harness to our ecma402 libs. Only `number_format` currently has a conformance harness; add harnesses for the remaining ECMA-402 APIs (collator, date/time format, display names, list format, locale, locale matching, plural rules, relative time, segmenter).
