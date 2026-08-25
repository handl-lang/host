# Runtime

The handl runtime evaluates expressions and mediates between the language and platform capabilities.

## The execution model

handl is interpreted by a small Rust VM. The VM is responsible for:

- Parsing source into an AST.
- Evaluating expressions and applying functions.
- Serializing and deserializing JSON values.
- Dispatching calls to modules.

## Values

All handl values are represented as `serde_json::Value` internally. This means the boundary between handl code and host code is just JSON. Functions that return rich data must explicitly serialize it to JSON.

## Modules

A module is a Rust type that implements the `Module` trait:

```rust
trait Module {
    fn name(&self) -> &str;
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>>;
}
```

The runtime collects all modules into a `ModuleHost`. When a handl expression calls `fs.read(...)`, the VM looks up the `fs` module, finds the `read` function, and calls it.

## Capabilities

Capability modules (`fs`, `env`, `http`, `kv`, etc.) are abstracted behind traits. The runtime provides the concrete implementation. This design lets the same script run with:

- real OS services in production,
- in-memory stubs in tests,
- browser or embedded implementations in other contexts.

## Errors

Runtime errors are reported as JSON-friendly values with a type and message. Common error kinds include `ArgumentCount`, `TypeMismatch`, and custom module errors.
