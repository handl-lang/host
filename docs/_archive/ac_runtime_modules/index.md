# ac_runtime_modules

# handl-runtime-core

Abstract module system for the handl runtime.

The handl language specification defines function calls as `fn(args)`. This
crate abstracts how functions are organized into named **modules** and how
a composite host dispatches qualified names like `math.abs(x)`.

## Architectural rule

> **The VM knows nothing about modules.** The `Module` trait groups
> functions by namespace; `ModuleHost` composes modules into a single
> `Host` implementation. The VM sees only `Host::function(name)`.

This crate defines the interface only. Concrete module implementations
live in `handl-runtime-std`.

## Modules

### [`ac_runtime_modules`](ac_runtime_modules.md)

*2 modules*

### [`host`](host.md)

*1 module*

### [`host::host`](host/host.md)

*1 struct*

### [`module`](module.md)

*2 modules*

### [`module::module`](module/module.md)

*1 trait*

### [`module::module_stub`](module/module_stub.md)

*1 static, 4 structs*

