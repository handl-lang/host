# RFC-0003: Linter in handl (Refactor of the Bootstrap Linter)

## Status

Low priority. Replaces the temporary bootstrap linter of
[ADR-0087](../adrs/0087-bootstrap-linter.md) with a handl implementation.
Carries forward the useful conventions of ADR-0015 (severity levels,
exit-code contract, host-namespaced rules) into the handl-native design.

## Summary

Rewrite the linter in handl: rules are pure arrows `Ast ~> Diagnostics`,
selected by composition rather than registry, generic over any handl
language that provides an AST value.

## Detailed design

- **Rule shape:** `rule : { ast, tokens, file } ~> [Diagnostic]`. Rules are
  pure — no IO, no host calls — which makes them inspectable by the analyzer
  handler (same-description principle, ADR-0052).
- **Diagnostics as values:** `Diagnostic { rule, severity, span, message,
fix? }` as a handl record; `Fix` is a text-edit record so `--fix`
  application can itself be expressed and composed in handl.
- **Rule selection by composition:** a lint configuration is a list of rule
  arrows; `lint = fold (>>> collect) rules`. Host-specific rules
  (ADR-0015's namespacing concern) become ordinary modules imported into the
  composition, namespaced by module path — no special machinery.
- **`--fix` as a pipeline:** `lint-fix = lint >>> take-fixable >>> apply-edits
  > > > reparse >>> lint` iterated to a fixpoint, mirroring the bootstrap
  > > > behavior in ADR-0087.
- **Cross-module rules** (unused export, dependency declaration per the old
  ADR-0015 list) run over module _groups_: the runner builds the module
  graph once and hands each rule the reachable subgraph.
- **Coverage of the bootstrap rule set:** all ADR-0087 rules port 1:1; their
  acceptance tests are the same fixture files, relocated.

## Migration

1. Port rule fixtures; run handl lint and `plugin-ext-lint` side by side in
   CI over `handl-packages/` until outputs match.
2. Flip `hnl lint` to the handl implementation.
3. Retire `crates/plugin-ext-lint` as the primary (Rust version remains the
   fallback per ADR-0085 rule 6).

## Drawbacks

- Lint rules over large module graphs are perf-sensitive; the compiler
  backend (bytecode/cranelift engines) must be ready for the cross-module
  phase.

## Related

- [ADR-0087](../adrs/0087-bootstrap-linter.md), ADR-0015,
  [RFC-0004](0004-typechecker-in-handl.md) — `unbound-wire` delegates to the
  handl typechecker's scope data, [RFC-0007](0007-cli-in-handl.md).
