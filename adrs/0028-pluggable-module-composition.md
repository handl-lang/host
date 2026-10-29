---
title: Pluggable Module Composition
type: adr
date: 2026-08-24
status: proposed
---

# Pluggable Module Composition

## Context

The handl compiler already keeps lexing, parsing, resolving, typechecking, and formatting in separate crates. The existing `ParseStage` dispatches to a `Frontend` using only the `Source.format` string, and `FileResolver` derives that format from the file extension. This is the beginning of a multi-language import pipeline, but it is hard-coded: the `frontend_for(format)` map is a static match, and the `Dialect` trait represents a single bundled syntax.

Because the pipeline is decoupled, the module system can already import files with different lexers, parsers, and syntaxes. We want to make this first-class: each handl module should declare which kits and plugins it uses, and the compiler should compose the correct pipeline for that module.

## Decision

We will replace the static `frontend_for(format)` mechanism and the monolithic `Dialect` trait with a **pluggable, per-module composition** system.

### `module.toml` manifest

Every module folder may contain a `module.toml` manifest. The manifest describes the language kit and/or the per-stage plugins used by that module. Nested modules can have their own `module.toml`, so a project can mix syntaxes and toolchains.

Example:

```toml
[language]
kit = "handl-kit-canonical"
lexer = ["handl-lexer-multi-line-strings"]
parser = ["handl-parser-type-annotations"]

[passes]
typecheck = ["handl-typecheck-core", "handl-typecheck-import"]
format = "handl-format-canonical"
```

### Naming convention

All kits and plugins are named with the `handl-` prefix to avoid conflicts with external packages and to make discovery deterministic:

- Kits: `handl-kit-*`
- Lexers: `handl-lexer-*`
- Parsers: `handl-parser-*`
- Typecheckers: `handl-typecheck-*`
- Formatters: `handl-format-*`
- Generic passes: `handl-pass-*`
- Multi-pass plugins: `handl-plugin-*`

### Composer

A new `Composer` takes the `module.toml` configuration, reads each plugin's pass declarations, and assembles the right `(lexer, parser, passes...)` for the module. It also validates that the configured passes are coherent: every required pass has an implementing plugin and no conflicting duplicate is present. This replaces the current `handl_frontend::frontend_for` static map. The `ParseStage` (and later stages) will invoke the `Composer` instead of `frontend_for(format)`.

### Pluggable passes

All compiler passes become pluggable:

- `lexer`
- `parser`
- `resolve`
- `typecheck`
- `format`
- `generate`

Each pass is a plugin implementing a common `Pass` trait. `compiler-pass-parse`, `compiler-pass-typecheck`, etc. become thin orchestrators that run the configured plugins in order.

### Plugin pass declarations

Every plugin declares the passes it implements. A single plugin may implement multiple passes, and those declarations live in plugin metadata rather than the user-facing manifest.

Example plugin metadata:

```toml
name = "handl-plugin-single-comments"
passes = ["lexer", "typecheck", "format"]
```

The `Composer` expands each plugin into its declared pass slots. For example, `handl-plugin-single-comments` would contribute to the `lexer`, `typecheck`, and `format` pipelines for the module. A manifest may still list dedicated stage plugins such as `handl-lexer-*` or `handl-format-*` to override or extend a specific pass.

The `Composer` validates that every pass required by the manifest has at least one implementing plugin and detects conflicts or duplicate implementations before the pipeline runs.

### AST strategy

Plugins may produce different ASTs. To keep the rest of the pipeline tractable, the recommended default is a **lower** or **canonicalize** pass that turns a plugin-specific AST into a shared canonical IR. Later stages (resolve, typecheck, generate) then operate on the canonical IR. If a module opts out of lowering, it must also provide a complete, AST-specific pass chain.

### Resolver integration

`FileResolver`, and the virtual/HTTP/Git resolvers by extension, will discover `module.toml` when resolving a module path. If the resolved path is a directory, the resolver reads the manifest and loads the declared entry file. The resolved `ModuleConfig` travels through `ResolvedModule` → `Source` → the `Composer`.

### Dialect trait

The existing `handl-dialect` `Dialect` trait and canonical dialect crate are deprecated in favor of this plugin model. Syntax, schema vocabulary, and special keys become `handl-lexer-*`, `handl-parser-*`, and `handl-pass-*` plugins rather than a single bundled dialect.

## Consequences

- `Source.format` and `frontend_for(format)` will be removed in favor of `Source.module_config` and `Composer::compose(config)`.
- `handl_frontend` is effectively obsolete in its current form; it will be replaced by or evolve into a `Composer` crate.
- `handl-dialect` and `handl-dialect-canonical` will be removed or significantly reduced.
- Every compiler pass crate becomes a plugin host rather than a single implementation.
- A new plugin registry is required to map `handl-*` names to concrete trait implementors.
- `module-resolver-*` crates must be updated to read `module.toml` and pass `ModuleConfig`.
- Tests will move from “this syntax” vs “that syntax” to “this kit/plugins combination” vs “that kit/plugins combination”.
- Nested modules can mix syntaxes, e.g. a root project in `handl-kit-canonical` importing a JSON-config sub-module.

## Status

Proposed
