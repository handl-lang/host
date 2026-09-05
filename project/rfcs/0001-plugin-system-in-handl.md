# RFC-0001: The handl Plugin System, Ported to handl

## Status

Low priority. Ports the plugin system to handl once the language is
stable. The existing Rust implementation (`platform` crate:
`Capability`/`CapabilityId`, `CapabilityRequest`/`CapabilityResponse`,
`Plugin`, `PluginRegistry`, `Kit`, `KitRegistry`, `Host`, `Boundary`) is
**good and stays**: it is generic, modular, and tested, and the `tooling-*`
crates build on it (ADR-0085). This RFC moves the same — already-generic —
design into handl; the Rust version remains as bootstrap and fallback.

## Summary

The plugin system stops being platform infrastructure written in Rust and
becomes **handl code**: capabilities, plugins, kits, and hosts are values
and arrows in the language, interpreted by handlers (per ADR-0052: the
runtime runs them, the compiler lowers them, the analyzer inspects them).

## Motivation

- ADR-0044 makes language-as-data the core model; a plugin is just data
  describing contributions. Expressing it in handl removes the Rust/host
  split from the extension story.
- The existing plugin system (`platform`, `plugin-ext-*`) already solves
  registry/loading/manifests with tests behind it; handl modules can
  express the same things natively once the port lands.
- Backlog "Self-hosted `Host` and meta-circular composition" already sketched
  this direction; this RFC makes it concrete.

## Detailed design

1. **Capability = value.** A `CapabilityRequest` is a record
   `{ id, version, requirements }`; a `CapabilityResponse` is a record.
   Arrow kind signatures type both ends, so a plugin is typechecked against
   the capabilities it claims.
2. **Plugin = module.** A plugin is an ordinary handl module exporting
   `contributions` (a list of capability providers — arrows
   `Request ~> Response`) and a `manifest` (id, version, permissions).
   No new kernel primitive: registering a plugin is binding a module.
3. **Kit = module list.** A `Kit` is a curated list of plugin modules
   (preserving ADR-0039's "kits are curated plugin lists"); composition is
   list concatenation with conflict checking as a pure arrow.
4. **Host = handler.** The `Host` is a handler for capability requests:
   the _same_ description dispatches to a native Rust handler (bootstrap),
   an in-handl handler (self-hosted), or a WASM handler (sandboxed —
   absorbing the backlog's "Sandboxed Target plugins" item).
5. **Permission manifests survive.** The manifest declares required
   capabilities (`network`, `filesystem`, `process`); the host handler
   enforces them by construction (capability discipline, ADR-0046).
6. **Loading:** native plugins remain Rust crates callable via FFI
   (ADR-0083) during transition; handl plugins load as modules.

## Migration

Phase 1: define the manifest/record shapes in handl, typechecked by
RFC-0004's typechecker. Phase 2: reimplement `PluginRegistry`/`KitRegistry`
as handl arrows with a Rust shim. Phase 3: once the handl host passes the full existing plugin test suite,
the Rust registries are retired for handl-authored plugins — kept as
bootstrap/fallback for native (FFI, ADR-0083) plugins, with
`plugin-load` and `plugin-loader-*` reduced to that role.

## Drawbacks

- Bootstrap circularity: the handl host still needs a minimal Rust loader;
  mitigated by keeping FFI-native stdlib (ADR-0083).
- Performance-sensitive dispatch paths must be compiled, not tree-walked.

## Related

- [RFC-0007 — CLI in handl](0007-cli-in-handl.md) — the CLI is the first
  consumer of the handl plugin system.
- ADR-0035 (loaders as plugins), ADR-0039 (kits), ADR-0046 (capability
  discipline), ADR-0052 (kernel), ADR-0083 (FFI-native stdlib).
