# RFC-0004: Typechecker in handl (Refactor of the Bootstrap Typechecker)

## Status

Low priority. Replaces the temporary bootstrap typechecker of
[ADR-0088](../adrs/0088-bootstrap-typechecker.md) with a handl
implementation, and completes the checking scope the bootstrap deliberately
deferred (traits ADR-0075, refinements ADR-0070, invariants ADR-0073, row
variables).

## Summary

Implement the typecheck engine in handl as a fixpoint pipeline over arrow
graphs, so the language's own typechecker is written — and typechecked — in
the language it checks.

## Detailed design

- **Types are values.** Types, arrow kinds, and constraints are handl
  values (ADR-0078 value types); the checker is arrows over them.
- **Pipeline as arrows:**

  ```text
  collect-bindings >>> infer-bottom-up >>> unify-fixpoint >>> elaborated-ast + diagnostics
  ```

  The fixpoint stages map onto `loop` (ADR-0052), giving the solver's
  iteration structure directly in the calculus; termination is guaranteed
  by a monotonicity `law` checked by ADR-0075 traits.
- **Structural unification with row variables** (ADR-0074 ID.7) is the core
  arrow library; branded values (ADR-0055) supply nominal tags where
  identity matters.
- **Diagnostics with spans** reuse the `Diagnostic` record from RFC-0003.
- **The self-check property:** the typechecker source typechecks under
  itself — the bootstrap checker (ADR-0088) is used as an independent
  oracle during migration, then as a regression cross-check.

## Migration

1. Implement the checker in handl against the bootstrap's table-driven test
   corpus (same fixtures, same expected reports).
2. Add the deferred scope (traits, refinements, invariants, row variables)
   incrementally, each with its own test corpus.
3. Flip `hnl check`; retire `crates/plugin-engine-typecheck` as the primary
   (Rust version remains the fallback per ADR-0085 rule 6), and re-host the
   `plugin-engine-typecheck*` engine path once its consumers move too.

## Drawbacks

- The most perf-sensitive of all tooling: solver unification in
  interpreted handl will be too slow for large modules; this RFC is blocked
  on the compiled backends (bytecode, cranelift) being production-ready.
- Highest self-hosting risk (a checker bug silently accepts bad code);
  mitigated by the differential-testing oracle above.

## Related

- [ADR-0088](../adrs/0088-bootstrap-typechecker.md), ADR-0054, ADR-0052,
  ADR-0070, ADR-0073, ADR-0075, ADR-0078, [RFC-0007](0007-cli-in-handl.md).
