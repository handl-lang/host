---
title: Zero-Monolith, Plugin-Driven Platform
type: adr
date: 2026-08-29
status: accepted
---

# ADR-0034: Zero-Monolith, Plugin-Driven Platform

## Status

Accepted

## Context

ADR-0033 unified plugin crate naming, which made the workspace navigable and the kit manifests mechanically enumerable. Naming alone is not sufficient to make the platform language-agnostic: the base framework crates still contain hard-coded canonical language assumptions, concrete value types, and monolithic statement/expression shapes. This means `kit-canonical` is not truly a curated set of plugins loaded by a generic host; it is still wired into the foundations of `handl-plugin-lexer`, `handl-plugin-parser`, `handl-plugin-ast`, `handl-platform`, `handl-plugin-runtime`, and `handl-plugin-target-rust`.

The goal of the Handl platform is to be a toolkit for arbitrary languages built from composable plugins. The canonical language is one such kit. Before we can expose non-canonical kits, the base framework must be scrubbed of all language-specific code. Only then can a kit be swapped, subset, or extended without touching the framework.

## Decision

The Handl platform will contain **zero monolithic, language-specific code in the base framework crates**.

The framework, plugins, kits, and hosts have the following concrete boundaries:

### 1. Base framework crates are pure, generic abstractions

Base crates define traits, registries, and generic data structures. They do not know the canonical language.

- `handl-plugin-lexer` defines `Token`, `Lexer`, and the `TokenKind` capability trait. No concrete token kind lives in the base crate.
- `handl-plugin-parser` defines `Parser`, `Grammar`, and the `GrammarRule` capability trait. No concrete grammar rule or canonical statement/expression shape lives in the base crate.
- `handl-plugin-ast` defines the generic `Node` type and per-shape capability traits. No concrete AST vocabulary lives in the base crate.
- `handl-platform` defines `Host`, `Plugin`, `RuleAcceptor`, `Operation`, `Behavior`, `ContributionContext`, `AstNode`, and `DataTypes`. No concrete `DefaultHost`, `Env`, or language-specific runtime value lives in the base crate.
- The `handl-expr` compatibility crate has been dissolved; fixed `StmtKind` enumerations and canonical expression grammars now live in individual `plugin-ast-*` and `plugin-grammar-*` crates.

### 2. Kits are curated sets of plugins

- `kit-canonical` is one kit: the built-in, configured canonical language.
- A kit is a manifest plus selected default option overrides. It does not contain language implementation directly; it lists the plugins that implement the language.
- New kits (e.g., `kit-minimal`, `kit-server`) can be assembled by choosing a different set of plugins without changing the framework.

### 3. CLI and LSP are hosts, not kits

- `handl-host-cli` and `handl-host-lsp` are executable hosts.
- They load one or more kits and register operation `RuleAcceptor`s.
- They do not hard-code the canonical language; they discover it through the kit manifest.

### 4. VM, target, and runtime packages are tiny composable plugins

- `handl-plugin-vm`, `handl-plugin-target-*`, and `handl-plugin-runtime` must not contain language-specific builtin tables, concrete `Env` implementations, or value-type special cases.
- `plugin-target-rust` and other `plugin-target-*` crates are the only places for target-specific builtins and code generation.
- `handl-plugin-runtime` and `handl-plugin-typecheck-core` operate over capability traits, not a fixed `NonFunctionValue` or `handl-plugin-ast` dependency.
- `handl-plugin-vm-host-eval` and `handl-plugin-vm-backend-*` provide generic execution hosts; concrete value semantics are supplied by plugins.

## Consequences

### Positive

- Any language can be built by selecting plugins and composing a kit, without forking the framework.
- `handl-plugin-lexer` and `handl-plugin-parser` can be reused by non-canonical kits, embedded DSLs, and alternative surface syntaxes.
- `kit-canonical` becomes a pure, auditable manifest of the canonical language features.
- CLI and LSP hosts become smaller because they delegate all language behavior to the kit.
- Target, VM, and runtime packages become independently testable and replaceable.
- Extension points (`plugin-ext-eval`, `plugin-ext-typecheck`, `plugin-ext-fmt`, `plugin-ext-lint`, `plugin-ext-build`, `plugin-ext-compile`) become generic operations that consume `Behavior` rules, not canonical-language special cases.

### Negative

- The base crates must gain richer generic extension points and trait-based registries before the canonical implementation can be fully extracted.
- Many small plugins (`plugin-ast-*`, `plugin-grammar-*`, `plugin-token-*`, `plugin-target-*`) must be created or stabilized, which increases crate count and workspace maintenance.
- Plugin loading, dependency resolution, and option merging become performance-critical because the host now delegates almost every language decision.
- Documentation and debugging become harder until a discovery mechanism lists which plugins provide a given token, grammar rule, or runtime builtin.

### Migration plan

The audit summary produced during ADR-0033 implementation is the migration backlog. It is divided into three phases.

#### Phase 0 — Blockers (must be removed before any non-canonical kit can load)

- `kit-canonical`: remove `CanonicalKind`, `CanonicalFactory`, `CanonicalLexer`, and `KitParser` wiring from the base framework; move them into `plugin-token-*`, `plugin-grammar-*`, and `plugin-ast-*` crates.
- `handl-expr`: dissolved; the `StmtKind` monolith has been split into per-statement `plugin-ast-*` shapes and capability traits.
- `plugin-target-rust`: remove the `BuiltinHost` table and `RustVm` concrete implementation from the framework; make them target plugins.
- `handl-plugin-lexer/src/primitives.rs` and the default factory methods: remove canonical primitive token defaults; these are provided by `plugin-token-*` crates.

#### Phase 1 — Coupled framework code

- `plugin-ext-typecheck`: remove the `handl-kit-type` direct call; query value-type capabilities through the `DataTypes` plugin.
- `plugin-ext-eval`: remove the `hello.greet` and JSON special-case; handle them through `handl-plugin-eval-hello` and `handl-plugin-vm-values-json`.
- `plugin-runtime/src/runtime.rs` and `plugin-runtime/src/typecheck.rs`: remove the `NonFunctionValue` and `handl-plugin-ast` dependencies; operate over `Value` capability traits.
- `platform/src/vm.rs`: remove the concrete `DefaultHost` and `Env`; replace them with `Host` plugin composition.

#### Phase 2 — Cleanup and residual defaults

- `parser-common`: removed; its engine lives in `handl-plugin-parser` and canonical-specific parts live in `plugin-grammar-*` crates.
- `plugin-ast/src/expr.rs`: remove the fixed shape vocabulary; each shape becomes a `plugin-ast-*` crate.
- `plugin-ext-fmt`, `plugin-ext-lint`, `plugin-ext-build`, `plugin-ext-compile`: complete placeholders and ensure each is a generic `RuleAcceptor` with no canonical-language special cases.
- `plugin-lexer-common`: remove default rules that embed canonical defaults; move them to `plugin-token-*` crates.
- `dialect` crate: removed; syntax and schema vocabularies are now `kit-*` options.

## Alternatives considered

- **Keep a small canonical core inside the framework**: rejected because it makes every alternative language a second-class fork and prevents `kit-canonical` from being a true plugin bundle.
- **Make the framework dynamic at runtime**: rejected because we want zero-cost static dispatch and compile-time trait safety.
- **Decompose only at the crate level, not the plugin level**: rejected because it still produces monolithic grammar/AST modules that cannot be subset or reused by other kits.

## Implementation status

Implemented. The P0, P1, and P2 work items above have been completed.

## Related work

- ADR-0001: Generic Node, Trait-Based Capabilities, and Plugin-Driven Canonical Kit
- ADR-0002: Single `Plugin` Model for Host, Operations, Libraries, Targets, and Options
- ADR-0033: Unified Plugin Crate Naming
- `docs/explanations/design-principles.md`
- `kit-canonical` crate manifest
