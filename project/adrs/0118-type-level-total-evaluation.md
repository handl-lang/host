# ADR-0118: Type-Level Total Evaluation of handl Expressions

## Status

Proposed

## Context

ADR-0105 replaced the Rust NBE typechecker with a Handl-authored model in `project/design/adr0103/typecheck_model.hnl`. The model currently unifies types structurally but does not reduce type-level handl expressions.

For example, `array.take<T, N>` is declared to return:

```hnl
List(T, math.min(N, array.len(xs)))
```

A call `array.take([1, 2, 3, 4, 5], 2)` should therefore produce `List(Int, 2)`. Today `hnl typecheck` accepts both `taken: List(Int, 2) = ...` and `taken: List(Int, 3) = ...` because the length term is stored as an unevaluated expression and never compared.

ADR-0032 establishes that types are values and share the same `Value` space as runtime terms. This makes it natural to evaluate type-level expressions with the same handl machinery used at runtime, provided that evaluation is restricted to the total fragment.

## Decision

Treat the type-level language as handl itself, restricted to a total subset. Before unifying type arguments of indexed families such as `List(T, n)`, the typechecker will:

1. Substitute the concrete call-site type and value arguments into the function's declared return type.
2. Reduce the resulting handl expressions by evaluating them in a **total** interpreter (no effects, no general recursion, only structurally terminating definitions marked with `~[Total]~>`).
3. Compare the reduced indices during unification. If the reduced values differ and neither side is a metavariable, report a type error.
4. Leave expressions that cannot be reduced (because they contain unresolved variables or metavariables) in normal form and continue unification; do not fail unless a concrete mismatch is found.

The same handl syntax, standard-library functions, and user-defined arrows may be used at the type level, as long as they are total. No special type-level surface syntax is introduced.

## Consequences

- `List(T, n)` and similar indexed families can carry and check statically known length information.
- Calling `array.take([1, 2, 3, 4, 5], 2)` and asserting the result has type `List(Int, 3)` will fail typechecking.
- Type-level programming reuses handl's existing evaluation engines, libraries, and tooling.
- The total evaluator must prove or enforce termination; non-total functions cannot be invoked at the type level.
- The Handl typecheck model (`typecheck_model.hnl`) will need a new normalization step, or a native `eval_total` op that the model can call before unifying indices.

## Related work

- ADR-0032 (Types are Values)
- ADR-0054 (Generic Typecheck Engine)
- ADR-0105 (Handl-authored Typecheck Model)
- `project/design/adr0103/typecheck_model.hnl`
- `playground/take-demo.hnl` and `handl-packages/collections/array/take.hnl`
