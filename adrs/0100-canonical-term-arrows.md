---
title: Canonical surface as the arrow calculus
type: adr
date: 2026-09-14
status: proposed
status_note: Proposed
---


# Canonical surface as the arrow calculus

## Decision


Make the canonical internal form the arrow-calculus `Term` with no `function`,
`call`, `block`, `assign`, or `object` nodes. Every canonical program lowers
to a `Term` graph built from the existing constructors:

- `Arr` — pure constant arrow / lifted pure operation (`arr`)
- `Const` — pure constant value (used as an argument)
- `Op` — named native/foreign operation
- `Seq` — composition (`>>>`)
- `First` — act on first component
- `Second` — act on second component (derived from `first` + `arr swap`)
- `Choice` — branch on `Either`
- `Loop` — feedback
- `App` — higher-order arrow application
- `Handle` — capability handler

Canonical lowering:

- `x ~> body` → point-free `Term` sugar. The parser/lowerer eliminates the
  variable `x` using `arr` of pure functions and the product combinators
  (`first`, `second`, `&&&`, `***`) as in the research prototype's
  "Lambda-to-Arrow Translation". If `body` is not point-free expressible, it
  is lifted to a pure `FunctionValue` and wrapped in `Arr`.
- `f(x)` / `f y` → `App { callee: f, args: [x] }` when `f` is dynamic, or
  `Seq` when `f` is a known arrow.
- `do { a = b; c }` → `Seq(b, ...)` with the result threaded through the
  input. `=` is not a semantic node.
- `{ first: a, last: b }` → an `Op`/`Arr` that constructs the record from the
  arrow inputs.
- `a + b`, `a * b` → `Op { "math.add" / "math.mul", ... }`.
- literals (`10`, `7`) → `Const`.

The existing generic AST/eval runtime is replaced or augmented by an engine
that interprets `Term` directly. `eval.yaml` becomes a description of how each
`Term` constructor is interpreted.


## Why

The canonical surface syntax (`~>`, `>>>`, `***`, `&&&`, `do`, `=`) is intended to be arrow-native, but the current runtime keeps a mixed AST with `block`, `assign`, `function`, `call`, and `object` nodes. Those are not arrow calculus forms; they are lambda-style concepts that the design intended to eliminate.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- `Term` does **not** gain `Lam` or `Var`. The kernel stays combinator-based.
- `~>` and `do` are surface sugar, not semantic forms.
- A canonical-to-`Term` lowerer must implement the lambda-to-arrow
- All canonical tooling (parser, typechecker, formatter, linter) must produce
- Runtime eval engines and back-end compilers (JS, LLVM) interpret `Term`

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0100-canonical-term-arrows.md`](../design/0100-canonical-term-arrows.md).
