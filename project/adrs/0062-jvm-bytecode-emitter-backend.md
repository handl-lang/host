# ADR-0062: JVM Bytecode Emitter Backend

## Status

Proposed

Adds a fifth eval engine alongside `plugin-engine-eval-tree-walk`,
`plugin-engine-eval-bytecode`, `plugin-engine-eval-cranelift`, and the
Truffle engine of [ADR-0061 — Truffle
Backend](./0061-truffle-backend.md). Targets the arrow-calculus
`SchemeV2` kernel from [ADR-0052 — Arrow Calculus
Kernel](./0052-arrow-calculus-kernel.md) (v1 cutover complete; v2 is the
sole default). This ADR and ADR-0061 are **sibling ADRs**: the two
backends are not mutually exclusive and are intended to coexist.

## Context

ADR-0061 picked the Truffle interpreter path for a JVM target, trading a
GraalVM runtime dependency for a smaller implementation surface and
polyglot interop. That leaves a gap: deployment contexts where a
GraalVM JDK is unavailable but a stock JVM is. A stock-JVM backend is
the peer of `plugin-engine-eval-cranelift` for the JVM — a code
generator that lowers `SchemeV2` to `.class` files, runnable on any JDK
without a special runtime.

ADR-0061 explicitly left this path open ("a future
`plugin-engine-eval-jvm` crate that emits `.class` files directly is
explicitly _not_ rejected by this ADR"). This ADR picks it up.

### Why both backends

The two JVM backends serve different deployment profiles:

|                  | Truffle (ADR-0061)                      | Bytecode emitter (this ADR)                     |
| ---------------- | --------------------------------------- | ----------------------------------------------- |
| Runtime          | GraalVM JDK                             | Stock JVM                                       |
| What you write   | Interpreter (port of `V2Interpreter`)   | Code generator (`SchemeV2` → `.class`)          |
| JIT              | GraalVM partial evaluation              | Stock JVM JIT on emitted bytecode               |
| Polyglot interop | Free via Truffle `Polyglot` API         | Manual via JNI/reflection                       |
| Fit with thesis  | Handler interpretation on a second host | Compilation, like cranelift                     |
| AOT to native    | `native-image` (natural)                | `native-image` on emitted classes (awkward)     |
| Maintenance cost | Interpreter + JNI bridge                | Bytecode generator + constant pool + stack maps |

Neither subsumes the other. The Truffle engine is the better fit for
interactive/polyglot workloads on GraalVM; the bytecode emitter is the
better fit for ahead-of-time compilation and stock-JVM deployment. Both
interpret the same `SchemeV2` data and produce the same results —
conformance is the existing `v2_parity.rs` corpus, extended to five
engines.

### Relationship to the cranelift engine

The bytecode emitter is structurally a peer of
`plugin-engine-eval-cranelift`:

| cranelift                                           | JVM bytecode emitter                                        |
| --------------------------------------------------- | ----------------------------------------------------------- |
| `compile_v2.rs` — `SchemeV2` → cranelift IR         | `compile_v2.rs` — `SchemeV2` → JVM bytecode                 |
| `standalone.rs` — emit `.o` + C driver + static lib | `standalone.rs` — emit `.class`/`.jar` + embedded constants |
| `helper_v2_*` runtime stubs                         | JVM runtime helpers (static methods)                        |
| `cranelift-object` for on-disk binaries             | `bytecode`/`classfile` crate for `.class` emission          |

The lowering table is the same six-primitive mapping; only the target IR
differs.

## Decision

Add a new crate `handl-plugin-engine-eval-jvm` that implements
`EvalEngine` by compiling `SchemeV2` to JVM bytecode at eval time and
executing it on an in-process JVM, with an optional ahead-of-time mode
that emits `.class`/`.jar` files to disk.

### Architecture

```
Rust host (handl-runtime)
   │
   │  EvalEngine::eval(lang, req)
   ▼
handl-plugin-engine-eval-jvm   (Rust crate)
   │
   │  compile_v2: SchemeV2 → JVM bytecode (in-memory or on-disk)
   ▼
JVM classloader  (loads generated classes)
   │
   │  invokes compiled method: fn(Value) → Value
   ▼
Stock JVM JIT  (optimizes emitted bytecode)
   │
   │  Foreign(name) → JNI callback to Rust host
   ▼
Rust host (capability dispatch)
```

### Crate layout

Mirrors `plugin-engine-eval-cranelift`:

- `src/lib.rs` — `JvmBytecodeEngine` implementing `EvalEngine`;
  `RuntimeConfig` wiring (`with_eval("jvm")`).
- `src/compile_v2.rs` — `SchemeV2` → JVM bytecode. One method per
  `SchemeV2` variant, emitted into a generated class. Uses a JVM
  bytecode library (candidate: the `bytecode` or `classfile` crate, or
  direct `.class` file emission per the JVMS). The six primitives map
  as:
  - `Seq` → straight-line bytecode (output of `fst` feeds `snd` via a
    local variable slot).
  - `Choice` → `if`/`goto` with a merge frame, mirroring cranelift's
    `brif`/`jump` + merge block.
  - `Loop` → back-edge + 3-way exit (left/right/pair), mirroring
    cranelift's native back-edge blocks with stack-slot iteration
    counter.
  - `First` → unpair `{"__pair__": [a, b]}`, run `inner` on `a`, re-pair
    result with `b`. No sub-program needed — straight-line bytecode.
  - `Arr` → invoke the registered native op or foreign function.
  - `App` → dynamic invocation of a compiled arrow method received as a
    runtime value.
- `src/standalone.rs` — ahead-of-time mode: emit a `.jar` containing the
  compiled classes plus the serialized `Ast`/literal pool/name pool as
  embedded resource constants, plus a `main` class that deserializes
  and runs. The JVM analogue of cranelift's `standalone.rs` (`.o` + C
  driver + static lib).
- `src/runtime.rs` — the JVM-side runtime helpers (static methods) that
  the emitted bytecode calls: value boxing/unboxing, pair/either
  construction, env projection, foreign dispatch. The JVM equivalent of
  cranelift's `helper_v2_*` functions.
- `src/bridge.rs` — JNI lifecycle: start the JVM (via the `jni` crate),
  load generated classes, marshal `Value` across the boundary as JSON.

### Value representation

Same JSON-shaped `serde_json::Value` with `{"__pair__": [...]}` /
`{"__either__": "left"|"right", "value": v}` as the other engines. The
emitted bytecode operates on `Object` references carrying this
structure; the JNI boundary marshals `Value` as JSON strings, identical
to the Truffle engine's bridge. No lossy conversion.

### Capability and `Foreign` dispatch

`ArrSource::Foreign(name)` resolves through a JNI callback into the Rust
host's `ModuleLookup`, the same mechanism the cranelift engine uses.
The capability audit (ADR-0046) inspects `SchemeV2` data, not the
engine, so the bytecode emitter inherits the same capability discipline
as the other engines. No polyglot interop is provided by this engine —
that is the Truffle engine's differentiator. A `handl` program on the
bytecode emitter can only call back into the Rust host, not into
arbitrary JVM guests.

### Versioning

Reports `SchemeVersion::v2()`. v1 is removed (ADR-0052 cutover
complete); there is no v1 path to support. This corrects a stale note
in earlier drafts that referenced v1 cutover as pending.

### Ahead-of-time vs JIT modes

Two modes, selected at engine construction time:

1. **JIT (default):** compile `SchemeV2` to bytecode in memory, load via
   an in-process JVM classloader, execute. The JVM's own JIT optimizes
   the emitted bytecode. Analogous to cranelift's JIT mode.
2. **AOT:** emit `.class`/`.jar` files to disk via `standalone.rs`,
   runnable later on any stock JVM with the runtime helper jar on the
   classpath. Analogous to cranelift's `cranelift-object` on-disk binary
   mode.

### What stays in Rust

Per ADR-0058, engine code changes are acceptable only for new combinator
primitives. This ADR adds **no new kernel primitives** — the bytecode
emitter compiles the existing six. The Rust side is the code generator
and JNI bridge; no language-specific logic lives in the engine. The
emitted bytecode is a compilation of `SchemeV2` data, not a new
calculus.

### What is not in this ADR

- **Polyglot interop.** That is the Truffle engine's concern
  (ADR-0061). The bytecode emitter resolves `Foreign` only through the
  Rust host. If polyglot interop is needed, use the Truffle engine.
- **Short-circuiting `and`/`or`.** The `and`/`or` ops exist in v2 but
  are strict (evaluate both args — see
  `plugin-engine-eval-tree-walk/src/lib.rs` lines 1155–1156).
  Short-circuiting is a handler extension, not a backend concern; it
  would be added to all engines uniformly via a separate ADR, not
  per-backend.
- **`native-image` AOT.** GraalVM's `native-image` can AOT-compile the
  emitted `.class` files to a native binary, but that crosses into the
  Truffle engine's natural territory. This ADR's AOT mode targets
  stock-JVM `.jar` deployment.

## Consequences

- **A fifth `EvalEngine` implementation.** The bytecode emitter joins
  tree-walk, bytecode, cranelift, and Truffle. All five interpret the
  same `SchemeV2` data; conformance is the `v2_parity.rs` corpus
  extended to five engines.
- **Stock-JVM deployment is now possible.** Selecting
  `with_eval("jvm")` requires only a stock JDK, not GraalVM. This
  covers deployment contexts the Truffle engine cannot.
- **A JVM bytecode generator to maintain.** This is the real cost:
  constant pool management, method descriptors, stack-map frames, local
  variable slots, `Choice`→`goto`/merge, `Loop`→back-edge. It is a
  peer of `plugin-engine-eval-cranelift`'s `compile_v2.rs` in scope,
  targeting JVM bytecode instead of cranelift IR.
- **A `jni` workspace dependency** (shared with the Truffle engine —
  ADR-0061). The boundary marshals `Value` as JSON, not raw Rust
  pointers; the JVM is a guest of the Rust process.
- **No polyglot interop from this engine.** `Foreign` resolves only to
  the Rust host. Programs needing JVM-guest interop use the Truffle
  engine.
- **Two JVM backends, one conformance bar.** The `v2_parity.rs` corpus
  already runs all cases through every engine; adding two JVM engines
  doubles the JVM-side conformance surface but does not change the
  kernel or `eval.yaml`. A divergence between the two JVM engines is a
  bug in one of them, not a language change.
- **No changes to `eval.yaml` or the kernel.** Same as ADR-0061: a new
  engine is a runtime addition, not a language change (ADR-0044 §4).
- **The cranelift `standalone.rs` pattern gets a second analogue.**
  cranelift emits `.o` + C driver; the bytecode emitter emits `.jar` +
  `main` class. The two AOT-on-disk patterns are structurally parallel.
