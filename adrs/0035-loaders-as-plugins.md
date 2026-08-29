# ADR-0035: Loaders as Abstract Plugins

## Status

Proposed

## Context

The Handl platform has three physical plugin loading mechanisms today:

- `handl-loader-native` — loads `.so`/`.dylib`/`.dll` artifacts using `libloading`.
- `handl-loader-wasm` — loads `.wasm` modules using `wasmi`.
- `handl-loader-web` — loads plugins over HTTP using `ureq`.

All three implement the `PluginLoader` trait defined in `handl-loader`. A `LoaderRegistry` tries each loader in order until one accepts a `PluginManifest`. This works, but it sits outside the single `Plugin` model described in ADR-0002:

- `DefaultHost` has no `LoaderRegistry`; it is not part of the host loading flow.
- `ConfigHostApplier` is currently a no-op, so `.handl.toml` cannot select loaders.
- `ToolingHost` is the only place a `LoaderRegistry` is wired, and it is hard-coded in tests.
- `handl-platform` and `handl-loader` form a near-cycle: the platform needs the loaders to load plugins, and the loaders depend on the platform to implement `Plugin`.

At the same time, the platform is migrating crate naming to the `plugin-*` convention under ADR-0033. Keeping a special `loader-*` family alongside `plugin-*` is inconsistent and confusing.

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

## Consequences

- `handl-platform` no longer needs to know about physical loading. It only exposes `Plugin`, `RuleAcceptor`, and the capability model.
- The `handl-platform`/`handl-loader` dependency cycle disappears.
- Loaders become composable: a kit can include the right loader set for its target, and users can override via `.handl.toml`.
- `ToolingHost` can be simplified because `DefaultHost` now supports loading through the capability system.
- `host-cli`, LSP, and other hosts no longer hard-code `NativeLoader`, `WasmLoader`, or `WebLoader`; they just run the `load` capability.
- The `LoaderHostPlugin` must be available before other loaders contribute. Each loader plugin will auto-register it if it is missing, so a single `plugin-loader-native` in `.handl.toml` is enough to bootstrap the whole mechanism.
- Existing `handl-loader` and `ToolingHost` code needs to be refactored, but the public `PluginLoader` trait interface stays largely the same.

## References

- ADR-0002: Single `Plugin` Model for Host, Operations, Libraries, Targets, and Options
- ADR-0033: Unified Plugin Crate Naming
