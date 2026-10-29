---
title: Wiring `.hnl` modules into `HandlNativeLint` and `HandlNativeRewrite`
type: adr
date: 2026-09-14
status: proposed
status_note: Proposed
---

# Wiring `.hnl` modules into `HandlNativeLint` and `HandlNativeRewrite`

## Context

`HandlNativeLint` (`plugin-lib-handl/src/lint.rs`) and `HandlNativeRewrite` (`plugin-lib-handl/src/transform.rs`) already implement the adapters that let lint and transform rules dispatch into pure-handl functions via the `handl:<mod>.<fn>` name prefix. They are tested in unit tests but are not wired into the production hosts (`plugin-ext-lint`, `host-tooling`, `host-cli` `transform`, `plugin-engine-transform-rewrite`).

The canonical kit (`handl-kit-canonical`) builds a `ModuleRegistry` of every stdlib module, many of which are `.hnl`-backed `HandlModule`s. The same `HandlModule`s must be made available to `HandlNativeLint` and `HandlNativeRewrite` so that `lints.yaml` rules with `check: !Native "handl:..."` and `transforms.yaml` rules with `rule: !Native "handl:..."` resolve at runtime instead of reporting "unknown native".

A historical implementation existed on the `wire/handl-natives` branch. That branch is now stale: it was written before the current `Kit`/`build_stdlib_registry` shape (with per-module `want` filters, `get_module` caching, and the removal of `plugin-lib-rand-native` / `plugin-lib-time-native` / `plugin-engine-typecheck-nbe`), so it cannot be merged mechanically.

## Decision

Rewrite the wiring as a clean, modern implementation:

1. Extend `handl-kit-canonical` to track every `.hnl`-backed `HandlModule` it registers, alongside the existing `ModuleRegistry`. The collection can live as a `HandlModules` struct paired with the registry (`KitModules`).
2. Expose `Kit::handl_lint_adapter()` and `Kit::handl_rewrite_adapter()` methods that build fully populated `HandlNativeLint` and `HandlNativeRewrite` instances from the tracked `HandlModule`s.
3. Wire the lint adapter into `plugin-ext-lint::lint_engine` and `host-tooling::diagnose` by adding it as a resolver on the existing `NativeLintRegistry`.
4. Wire the rewrite adapter into `host-cli` `transform` by layering it over the `desugar.*` native registry.
5. Add a `RewriteEngine::with_desugar_and_handl_natives` constructor that installs both the handl-native registry and the `desugar` fallback table used by kind-based dispatch.

The implementation must use current APIs only: `ModuleRegistry::register_arc`, `HandlNativeLint::with_module_arc`, `HandlNativeRewrite::with_module_arc`/`with_fallback`, `NativeLintRegistry::add_resolver`, and the existing `Kit::build_kit` flow.

## Consequences

- `.hnl`-authored lint and transform rules shipped in `lints.yaml` and `transforms.yaml` become runnable in the CLI and host tooling.
- `handl-kit-canonical` becomes the single source of truth for which `.hnl` modules are available to handl-native adapters.
- The stale `wire/handl-natives` branch can be deleted once the ADR is accepted and the implementation is merged; the branch itself is not a safe merge target.
