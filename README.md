# handl

`handl` is a modular language platform. The root `handl` crate is a thin facade over `handl-platform` and the major workspace crates. Hosts and applications can depend on one package and enable only the Cargo features they need.

## Platform-first architecture

The core contracts live in a single crate:

- `crates/platform` (`handl-platform`) — `Capability`, `Plugin`, `Extension`, `Kit`, `Host`, and `Boundary`.

These abstractions are the only shared dependency of the rest of the workspace. The compiler, VM, source layer, and CLI all consume the same platform types.

## Capabilities

A *capability* is something the platform can do. Default capability crates are `handl-capability-*` and are pre-registered by the platform's default host:

- `handl-capability-fmt`
- `handl-capability-lint`
- `handl-capability-typecheck`
- `handl-capability-eval`
- `handl-capability-compile`
- `handl-capability-build`
- `handl-capability-plugin`
- `handl-capability-kit`
- `handl-capability-extension`

Each capability is a normal plugin. It exposes `CapabilityMetadata` (an arbitrary JSON object) that hosts can use to build commands, menus, or RPC endpoints. The CLI is one such host; it does not hard-code `fmt`, `lint`, `eval`, etc.

## Plugins, kits, and hosts

- A `Plugin` implements capabilities and returns `CapabilityMetadata`.
- A `Kit` bundles plugins (deferred to later work).
- A `Host` consumes capabilities. `handl-cli` is the default command-line host; LSP, IDE, and web hosts are possible.
- A `Boundary` describes module/package/process/worker/server/client/RPC/deployment boundaries.

Management of plugins, kits, and extensions is itself implemented through capability plugins, not special-cased CLI logic.

## Compiler

The compiler pipeline is built from pluggable `Pass` hosts. `handl-compiler` assembles `Pass` objects that it discovers through the platform host, so new passes can be added without editing the compiler crate.

## CLI

The `handl` CLI is a thin, metadata-driven orchestrator. It loads the default host, reads registered `CapabilityMetadata`, and generates subcommands from whatever the metadata contains. Adding a new plugin can add a new `handl <command>` automatically.

## Root facade

`src/lib.rs` re-exports:

- All `handl-platform` modules (`pub use handl_platform::*;`)
- Non-optional workspace crates: `handl-ast`, `handl-ast-canonical`, `handl-ast-common`, `handl-dialect`, `handl-source`, `handl-vm-values-json`
- Optional workspace crates behind their existing features, including `handl-vm`, `handl-vm-host-eval`, `handl-dialect-canonical`, `handl-compiler`, `handl-typecheck-core`, `handl-typegen-core`, `handl-module-resolver-core`, `handl-store`, and `handl-codebase`

Enable or disable default features to control what gets compiled.

## Workspace notes

- `handl-platform` is the only crate every other crate should depend on for platform contracts.
- `plugin-*`, `kit-*`, and `host-cli` support crates are expected to move to separate repositories later.
- The old `crates/cli-plugin` crate has been removed; its concepts live in `handl-platform`.
