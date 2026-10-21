# Design principles

Principles that shape the platform. Language-surface principles are
unpublished with the language; these are the ones that govern the
kernel, engines, and host.

1. **Runtime-agnostic.** The platform is independent of any runtime,
   host, or execution target. Programs can run in browsers, game
   engines, servers, CLI tools, embedded hardware, or GPUs — the host
   determines the execution model.
2. **Declarative.** A document describes _what_ values it contains, not
   _how_ to compute them.
3. **Format-agnostic.** Documents can be written in any format that maps
   to JSON-compatible values.
4. **Embeddable.** The core is a small kernel designed to be embedded in
   a host application, not a standalone ecosystem — the Lua/Rhai
   embedding model, not the Python/Unison model. No runtime owns the
   codebase, no toolchain must be shipped, no garbage collector is
   imposed on the host.
5. **Host provides the world.** The kernel defines no built-in
   functions, no standard library, no I/O primitives. Everything a
   program can do is provided by the host: which functions exist, which
   namespaces are available, what side effects are permitted.
6. **Content addressing is optional.** Definitions can be identified by
   hash for deduplication, reproducible builds, and cross-machine
   sharing — but it is a storage layer, not a constraint.
7. **Composition over keywords.** The kernel has a small set of
   primitives. Complex patterns emerge from composition, not from
   dedicated constructs.
8. **A language is data.** A `Language` value is assembled from
   fragments; engines dispatch on that data. Adding a language
   construct is a data change — never an engine recompile.
9. **Tree-shakeable.** Major subsystems are gated by Cargo features. A
   host enables only the capabilities it needs, and the compiler omits
   the rest.
10. **Everything is a plugin.** Every concept is a `Plugin`: semantics,
    tooling, targets, the garbage collector, the standard library. A
    `Host` is an empty composer that loads kits and plugins and
    dispatches `CapabilityRequest`s. Nothing is hard-coded except the
    composition primitives (`Plugin`, `Host`, `Operation`, `Behavior`,
    `RuleAcceptor`, `Contribution`). There is no privileged subsystem.
11. **Evaluation is an operation, not a subsystem.** There is no
    separate "VM" layer. Evaluation is the `Eval` operation, served by
    `EvalBehavior` rules contributed by plugins — no `VirtualMachine`
    trait sits alongside the operation system.
12. **Garbage collection is a plugin.** A GC strategy is provided by a
    target or library plugin and selected by the host. A host may run
    with no GC, a reference-counting GC, a tracing GC, or a
    region-based collector — all plugin choices.
13. **Sandboxing is a kit/plugin property.** Whether a program runs
    sandboxed is determined by which plugins are exposed and which
    engine runs it, not by a kernel flag.
14. **Targets are plugins.** A target plugin provides the execution
    backend (interpreter, compiler, JIT, wasm runtime) by contributing
    rules. Targets are not a separate layer above plugins — they are
    plugins.
