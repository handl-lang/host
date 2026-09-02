# ADR-0071: Failure-Tracking Arrows for Backtracking Parsers

## Status

Accepted

Builds on [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md).

## Context

The current `Choice` combinator models conditional branching: run the
left arm OR the right arm based on a branch decision. This works for
deterministic choice (if-then-else, pattern matching) but not for
**backtracking** — where the parser tries branch A, and if it fails,
falls back to branch B.

Backtracking parsers need **failure as a first-class notion**:
- `parse_A` may fail (the input doesn't match)
- On failure, the parser backtracks and tries `parse_B`
- The output is either a successful parse or a failure value

The question is whether backtracking requires extending the arrow
calculus with a new combinator, or whether the existing combinators
already suffice.

- Recursive descent parsers without backtracking work (ADR-0070 proves
  this with the Pratt parser)
- Context-free grammars with arbitrary backtracking appear to need
  something beyond plain `Choice`
- PEG (parsing expression grammar) parsers with ordered choice appear
  to need the same

## Decision

Do NOT extend the arrow calculus. Model failure as a value using sum
types, which the calculus already supports via `left` (Choice).

### Result-type modeling

Model parser output as a sum type `Result AST + Error`:

```
parse_A : Input ⇝ Result AST + Error
```

Backtracking is then expressed entirely with existing combinators:

```
backtrack = comp(parse_A, choice(extract_success, comp(extract_error, parse_B)))
```

Concretely: run `parse_A`, then use `left` (Choice) to branch on the
result — `inl(success)` returns the AST, `inr(error)` runs `parse_B`
on the original input. This is plain `Choice` over a sum-typed output,
not a new combinator.

This stays entirely within the existing 5 combinators (Seq, First,
Choice/left, Loop, App). No new calculus primitives are introduced.

### Contracts

Contracts distinguish success from failure. `parse_A` guarantees:

```
IsSuccess(output) ∨ IsError(output)
```

The success branch carries the actual parse guarantee (e.g., "the
output AST is well-formed and consumes a valid prefix of the input").
The error branch carries the failure guarantee (e.g., "the error
records the position where parsing failed").

### Inference implications

The inference engine already handles `left` via WP branching through
Choice. When the output is a sum type, WP branches:

- `inl` branch: `wp` assumes the success invariant and propagates the
  parse guarantee
- `inr` branch: `wp` assumes the error invariant and propagates into
  the backtrack arm

No new inference rules are needed. The disjunction `IsSuccess ∨ IsError`
is handled by the existing Choice branching mechanism — the same
mechanism that handles if-then-else and pattern matching. The fixpoint
computation is unchanged: it discovers invariants over the sum-typed
feedback channel, same as any other Loop.

## Consequences

### Positive

- No new calculus primitives — backtracking uses only Seq, First,
  Choice/left, Loop, App
- No new inference rules — WP branching through Choice already handles
  the disjunction
- Failure is a value, not an effect — the graph remains deterministic
  dataflow, fully within the scope defined by ADR-0070
- Leverages existing Choice/left machinery — sum types are already part
  of the calculus
- Generalizes beyond parsers: any computation with a success/failure
  outcome (error handling, optional values, partial functions) uses the
  same pattern

### Negative

- More verbose graph structure — each backtracking point requires an
  explicit `choice` with success/error extraction, rather than a single
  combinator
- The parser author must manage the `Result` type explicitly (construct
  it on success/failure, destructure it at branch points)
- Sum types in the refinement type system must be well-supported
  (ADR-0068 covers predicates as types; sum types with contracts are
  the natural extension)

## Status

Accepted. The approach is to model failure as a value using sum types
and existing Choice/left machinery. No calculus extension is required.
Backtracking parsers are expressible within the current architecture.

## Related ADRs

- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0073 — State Machines as Loops](./0073-state-machines-as-loops.md)
- [ADR-0069 — Modules](./0069-modules.md)
