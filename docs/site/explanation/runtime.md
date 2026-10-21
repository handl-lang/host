# Runtime

The handl runtime is not a single, hard-coded VM. It is the set of
operation plugins that a `Host` loads and executes, plus the engine that
evaluates `Term`s. There is no separate "VM" subsystem — evaluation is
the `Eval` operation, served like any other operation.

## The execution model

A `Host` loads kits, resolves dependencies, and runs `contribute` to a
fixed point. After loading, `Host::execute` dispatches a
`CapabilityRequest` to the plugin that provides the requested capability.

The language pipeline is also a set of operation executions. A host that
loads the lex, parse, typecheck, and eval contract surfaces can run those
operations by executing the corresponding operation plugin — each one an
engine parameterized by the same `Language` data
(see [Language as data](./language-as-data)).

## Terms and values

Programs reach the engines as `Term`s — nodes in the closed arrow
calculus — over the shared `Value` representation (both defined in the
[`kernel`](https://github.com/handl-lang/kernel) repo). The engine layer
evaluates `Term`s against the `Language`'s `eval` schemes; the reference
interpreter is `hnl-engine-exec-tree-walk`, which walks the
`Term` graph directly.

## Operations and engines

Evaluation is the `Eval` operation. `handl-platform` defines the `Eval`
contract (`Eval`/`EvalRequest`/`EvalBehavior`) and `BasicEvaluator`, the
`RuleAcceptor` that collects one behavior per `AstNode.kind`; the
`handl-plugin-ext-eval` crate is the published capability surface. A
compiler target does not use `Eval` — it uses the `Compile` operation
with `CompileBehavior` rules instead, since compilation processes the
whole program rather than evaluating per node. "Targets are plugins"
holds in both cases: an interpreter target is a set of `Eval` rules; a
compiler target is a set of `Compile` rules.

Modules — capability namespaces such as filesystem, env, or clock access
— are contributed by library plugins through the `Module`/`Function`
traits in `handl-platform`. The host does not hard-code a module list; it
discovers modules through the loaded plugins.

## Garbage collection

Garbage collection is provided by a target or library plugin and selected
by the host. The kernel imposes no GC. A host may run with no GC, a
reference-counting GC, a tracing GC, or a region-based collector — all
are plugin choices. See design principle #15.

## Sandboxing

Whether a program runs sandboxed is determined by which modules are
exposed and which engine runs it, not by a kernel flag. A kit that
exposes only pure modules and selects a non-IO engine produces a sandbox.
The kernel has no sandbox concept. See design principle #16.

## Capabilities

The same program can run with:

- real OS services in production,
- in-memory stubs in tests,
- browser or embedded implementations in other contexts.

Which one a given host uses is determined by the target and library
plugins it loads.

## Errors

Errors are reported as structured values with a kind and message. The
`plugin-ext-diag` contract surface defines the shared `Diagnostic`,
`Severity`, `Span`, and `Fix` vocabulary every operation renders through.
