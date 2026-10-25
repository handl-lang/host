# Backlog

## Tooling (deferred — self-hosting, see `project/rfcs/`)

_Not now (2026-10-18): RFC-0001 stays parked until the language and
compiled backends stabilize._

The ADR-0085..0094 bootstrap tooling implementation is near-term work and
tracked in `TODO.md`. Everything below is the _low priority_ self-hosting
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
  `plugin-engine-test`; adds parallel execution (ADR-0072), properties,
  benchmarks.
- **RFC-0006 — LSP server in handl.** Hardest target, intentionally last;
  session as a fold over the message stream, features as arrows, thin Rust
  transport adapter. Blocked on RFC-0004 perf.
- **RFC-0007 — CLI in handl with tool-integration plugins.** Realizes
  ADR-0029: generic, composable, extensible, replaceable, modular,
  documented CLI that loads integrations contributed by the tool RFCs and
  is decoupled from them.

- **VS Code extension.** The `editor-support/vscode` skeleton exists; full
  feature work is deferred until RFC-0006 LSP-in-handl matures.

Order: RFC-0001 → 0002/0003 → 0005 → 0007 → 0004 → 0006.

## Compiler backends

- **CommonJS output for `hnl compile js`** — `JsOutputFormat` currently
  emits ESM only. Deferred: ESM covers the node/browser targets in use;
  revisit if a CommonJS-only embedder appears. (`web` repo,
  `modules/compile-js`.)

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

## Test framework primitives (ADR-0092 unblockers)

- Add `time.sleep_ms` to `handl-packages/time`
  (medium priority — deferred from the initial `time` contract; effectful,
  belongs behind a capability, not needed by `benchmark`/`property`/`fuzz`).
- Strengthen `rand.seeded_*` contract guarantees: add a determinism
  predicate (e.g. `Deterministic`/`Eq`) to `plugin-ext-refinement`'s `Pred`
  vocabulary so "same seed + args ⇒ same output" is a real `guarantee:`
  rather than a `-- LAW:` comment.

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

## host-tensor / lib-tensor

The pair is at 28 ops with 11 registered deriv pairs (every
non-constructor op has an adjoint); lib-tensor mirrors all 28 and
verifies 45/45 differentially under `hnl-run`.

- **dtype beyond `f64`** — kernel slice first: `handl-value`'s
  `TensorValue` is `Vec<f64>`; a second dtype needs an enum payload +
  codec there (same shape the `Tensor` variant took). Then here:
  `Tensor.data` becomes the enum and each op picks a rule — same-dtype
  with an error on mixing is the likely default (promotion lattices are
  where the bugs live). The wire field is already reserved
  (`"dtype": "f64"`; decode rejects everything else today).
- **`.hnl`-function autodiff** — `tensor.derive` is a registered-pairs
  table, deliberately not autodiff: a host `Function` sees only
  evaluated argument values, so a `.hnl` arrow's name resolves to
  `null`. Real autodiff on `.hnl` functions needs engine machinery
  (source-to-source transform or a tape in eval) — an engine slice,
  not a host-tensor one.

## Type-system surface sugar

- **`exists m. T` existential sugar** — surface spelling that lowers to
  the `Sigma(Nat, m ~> T)` encoding (explicit `Sigma` is the decision;
  sugar deferred). Covers `filter`-style return types
  (`Vec{?m, a}` — `?m` index holes don't lex today) and Sigma
  destructure ergonomics (pair pattern on the intro `[m, v]` binds the
  witness rigidly — `var{m}` — since the phase-3 Sigma-destructure
  fix; the remaining friction is surface-only).

## Phase-4 user decisions (from `design/phase4-typesystem-spec.md`, NEEDS-USER table)

All nine items block a workstream — pick before implementation:

1. **`exists` binder sort** — `exists m. T` (m implicitly `Nat`) vs `exists m : S. T` (explicit sort). (WS-A)
2. **Sigma destructure** — `[m, v]` pair pattern vs `.1`/`.2` projections vs both. (WS-A)
3. **Open-row tail spelling** — `IO<{puts | r}>` vs `IO<{puts, ..r}>` vs `IO<Row<puts, r>>`. (WS-D)
4. **Row-binder introduction** — `f<r : Sig>` explicit vs implicit free row vars. (WS-D)
5. **SMT driver** — z3 vs cvc5, env-gated via `HNL_INDEX_SOLVER`/`HNL_SMT_BIN`; or defer. (WS-B)
6. **Self-recursion mechanism (compiled backends)** — reuse `with_self_ref_bind` vs port `eval_rec` vs surface `rec`/`fix`. (WS-C)
7. **`op`/`cap` decl disposition** — inert+documented vs checked `stmt.op` into `Sig_m` vs delete. This is the real `extern` question; `extern` itself is rejected. (WS-F)
8. **`handle … acc`** — build design B (handler-accumulator) vs bless the runState free-monad fold. Dogfood `lib-json-handl` informs this. (WS-E)
9. **LSP workspace index scope** — `.handl.toml` dep graph vs workspace-root scan. (WS-G)

## Flagged leftovers (session-found, not yet in a workstream)

- **`hnl test` discovery bug** — a top-level `x := if … else …` binding
  zeroes out all `test_*` discovery for the file (nested `if` is fine).
  Worked around in `host/tests/smoke/export_run.hnl`; real fix needed in
  the test-discovery pass.
- **`VCons` reconstruction in annotated match arms** — ctor instantiation
  leaks family param names (`n`,`a`) into user scope and collides with
  user vars. Fix = rename ctor-internal params `__fam_*` on bind.
  **Owned by user's other session — do not duplicate.**
- **Self-recursive `handle` SOF under `hnl run`** — related to compiled
  self-recursion (phase-4 WS-C / decision 6); repro documented in
  progress logs.
- **`compile-handl` residual emitter drift** — 2 test fails, pre-existing,
  flagged not-fixed.
- **LSP v2 remainder** — v1 shipped (diagnostics/hover/goto-def). The v2
  batch (meta-zonked hovers, completion, references/rename, multi-diag,
  UTF-16 cols, sequential `do`-scope) partially overlaps phase-4 WS-G —
  keep the itemized list here: zonked hover types, hover on non-identifier
  nodes, ctor/data bindings in defs, references+rename, completion.
- **Sigma ergonomics** — no eliminator surface beyond match; `<n>` index
  binders can't serve as term-level `Nat` witnesses (partially covered by
  the `exists` entry — keep both).
