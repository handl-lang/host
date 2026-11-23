# Runtime

The handl runtime is not a single, hard-coded VM. It is the set of
operation plugins that a `DefaultHost` loads and executes, plus the engine that
evaluates `Term`s. There is no separate "VM" subsystem — evaluation is
the `Eval` operation, served like any other operation.

## The execution model

A `DefaultHost` loads plugins and kits, then `load` makes a single
registration-order pass calling each plugin's `contribute` (rule
contributions whose acceptor is not yet registered are deferred and
retried). After loading, `DefaultHost::run` dispatches a
`CapabilityRequest` to the plugin that provides the requested capability.

The language pipeline is also a set of operation executions. A host that
loads the lex, parse, typecheck, and eval contract surfaces can run those
operations by executing the corresponding operation plugin — each one an
engine parameterized by the same `Language` data
(see [Language as data](./language-as-data)).

## Terms and values

Programs reach the engines as `Term`s — nodes in the closed arrow
calculus defined by `hnl-engine` (the [`engine`
repo](https://github.com/handl-lang/engine)) — over the shared value
representation (`handl-value`'s `TaggedValue` in the
[`kernel`](https://github.com/handl-lang/kernel) repo, with
`serde_json::Value` on the wire boundary). The engine layer evaluates
`Term`s against the `Language`'s `eval` schemes: `lang.eval[node.kind]`
yields the scheme, and the scheme is pure data until an engine
interprets it.

## One term, several interpretations

The same `Term` graph is interpreted by interchangeable **eval
engines**, all implementing the `EvalEngine` contract from
`hnl-exec` (the [`exec`
repo](https://github.com/handl-lang/exec)):

| engine | repo | what it does |
| --- | --- | --- |
| tree-walk | [`exec-tree-walk`](https://github.com/handl-lang/exec-tree-walk) | The **reference interpreter** — walks the `Term` graph directly against the shared native-op table. It is the oracle every other eval engine must agree with. |
| bytecode | [`exec-bytecode`](https://github.com/handl-lang/exec-bytecode) | Compiles schemes to a stack-VM bytecode and interprets that — faster dispatch, identical results (its conformance suite compares against tree-walk). |
| cranelift | [`exec-cranelift`](https://github.com/handl-lang/exec-cranelift) | Compiles schemes to native code through Cranelift — in-process JIT or an object file for a standalone binary. Arrow bodies it does not cover fall back to the reference interpreter; that fallback is the one documented cross-engine dependency. |

The invariant across all three: **value-for-value agreement with the
tree-walk oracle**. An engine may change speed, never meaning — the
same rule the [engines overview](/engines/) states for every operation.
Which engine a host runs is a kit/target choice, not a language
property: the `Language` data and the `Term` schemes are identical
either way.

Eval is not the only operation with swappable engines. The same
pattern holds one level up — `tool-transform-rewrite` interprets
`transforms` fragments as guarded rewrite rules (conditions like
`KindAt`/`PayloadEq`/`AttrEq`/`SameAttr` decide whether a rule may
fire), `tool-typecheck` typechecks against the `types` schemes (with
a normalization-based engine in a private repo alongside the contract
crate's bootstrap checker), and `tool-fmt`/`tool-lint` render and
lint the AST. None of them is "the" definition of the language — the
`Language` data is.

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
