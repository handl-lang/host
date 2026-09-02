# ADR-0071: Failure-Tracking Arrows for Backtracking Parsers

## Status

Proposed

Builds on [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md).

## Context

The current `Choice` combinator models conditional branching: run the
left arm OR the right arm based on a branch decision. This works for
deterministic choice (if-then-else, pattern matching) but not for
**backtracking** — where the parser tries branch A, and if it fails,
falls back to branch B.

Backtracking parsers need **failure as a first-class effect**:
- `parse_A` may fail (the input doesn't match)
- On failure, the parser backtracks and tries `parse_B`
- The output is either a successful parse or a failure value

The current arrow calculus has no notion of failure. `Choice { l, r }`
runs one arm or the other, but neither arm can "fail" and trigger the
other. This means:

- Recursive descent parsers without backtracking work (ADR-0070 proves
  this with the Pratt parser)
- Context-free grammars with arbitrary backtracking don't work
- PEG (parsing expression grammar) parsers with ordered choice don't
  work

## Decision

Extend the arrow calculus with **failure-tracking arrows** to support
backtracking parsers. This is a research extension, not an engineering
task — it requires new calculus primitives and new inference rules.

### Proposed extension

Add a `Try` combinator to the arrow calculus:

```
try(A, B) : X ⇝ Y
  -- Run A. If A succeeds, return its output.
  -- If A fails, run B on the original input.
```

This is different from `Choice`:
- `Choice { l, r }` — the branch decision is external (part of the input)
- `Try { l, r }` — the branch decision is internal (A fails → try B)

### Inference implications

The inference engine needs to handle `Try`:
- `wp(try(A, B), post) = wp(A, post) ∧ wp(B, post)` — both arms must
  establish the postcondition (since either could run)
- The guarantee of `try(A, B)` is `guarantee(A) ∨ guarantee(B)` —
  disjunction, not conjunction
- Failure propagation: if A fails, its guarantee doesn't apply; only
  B's guarantee holds

This requires the inference engine to handle **disjunctive guarantees**,
which the current engine doesn't support (it conjoins via Seq, branches
via Choice, but doesn't disjoin).

### Alternative: model failure as a value

Instead of extending the calculus, model failure as a sum type:
```
parse_A : Input ⇝ Result AST + Error
```

Then backtracking is:
```
backtrack = comp(parse_A, choice(extract_success, comp(extract_error, parse_B)))
```

This stays within the current calculus but requires:
- Sum types in the refinement type system
- Pattern matching on Result
- Contracts that distinguish success from failure

This is more engineering than research, but still significant.

## Consequences

### Positive

- Enables backtracking parsers (PEG, context-free grammars)
- Failure as a first-class effect is useful beyond parsers (error
  handling, optional values, partial functions)

### Negative

- New calculus primitive (`Try`) or new type machinery (Result + pattern
  matching)
- Inference engine needs disjunctive guarantee support
- More complex fixpoint computation (disjunction in the invariant domain)
- Research risk: may not terminate or may produce weak results

## Status

This is a research direction, not an implementation plan. The current
architecture handles recursive descent parsers without backtracking
(proven by the Pratt parser). Full backtracking support requires this
extension.

## Related ADRs

- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
