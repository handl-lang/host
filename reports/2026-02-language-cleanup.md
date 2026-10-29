---
title: 'Report: Language cleanup, transform engine, and engine-parity push'
type: report
date: 2026-02
---

# Report: Language cleanup, transform engine, and engine-parity push

Date: 2026-02 (this session). Merge heads: `c0ba7dbf`, `efa6cf7e`, `77c61969`.

This report covers three merged rounds of work: the ADR-0094 transform-engine
roadmap completion, language cleanup (`let` removal, genericity audit), and
the final arc (broken-main repair, dialect routing, import unification,
compiled-engine parity, AST caching).

## 1. Transform engine roadmap (ADR-0094) — complete

- **`AstNode.span` populated** across all Pratt productions
  (`e26d7bbc`): every node carries byte offsets — atoms, infix, delimited
  containers, statements.
- **AST-derived lint fixes** (`8fa2d273`): `simplify-paren` computes fix
  ranges from node spans (token-walk helpers deleted); LSP quickfix and
  `hnl transform --plan` ride real spans.
- **Lower-pass audit** (`9ed85387`): honest result — the object-key
  heuristic was the only real lowering candidate (already covered by
  `transform.lower.entry`); documented retained fallbacks + cranelift
  non-opt-in (dep cycle); added a bypass test.
- **Check-path validation** (`9e677d2c`): the typechecker bootstrap now
  embeds `transforms.yaml`, lowers `desugar:`, and runs
  `validate_transforms`; errors surface as `language error: ...`.
  `hnl check` added as a `typecheck` alias.
- **Parallel engine verdict** (`7542f64d`): sweep 500→50k stmts showed no
  crossover — merge overhead dominates. Default threshold raised to 1M
  nodes; shared-arena noted as the future lever.
- **Formatter-preserving emitter** (`109c79f1` + `9c78787a`):
  `plugin-engine-fmt::PreserveEmitter` — trace-driven splice: unchanged
  subtrees copy `src[span]` verbatim, changed subtrees re-render via the
  data-driven `FormatEngine`. Powers `hnl transform --emit source` /
  `--write`; refuses on lowering passes that produce internal-form nodes.
- **LSP rewrite action** (`43465f35`): `apply_rule` → `PreserveEmitter` →
  `WorkspaceEdit` bridge; "Apply rewrite" quickfix for simplify-paren.
- **ADR-0093 retro-doc** (`3bf183d3`): the str/char/unicode layering ADR
  was missing; written retroactively incl. the four runtime-fix rationales.

## 2. Language cleanup

- **`let` removed entirely** (`ea8fcd4e`): grammar/ast/eval/types rows
  deleted; `token.keyword.let`/`lex.keyword.let` kept as a tombstone so
  `let x :: v` fails with `unexpected token` rather than silently lexing
  as an identifier. `stmt.let` had no producer — deleted. Corpus cases
  rewritten to `do { x = y; ... }` binds. ADR-0079 amended.
- **Pratt `match`/`data`/`from` branches closed** (`b12df6d8`,
  `4f700d54`): three new generic `AstShapeRef` fields —
  `child_binding_powers`, `keyword_clauses` (optional `open <expr> close`
  after a fixed child), `payload_extract` — plus `trailing_tokens`
  (`TokenStep` = one-of kinds + optional + payload capture) covering
  `import * as alias`. ~150 lines of special-casing deleted; the Pratt
  dispatch is now fully data-driven.
- **Token categories** (`845d0704`): `TokenKindDef` gained optional
  `category`/`pair`/`doc` fields; all `token.*` literals eliminated from
  host-lsp, host-tooling, plugin-ext-fmt, plugin-ext-lint.
- **Final genericity audit** (`bed3ef8e`): "~70 hits" was stale — real
  residual is ~29, all documented bootstrap/identity-specific.
  `arrow_parser` ruled intentional bootstrap (combinator vocabulary).
- **ADR-0092 pure-handl test framework** (`27e299c3`, cherry-picked from
  `chore/kit-data-config-fold`): all 12 Rust handls →
  `handl-packages/test/*.hnl` (`assert`, `property`, `benchmark`, `fuzz`,
  `snapshot`, `test`); `plugin-engine-test::framework` reduced to the
  assert-failure side-channel.
- **Declarative desugar** (`6af2e51f` + `7760f9df`, reconciled in
  `d6708bdc`): 5 of 8 desugar natives migrated to `transforms.yaml`
  (`while`, `do-while`, `for`, `nullish`, `opt-chain`) — the assumed
  blockers (fresh-name generation, recursion) turned out unneeded for
  fixed `__`-binders; `RewriteStep::Fresh` landed for hygiene anyway.
  Only `desugar.data`/`desugar.match_recursor` stay native (constructor
  iteration + env lookup — macro-system territory).
- **ADR-0089 tail** (`1dbcbe67`): suite events already emitted
  per-module; reporters now render `== module ==` headers, `suites` JSON
  array, and `expected`/`actual`/`diff` failure detail.

## 3. Final arc — repair + parity

- **Broken main repaired** (`d6708bdc`): `transforms.yaml` had two
  concatenated rule generations → every `hnl` command failed at Language
  assembly. The tangle also exposed **three pre-existing eval bugs**:
  - block `x = y` never updated enclosing scopes (define-only);
  - `rec` fixpoint couldn't detect nested writes;
  - `__fresh_*` names collided with the `_` wildcard convention.
    All fixed in tree-walk + bytecode. **Semantic change: block assigns are
    now update-or-define** — `if c { x = 5 }` mutates an outer `x`
    (required for the desugar to be meaningful; documented in AGENTS.md).
- **Dialect routing** (`f3e9e73b`): the "32 unparseable files" were
  mostly _dialect_ files — `hnl typecheck` now routes contract files
  (`axiom`/`requires:`/`guarantee:`) to `parse_contracts` and arrow-module
  files (`cap`/`op`/`handl`/`tag`, `arr`/`>>>`/`|||` graphs) to
  `parse_arrow_module`. `axiom` got a canonical alias row. Legacy `->`/
  `|>`/`rec {}`/leading-`>>>` sources rewritten. **Parse failures:
  30 → 0; typecheck failures: 128 → 57** (remaining are genuine
  unbound-wire env gaps).
- **Import unification** (`ecb81343`, ADR-0096 amendment): `import
"<spec>"`, `from "<spec>" import <name> [as x]`, `from "<spec>" import
  - [as x]`all resolve through`prim_import`in all three engines with
named-projection errors; module-name (host-chain) declarations
documented as decorative under`hnl run`/`test`; arrow-loader
`import x from "p"` documented as dialect-fixed.
- **`Bind` on compiled engines** (`15595901`): `eval_rec` +
  update-or-define `ScopeStack::assign` in bytecode and cranelift —
  **all 44 handl-defining-handl corpus cases now run on all three
  engines**; new `bind_rec_parity.rs` exercises real `do {}`/`while`/
  `do-while`/`for` source through `Runtime::run` per engine.
- **AST cache** (`c12c498d`): `HandlFunction::call` caches parsed
  applied-form ASTs per `(module_id, func, arity)`; tree-walk gained
  `eval_shared(&Rc<Ast>)` eliminating per-call arena clones.
  **~19× warm-call speedup, ~7× on loader workloads.** `loader.hnl`
  remains **opt-in**: interpreted `str.*`/`char.*` eval is still ~43 s on
  a 22k-char module (module-record caching was tried and reverted — env
  serialization OOMs).
- **Docs** (`454c5fbe`, `3f944937`): README crate map + `hnl` subcommands
  rewritten; ADR-0095 collision resolved (module-resolution → 0096);
  dogfood gate documented (CI workflow exists, informational until clean).

## Remaining open items (by design or blocked)

| Item                                                                 | Status                                                                                      |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `desugar.data` / `desugar.match_recursor` natives                    | Permanent native floor unless transform vocabulary grows env lookup + constructor iteration |
| `f(...xs)` call-site spread                                          | Needs an eval-splice op; no live usage yet                                                  |
| `loader.hnl` as default                                              | Blocked on interpreted-eval cost; AST cache landed but insufficient alone                   |
| Typecheck env gaps (~57 files, `[unbound-wire]`)                     | Match-arm binder scoping, module envs — real remaining checker work                         |
| Dialect routing for `lint`/`fmt`/`run`/`test`                        | Only `typecheck` routes dialects (dep-cycle constraint)                                     |
| RFC-0001–0007 self-hosting, arrow macros, registry module resolution | Backlog by design                                                                           |
| `{ x :: v }` multi-statement brace bodies                            | Canonical form is `do { ... }`; object-literal ambiguity documented                         |
