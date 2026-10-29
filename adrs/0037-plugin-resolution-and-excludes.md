---
title: Plugin Resolution and Excludes
type: adr
date: 2026-08-29
status: proposed
status_note: Proposed
---

# ADR-0037: Plugin Resolution and Excludes

## Context

`HostConfig.excludes` is parsed from `.handl.toml` but never applied. A user can write:

```toml
excludes = ["some.plugin.id"]
```

and the plugin is still loaded and active.

Additionally, `PluginRegistry::resolve` silently picks the last-registered plugin for a capability when multiple plugins provide it. There is no runtime conflict detection. The `resolve.rs` module has a full manifest-level resolver (`resolve()`) that handles exclusive capabilities, version matching, and conflict declarations, but it operates on `PluginManifest` slices and is not wired into the host loading flow.

This leaves three gaps:

1. Excludes are ignored.
2. Capability conflicts are silent.
3. The manifest resolver and the runtime registry are disconnected.

## Decision

### 1. Excludes are applied by the applier after loading

`DefaultApplierPlugin::contribute` will, after loading kits and plugin manifests, remove any plugin whose id matches an entry in `config.excludes`. This requires `PluginRegistry::remove_by_id`.

Per-kit excludes (`KitReference.exclude`) are handled the same way — the applier collects them and removes matching plugins after the kit is loaded.

### 2. Conflict detection is a host method, not a capability

`DefaultHost` gains a `resolve(&self) -> Result<ResolvedComposition, ResolutionError>` method that:

- Collects `PluginManifest`-like data from the live registry (id, version, capabilities, requires, conflicts, exclusive).
- Calls `resolve::resolve()` with the collected manifests.
- Returns the resolved composition or an error.

This does not modify the registry. It is a validation step that the CLI or tests can call after `host.load()` to detect conflicts before running commands.

### 3. The applier does not auto-resolve

The applier loads and excludes. It does not call `host.resolve()`. Conflict detection is explicit, not implicit. This keeps loading fast and avoids surprising failures when multiple providers are intentional (e.g., during development).

## Consequences

- `PluginRegistry` gains `remove_by_id`.
- `DefaultHost` gains `resolve`.
- `.handl.toml` `excludes` and per-kit `exclude` entries are now enforced.
- Conflict detection is opt-in via `host.resolve()`.
- The manifest resolver (`resolve.rs`) is reused, not replaced.

## References

- ADR-0034: Zero-Monolith Plugin-Driven Platform
- ADR-0035: Loaders as Abstract Plugins
- ADR-0036: Host Applier as a Plugin
