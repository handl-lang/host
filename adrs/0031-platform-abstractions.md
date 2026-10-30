---
title: Platform Abstractions
type: adr
date: 2026-08-25
status: proposed
status_note: Accepted — one `handl-platform` crate holds the core contracts; capabilities are data.
---

# ADR-0031: Platform Abstractions

## Decision

1. **One crate, `handl-platform`**, holds the core contracts as modules:
   `capability`, `plugin`, `extension`, `kit`, `host`, `boundary`.
2. **`CapabilityMetadata` is opaque `serde_json::Value`**; the platform
   defines no CLI-specific keys — each host interprets the object.
3. **Default capabilities ship as `handl-ext-*` plugins** (fmt, lint, typecheck,
   eval, compile, build, extension, kit, plugin); concrete command semantics
   live only there.
4. **Hosts generate their surface from capability metadata**; the CLI does not
   know `fmt`/`lint`/`plugin`, and the management capabilities operate on the
   platform registries, not as CLI special cases.
5. **Existing crates migrate**: `host-cli-plugin` empties into
   `platform::plugin`; `compiler-pass-*` become pass hosts; `vm` aligns with
   `platform::host`.
6. **Out of scope** until the layer is stable: `Framework`, `Middleware`,
   `Rpc`, and the `handl-document` unification.

## Why

Each host had its own plugin/registry types and the CLI hardcoded its
subcommands, so a new host or capability meant a new abstraction. One platform
crate with opaque capability metadata lets every host surface the same
capability differently.

## Not doing
- **Separate `handl-plugin`/`extension`/`framework` crates** — modules first.
- **A CLI-specific metadata schema** — metadata is opaque.
- **Command semantics in the CLI** — they live in `handl-ext-*`.
- **Framework/RPC work now** — deferred until the contracts settle.

## Consequences
- Hosts are thin; capabilities are plugins with metadata.
- Management commands use the same registries as everything else.
- The migration is mechanical: move types, generalize, align hosts.

## Enforcement
- `crates/handl-platform/` — the six modules.
- `crates/handl-ext-*/` — the default capabilities.
- Host tests that generate subcommands from metadata.

## Design
+Module contents, metadata example, built-in capabilities and migrations:
+[`design/0031-platform-abstractions.md`](../design/0031-platform-abstractions.md).
