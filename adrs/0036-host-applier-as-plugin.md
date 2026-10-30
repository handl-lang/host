---
title: Host Applier as a Plugin
type: adr
date: 2026-08-29
status: accepted
status_note: Accepted
---


# ADR-0036: Host Applier as a Plugin

## Decision


The `HostApplier` mechanism will be replaced by an applier plugin.

1. **Create `handl-plugin-applier`** (or a similarly named default applier plugin crate).
   - Provide `DefaultApplierPlugin`, a `Plugin` that reads a host config file and applies it to the host.
   - In `contribute`, read `.handl.toml` (or another supported config source), parse it into `HostConfig`, and register the requested kits, plugins, excludes, and option overrides.
   - Kits and plugins are loaded by running capabilities (`kit/load` and `load`) on the same host, not by calling hard-coded loader/applier code.

2. **Turn `BasicKitLoader` into a `KitLoaderPlugin`** or move its behavior into `handl-plugin-applier`.
   - The default applier provides a `kit/load` capability.
   - `kit/load` accepts a kit name or path and returns a `Kit` that the applier then registers with the host.

3. **The CLI only bootstraps**.
   - `host-cli` gains `--applier <id>` (default `applier.default`) and `--config <path>` (default `.handl.toml`).
   - It creates a `DefaultHost`, registers the default applier plugin (and the default kit/loader plugins if they are not already provided by the applier), tells the applier where the config file is, and calls `host.load()`.
   - `host.load()` runs `contribute` to a fixed point; the applier reads the config and registers the rest.

4. **The existing `HostApplier` trait and `ConfigHostApplier` struct can be deprecated**.
   - `HostConfig` and `KitReference`/`PluginReference` remain in `handl-platform` as the in-memory model.
   - The actual _act_ of applying the config moves from `handl-platform` to a plugin.


## Why

With ADR-0035, physical plugin loaders became first-class plugins. The host now loads external plugins through the `load` capability rather than a hard-coded `LoaderRegistry`. However, the CLI and `DefaultHost` still hard-code how a host configuration is applied:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- The CLI no longer imports `ConfigHostApplier` or `BasicKitLoader` directly.
- `.handl.toml` becomes the primary input; the CLI becomes a thin launcher.
- A kit can ship its own applier plugin and be selected with `--applier`.
- Bootstrapping is explicit and minimal: the only hard-coded decisions are which applier to start with and where the config lives.
- The `HostConfig` parsing code in `handl-platform` is still reused by the applier plugin.

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0036-host-applier-as-plugin.md`](../design/0036-host-applier-as-plugin.md).
