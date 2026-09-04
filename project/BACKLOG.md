# Backlog

## Tooling (low priority — self-hosting, see `project/rfcs/`)

Bootstrap tooling (ADR-0085..0091 — improving the existing `plugin-*`
crates; no new tooling crates) is high priority and tracked in TODO.md. Everything below is the *low priority* self-hosting
follow-up: porting the tooling to handl itself once the language is stable.
The generic Rust substrate (plugin system, `plugin-ext-*`,
`plugin-engine-*`) stays as bootstrap and fallback (ADR-0085 rule 6).

- **RFC-0001 — Plugin system in handl.** Extract the Rust plugin system
  (`platform` crate: Capability/Plugin/Kit/Host) into handl: capabilities
  and plugins as values, kits as module lists, hosts as handlers.
- **RFC-0002 — Formatter in handl.** Refactor of bootstrap `plugin-ext-fmt`;
  formatter as a pure arrow, rules compose by `>>>`, idempotence as a law.
- **RFC-0003 — Linter in handl.** Refactor of bootstrap `plugin-ext-lint`;
  rules are pure arrows `Ast ~> [Diagnostic]`, `--fix` as a fixpoint
  pipeline, cross-module rules over the module graph.
- **RFC-0004 — Typechecker in handl.** Refactor of bootstrap
  `plugin-engine-typecheck`; adds the deferred scope (traits, refinements,
  invariants, row variables). Blocked on compiled backends (perf).
- **RFC-0005 — Test runner in handl.** Refactor of bootstrap
  `plugin-lib-test`; adds parallel execution (ADR-0072), properties,
  benchmarks.
- **RFC-0006 — LSP server in handl.** Hardest target, intentionally last;
  session as a fold over the message stream, features as arrows, thin Rust
  transport adapter. Blocked on RFC-0004 perf.
- **RFC-0007 — CLI in handl with tool-integration plugins.** Realizes
  ADR-0029: generic, composable, extensible, replaceable, modular,
  documented CLI that loads integrations contributed by the tool RFCs and
  is decoupled from them.

Order: RFC-0001 → 0002/0003 → 0005 → 0007 → 0004 → 0006.

## Arrow macros (ADR-0084)

- Ref: [ADR-0084 — Arrow Macros](../project/adrs/0084-arrow-macros.md)
- Type-safe compile-time metaprogramming. Macros are arrows (`Ast ~> Ast`), typechecked before expansion. No new calculus primitives.
- Subsumes `const` (ADR-0083 §5.1) and `data` (ADR-0082) as degenerate macros. Enables derivation (`derive_show`, `derive_eq`) and DSL embedding (SQL, regex, HTML).
- Low priority — not blocking ADR-0083 (FFI-Native Stdlib). `const` + `prim.or` cover the immediate POSIX constants need.

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

## Platform plugin security

Security is **out of scope for the initial platform refactor** (see `docs/adr/0002-handl-platform-composable-host.md`). Future work, in rough priority order:

1. **Sandboxed `Target` plugins.**
   - `WasmTarget` plugin that compiles generated `handl` code to WebAssembly and runs it in a VM.
   - `InterpreterTarget` plugin with an in-process, capability-limited interpreter.
   - The `Host` and operation plugins stay native; only the generated user code is sandboxed.

2. **Plugin permission manifest.**
   - `Plugin` manifest can declare required permissions: `register-plugin`, `network`, `filesystem`, `process`, etc.
   - `Host` enforces declared permissions before loading a plugin.
   - `ContributionContext` is restricted based on the loaded plugin's permission set.

3. **Signed kits and libraries.**
   - Kit publishers sign manifests.
   - `handl-host-cli` refuses to load a kit with a missing or invalid signature.
   - Key distribution and revocation policy (TBD).

4. **Reproducible, hash-pinned dependency trees.**
   - `Kit` manifests pin exact plugin versions and dependency hashes.
   - Host validates the loaded dependency tree against the pinned hashes before execution.

## Self-hosted `Host` and meta-circular composition

The new plugin model is intentionally minimal. This makes it possible, eventually, to write the `Host` itself in `handl` rather than Rust. Long-term research direction, not part of the initial refactor:

1. **`handl` as a host implementation language.**
   - Write `Host`, `PluginRegistry`, `ContributionContext`, and the `Plugin` model in `handl`.
   - The `handl` program compiles to a native or WASM target and behaves the same as the Rust `Host`.
   - Still requires a minimal Rust bootstrap runtime that can load and run the compiled handl `Host`.

2. **Plugins that manage the `Host`.**
   - A `HostFactory` plugin can create and configure new `Host` instances.
   - A `Host` can spawn isolated child hosts with a subset of plugins, useful for tests, workers, and sandboxed compilation units.
   - The `Host` itself becomes a capability that language-level code can request and compose.

3. **Security through the `handl` language.**
   - `handl` is designed for sandboxing and capability tracking.
   - Once the `Host` is written in `handl`, capabilities (`contribute`, `register-plugin`, `execute`) can be checked by the language runtime.
   - User `handl` code can safely spawn and inspect sub-hosts because the language enforces effect and permission boundaries.

## Polyglot module syntax

The decoupled parser/lexer/formatter model makes it possible to load modules written in different syntaxes at runtime. This is not part of the initial platform refactor.

1. **Pluggable `Parser` by file extension / syntax marker.**
   - `handl-ext-parser` accepts `Behavior<Parser>` rules keyed by a syntax id.
   - A plugin like `handl-plugin-yaml-syntax` contributes a `yaml` parser.
   - The `Parser` plugin dispatches to the correct `Behavior` based on file extension or an inline `syntax` declaration.

2. **Pluggable `Lexer` and `Formatter` per syntax.**
   - Each syntax plugin can also contribute `Behavior<Lexer>` and `Behavior<Format>` for its own token and output rules.
   - `BasicLexer` and `BasicFormatter` dispatch by syntax id before dispatching by `AstNode.kind`.

3. **Modules with their own language.**
   - A module can declare `syntax "my-dsl"`.
   - The host loads the corresponding syntax plugin if not already present.
   - The module is parsed to the common `AstNode` shape and participates in the normal pipeline.
