# Canonical surface as the arrow calculus

## Status

Proposed

## Context

The canonical surface syntax (`~>`, `>>>`, `***`, `&&&`, `do`, `=`) is
intended to be arrow-native, but the current runtime keeps a mixed AST with
`block`, `assign`, `function`, `call`, and `object` nodes. Those are not arrow
calculus forms; they are lambda-style concepts that the design intended to
eliminate.

The reference model is the **arrow calculus** used in the research prototype
(`research/arrow-calculus-kernel-poc/PROTOTYPE.md`) and the papers of Hughes,
Lindley–Wadler–Yallop, and Sanada:

- `arr` — lift a pure function to an arrow
- `>>>` / `comp` — sequential composition
- `first` — act on the first component of a pair
- `choice` — branch on `Either`
- `loop` — feedback

Crucially, the research notes and the prototype explicitly state that the
arrow calculus **has no `λ` and no binder** (`docs/explanations/local-data-without-closures.md`):
"The arrow calculus has no `λ`, so it has no binder, so it has nothing
captured." Variables and `let` may appear in the _surface_ and in an
intermediate `Expr` (as in the prototype's `Var`, `Lambda`, `Let`), but the
kernel `Term` does not contain them.

The internal `Term` kernel in `plugin-engine/src/term.rs` already matches this
model: `Arr`, `Op`, `Seq`, `First`, `Choice`, `Loop`, `App`, `Handle`, etc. It
does not have a `Lambda` or `Var` constructor. The `Arr` constructor is
"Sanada's `arr`" — a pure constant arrow — and `Op` is the named operation
form. `~>` and `do` are therefore _syntactic sugar_ that the parser/lowerer
eliminates into point-free arrow graphs.

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

## Rationale for `Arr`, `Op`, and `Handle`

The three leaf-like constructors are not interchangeable:

- `Arr` = a pure, fixed arrow whose behaviour is supplied by the host runtime.
  The `~[FFI]~>` grade on `call` in `handl-packages/ffi/ffi.hnl` is this
  pattern: a shallow, runtime-injected effect with no user-visible continuation.
- `Op` = an effect **request** with an explicit continuation `k`. It is required
  for user-space algebraic handlers because the handler must inspect and rebuild
  the continuation tree (`Pure v | Op op arg k` in the freer-arrow model).
- `Handle` = a handler scope / model that interprets `Op` requests and removes
  the corresponding grade from the row.

`Op` cannot be collapsed into a graded `Arr` of type `(HandlerEnv, A) -> B`,
because that makes the continuation an opaque function and the handler cannot
pattern-match on the rest of the computation to rewire it. `Arr` + grade is the
right primitive for host/FFI effects; `Op` + `Handle` is required for user-space
algebraic handlers. The `Term` kernel therefore retains all three.

## Consequences

- `Term` does **not** gain `Lam` or `Var`. The kernel stays combinator-based.
- `~>` and `do` are surface sugar, not semantic forms.
- A canonical-to-`Term` lowerer must implement the lambda-to-arrow
  translation, building pairs and fanout from `arr`, `comp`, and `first`.
- All canonical tooling (parser, typechecker, formatter, linter) must produce
  or consume `Term` graphs.
- Runtime eval engines and back-end compilers (JS, LLVM) interpret `Term`
  directly.
- This is a breaking, multi-crate refactor.
