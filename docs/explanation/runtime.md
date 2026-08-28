# Runtime

The handl runtime is not a single, hard-coded VM. It is the set of operation plugins that a `Host` loads and executes, plus the target plugin that evaluates expressions.

## The execution model

A `Host` loads kits, resolves dependencies, and runs `contribute` to a fixed point. After loading, `Host::execute` dispatches a `CapabilityRequest` to the plugin that provides the requested capability.

The language pipeline is also a set of operation executions. A host that loads `handl-ext-lexer`, `handl-ext-parser`, `handl-ext-type`, `handl-ext-lint`, `handl-ext-fmt`, and `handl-ext-eval` can run `lex`, `parse`, `typecheck`, `lint`, `format`, and `eval` by executing the corresponding operation plugin.

## Values

All handl values are represented as `serde_json::Value` internally. This means the boundary between handl code and host code is just JSON. Functions that return rich data must explicitly serialize it to JSON.

## Operations and targets

Evaluation is the `Eval` operation. `handl-ext-eval` defines the `Eval` operation and `EvalBehavior`. `handl-target-rust-interpreter` is the default `RuleAcceptor` that runs the interpreter. Other targets, such as `handl-target-wasm` or `handl-target-llvm`, can register their own `Eval` behavior without changing the language core.

Modules such as `fs`, `env`, `http`, and `kv` are contributed by library or target plugins. The host does not hard-code a module list. It discovers them through the loaded plugins.

## Capabilities

The same script can run with:

- real OS services in production,
- in-memory stubs in tests,
- browser or embedded implementations in other contexts.

Which one a given host uses is determined by the target and library plugins it loads.

## Errors

Runtime errors are reported as JSON-friendly values with a type and message. Common error kinds include `ArgumentCount`, `TypeMismatch`, and custom module errors.
