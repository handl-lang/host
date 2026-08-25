# Module System: Async + Operation Dispatch

## Problem

The VM (`handl-vm`) supports three dispatch channels via the `Host` trait:

1. `Host::function` — sync `Function` trait
2. `Host::async_function` — async `AsyncFunction` trait (returns `Future`)
3. `Host::operation` — typed `Operation` trait (signals via `Error::Operation`, enables algebraic effect handling with continuations)

`Context` also has `Eval` (sync) and `AsyncEval` (async) callbacks for higher-order functions, and `Expr::Handle` for handler expressions.

But the module system doesn't wire any of this through:

- `Module` trait (`runtime-core/src/module/module.rs`) only exposes `fn function(&self, name: &str) -> Option<&dyn Function>`.
- `ModuleHost` (`runtime-core/src/host/host.rs`) only implements `Host::function`. Does NOT implement `Host::async_function` or `Host::operation`.

This means no module registered through `ModuleHost` can provide async functions or operations, even though the VM fully supports them.

## P0: Extend `Module` trait + `ModuleHost`

### `Module` trait changes

Add `async_function` and `operation` methods with default `None` implementations:

```rust
pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn function(&self, name: &str) -> Option<&dyn Function> { None }
    fn async_function(&self, name: &str) -> Option<&dyn AsyncFunction> { None }
    fn operation(&self, name: &str) -> Option<&dyn Operation> { None }
}
```

### `ModuleHost` changes

Implement `Host::async_function` and `Host::operation` to delegate to modules, same split-on-`.` dispatch as `function`:

```rust
impl Host for ModuleHost {
    fn function(&self, name: &str) -> Option<&dyn Function> { /* existing */ }
    fn async_function(&self, name: &str) -> Option<&dyn AsyncFunction> { /* same dispatch */ }
    fn operation(&self, name: &str) -> Option<&dyn Operation> { /* same dispatch */ }
}
```

### Backward compatibility

- Existing modules that only implement `function` continue to work — the new methods default to `None`.
- Existing `Module::function` becomes optional with a default `None` impl, OR we keep it required (preferred — every module should have at least one function).

### Files touched

- `crates/runtime-core/src/module/module.rs` — add methods to `Module`
- `crates/runtime-core/src/host/host.rs` — implement `async_function` + `operation` on `ModuleHost`
- `crates/runtime-core/src/module/module_stub.rs` — update stub if needed
- `crates/runtime-core/src/module/module_tests.rs` — add tests for async/operation dispatch

## P1: Platform services async + operations

### Affected crates

- `lib-platform` (traits + module adapters)
- `lib-platform-std` (concrete implementations)

### `HttpClient` — async + operation

Current: `fn request(&self, req: &HttpRequest) -> Result<HttpResponse, PlatformError>` (blocking).

Add:
- `async fn request_async(&self, req: &HttpRequest) -> Result<HttpResponse, PlatformError>` to `HttpClient` trait (or a separate `AsyncHttpClient` trait).
- `HttpClientModule` implements `AsyncFunction` for async variants of `get`/`post`/`put`/`patch`/`delete`/`head`/`request`.
- Optionally expose `http.request` as an `Operation` — handler can intercept for logging, rate-limiting, permission checks, or mocking.

### `Fs` — async + operation

Same pattern. `read_file`/`write_file` get async variants. `fs.read` as an `Operation` enables handler interception (permission checks, sandboxing).

### `Cmd` — async + operation

`cmd.run` is inherently blocking (waits for process). Async variant allows concurrent command execution. As an `Operation`, handlers can enforce allowlists, timeouts, or deny execution.

### Design decision: trait extension vs separate trait

Two options:
- **(A) Extend existing traits** — add `async_*` methods to `Fs`, `HttpClient`, `Cmd`. Simple, but forces implementors to provide both sync and async.
- **(B) Separate async traits** — `AsyncFs`, `AsyncHttpClient`, `AsyncCmd`. Implementors choose. More flexible, more boilerplate.

Recommend (B) for platform traits — not every backend needs both. The module adapter can accept either.

## P2: `fp.map`/`filter`/`reduce` async

### Affected crate

- `lib-utils` (`fp` module)

### Current state

`fp.map`, `fp.filter`, `fp.reduce` use `ctx.eval_expr` (sync `Eval` callback). They can't work with async predicates.

### Change

Add async variants (`fp.map_async`, `fp.filter_async`, `fp.reduce_async`) that implement `AsyncFunction` and use `ctx.eval_expr_async` (`AsyncEval` callback).

Or: make the existing functions check if `AsyncEval` is available and use it, falling back to sync. But this changes semantics — better to have explicit async variants.

### Files touched

- `crates/lib-utils/src/fp/fp.rs` — add async function structs
- `crates/lib-utils/src/fp/fp_tests.rs` — async tests
- `crates/lib-utils/src/fp/mod.rs` — re-exports if needed

## P3: `KeyValueStore` async

### Affected crates

- `lib-kv` (trait + module adapter)
- `lib-kv-hashmap` (implementation)

### Change

Add `AsyncKeyValueStore` trait with async methods. Module adapter implements `AsyncFunction`. The HashMap backend can implement both sync and async trivially (async just wraps sync).

Network backends (future: Redis, SQLite) would only implement async.

## P3: `DefaultHost` expansion

### Affected crate

- `host` (default host + functions)

### Current state

Only `add`, `mul`, `concat`, `len`.

### Change

Add more built-in functions. Also implement `Host::async_function` and `Host::operation` if built-in operations make sense (e.g. a `yield` operation for cooperative multitasking).

## Implementation order

1. P0 (`Module` + `ModuleHost`) — unblocks all downstream
2. P1 (`http` async + operation) — highest user impact
3. P1 (`fs` async + operation)
4. P1 (`cmd` async + operation)
5. P2 (`fp` async)
6. P3 (`kv` async, `DefaultHost` expansion)
