# handl

`handl` is a small, embeddable, runtime-agnostic language. Its kernel is minimal (literals, path lookups, function calls, and `handle` expressions); control flow, I/O, formatting, type checking, and compilation targets are provided by hosts and plugins.

This repository is the reference Rust platform — a Cargo workspace built around a single core-contract crate, `handl-platform`, that the compiler, VM, source layer, and CLI all consume.

## Core abstractions

The `handl-platform` crate (`crates/platform`) defines the shared types:

- `Capability` / `CapabilityId` — something the platform can do.
- `Plugin` / `PluginRegistry` — an implementation of one or more capabilities.
- `Kit` — a bundle of plugins.
- `Extension` / `Host` — the interface between plugins and consumers such as the CLI, LSP, or a web UI.
- `Boundary` — module / package / process / worker / server / client / RPC / deployment boundaries.

These abstractions are the only shared dependency of the rest of the workspace.

## Capabilities

Default capabilities are implemented as ordinary plugins in `crates/capability-*`:

- `handl-capability-fmt`
- `handl-capability-lint`
- `handl-capability-typecheck`
- `handl-capability-eval`
- `handl-capability-compile`
- `handl-capability-build`
- `handl-capability-plugin`
- `handl-capability-kit`
- `handl-capability-extension`

A plugin exposes `CapabilityMetadata` (a `serde_json::Value`) that hosts use to build commands, menus, or RPC endpoints. `handl-cli` is one such host and does not hard-code `fmt`, `lint`, `eval`, etc.

## Compiler

The compiler is built from pluggable `Pass` objects. New passes can be added without editing the compiler crate. Passes are discovered through the platform host and assembled by `handl-compiler`.

## CLI

The `hnl` binary (built by the `handl-cli` crate) is a thin, metadata-driven orchestrator. It loads a host, reads registered capabilities, and generates subcommands from plugin metadata. Adding a plugin can add a new `hnl <command>` automatically.

## Root facade

The `handl` crate is a convenience facade. It re-exports `handl-platform` and the major workspace crates behind Cargo features, so hosts can depend on one package and enable only what they need.

## Build and test

```bash
cargo check --workspace
cargo test --workspace
```

Run the CLI:

```bash
cargo run --bin hnl -- <command>
```

See `.devin/AGENTS.md` for more commands and `docs/LANG.md` / `docs/RUNTIME.md` for language and runtime details.
