# RFC-0005: Test Runner in handl (Refactor of the Bootstrap Test Runner)

## Status

Low priority. Replaces the temporary bootstrap test runner of
[ADR-0089](../adrs/0089-bootstrap-test-runner.md) with a handl
implementation.

## Summary

Rewrite the test runner in handl. Discovery, selection, and reporting
become arrows over test-plan values; execution stays delegated to the
runtime handler. Adds parallel execution, properties, and benchmarks —
the pieces deliberately excluded from the bootstrap.

## Detailed design

- **Test plan as data:** a `TestPlan` is a handl value (list of
  `{ module, name, description }`). Discovery is an arrow
  `Paths ~> TestPlan`; filtering is a pure arrow `TestPlan ~> TestPlan`
  (so `--filter` is just composition).
- **Execution via handler dispatch:** running a test is a capability
  request to the runtime handler; a local handler runs it in-process, a
  distributed handler can shard the plan across workers (absorbing the
  backlog's async/operation-dispatch work). The runner never interprets
  assertions itself — unchanged from ADR-0089.
- **Parallel execution** maps naturally onto ADR-0072's concurrent arrows:
  `test-plan >>> fan-out >>> collect-results`, with per-test isolation
  guaranteed because handl values are immutable.
- **Properties and fuzzing:** a `property "name" { ... }` declaration form
  beside `test`; the runner supplies shrinking via stdlib generators.
  Benchmarks (`bench "name"`) are collected but not run under `hnl test`.
- **Reporting:** unchanged contract from ADR-0089 (PASS/FAIL/SKIP lines,
  `--json`, exit 1 on failure) so CI consumers don't change during
  migration.

## Migration

1. Port the runner's own conformance tests; run both runners on the stdlib
   suite until reports are identical.
2. Flip `hnl test`; add `property`/`bench` declarations.
3. Retire `crates/plugin-lib-test` as the primary (Rust version remains
   the fallback per ADR-0085 rule 6), removing the `test` declaration
   handling from the Rust parse plugin at the same time.

## Drawbacks

- Concurrency semantics of the eval engine must be stable (ADR-0072) before
  parallel runs are trustworthy.

## Related

- [ADR-0089](../adrs/0089-bootstrap-test-runner.md), ADR-0072
  (concurrent arrow calculus), [RFC-0007](0007-cli-in-handl.md).
