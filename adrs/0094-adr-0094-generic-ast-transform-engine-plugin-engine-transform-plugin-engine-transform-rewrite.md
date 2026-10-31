+++
id = "ADR-0094"
title = "ADR-0094: Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)"
status = "open"
priority = "medium"
+++

---
title: Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)
type: adr
date: 2026-09-06
updated: 2026-10-29
status: accepted
status_note: |-
  Accepted — implemented on `adr0094-transform-engine` (phases 1–5; see
  "Migration sequence", all steps landed).

  An instance of the data-driven engine program
  ([ADR-0044](./0044-language-as-data-and-runtime-split.md) "language as
  data") and the abstraction/impl crate split used by `plugin-engine-exec` →
  `plugin-engine-exec-{tree-walk,bytecode,cranelift}`. Subsumes the
  declarative desugar machinery of
  [ADR-0053](./0053-generic-pratt-parser.md) §7, the `when` conditions of
  ADR-0058, `KeepRest` of ADR-0060, and the child-role resolution of
  [ADR-0081](./0081-data-driven-ast-child-roles.md).

  > **Terminology.** A **transform** is a pure `Ast → Ast` rewrite: it never
  > evaluates, typechecks, or emits. A **pass** is a named, ordered set of
  > transform rules plus a traversal strategy (e.g. `desugar`, `lower`,
  > `optimize`). A **rule** (`TransformDef`) maps a node kind to a rewrite
  > within a pass.
---

# ADR-0094: Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)

## Decision

1. **Two crates, mirroring the exec split.** `plugin-engine-transform` is the
   contract and vocabulary (no rewriting logic);
   `plugin-engine-transform-rewrite` is the first implementation.
2. **`TransformDef` is a first-class `Fragment` in `handl-language`.** Its
   identity is `pass + ":" + node_kind`; `Language` gains
   `transforms: FragmentSet<TransformDef>` and `transforms_for(pass, kind)`.
3. **The rewrite vocabulary generalizes `AstDesugarChild`.** `Keep`, `KeepPath`,
   `KeepRest` and `Create` keep their semantics; a payload patch and a native
   escape (`Native(name)`) complete the rule shape.
4. **Traversal is part of the pass definition** — `BottomUp`, `TopDown` or
   `Fixpoint { max }` — with copy-on-write into a fresh arena and a
   `remap: Vec<Option<usize>>` table (the proven `apply_desugar` algorithm).
5. **`RuleSource` is the seam.** The default reads `lang.transforms`; an
   alternate source loads codemod/YAML rules without merging them into the
   `Language`.
6. **Desugar migrates onto the engine.** `handl-runtime::desugar_pass` becomes a
   thin delegate; the engine is the only rewrite path.
7. **Crate layout has no cycles**: contract/vocabulary, rewrite implementation,
   then consumers (desugar, lint, LSP code actions, formatter).

## Why

Every AST-rewriting stage hand-rolled the same machinery — an arena walk, a
remap table, per-kind dispatch, copy-on-write construction — and each new pass
added another Rust copy. ADR-0044 (language as data) requires transforms to be
**data** (`lang.transforms[pass][kind]`), not per-stage Rust; this engine is
that data path, and desugar was its first consumer.

## Not doing

- **None recorded.** The deferred items (a parallel implementation and a
  layout-preserving emitter) both landed later; the design doc records them.

## Consequences

- Any pass — `lower`, `optimize`, codemod, LSP refactor — is data plus a
  `run_pass` call, with no new engine code.
- Dispatch is `lang.transforms[pass][kind]` and child access is role-based; the
  old `try_rust_desugar` `kind ==` side channel is gone.
- `RewriteTrace` gives diagnostics and the LSP its input→output node mapping;
  `find_matches` exposes the match half to lint and LSP.
- Cost: every pass copies into a fresh arena with a remap table — O(n) per pass,
  accepted for immutable `Ast` and parallel-safe subtrees.

## Enforcement

- `crates/plugin-engine-transform/`, `crates/plugin-engine-transform-rewrite/`
  — the contract and the engine.
- `crates/handl-runtime/src/desugar_pass.rs` — desugar as a thin delegate.
- `crates/plugin-ext-lint/`, `crates/host-lsp/src/code_actions.rs` — the
  consumers that prove genericity.

## Design

The mechanism, tables, migration sequence and phase detail:
[`design/0094-generic-ast-transform-engine.md`](../design/0094-generic-ast-transform-engine.md).
