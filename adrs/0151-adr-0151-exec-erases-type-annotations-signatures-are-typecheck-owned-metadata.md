+++
id = "ADR-0151"
title = "ADR-0151: Exec erases type annotations — signatures are typecheck-owned metadata"
status = "open"
priority = "medium"
+++

---
title: Exec erases type annotations — signatures are typecheck-owned metadata
type: adr
date: 2026-10-21
status: accepted
applies_to: '`host` (`handl-runtime`, `plugin-lib-handl`, `plugin-lib-stdlib`), `kit-prelude-lang`, `lib-loader`'
---

# ADR-0151: Exec erases type annotations — signatures are typecheck-owned metadata

## Context

Module prep (`transform_export_to_object`) rewrites top-level
`export name <::|:=|:|=> expr` declarations into an object literal the
exec engine evaluates. It already erased two annotated forms —
`export Name: Type` binds `null`, and `export f: Sig := body` binds the
body — but two holes remained where *type expressions* still reached
the evaluator:

- `export Name := A ~[N]~> B` — a signature binding whose RHS `dim_arrow`
  evaluates its source/target children to mint an Arrow descriptor
  record. Boundary children are not evaluable: the lib-cli port's
  `export Run := ([String]) ~[1]~> Result<Output, Spawn>` crashed eval
  because `Result<Output, Spawn>` is a type-level generic application
  (a `dimension` node) whose former application is not a runtime arrow.
  Likewise `export Argv := (String, [String], ...) ~> [String]` mints a
  closure whose `[T]` list-type params can never bind.
- `eval.type-annot` — the canonical eval scheme projected the annotation
  child, bound its *value* into the environment, then evaluated the
  subject. Any `x: T` inside executed code therefore evaluated `T` at
  runtime, which crashes on the same generic-application shapes.

This is an Agda-style violation: annotations are static metadata owned
by the typecheck engine; the exec/tree-walk engine must erase them,
never evaluate them.

## Decision

1. **Signature exports erase to `null`.** `is_signature_rhs(rhs)`
   detects a signature RHS — a depth-0 graded arrow `A ~[N]~> B`, or an
   ungraded `(…, [T], …) ~> B` whose parenthesized params contain a bare
   `[`/`{` element (a list/record type that `closure` can never bind;
   `x: [T]` annotations and `xs[i]` indexes don't count) — and module
   prep binds `null`, the same value type-only `export Name: Type`
   gets. `dim_arrow` never mints a callable, so no runtime behavior is
   lost; typecheck reads the raw AST and is unaffected.
2. **The mirror stays in lockstep.** The transform is applied to every
   copy of export-to-object: `handl-runtime::module_prep`, the
   `plugin-lib-handl` bootstrap, the `plugin-lib-stdlib` loader, and
   lib-loader's `index.hnl` (`is_signature_rhs`/`type_only` mirror).
3. **`eval.type-annot` evaluates the subject only.** The canonical
   scheme in `kit-prelude-lang` no longer evaluates or binds the
   annotation child. `type.type-annot` still synthesizes the annotation
   for typechecking — only exec changed.

Commits: `host` cc1b65fd, `kit-prelude-lang` 8987453,
`lib-loader` 15c06c6a.

## Consequences

- Private top-level bindings keep evaluating — e.g.
  `read = (Int) ~[1]~> Nat` is still consumed as a `handle` target, and
  `f ~[2]~> g` inside do-blocks still mints the n-cell records `>>>[k]`
  pastes on. Only *exported* signature RHSs erase.
- **Caveat:** top-level `export law := f ~[2]~> g` cells also erase —
  the detector sees a depth-0 `~[` and cannot distinguish a
  morphism-cell from a signature arrow. Accepted as signature-metadata
  semantics: an exported `A ~[N]~> B` cell was never callable anyway
  (`dim_arrow` mints a descriptor record, not a function). Revisit only
  if exported higher-cell expressions need runtime values.
- The four export-transform mirrors must move together; a divergence is
  a loader-parity bug (lib-loader's `.hnl` copy is the self-hosting
  path).
