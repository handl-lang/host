# ADR-0002 (Remaining): Cleanup of `plugin-*` and `lib-*` Crates to Pure `Behavior<Op>`

## Status

Proposed

## Context

ADR-0002 (Single `Plugin` Model for Host, Operations, Libraries, Targets, and Options) has been implemented and archived. The core `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior` types, `AstNode`, `OptionsPlugin`, `HostConfig`, `Host::load`, and the removal of `LanguagePipeline`, `SyntaxNode`, `LanguageHost`, `Extension`, `CommandProvider`, and `Library` are done.

What remains is the cleanup of the remaining `plugin-*` and `lib-*` crates to use pure `Behavior<Op>` rule plugins.

## Decision

Complete the conversion of the remaining `plugin-*` and `lib-*` crates so that all language and library behavior is expressed as `Behavior<Op>` rules contributed to the appropriate `RuleAcceptor` plugins. No crate should still contain hard-coded stage wiring, legacy `Extension`/`CommandProvider`/`Library` patterns, or language-specific monolithic logic.

## Consequences

- All canonical and library crates become pure `Plugin`/`Behavior<Op>` compositions.
- `kit-canonical` is a thin manifest over the cleaned-up plugin set.
- The platform/host itself contains no language-specific rule logic.

## Related work

- `project/adrs/archived/0002-handl-platform-composable-host.md`
