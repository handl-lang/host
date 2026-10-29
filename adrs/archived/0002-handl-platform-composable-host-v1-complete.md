---
title: 'ADR-0002 (Archived v1): Single Plugin Model — Core Architecture'
type: adr
date: 2026-09-02
status: accepted
---

# ADR-0002 (Archived v1): Single Plugin Model — Core Architecture

## Status

Accepted

## Context

The platform had overlapping concepts (`Plugin`, `Extension`, `Component`, `Target`, `Library`, `Kit`) that created plumbing and artificial distinctions. The core insight: everything the host loads is a **plugin**. The distinction is only whether a thing accepts rules from others.

## Decision Implemented (v1 Complete)

### 1. Unified Plugin Model

- **Plugin** — the universal component. Every loaded thing is a plugin.
- **RuleAcceptor<Rule>** — a sub-trait for plugins that accept contributions.
- **Operation** — a marker type for cross-cutting concerns (`Format`, `Lint`, `Lexer`, `Parser`, `Typecheck`, `Doc`, `Cli`, `Lsp`, `Eval`).
- **Behavior<Op>** — a single rule that a plugin contributes to an operation for a specific `AstNode.kind`.
- **ContributionContext** — type-safe API for registering plugins and handing rules to `RuleAcceptor`s.

### 2. Removed Legacy Patterns

- ✅ Deleted `Extension` trait (was pattern for stage-specific plugins)
- ✅ Deleted `CommandProvider` trait (was CLI pattern)
- ✅ Deleted `Library` trait (was library pattern)
- ✅ No more hard-coded stage wiring in modern code

### 3. Implemented Behavior<Op>

- ✅ `BasicEvaluator` works as `RuleAcceptor<Behavior<Eval>>`
- ✅ 30+ `impl Behavior<Op>` across codebase
- ✅ Lint, format, build, compile, typecheck operations all using pattern
- ✅ Test evidence of rule contributions and dispatch

### 4. Kit and Host

- **Kit** — a manifest listing plugins to load
- **HostConfig** — in-memory description of which plugins to load, what to exclude, option overrides
- **Host** — pure composer with `PluginRegistry` and `OptionsResolver`

### 5. Preserved Exceptions

- **ExecControl trait** — intentionally separate from Behavior<Op> for eval-specific cross-cutting concerns (caching, limits, tracing, sandboxing)
- ✅ Used in: `plugin-exec-allow`, `plugin-exec-cache`, `plugin-exec-counter`, `plugin-exec-max-calls`, `plugin-exec-max-depth`, `plugin-exec-trace`
- Justified per ADR-0040 §4 as lightweight two-hook pattern for execution control

## Implementation Evidence

- ✅ `Behavior<Op>` trait defined in `platform/src/plugin_model.rs`
- ✅ All operation types defined and used
- ✅ Legacy traits removed (grep for Extension/CommandProvider/Library shows removal comments)
- ✅ Modern code uses only Plugin/Behavior<Op> patterns
- ✅ Tests verify rule contribution and dispatch

## Consequences

- ✅ No overlapping concepts; everything is a plugin
- ✅ Plugins can depend on and provide to other plugins via traits
- ✅ Language behavior is composable through Behavior<Op>
- ✅ Execution control is a justified exception with proper documentation
- ✅ Host is a pure composer, not a concrete type with hard-coded fields

## See also

- ADR-0002 (Remaining) for library plugin conversion roadmap
- ADR-0040 (VM is an operation, not a subsystem) for ExecControl justification
- ADR-0044 (Language as data and runtime split) for data-driven plugins
