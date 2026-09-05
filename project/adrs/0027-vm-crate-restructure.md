# VM Crate Restructure

## Status

Superseded by [ADR 0040 — VM is an Operation, Not a Subsystem](./0040-vm-is-an-operation-not-a-subsystem.md).

## Context

The workspace currently mixes `handl-runtime-*` and `handl-plugin-vm-*` package and directory names. The `crates/vm*` directories contain packages named `handl-runtime*`, while the `crates/runtime*` directories also contain packages named `handl-runtime*`. The source and internal documentation already refer to the project as `handl-vm`, but the published package names do not match.

In addition, the current higher-level runtime crates (`runtime-sandbox`, `runtime-type`, `runtime-server`) are hard-coded to the canonical value representation and the canonical tree-walking evaluator. This makes it impossible to swap the VM backend or the value representation, which blocks future work on bytecode and JIT execution targets.

We need a crate layout that:

- is internally consistent,
- keeps the VM abstraction generic over both value type and execution strategy,
- supports multiple VM backends,
- supports multiple value representations,
- removes the overloaded and inconsistent `runtime` namespace.

## Decision

We will move the entire VM surface into an `handl-plugin-vm-*` namespace with a strict separation between the abstraction, value representations, backends, and product crates.

### Crate layout

| Current crate / dir                                      | New crate / dir                                                                          | Responsibility                                                                                                                                                       |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/vm` (`handl-runtime`)                            | `crates/vm` (`handl-vm`)                                                                 | Core VM traits: `Value`, `Host`, `Context`, `VirtualMachine`, `Eval`, `Operation`, `Error`                                                                           |
| `crates/plugin-vm-common` (`handl-runtime-common`)       | `crates/plugin-vm-common` (`handl-plugin-vm-common`)                                     | Generic host building blocks: `Module<V>`, `ModuleHost<V>`, adapters, combinators, mocks, middleware, sandbox helpers                                                |
| `crates/plugin-vm-canonical` (`handl-runtime-canonical`) | `crates/plugin-vm-backend-rust-interpreter` (`handl-plugin-vm-backend-rust-interpreter`) | Canonical tree-walking interpreter implemented in Rust                                                                                                               |
| `crates/runtime` (`handl-runtime-values`)                | `crates/plugin-vm-values-json` (`handl-plugin-vm-values-json`)                           | JSON-backed value representation: `NonFunctionValue`, `FunctionValue`, `HostValue`, `Frontend`                                                                       |
| `crates/runtime-config` (`handl-runtime-config`)         | — delete —                                                                               | `CanonicalContext` and the concrete `Context` implementation move to `handl-plugin-vm-backend-rust-interpreter`; value types move to `handl-plugin-vm-values-json`   |
| `crates/runtime-core` (`handl-runtime-modules`)          | — delete —                                                                               | `Module` and `ModuleHost` move to `handl-plugin-vm-common`                                                                                                           |
| `crates/runtime-sandbox` (`handl-runtime-sandbox`)       | `crates/plugin-vm-config` (`handl-plugin-vm-config`)                                     | Generic, value-agnostic, pure configuration VM (defaults to `handl-plugin-vm-values-json` + `handl-plugin-vm-backend-rust-interpreter` behind a `canonical` feature) |
| `crates/runtime-type` (`handl-runtime-type`)             | `crates/plugin-vm-type` (`handl-plugin-vm-type`)                                         | Generic, value-agnostic type-as-values VM                                                                                                                            |
| `crates/runtime-server` (`handl-runtime-server`)         | `crates/plugin-vm-server` (`handl-plugin-vm-server`)                                     | Generic, value-agnostic server VM                                                                                                                                    |
| _new_                                                    | `crates/plugin-vm-transform` (`handl-plugin-vm-transform`)                               | Generic AST / expression transformation utilities                                                                                                                    |

### Planned future crates

| New crate                          | Responsibility                                                     |
| ---------------------------------- | ------------------------------------------------------------------ |
| `handl-plugin-vm-backend-bytecode` | Bytecode compiler + interpreter VM                                 |
| `handl-plugin-vm-backend-jit`      | JIT compiled VM                                                    |
| `handl-plugin-vm-values-*`         | Additional value representations as needed (e.g. compact, no-heap) |

### Design rules

1. `handl-vm` is the only abstraction crate. It defines `Value`, `Host`, `Context`, `VirtualMachine`, `Eval`, `Operation`, `Error`. It is generic over `V` and has no dependency on `serde_json`, the canonical AST, or any backend.
2. `handl-plugin-vm-values-json` is one concrete `Value` implementation. It depends on `handl-vm` and `serde_json`.
3. `handl-plugin-vm-backend-*` crates implement the `handl-vm` traits. They are allowed to be generic over `V` but may choose a default value representation (e.g. `handl-plugin-vm-values-json`) for convenience.
4. `lib-*` modules become generic over `V: Value`. They may use `handl-plugin-vm-values-json` as their default test and production value, but they must not hard-code `NonFunctionValue`.
5. `handl-plugin-vm-common` holds generic host-building blocks, including `Module<V>` and `ModuleHost<V>` (currently `handl-runtime-modules`).
6. `handl-plugin-vm-config`, `handl-plugin-vm-type`, `handl-plugin-vm-server`, and `handl-plugin-vm-transform` are generic over `V: Value` and over the VM backend. They depend on `handl-vm` and `handl-plugin-vm-common`, not on a specific `handl-plugin-vm-backend-*` crate.
7. The `handl-runtime-*` package namespace is retired. The `runtime` and `runtime-*` directory names are retired.

## Consequences

- Every `Cargo.toml` and `use` statement that refers to `handl-runtime*` must be updated.
- `crates/runtime` and `crates/runtime-core` and `crates/runtime-config` are removed from the workspace.
- `handl-plugin-vm-sandbox` no longer exists as a separate crate; its functionality either moves to `handl-plugin-vm-common` as a generic building block or becomes part of `handl-plugin-vm-config` as the default pure config VM.
- The `lib-*` modules must be refactored to operate through the `Value` trait rather than `NonFunctionValue`-specific accessors.
- Future `handl-plugin-vm-backend-bytecode` and `handl-plugin-vm-backend-jit` can be added without renaming or touching the abstraction or product crates.
- Future `handl-plugin-vm-values-*` can be added without touching the VM core.
- Consumers such as the `handl` CLI get a clear three-layer dependency: `handl-vm` (traits) + `handl-plugin-vm-values-json` (values) + `handl-plugin-vm-backend-rust-interpreter` (default backend), with the ability to substitute any layer.

(See status at top of document.)
