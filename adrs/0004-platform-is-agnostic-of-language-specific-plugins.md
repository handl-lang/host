+++
id = "ADR-0004"
title = "Platform is agnostic of language-specific plugins"
status = "open"
priority = "medium"
+++

---
title: Platform is agnostic of language-specific plugins
type: adr
date: 2026-08-29
status: accepted
status_note: accepted
---

# Platform is agnostic of language-specific plugins

## Context

`handl-platform` is intended to be the generic host for the HAndL language toolchain: it defines the `Plugin` trait, `Host` coordination, capability resolution, and boundaries. Language-specific concerns — parsing, AST nodes, grammar rules, typechecking, formatting, linting, evaluation, and the VM — should be supplied as plugins.

Currently `crates/platform` directly depends on several non-platform crates:

- `handl-plugin-vm` — re-exported as `Vm`.
- `handl-plugin-vm-values-json` — used by the value layer.
- `handl-kit-type` — re-exported for `Term`/`Value` primitives and `typecheck`.

This couples the platform to a specific language (HAndL) and its type system. If another language wants to use the same host, it must either pull in the HAndL parser/typechecker/VM or live with unused dependencies and conflicting assumptions.

## Decision

`handl-platform` must not depend on any language-specific crate. It may only depend on:

1. The `plugin-lib-*` crates that define the `Plugin` contract and any shared, language-neutral primitives.
2. Utility crates for serialization, collections, etc. that have no language-specific knowledge.

All of the following must be loaded as plugins through the `Plugin` system, not compiled into the platform:

- `plugin-ast-*` (AST node plugins)
- `plugin-parser` (parser)
- `plugin-grammar-*` (grammar rules)
- `plugin-typecheck-core` (typechecking)
- `plugin-runtime-*` (runtime rules)
- `plugin-vm-*` and `plugin-vm-values-*` (VM and value representations)
- `plugin-eval-*` (eval rules)
- `plugin-fmt-*`, `plugin-lint-*`, `plugin-diag-*` (formatting, linting, diagnostics)

The platform may keep shared, minimal types such as `Value`, `Term`, `Env`, and `CapabilityId` that plugins can build on. These should live in a dedicated, language-neutral crate (e.g. `plugin-lib-vm-values` or `lib-vm-values`) that the platform and all language plugins share.

## Consequences

- `crates/platform` drops direct dependencies on `handl-plugin-vm`, `handl-plugin-vm-values-json`, `handl-kit-type`, `handl-kit-type-schema-json`, and all `plugin-ast-*` / `plugin-grammar-*` / `plugin-runtime-*` crates.
- `platform/src/value.rs` must stop re-exporting `handl-kit-type` primitives and instead expose platform-neutral `Value`/`Term` types, or re-export from a new `lib-vm-values` crate.
- `platform/src/lib.rs` must stop re-exporting `Vm` from `handl-plugin-vm`; the VM becomes a plugin that the host loads.
- The existing `kit-*` and `plugin-*` crates become consumers of the platform instead of being compiled into it.
- A new language can be supported by writing a different set of `plugin-*` crates and loading them into the same host.
