# Platform architecture

The handl platform is organized as a workspace of Rust crates that compose into language, runtime, and capability layers.

## Crate layers

```
handl-cli              # command-line entry point
handl-ast, handl-parser
handl-vm, handl-vm-common
handl-lib-*            # capability modules (array, fs, http, ...)
```

- **Language crates** (`handl-ast`, `handl-parser`) define the syntax and parse source.
- **VM crates** (`handl-vm`, `handl-vm-common`) define the interpreter, the `Function` and `Module` traits, and `NonFunctionValue`.
- **Library crates** (`handl-lib-*`) implement modules. Each crate exposes one or more `*Module` adapters.

## Capability boundaries

A library crate like `handl-lib-os` defines the `Fs` trait but not the concrete `std::fs` implementation. The `handl-lib-os-std` crate provides a production backend, while `handl-lib-os` may provide a stub backend for tests. This separation keeps the language core platform-agnostic.

## Module host

The `ModuleHost` is where modules are registered. A host for a server might look like:

```rust
let host = ModuleHost::new()
    .with(MathModule)
    .with(ArrayModule)
    .with(FsModule::new(StdFs));
```

The VM uses the host to resolve every `module.function` call at runtime.

## Adding a new module

To add a module:

1. Create a crate `handl-lib-myfeature`.
2. Implement `Module` for `MyFeatureModule`.
3. Implement `Function<NonFunctionValue>` for each exposed function.
4. Register `MyFeatureModule` in the runtime’s `ModuleHost`.

For reference, see the existing `handl-lib-*` crates and the generated [reference pages](../reference/).
