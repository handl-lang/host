---
title: Backlog
type: backlog
date: 2026-08-24
updated: 2026-10-29
---

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

## Cargo `[patch]` tables & `--locked` lockfiles

Sweep over the 148 sibling repos with a root `Cargo.toml`: 61 had a lockfile
that `cargo … --locked` refused forever (`cannot update the lock file …
because --locked was passed`) — even immediately after a *successful*
`cargo update`.

- **Root cause** — cargo serializes `[[patch.unused]]` in HashMap iteration
  order. With one source contributing unused entries (the global
  `~/.cargo/config.toml` sepia patches) the order is stable in practice; as
  soon as a manifest `[patch]` entry is also unused, two sources contribute and
  every re-resolution writes a byte-different lockfile, so `--locked` can never
  converge. Proof: repeated `cargo metadata` on one repo produced different
  lockfile md5s over an identical unused *set*.
- **Fix** — unused patch entries are resolution-neutral, so delete them from
  the manifest: `host/scripts/dev-link-prune.py <repo>` reads
  `[[patch.unused]]` from the lockfile, drops those members, drops emptied
  tables, and keeps comments (including the `# dev-link:` markers); then one
  `cargo metadata` re-resolves. Result: byte-stable lockfile, `--locked` green,
  same dependency graph and versions.
- **State** — 147/147 repos that have a lockfile are `--locked`-clean
  (`lib-json-handl` has no lockfile: zero deps). 55 repos took a
  `chore: refresh stale Cargo.lock` commit touching only `Cargo.lock` +
  `Cargo.toml`; nothing was pushed.
- **Run the pruner after editing any `[patch]` table.** `removed=0
  remaining-unused=<n>` means the table set is already minimal (the remaining
  unused entries come from global config and are harmless on their own).
- **`dev-link.sh` reconciled** — three changes: the referenced-repo set is now
  a transitive closure (it stopped at two hops and so silently dropped tables
  for `lib-ffi`, `lib-perlin`, `lib-rand-stub`, `lib-simplex`, `lib-test`,
  `lib-type-handl`, which the host manifest carries by hand);
  a re-run that would emit a second table for an already-present
  `[patch."…/R"]` URL now **aborts with a report** instead of writing the
  manifest into cargo's `error: duplicate key`; and it prunes + re-resolves at
  the end. `DEV_LINK_FORCE=1` folds every table (hand-written and generated)
  into the managed region — verified equivalent on `host` (137 members,
  identical lockfile md5), `DEV_LINK_NO_PRUNE=1` skips the prune.
- **Duplicate names across patch sources are left alone** — a lockfile's
  `[[patch.unused]]` records only `name` + `version`, no source URL, so when
  one crate name is patched from two `[patch."…"]` sources the pruner cannot
  tell which copy cargo meant. It skips those instead of risking an unpatch of
  the used copy (`skipped-ambiguous=<n>`); consolidating such duplicates stays
  manual.
- **Pruner marker bug (fixed in the same session)** — the first version
  dropped a region's trailing `# dev-link: end` marker when its last table was
  pruned, which makes the next `dev-link.sh` run append a whole second region.
  It hit `sdkgen-cli` (repaired inside its lock commit); all 148 manifests now
  have matching begin/end markers.



Working deliverable: `lib-json-handl/src/json.hnl` (~880 ln) — pure-handl JSON
parse/toStr/pretty, `data` recursion, `runState_c` free-monad State chain;
39-item verified smoke. Gaps found:

**S1 — blockers**
- `type.unknown` untouchable (no `==`/`<`/`+`/`if` on host/while results) —
  **likely fixed by engine-typecheck-nbe `e094277` ("type.unknown is the
  dynamic top") landed mid-session — re-verify before scheduling.**
- Closed-record unification rejects heterogeneous ctor payloads in shared slots.
- Record/array literals crash in `type.unknown`-expected positions
  (`check_object_record: no 'fields' object`).
- `check_json_value` internal crash on meta/unknown/mismatched-record expected
  types — should be a unification diagnostic, not an internal error.
- `hnl run` integrated checker is stricter than `hnl typecheck` — same file
  passes one, fails the other; no `run --unchecked` escape hatch.
- **`str.replace` broken** — pattern never matches, `\` inserted before last
  char (engine bug, arg order matches no documented permutation).
- **Char-pattern `match` emits escapes one position late** — engine-level
  match-bind/`char.chars` off-by-one (repro `/tmp/hnl-dogfood3/pe2.hnl`).
- Postfix `f(x)[i]` binds the index to the arrow *name*, not the call result
  (parse/scope bug — `r := f(x); r[0]` works).
- `data` ctors are file-local at runtime; `import` of `data` modules fails
  desugaring + `call_host` misses; `hnl test` can't import `data` files at all
  (lib ships `mk_*` mints as workaround).
- `IO<sig>` can't be constructed in checked code (only the runtime
  `{"__former__":…}` spelling; `lib-type`'s `export IO` doesn't materialize).

**S2 — friction**
- `:=` arrow params monomorphic across call sites (one wrapper per arg type).
- `while` bodies no longer unchecked islands — unknown rules apply inside.
- `+` is Number-only (`str.concat` for strings).
- `export` in-module: only `import *` / `import * as j` work; no visibility.
- Recursion needs `: T =`; no mutual recursion.
- Tuple `kv[1]` indexing fails (`array_element_type`); `match` destructure works.
- Stray `}` silently re-nests file → `unbound wire` far from the real error.
- Numbers print as floats (`3.0`).

**S3 — cosmetic**: `()` not a Unit literal; `J` reserved (blocks `data J ::`);
`Unknown` annotation degrades arrow to LamVal; `hnl test` 0-tests + can't
import `data`; `ci.sh` hardcodes `../host-cli/target/debug/hnl`;
match-on-unknown is a working escape valve worth documenting.

**New NEEDS-USER decision (adds to the 9)**: **IO sequencing** — `handle`'s
per-entry dispatch discards ordering + result-feedback, so real State needs the
explicit free-monad chain (`runState_c`). Options: (a) sequencing/bind
primitive in the `IO` former, or (b) `handle` feeding op results back into the
running computation. Without one, every real effect is hand-rolled chains.

## Docs sweep — state and leftovers

Every public repo publishes a Diátaxis site at
<https://handl-lang.github.io/docs/>: 27 products with the four quadrants
(`index`, `reference`, `how-to`, `explanation`) plus `docs/site.yaml`. The hub
(`docs` repo) owns one allowlist (`scripts/public_repos.py`, imported by all
three generators), generates per-product sidebars from what was vendored, and
ships `.nojekyll` — without it Pages ran a legacy Jekyll build and deploys
silently never took effect.

Remaining:

- **`research-dts` — DONE.** Unarchived, its docs commit pushed (`6be8055`),
  re-archived. Site is published and current.
- **`host`, `graphopt`, `eqsat`, `gamekit` got README pointers only.** Their
  `docs/site/` trees predate this sweep and were not re-read for accuracy; the
  other 23 repos were authored from source. Refresh when someone owns them.
- **38 sibling repos are `--locked`-clean but not compile-verified.**
  `engine-exec-bytecode/src/vm.rs:1842` fails `E0308` (`replace` wants `usize`,
  gets `Option<usize>`) in an in-flight edit, and everything downstream cannot
  build. Re-run the verify pass (`cargo test --workspace --lib --locked` under
  the global flock) once it compiles. The same blocker holds the `hnl` extension
  build, and with it the `lib-json-handl` re-verify against `e094277`.
  _Fixed 2026-10-26 (`1e0d2d0`-era change to `std::mem::replace`); the rest of
  the verify pass is still owed._

## Effects / engines (2026-10-26, after multi-shot `resume`)

Context: `design/multishot-resume.md`, `notes/multishot-resume-cranelift.md`.
`engine_parity` is 43/44; the last red is below.

- **Bytecode: nullary-ctor pattern in a closure loses its capture.** A
  `match p { VNil: …, VCons(a, b): … }` inside an arrow body takes the
  *first* arm for every value on the bytecode VM (tree-walk and cranelift
  are correct; root-position matches are correct; families without index
  parameters are correct). Instrumented cause: the arrow's captured map
  holds only `["VCons"]`, so `pattern_match`'s `scope_value("VNil")`
  returns `None` and the pattern degrades to a catch-all binder. The
  capture set is filtered by `subtree_name_candidates`; the minting path
  for this arrow is not the bytecode `Instr::Closure` fast path nor the
  shared `closure` op (both were tried and reverted). Next step:
  instrument the mint and print the candidate set. Repro:
  `w : Vec ~> Number = p ~> match p { VNil: 0, VCons(a, b): 1 }` applied to
  `VCons(Zero, VNil)`.
- **`perform` inside a `rec`/loop fixpoint region** is unsupported by
  design: the fixpoint re-evaluates its statements once per pass, so the
  same `perform` would consume a new journal entry each pass and replay
  can never converge. Supporting it needs journal memoization keyed by
  (pass, op index).


## ADR corpus — memory pass (after the 2026-10-29 compaction)

The size/rule backlog is **zero**: every live ADR across the org is ≤8 KB and
≤10 top-level rules; detail lives in each repo's `project/design/` (port plans
in `project/rfcs/`). Archived and superseded records are history and are not
compacted.

What remains is the memory pass — sections that were absent in the legacy
corpus and where neither the ADR nor its design doc records an answer:

- **`Not doing` placeholders**: 48 records carry
  `none recorded — legacy record…`. Replace only with alternatives the record
  actually rejected — never invent one.
- **`Enforcement` placeholders**: 35 records carry
  `Code and tests are named in the design doc.` Replace with the crate/test
  that makes the rule true.

Find them:

```bash
grep -rl 'none recorded — legacy record' --include='*.md' */project/adrs
grep -rl 'Code and tests are named in the design doc' --include='*.md' */project/adrs
```

Per-repo counts: `python3 host/scripts/check-adrs.py --repo <repo>` reports the
reminder counts; `--strict` gates new/modified records; `--changed REV` is the
PR ratchet.

Also tracked here, not by this repo: `host` main's working tree and
`graphopt`'s `Cargo.lock` belong to parallel sessions.

