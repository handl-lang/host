+++
id = "ADR-0035"
title = "ADR-0035: Loaders as Abstract Plugins"
status = "open"
priority = "medium"
+++

---
title: Loaders as Abstract Plugins
type: adr
date: 2026-08-29
status: proposed
status_note: Proposed
---


# ADR-0035: Loaders as Abstract Plugins

## Decision


Loaders will become first-class plugins. Instead of a `LoaderRegistry` owned by the host, the platform will provide a `LoaderHostPlugin` that other plugins contribute `PluginLoader` rules to. External plugin loading becomes a normal capability call.

Specifically:

1. Create `handl-plugin-load`.
   - Define the `PluginLoader` trait (moved from `handl-loader`).
   - Define the `Load` operation and a `LoaderRule` contribution type.
   - Provide `LoaderHostPlugin`, a `RuleAcceptor` that collects `LoaderRule`s and exposes a `load` capability.
   - The `load` capability accepts a `PluginManifest` payload, tries each collected `PluginLoader`, and registers the resulting `dyn Plugin` back into the host via the mutable `PluginContext`.

2. Rename the existing loader crates to the `plugin-loader-*` namespace:
   - `loader` → `plugin-load` (the host-side rule/operation definitions).
   - `loader-native` → `plugin-loader-native`.
   - `loader-wasm` → `plugin-loader-wasm`.
   - `loader-web` → `plugin-loader-web`.

3. Each `plugin-loader-*` crate implements both `Plugin` and `PluginLoader`.
   - In `contribute`, it ensures a `LoaderHostPlugin` is registered, then contributes a `LoaderRule` wrapping itself.
   - Because `contribute` takes `&self`, the loader must be `Clone` or wrap a cheaply-cloneable handle.

4. `DefaultHost` gains no new field or special-case API.
   - Host code that needs to load an external plugin calls the `load` capability like any other:

   ```rust
   host.run(&CapabilityRequest {
       capability: "load".into(),
       payload: serde_json::to_value(manifest)?,
   })?;
   ```

5. `.handl.toml` selects loaders through ordinary `[[plugin]]` entries, not a custom `[[loader]]` section:

   ```toml
   [[plugin]]
   register = "handl-plugin-loader-native"

   [[plugin]]
   register = "handl-plugin-loader-wasm"
   ```

6. Remove `handl-loader` as a separate framework crate. Its concerns are absorbed by `handl-plugin-load` and the loader-specific plugin crates.


## Why

The Handl platform has three physical plugin loading mechanisms today:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- `handl-platform` no longer needs to know about physical loading. It only exposes `Plugin`, `RuleAcceptor`, and the capability model.
- The `handl-platform`/`handl-loader` dependency cycle disappears.
- Loaders become composable: a kit can include the right loader set for its target, and users can override via `.handl.toml`.
- `ToolingHost` can be simplified because `DefaultHost` now supports loading through the capability system.
- `host-cli`, LSP, and other hosts no longer hard-code `NativeLoader`, `WasmLoader`, or `WebLoader`; they just run the `load` capability.

## Enforcement

- `plugin-loader-*` — where this is true.
- `plugin-load` — where this is true.
- `plugin-loader-native` — where this is true.
- `plugin-loader-wasm` — where this is true.
- `plugin-loader-web` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0035-loaders-as-plugins.md`](../design/0035-loaders-as-plugins.md).
