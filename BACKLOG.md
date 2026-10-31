# Backlog

## title: Backlog type: backlog date: 2026-08-24 updated: 2026-10-29

## Tooling (deferred — self-hosting, see project/rfcs/ )

- T-015 RFC-0001 — Plugin system in handl. Extract the Rust plugin system ( platform crate: Capability/Plugin/Kit/Host) into handl: capabilities and plugins as values, kits as module lists, hosts as handlers.
- T-016 RFC-0002 — Formatter in handl. Refactor of bootstrap plugin-ext-fmt ; formatter as a pure arrow, rules compose by >>> , idempotence as a law.
- T-017 RFC-0003 — Linter in handl. Refactor of bootstrap plugin-ext-lint ; rules are pure arrows Ast ~> \[Diagnostic\] , --fix as a fixpoint pipeline, cross-module rules over the module graph.
- T-018 RFC-0004 — Typechecker in handl. Refactor of bootstrap plugin-engine-typecheck ; adds the deferred scope (traits, refinements, invariants, row variables). Blocked on compiled backends (perf).
- T-019 RFC-0005 — Test runner in handl. Refactor of bootstrap plugin-engine-test ; adds parallel execution (ADR-0072), properties, benchmarks.
- T-020 RFC-0006 — LSP server in handl. Hardest target, intentionally last; session as a fold over the message stream, features as arrows, thin Rust transport adapter. Blocked on RFC-0004 perf.
- T-021 RFC-0007 — CLI in handl with tool-integration plugins. Realizes ADR-0029: generic, composable, extensible, replaceable, modular, documented CLI that loads integrations contributed by the tool RFCs and is decoupled from them.
- T-022 VS Code extension. The editor-support/vscode skeleton exists; full feature work is deferred until RFC-0006 LSP-in-handl matures.

## Compiler backends

- T-023 CommonJS output for hnl compile js — JsOutputFormat currently emits ESM only. Deferred: ESM covers the node/browser targets in use; revisit if a CommonJS-only embedder appears. ( web repo, modules/compile-js .)

## Arrow macros (ADR-0084)

- T-024 Ref: ADR-0084 — Arrow Macros
- T-025 Type-safe compile-time metaprogramming. Macros are arrows ( Ast ~> Ast ), typechecked before expansion. No new calculus primitives.
- T-026 Subsumes const (ADR-0083 §5.1) and data (ADR-0082) as degenerate macros. Enables derivation ( derive\_show , derive\_eq ) and DSL embedding (SQL, regex, HTML).
- T-027 Low priority — not blocking ADR-0083 (FFI-Native Stdlib). const + prim.or cover the immediate POSIX constants need.

## lib-utils

- T-028 Add non-crypto hashes — hash.fnv1a(input), hash.murmur3(input, seed?), hash.xxhash(input, seed?). Fast non-cryptographic hashes returning numbers (not hex digests). For sharding, bucket keys, dedup. NOT in lib-crypto (not security-related). Low priority — build only if needed.
- T-029 Add matrix — 2D operations (transpose, multiply, determinant, identity, inverse). Useful but niche; only if handl users actually need linear algebra.
- T-030 Add encode.\* — encode.base64(data), encode.hex(data), decode.base64(str). Common in automation for handling binary data, tokens. Pure compute, lib-utils.
- T-031 Add log.\* — log.info(msg), log.warn(msg), log.error(msg), log.debug(msg). Structured output to stderr. Platform-dependent (writes to stdout/stderr), fits in lib-platform + lib-platform-std.
- T-032 serialize.\* — JSON/YAML/TOML parse/stringify (but serde\_json may suffice inline)
- T-033 stream.\* — streaming/chunked data processing
- T-034 format / template — string interpolation. Two designs: (A) str.format(template, values) as a pure function in lib-utils (fits existing pattern), or (B) scope-resolving interpolation in the runtime layer. See notes/format-template.md for details.

## lib-graph

- T-035 Add graph theory production-ready bindings/lib. feature complete.

## Test framework primitives (ADR-0092 unblockers)

- T-036 Add time.sleep\_ms to handl-packages/time (medium priority — deferred from the initial time contract; effectful, belongs behind a capability, not needed by benchmark / property / fuzz ).
- T-037 Strengthen rand.seeded\_\* contract guarantees: add a determinism predicate (e.g. Deterministic / Eq ) to plugin-ext-refinement 's Pred vocabulary so "same seed + args ⇒ same output" is a real guarantee: rather than a -- LAW: comment.

## lib-utils (combinators)

- T-038 AST-based inlining — substitute\_param uses byte-level string scanning, not AST-based substitution. Correct for all tested cases but theoretically fragile with exotic string escapes. See notes/ast-based-inlining.md.

## lib-re — Regular Expressions

- T-039 New crate lib-re for pattern matching, backed by Rust's regex crate.
- T-040 Keeps str simple; puts pattern complexity behind an opt-in module.
- T-041 See notes/lib-re.md for API details.

## lib-fuzzy — Approximate String Matching

- T-042 New crate lib-fuzzy for fuzzy string matching beyond basic str.levenshtein / str.similarity .
- T-043 Jaro-Winkler, n-gram, Damerau-Levenshtein, fuzzy search/filter.
- T-044 See notes/lib-fuzzy.md for API details.

## lib-text — Text Processing

- T-045 New crate lib-text for higher-level text operations (word count, tokenization, n-grams, dedent, indent).
- T-046 Does not belong in str (too high-level) or intl (not locale-aware).
- T-047 See notes/lib-text.md for API details.

## lib-diff — String Diffing

- T-048 New crate lib-diff for character/word/line-level diffing and patch application.
- T-049 See notes/lib-diff.md for API details.

## Module system: async + operation dispatch

- T-050 P0: Extend Module + ModuleHost — add async\_function() and operation() to Module (default None ), implement Host::async\_function and Host::operation on ModuleHost . Unblocks everything below.
- T-051 P1: http / fs / cmd async + operations — add async variants to platform traits; expose I/O operations as Operation for handler interception, permission control, cross-machine resumability.
- T-052 P2: fp.map / filter / reduce async — async higher-order functions using ctx.eval\_expr\_async ( AsyncEval callback).
- T-053 P3: kv async — async KeyValueStore trait for network backends (Redis, SQLite).
- T-054 P3: DefaultHost expansion — more built-ins beyond add / mul / concat / len .

## Tree-shaking / Dead code elimination

- T-055 Crate-level: DONE — handl facade is tree-shakeable via Cargo features (22 feature flags, optional = true deps, default = everything for backwards compat).
- T-056 Language-level: NOT DONE — no DCE pass exists. All declared fields are computed; no reachability analysis. A DeadFieldElimPass could slot into the compiler pipeline via the Pass trait. See notes/tree-shaking.md for design and challenges.

## Platform plugin security

## Self-hosted Host and meta-circular composition

## Polyglot module syntax

## host-tensor / lib-tensor

- T-057 dtype beyond f64 — kernel slice first: handl-value 's TensorValue is Vec\<f64> ; a second dtype needs an enum payload + codec there (same shape the Tensor variant took). Then here: Tensor.data becomes the enum and each op picks a rule — same-dtype with an error on mixing is the likely default (promotion lattices are where the bugs live). The wire field is already reserved ( "dtype": "f64" ; decode rejects everything else today).
- T-058 .hnl -function autodiff — tensor.derive is a registered-pairs table, deliberately not autodiff: a host Function sees only evaluated argument values, so a .hnl arrow's name resolves to null . Real autodiff on .hnl functions needs engine machinery (source-to-source transform or a tape in eval) — an engine slice, not a host-tensor one.

## Type-system surface sugar

- T-059 exists m. T existential sugar — surface spelling that lowers to the Sigma(Nat, m ~> T) encoding (explicit Sigma is the decision; sugar deferred). Covers filter -style return types ( Vec{?m, a} — ?m index holes don't lex today) and Sigma destructure ergonomics (pair pattern on the intro \[m, v\] binds the witness rigidly — var{m} — since the phase-3 Sigma-destructure fix; the remaining friction is surface-only).

## Phase-4 user decisions (from design/phase4-typesystem-spec.md , NEEDS-USER table)

## Flagged leftovers (session-found, not yet in a workstream)

- T-060 hnl test discovery bug — a top-level x := if … else … binding zeroes out all test\_\* discovery for the file (nested if is fine). Worked around in host/tests/smoke/export\_run.hnl ; real fix needed in the test-discovery pass.
- T-061 VCons reconstruction in annotated match arms — ctor instantiation leaks family param names ( n , a ) into user scope and collides with user vars. Fix = rename ctor-internal params \_\_fam\_\* on bind. Owned by user's other session — do not duplicate.
- T-062 Self-recursive handle SOF under hnl run — related to compiled self-recursion (phase-4 WS-C / decision 6); repro documented in progress logs.
- T-063 compile-handl residual emitter drift — 2 test fails, pre-existing, flagged not-fixed.
- T-064 LSP v2 remainder — v1 shipped (diagnostics/hover/goto-def). The v2 batch (meta-zonked hovers, completion, references/rename, multi-diag, UTF-16 cols, sequential do -scope) partially overlaps phase-4 WS-G — keep the itemized list here: zonked hover types, hover on non-identifier nodes, ctor/data bindings in defs, references+rename, completion.
- T-065 Sigma ergonomics — no eliminator surface beyond match; \<n> index binders can't serve as term-level Nat witnesses (partially covered by the exists entry — keep both).

## Cargo \[patch] tables & --locked lockfiles

- T-066 Root cause — cargo serializes \[\[patch.unused\]\] in HashMap iteration order. With one source contributing unused entries (the global ~/.cargo/config.toml sepia patches) the order is stable in practice; as soon as a manifest \[patch\] entry is also unused, two sources contribute and every re-resolution writes a byte-different lockfile, so --locked can never converge. Proof: repeated cargo metadata on one repo produced different lockfile md5s over an identical unused set .
- T-067 Fix — unused patch entries are resolution-neutral, so delete them from the manifest: host/scripts/dev-link-prune.py \<repo> reads \[\[patch.unused\]\] from the lockfile, drops those members, drops emptied tables, and keeps comments (including the # dev-link: markers); then one cargo metadata re-resolves. Result: byte-stable lockfile, --locked green, same dependency graph and versions.
- T-068 State — 147/147 repos that have a lockfile are --locked -clean ( lib-json-handl has no lockfile: zero deps). 55 repos took a chore: refresh stale Cargo.lock commit touching only Cargo.lock + Cargo.toml ; nothing was pushed.
- T-069 Run the pruner after editing any \[patch] table. removed=0 remaining-unused=\<n> means the table set is already minimal (the remaining unused entries come from global config and are harmless on their own).
- T-070 dev-link.sh reconciled — three changes: the referenced-repo set is now a transitive closure (it stopped at two hops and so silently dropped tables for lib-ffi , lib-perlin , lib-rand-stub , lib-simplex , lib-test , lib-type-handl , which the host manifest carries by hand); a re-run that would emit a second table for an already-present \[patch."…/R"\] URL now aborts with a report instead of writing the manifest into cargo's error: duplicate key ; and it prunes + re-resolves at the end. DEV\_LINK\_FORCE=1 folds every table (hand-written and generated) into the managed region — verified equivalent on host (137 members, identical lockfile md5), DEV\_LINK\_NO\_PRUNE=1 skips the prune.
- T-071 Duplicate names across patch sources are left alone — a lockfile's \[\[patch.unused\]\] records only name + version , no source URL, so when one crate name is patched from two \[patch."…"\] sources the pruner cannot tell which copy cargo meant. It skips those instead of risking an unpatch of the used copy ( skipped-ambiguous=\<n> ); consolidating such duplicates stays manual.
- T-072 Pruner marker bug (fixed in the same session) — the first version dropped a region's trailing # dev-link: end marker when its last table was pruned, which makes the next dev-link.sh run append a whole second region. It hit sdkgen-cli (repaired inside its lock commit); all 148 manifests now have matching begin/end markers.

## S1 — blockers

- T-073 type.unknown untouchable (no == / < / + / if on host/while results) — likely fixed by engine-typecheck-nbe e094277 ("type.unknown is the dynamic top") landed mid-session — re-verify before scheduling.
- T-074 Closed-record unification rejects heterogeneous ctor payloads in shared slots.
- T-075 Record/array literals crash in type.unknown -expected positions ( check\_object\_record: no 'fields' object ).
- T-076 check\_json\_value internal crash on meta/unknown/mismatched-record expected types — should be a unification diagnostic, not an internal error.
- T-077 hnl run integrated checker is stricter than hnl typecheck — same file passes one, fails the other; no run --unchecked escape hatch.
- T-078 str.replace broken — pattern never matches, \ inserted before last char (engine bug, arg order matches no documented permutation).
- T-079 Char-pattern match emits escapes one position late — engine-level match-bind/ char.chars off-by-one (repro /tmp/hnl-dogfood3/pe2.hnl ).
- T-080 Postfix f(x)\[i] binds the index to the arrow name , not the call result (parse/scope bug — r := f(x); r\[0] works).
- T-081 data ctors are file-local at runtime; import of data modules fails desugaring + call\_host misses; hnl test can't import data files at all (lib ships mk\_\* mints as workaround).
- T-082 IO\<sig> can't be constructed in checked code (only the runtime {"\_\_former\_\_":…} spelling; lib-type 's export IO doesn't materialize).

## S2 — friction

- T-083 := arrow params monomorphic across call sites (one wrapper per arg type).
- T-084 while bodies no longer unchecked islands — unknown rules apply inside.
- T-085 + is Number-only ( str.concat for strings).
- T-086 export in-module: only import \* / import \* as j work; no visibility.
- T-087 Recursion needs : T = ; no mutual recursion.
- T-088 Tuple kv\[1] indexing fails ( array\_element\_type ); match destructure works.
- T-089 Stray } silently re-nests file → unbound wire far from the real error.
- T-090 Numbers print as floats ( 3.0 ).

## Docs sweep — state and leftovers

## Remaining:

- T-091 research-dts — DONE. Unarchived, its docs commit pushed ( 6be8055 ), re-archived. Site is published and current.
- T-092 host , graphopt , eqsat , gamekit got README pointers only. Their docs/site/ trees predate this sweep and were not re-read for accuracy; the other 23 repos were authored from source. Refresh when someone owns them.
- T-093 38 sibling repos are --locked -clean but not compile-verified. engine-exec-bytecode/src/vm.rs:1842 fails E0308 ( replace wants usize , gets Option\<usize> ) in an in-flight edit, and everything downstream cannot build. Re-run the verify pass ( cargo test --workspace --lib --locked under the global flock) once it compiles. The same blocker holds the hnl extension build, and with it the lib-json-handl re-verify against e094277 . Fixed 2026-10-26 ( 1e0d2d0 -era change to std::mem::replace ); the rest of the verify pass is still owed.

## Effects / engines (2026-10-26, after multi-shot resume )

- T-094 Bytecode: nullary-ctor pattern in a closure loses its capture. A match p { VNil: …, VCons(a, b): … } inside an arrow body takes the first arm for every value on the bytecode VM (tree-walk and cranelift are correct; root-position matches are correct; families without index parameters are correct). Instrumented cause: the arrow's captured map holds only \["VCons"\] , so pattern\_match 's scope\_value("VNil") returns None and the pattern degrades to a catch-all binder. The capture set is filtered by subtree\_name\_candidates ; the minting path for this arrow is not the bytecode Instr::Closure fast path nor the shared closure op (both were tried and reverted). Next step: instrument the mint and print the candidate set. Repro: w : Vec ~> Number = p ~> match p { VNil: 0, VCons(a, b): 1 } applied to VCons(Zero, VNil) .
- T-095 perform inside a rec /loop fixpoint region is unsupported by design: the fixpoint re-evaluates its statements once per pass, so the same perform would consume a new journal entry each pass and replay can never converge. Supporting it needs journal memoization keyed by (pass, op index).

## ADR corpus — memory pass (after the 2026-10-29 compaction)

## What remains is the memory pass — sections that were absent in the legacy corpus and where neither the ADR nor its design doc records an answer:

- T-096 Not doing placeholders : 48 records carry none recorded — legacy record… . Replace only with alternatives the record actually rejected — never invent one.
- T-097 Enforcement placeholders : 35 records carry Code and tests are named in the design doc. Replace with the crate/test that makes the rule true.
