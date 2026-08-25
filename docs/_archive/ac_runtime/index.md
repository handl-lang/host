# ac_runtime

# handl-vm

Core virtual-machine abstractions for the handl language.

This crate defines the evaluation contract: `VirtualMachine`,
`Context`, `Host`, `Function`, `Operation`, `Error`, etc.
It does not contain a concrete implementation; it is generic over
the value type `V` via the [`Value`] trait.

The canonical tree-walking VM lives in `handl-vm-canonical`. Reusable
helpers, mocks, and host combinators live in `handl-vm-common`.

## Modules

### [`ac_runtime`](ac_runtime.md)

*7 modules*

### [`error`](error.md)

*1 enum*

### [`eval`](eval.md)

*2 traits*

### [`function_repr`](function_repr.md)

*1 trait*

### [`host`](host.md)

*1 struct, 5 traits*

### [`host::Function`](host/Function.md)

*1 function*

### [`host::GenericFunction`](host/GenericFunction.md)

*1 function*

### [`host::Host`](host/Host.md)

*5 functions*

### [`operation`](operation.md)

*1 struct, 1 trait*

### [`operation::Operation`](operation/Operation.md)

*1 function*

### [`runtime`](runtime.md)

*2 traits*

### [`runtime::Context`](runtime/Context.md)

*1 function*

### [`runtime::VirtualMachine`](runtime/VirtualMachine.md)

*1 function*

### [`value`](value.md)

*1 trait*

