# VM Crate Restructure

## Context

The workspace currently mixes `handl-runtime-*` and `handl-vm-*` package and directory names. The `crates/vm*` directories contain packages named `handl-runtime*`, while the `crates/runtime*` directories also contain packages named `handl-runtime*`. The source and internal documentation already refer to the project as `handl-vm`, but the published package names do not match.

In addition, the current higher-level runtime crates (`runtime-sandbox`, `runtime-type`, `runtime-server`) are hard-coded to the canonical value representation and the canonical tree-walking evaluator. This makes it impossible to swap the VM backend or the value representation, which blocks future work on bytecode and JIT execution targets.

We need a crate layout that:
- is internally consistent,
- keeps the VM abstraction generic over both value type and execution strategy,
- supports multiple VM backends,
- supports multiple value representations,
- removes the overloaded and inconsistent `runtime` namespace.

## Decision

We will move the entire VM surface into an `handl-vm-*` namespace with a strict separation between the abstraction, value representations, backends, and product crates.

### Crate layout

| Current crate / dir | New crate / dir | Responsibility |
|---|---|---|
| `crates/vm` (`handl-runtime`) | `crates/vm` (`handl-vm`) | Core VM traits: `Value`, `Host`, `Context`, `VirtualMachine`, `Eval`, `Operation`, `Error` |
| `crates/vm-common` (`handl-runtime-common`) | `crates/vm-common` (`handl-vm-common`) | Generic host building blocks: `Module<V>`, `ModuleHost<V>`, adapters, combinators, mocks, middleware, sandbox helpers |
| `crates/vm-canonical` (`handl-runtime-canonical`) | `crates/vm-backend-rust-interpreter` (`handl-vm-backend-rust-interpreter`) | Canonical tree-walking interpreter implemented in Rust |
| `crates/runtime` (`handl-runtime-values`) | `crates/vm-values-json` (`handl-vm-values-json`) | JSON-backed value representation: `NonFunctionValue`, `FunctionValue`, `HostValue`, `Frontend` |
| `crates/runtime-config` (`handl-runtime-config`) | — delete — | `CanonicalContext` and the concrete `Context` implementation move to `handl-vm-backend-rust-interpreter`; value types move to `handl-vm-values-json` |
| `crates/runtime-core` (`handl-runtime-modules`) | — delete — | `Module` and `ModuleHost` move to `handl-vm-common` |
| `crates/runtime-sandbox` (`handl-runtime-sandbox`) | `crates/vm-config` (`handl-vm-config`) | Generic, value-agnostic, pure configuration VM (defaults to `handl-vm-values-json` + `handl-vm-backend-rust-interpreter` behind a `canonical` feature) |
| `crates/runtime-type` (`handl-runtime-type`) | `crates/vm-type` (`handl-vm-type`) | Generic, value-agnostic type-as-values VM |
| `crates/runtime-server` (`handl-runtime-server`) | `crates/vm-server` (`handl-vm-server`) | Generic, value-agnostic server VM |
| _new_ | `crates/vm-transform` (`handl-vm-transform`) | Generic AST / expression transformation utilities |

### Planned future crates

| New crate | Responsibility |
|---|---|
| `handl-vm-backend-bytecode` | Bytecode compiler + interpreter VM |
| `handl-vm-backend-jit` | JIT compiled VM |
| `handl-vm-values-*` | Additional value representations as needed (e.g. compact, no-heap) |

### Design rules

1. `handl-vm` is the only abstraction crate. It defines `Value`, `Host`, `Context`, `VirtualMachine`, `Eval`, `Operation`, `Error`. It is generic over `V` and has no dependency on `serde_json`, the canonical AST, or any backend.
2. `handl-vm-values-json` is one concrete `Value` implementation. It depends on `handl-vm` and `serde_json`.
3. `handl-vm-backend-*` crates implement the `handl-vm` traits. They are allowed to be generic over `V` but may choose a default value representation (e.g. `handl-vm-values-json`) for convenience.
4. `lib-*` modules become generic over `V: Value`. They may use `handl-vm-values-json` as their default test and production value, but they must not hard-code `NonFunctionValue`.
5. `handl-vm-common` holds generic host-building blocks, including `Module<V>` and `ModuleHost<V>` (currently `handl-runtime-modules`).
6. `handl-vm-config`, `handl-vm-type`, `handl-vm-server`, and `handl-vm-transform` are generic over `V: Value` and over the VM backend. They depend on `handl-vm` and `handl-vm-common`, not on a specific `handl-vm-backend-*` crate.
7. The `handl-runtime-*` package namespace is retired. The `runtime` and `runtime-*` directory names are retired.

## Consequences

- Every `Cargo.toml` and `use` statement that refers to `handl-runtime*` must be updated.
- `crates/runtime` and `crates/runtime-core` and `crates/runtime-config` are removed from the workspace.
- `handl-vm-sandbox` no longer exists as a separate crate; its functionality either moves to `handl-vm-common` as a generic building block or becomes part of `handl-vm-config` as the default pure config VM.
- The `lib-*` modules must be refactored to operate through the `Value` trait rather than `NonFunctionValue`-specific accessors.
- Future `handl-vm-backend-bytecode` and `handl-vm-backend-jit` can be added without renaming or touching the abstraction or product crates.
- Future `handl-vm-values-*` can be added without touching the VM core.
- Consumers such as the `handl` CLI get a clear three-layer dependency: `handl-vm` (traits) + `handl-vm-values-json` (values) + `handl-vm-backend-rust-interpreter` (default backend), with the ability to substitute any layer.

## Status

Proposed
