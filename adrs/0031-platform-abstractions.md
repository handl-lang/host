---
title: Platform abstractions and plugin system
type: adr
date: 2026-08-25
status: proposed
status_note: Proposed
---

# ADR 0031: Platform abstractions and plugin system

## Context

The Handl repository has grown a large number of small crates (`handl-ast`, `handl-vm`, `handl-compiler`, `handl-host-cli-plugin`, etc.) but it does not yet have a single, stable platform layer that defines how capabilities, plugins, extensions, hosts, and boundaries compose. Existing ADRs 0028, 0029, and 0030 describe pluggable composition, the CLI as a capability orchestrator, and a `lib-lang` façade, but they leave the platform contract undefined.

We also have stale documentation (`docs/api/ac_*`, `README.md` referencing non-existent crates) and a `handl-host-cli-plugin` crate that is hard-coded for the CLI rather than being a generic platform mechanism.

This ADR defines the smallest set of platform abstractions needed to make the rest of the architecture coherent.

## Decision

Introduce a single new crate `handl-platform` that contains the core platform contracts as modules. No separate `handl-plugin`, `handl-extension`, or `handl-framework` crates for now.

The `handl-platform` modules are:

- `capability` — `Capability`, `CapabilityId`, `CapabilityMetadata`
- `plugin` — `Plugin`, `PluginManifest`, `PluginRegistry`, `InProcessPlugin`, `ProcessPlugin`
- `extension` — `Extension`, `ExtensionMetadata`
- `kit` — `Kit`, `KitManifest`, `KitRegistry`
- `host` — `Host`
- `boundary` — `Boundary`, `BoundaryKind`

`CapabilityMetadata` is any JSON-serializable object. The platform layer does not define CLI-specific keys such as `command` or `aliases`. It is exposed as an opaque `serde_json::Value` so hosts can interpret it independently:

```rust
pub type CapabilityMetadata = serde_json::Value;
```

A plugin may return metadata like:

```json
{
  "command": "compile",
  "description": "Compile a Handl module",
  "aliases": ["build"]
}
```

The `handl-host-cli` host may read the `command` field from this object and use it to generate a `clap` subcommand, but it does not require or hard-code any specific schema. Other hosts (LSP, web UI, CI) may surface the same capability using different fields.

### Built-in default capabilities

The platform ships with a set of default capability crates named `handl-ext-{name}`. Each crate is a regular plugin that implements one capability and is pre-registered by the default `Host`:

- `handl-ext-fmt`
- `handl-ext-lint`
- `handl-ext-typecheck`
- `handl-ext-eval`
- `handl-ext-compile`
- `handl-ext-build`
- `handl-ext-extension` — manages extensions (`add`, `remove`, `info`)
- `handl-ext-kit` — manages kits (`add`, `remove`, `info`)
- `handl-ext-plugin` — manages plugins (`add`, `remove`, `info`)

These crates are the only place where concrete command semantics live. The `handl-host-cli` binary does not know about `fmt`, `lint`, `plugin`, etc. It loads the default `Host`, reads `CapabilityMetadata` from the registry, and generates subcommands from whatever the metadata contains.

The management capabilities (`extension`, `kit`, `plugin`) operate on the platform registries defined in `handl-platform`. They are not special-cased in the CLI.

### Existing crate migrations

- `crates/host-cli-plugin` is emptied; its `PluginRegistry` and `CliPlugin` logic move into `handl-platform::plugin` and are generalized.
- `crates/compiler-pass-*` become pass hosts that use the platform `Plugin`/`Capability` traits to run a `Vec<Box<dyn Pass>>` assembled by `handl-compiler`.
- `crates/vm` / `crates/plugin-vm-host-eval` align their `Host` with `handl-platform::host`.

### Out of scope

For now we intentionally defer `Framework`, `Middleware`, `Rpc`, and the `handl-document` unification work described in ADRs 0028/0030. They can be added once the platform layer is stable.

## Consequences

- A single, small crate owns all platform contracts, making dependency direction explicit: everything else depends on `handl-platform`.
- The CLI becomes a pure metadata-driven host. New capabilities appear automatically when a plugin registers them.
- Built-in commands (`fmt`, `lint`, `plugin`, `kit`, etc.) are implemented as ordinary capability plugins, not as CLI code.
- The compiler pipeline can be assembled from plugins without the compiler knowing the concrete pass implementations.
- Stale `docs/api/ac_*` documentation and the `Peregrin.toml` reference can be removed as part of this work.
- `handl-host-cli-plugin` can be deleted after the migration.
- `Framework`, `Middleware`, `Rpc`, and `handl-document` remain out of scope until this ADR is accepted and implemented.
