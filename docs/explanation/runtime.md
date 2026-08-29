# Runtime

The handl runtime is not a single, hard-coded VM. It is the set of operation plugins that a `Host` loads and executes, plus the target plugin that evaluates expressions. There is no separate "VM" subsystem — evaluation is the `Eval` operation, served by target plugins like any other operation.

## The execution model

A `Host` loads kits, resolves dependencies, and runs `contribute` to a fixed point. After loading, `Host::execute` dispatches a `CapabilityRequest` to the plugin that provides the requested capability.

The language pipeline is also a set of operation executions. A host that loads `handl-ext-lexer`, `handl-ext-parser`, `handl-ext-type`, `handl-ext-lint`, `handl-ext-fmt`, and `handl-ext-eval` can run `lex`, `parse`, `typecheck`, `lint`, `format`, and `eval` by executing the corresponding operation plugin.

## Values

All handl values are represented as `serde_json::Value` internally. This means the boundary between handl code and host code is just JSON. Functions that return rich data must explicitly serialize it to JSON. There is no generic value-type parameter — the kernel commits to JSON values, and the `Eval` contract operates on `serde_json::Value`.

## Operations and targets

Evaluation is the `Eval` operation. `handl-ext-eval` defines the `Eval` operation and `EvalBehavior`. There is no single `plugin-target-rust-interpreter` crate — the interpreter target is emergent from which `plugin-eval-*` plugins the kit loads. Each `plugin-eval-*` plugin contributes one `Behavior<Eval>` rule for a specific AST kind (value, call, apply, array, object, index, field-access, handle, var, assign, function), and `plugin-eval-core` provides the eval kernel builtins (`var`/`resolve`/`apply`). A compiler target (wasm/JIT/bytecode) does not use `Eval`; it uses the `Compile` operation with `CompileBehavior` rules instead, since compilation processes the whole AST rather than evaluating per-node. "Targets are plugins" holds in both cases: an interpreter target is a set of `Eval` rules; a compiler target is a set of `Compile` rules. Both contribute rules into `BasicEvaluator`, the same `RuleAcceptor` pattern used by every other operation. There is no `VirtualMachine` trait and no `Value` trait parallel to the platform's operation system.

Modules such as `fs`, `env`, `http`, and `kv` are contributed by library or target plugins. The host does not hard-code a module list. It discovers them through the loaded plugins.

## Garbage collection

Garbage collection is provided by a target or library plugin and selected by the host. The kernel imposes no GC. A host may run with no GC, a reference-counting GC, a tracing GC, or a region-based collector — all are plugin choices. See design principle #15.

## Sandboxing

Whether a program runs sandboxed is determined by which stdlib plugins are exposed and which target plugin runs it, not by a kernel flag. A kit that exposes only pure modules and selects a non-IO target produces a sandbox; a kit that exposes `fs`, `env`, and `http` over a native target does not. The kernel has no sandbox concept. See design principle #16.

## Capabilities

The same script can run with:

- real OS services in production,
- in-memory stubs in tests,
- browser or embedded implementations in other contexts.

Which one a given host uses is determined by the target and library plugins it loads.

## Errors

Runtime errors are reported as JSON-friendly values with a type and message. Common error kinds include `ArgumentCount`, `TypeMismatch`, and custom module errors.
