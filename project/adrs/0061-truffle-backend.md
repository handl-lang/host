# ADR-0061: Truffle Backend

## Status

Proposed

Adds a fourth eval engine alongside `plugin-engine-eval-tree-walk`,
`plugin-engine-eval-bytecode`, and `plugin-engine-eval-cranelift`. Targets
the arrow-calculus `SchemeV2` kernel from [ADR-0052 — Arrow Calculus
Kernel](./0052-arrow-calculus-kernel.md) (v1 cutover complete; v2 is the
sole default) and the data-driven engine model from [ADR-0044 — Language
as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
§4. Follows the "execution is one handler interpretation" thesis of
ADR-0052 §"Arrow Handlers" and the polyglot-runtime framing of
`docs/explanations/polyglot-runtimes.md`, which cites GraalVM/Truffle as a
peer architecture.

**Sibling ADR:** [ADR-0062 — JVM Bytecode Emitter
Backend](./0062-jvm-bytecode-emitter-backend.md) adds a stock-JVM code-gen
peer to this engine. The two are not mutually exclusive; this ADR's
"stock-JVM bytecode emission is a future fifth engine" framing is
realized by ADR-0062.

## Context

ADR-0044 §4 established that eval engines are fixed interpreters of
`Language.eval` data. ADR-0052 reduced the kernel to six arrow-calculus
primitives (`arr`/`>>>`/`first`/`choice`/`loop`/`app`) and stated the
semantic thesis explicitly: *execution is one possible handler
interpretation, rather than a primitive property of arrows* (Sanada's
model). Three engines already implement the `EvalEngine` trait
(`crates/plugin-engine-eval/src/lib.rs`):

- `plugin-engine-eval-tree-walk` — reference interpreter (interprets
  `SchemeV2` directly).
- `plugin-engine-eval-bytecode` — compiles `SchemeV2` to a custom ISA.
- `plugin-engine-eval-cranelift` — compiles `SchemeV2` to native machine
  code via cranelift JIT; supports on-disk binaries via `cranelift-object`.

A JVM target is the obvious next backend. Two strategies are available:

1. **JVM bytecode emitter** — a `plugin-engine-eval-jvm` crate that lowers
   `SchemeV2` to `.class` files, structurally mirroring
   `plugin-engine-eval-cranelift`'s `compile_v2.rs` + `standalone.rs`. This
   is a peer of the cranelift engine with a different code generator.

2. **Truffle interpreter on GraalVM** — port the `V2Interpreter`
   tree-walk handler to a Truffle interpreter in Java, getting JIT and
   polyglot interop from the GraalVM framework rather than from a
   hand-written code generator.

This ADR picks strategy 2.

### Why Truffle, and why also a bytecode emitter

The project's stated philosophy is *execution is one handler
interpretation*. The tree-walk `V2Interpreter` is the canonical handler;
the bytecode and cranelift engines are *optimizations* of that handler,
each requiring a bespoke code generator. A Truffle port preserves the
handler shape directly: the Java interpreter is structurally the same
walk over `SchemeV2` as `V2Interpreter`, and the GraalVM JIT specializes
it automatically. We get native-tier performance without writing a
code generator, and we inherit GraalVM's polyglot value interop — the
same property `docs/explanations/polyglot-runtimes.md` cites as the
distinguishing feature of GraalVM/Truffle among existing polyglot
runtimes.

A bytecode emitter (ADR-0062) produces stock-JVM `.class` files (broader
deployment, no GraalVM dependency) at the cost of writing and maintaining
a JVM bytecode generator. The two backends serve different deployment
profiles and coexist as sibling engines: Truffle for
interactive/polyglot workloads on GraalVM, bytecode emitter for
ahead-of-time compilation and stock-JVM deployment. Both interpret the
same `SchemeV2` data; conformance is one corpus, five engines.

### What Truffle gives us

- **JIT for free.** The Truffle framework specializes the interpreter
  at runtime based on observed types and shapes. This is the same
  outcome as the cranelift JIT, obtained by writing an interpreter
  rather than a compiler.
- **Polyglot value interop.** GraalVM's `Polyglot` API lets a `handl`
  program pass values to and receive values from Java, Kotlin, Scala,
  Clojure, JavaScript, Python, and other Truffle guests without a
  bespoke FFI. The `Foreign(name)` capability boundary in `SchemeV2`
  maps to a Truffle foreign-call boundary.
- **No code generator to maintain.** The cranelift engine carries
  `compile_v2.rs`, `standalone.rs`, and a `helper_v2_*` runtime stub
  set. The Truffle engine carries an interpreter and a value bridge.
- **Handler fidelity.** The Java interpreter is a direct port of
  `V2Interpreter`, including the multi-step `Loop` semantics
  (single-step pair return vs. multi-step `Either` iteration, ADR-0052
  open question #1) and the `{"__pair__": [...]}` /
  `{"__either__": ...}` value shapes. The handler semantics stay
  identical across the Rust and JVM engines; only the host runtime
  differs.

## Decision

Add a new crate `handl-plugin-engine-eval-truffle` that implements
`EvalEngine` by delegating to a Truffle interpreter running on a JVM
host. The crate is the Rust-side bridge; the interpreter itself is
authored in Java (or a JVM language) and loaded by the Rust host via
JNI.

### Architecture

```
Rust host (handl-runtime)
   │
   │  EvalEngine::eval(lang, req)
   ▼
handl-plugin-engine-eval-truffle   (Rust crate, JNI bridge)
   │
   │  JNI: pass serialized Language + Ast + EvalRequest
   ▼
HandlTruffleEngine  (JVM, Truffle interpreter)
   │
   │  interprets SchemeV2 via Truffle nodes
   ▼
GraalVM JIT  (specializes hot paths at runtime)
   │
   │  Foreign(name) → Polyglot invoke
   ▼
JVM / polyglot host
```

### Crate layout

`handl-plugin-engine-eval-truffle` mirrors the structure of
`plugin-engine-eval-cranelift`:

- `src/lib.rs` — `TruffleEngine` implementing `EvalEngine`; `RuntimeConfig`
  wiring (`with_eval("truffle")`).
- `src/bridge.rs` — JNI boundary: serialize `Language` + `Ast` +
  `EvalRequest` to JSON, call into the JVM, deserialize the `Value`
  result. Reuses the same JSON value representation as the other engines
  (`serde_json::Value` with `{"__pair__": [...]}` /
  `{"__either__": ...}` shapes).
- `src/loader.rs` — locates and starts the embedded JVM (via
  `jni` crate), loads the `HandlTruffleEngine` class.
- `jvm/` — the Truffle interpreter source (Java), built into a JAR
  resource via a `build.rs` step that shells out to `javac` + `jar` when
  a JDK is available. The JAR is embedded with `include_bytes!` and
  written to a temp dir at runtime, the same pattern
  `plugin-engine-eval-cranelift` uses for the cranelift-emitted `.o`.

### The Truffle interpreter

A direct port of `crates/plugin-engine-eval-tree-walk/src/v2`:

- One `Node` subclass per `SchemeV2` variant: `ArrNode`, `SeqNode`,
  `FirstNode`, `ChoiceNode`, `LoopNode`, `AppNode`. Each `execute(VirtualFrame)`
  interprets its primitive.
- `ArrNode` dispatches on `op` to a `NativeOp` registry (the JVM
  equivalent of the Rust `native_ops` map) or to a `Foreign` lookup
  (Truffle `Polyglot` invoke).
- `ChoiceNode` uses Truffle's `ConditionProfile` for the left/right
  routing; `LoopNode` uses a `LoopNode`/`RepeatingNode` pair for the
  multi-step `Either` form and a plain block for the single-step pair
  form.
- `FirstNode` unpacks `{"__pair__": [a, b]}`, runs `inner` on `a`,
  re-pairs the result with `b`.
- `AppNode` is dynamic arrow invocation — in Truffle terms, a call to
  another `Node`'s `execute` received as a runtime value.

The value representation is `Object` carrying the same JSON-shaped
structure as the Rust `Value`. Pairs and Eithers use the existing
`{"__pair__": [...]}` / `{"__either__": "left"|"right", "value": v}`
convention so that values cross the JNI boundary as plain JSON without
lossy conversion.

### Capability and `Foreign` dispatch

`SchemeV2`'s `ArrSource::Foreign(name)` is the capability boundary. In
the Truffle engine, a `Foreign(name)` op resolves through:

1. A JVM-side registry of polyglot host functions (Java/Kotlin/Scala
   functions registered as Truffle guest bindings), *or*
2. A callback into the Rust host via JNI, for capabilities the Rust
   host owns (e.g. `fs.read` when the Rust host is the process owner).

The choice is per-`Foreign` name and configured at engine construction
time, mirroring how `plugin-engine-eval-cranelift` resolves `Foreign`
through `ModuleLookup`. The capability audit (ADR-0046) is unchanged —
it inspects `SchemeV2` data, not the engine, so the Truffle engine
inherits the same capability discipline as the other three.

### Versioning

The engine reports `SchemeVersion::v2()`. v1 is removed (ADR-0052
cutover complete); there is no v1 path to support.

### What stays in Rust

Per ADR-0058, engine code changes are acceptable only for new combinator
primitives. This ADR adds **no new kernel primitives** — the Truffle
engine interprets the existing six. The Rust side of the crate is
bridge code only: serialization, JNI, JVM lifecycle. No language-
specific logic lives in the Rust half. The Java side is a handler, not
a new calculus; it interprets the same `SchemeV2` data the other
engines do.

### What is not in this ADR

- **Stock-JVM bytecode emission.** That is [ADR-0062](./0062-jvm-bytecode-emitter-backend.md),
  the sibling engine for stock-JVM deployment. The two coexist.
- **Self-hosting on the JVM.** ADR-0052 Phase D describes a bootstrap
  rewrite in arrow calculus / `proc`. Running that bootstrap on the
  Truffle engine is a consequence of having the engine, not a goal of
  this ADR.
- **Short-circuiting `and`/`or`.** The `and`/`or` ops exist in v2 but
  are strict (evaluate both args — see
  `plugin-engine-eval-tree-walk/src/lib.rs` lines 1155–1156).
  Short-circuiting is a handler extension, not a backend concern; it
  would be added to all engines uniformly via a separate ADR, not
  per-backend. (`match` and `handle` are already implemented in v2.)

## Consequences

- **A fourth `EvalEngine` implementation.** The Truffle engine joins
  tree-walk, bytecode, and cranelift. All four interpret the same
  `SchemeV2` data; conformance is the existing
  `crates/handl-defining-handl/tests/v2_parity.rs` corpus, extended to
  run the Truffle engine alongside the other three.
- **A GraalVM runtime dependency for this engine.** Selecting
  `with_eval("truffle")` requires a GraalVM JDK on the host. The other
  three engines remain GraalVM-free; the Truffle engine is opt-in via
  `RuntimeConfig`, the same mechanism that already selects
  `tree-walk`/`bytecode`/`cranelift`.
- **A JNI boundary in the Rust host.** The `jni` crate becomes a
  workspace dependency. The boundary carries serialized `Language` +
  `Ast` + `EvalRequest`/`Value` as JSON, not raw Rust types, so the
  JVM is never given a raw Rust pointer. The JVM is a guest of the
  Rust process, not a co-equal runtime.
- **Polyglot interop is now a `handl` capability.** A `handl` program
  running on the Truffle engine can call into Java/Kotlin/Scala/etc.
  via `Foreign(name)` resolved through Truffle's `Polyglot` API. This
  is a real new capability surface and should be tracked under the
  capability discipline of ADR-0046 — likely a new capability id such
  as `polyglot.invoke`.
- **The handler-interpretation thesis gets a second host.** ADR-0052
  states that execution is one handler interpretation. Today there is
  one canonical handler (`V2Interpreter`) and two optimizations of it
  (bytecode, cranelift). The Truffle engine is a *second host* for the
  same handler — the same `SchemeV2` walk, running on a different
  runtime, JIT'd by a different mechanism. This is the first real
  exercise of the thesis across a runtime boundary.
- **Build complexity increases.** The crate's `build.rs` shells out to
  `javac` + `jar` when a JDK is present and otherwise produces a
  no-op engine that errors at `eval` time with a clear "GraalVM JDK
  required" message. CI needs a GraalVM-equipped job to run the
  Truffle engine's tests; the existing jobs are unaffected.
- **The cranelift standalone-binary pattern gets a JVM analogue.** The
  `standalone.rs` pattern (embed AST + literal pool as constants, link
  a driver) maps to a Truffle AOT image via GraalVM's `native-image`.
  This is *not* part of the initial implementation but is now
  architectically available: a `handl` program could be AOT-compiled
  to a native binary via Truffle's `native-image`, the JVM analogue of
  cranelift's `cranelift-object` on-disk binary.
- **No changes to `eval.yaml` or the kernel.** The Truffle engine
  interprets existing data; no language constructs are added, removed,
  or rewritten. This is the ADR-0044 §4 contract holding: a new
  engine is a runtime addition, not a language change.
