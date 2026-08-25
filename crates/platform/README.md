# handl-platform

Core platform abstractions for the Handl language: capabilities, plugins, extensions, kits, hosts, and boundaries.

Capabilities are identified by `CapabilityId` and carry generic JSON-serializable `CapabilityMetadata` (`serde_json::Value`). No CLI-specific fields are defined here; hosts such as the CLI, LSP, or web UI may interpret the metadata independently.

This crate intentionally does not include concrete built-in capabilities (e.g. `fmt`, `lint`), middleware, RPC, or kit-as-product features yet. Those are defined in separate `handl-ext-*` crates or deferred to later phases.

## Modules

- `capability` — `CapabilityId`, `CapabilityMetadata`, and `Capability` trait.
- `plugin` — `Plugin`, `PluginRegistry`, `InProcessPlugin`, `ProcessPlugin`, `CapabilityRequest`, `CapabilityResponse`.
- `extension` — `Extension` and `ExtensionRegistry`.
- `kit` — `Kit`, `KitManifest`, and `KitRegistry`.
- `host` — `Host` trait and `DefaultHost`.
- `boundary` — `Boundary` and `BoundaryKind`.
- `error` — `PlatformError` with `thiserror`.
