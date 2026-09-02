# Staged Arrow Engines (SAE): Explicit Structure, Staging, and Interaction Nets for Stackless Spatial Execution

**Author:** Luis Emidio
**Date:** August 2026
**Status:** Theoretical Framework & Proposed Architecture (Revised)

This paper is a companion to the [Arrow-First Language Kernel](./PAPER.md). SAE is not a standalone language; it is one handler interpretation — the spatial-execution handler — for a fragment of that kernel. The kernel defines the calculus (arrows, handlers, first-class languages); SAE defines a particular execution strategy (staging + spatial graph + stackless reduction) for the subset of programs whose structure is statically resolvable.

## Abstract

Modern computing hardware is increasingly spatial, massively parallel, and dataflow-driven (GPUs, TPUs, FPGAs), yet dominant software models remain bound to the von Neumann style of sequential evaluation, lexical environments, and stack frames. Graph-rewriting approaches such as Interaction Nets avoid the stack but typically re-introduce runtime cost through dynamic node duplication.

This paper introduces the Staged Arrow Engine (SAE), a compiler and execution architecture that obtains stackless, zero-allocation spatial execution by keeping computation structure explicit and traversable. Multi-stage programming (Stage 0) resolves dynamic control flow and fan-out into a statically routed process graph; Stage 1 then executes that graph via linear memory buffers or Interaction Net rewrites that contain no runtime duplicators.

We show that the decisive advantage is not a particular calculus per se, but the preservation of explicit structure. Arrow Calculus provides a natural and compositional realisation of this principle (especially for process-oriented spatial routing); equivalent power with the same inspectability can be recovered by freer-style constructions. Opaque monadic bindings destroy the information Stage 0 requires. We formalise the architecture, clarify the hybrid treatment of residual dynamism, address trade-offs, and outline a concrete implementation path.

## 1. Introduction & Background

### 1.1 The Hardware–Software Semantic Gap

Contemporary accelerators favour explicit data routing across large grids of simple cores. In contrast, the λ-calculus tradition maps computation onto dynamic environments and stack frames. Compiling the latter onto the former demands complex escape analysis, aliasing proofs and synchronisation heuristics that frequently fail to yield efficient spatial code.

### 1.2 Limitations of Existing Graph Engines

Interaction Nets (and systems such as HVM) replace the global program counter with local graph rewriting. When they encode ordinary dynamic λ-terms, however, they rely on runtime duplicator and eraser nodes. The resulting cascade of node allocation produces unpredictable memory footprints, high allocator traffic and irregular pointer chasing—precisely the behaviours spatial hardware penalises.

### 1.3 The SAE Hypothesis (Revised)

The essential requirement for efficient spatial execution is that the computation graph remain **explicit and traversable** up to the point at which it is specialised. Once control flow, sharing and effect sets are visible, Stage 0 can eliminate them, emitting a fixed routing diagram. Stage 1 then runs that diagram with no dynamic allocation and no stack.

Arrow Calculus is a convenient vehicle for this style of programming: its combinators already describe process composition and wiring. The same information can be retained by freer constructions; it is lost by opaque monadic binds. Consequently the thesis is not "arrows replace λ-calculus," but:

> Explicit, inspectable computation structure enables staging into a static spatial graph; opacity prevents it.

## 2. Core Architecture & Formal Model

The SAE pipeline comprises two phases:

```
[ Stage 0: Meta-Evaluation ]  →  evaluates static parameters, unrolls, prunes
          ↓
[ Explicit Process Graph G ]  →  statically routed, typed wires, contiguous layout
          ↓
[ Stage 1: Spatial Engine ]   →  linear buffers / stackless Interaction-Net rules
                                 (zero runtime dups, zero heap allocation)
```

### 2.1 Theoretical Foundations

**Explicit structure as the primary resource.**
A computation is represented as a first-class, traversable graph (or free tree) of operations. Stage 0 may inspect, specialise and re-write this graph. After specialisation the graph contains only static routing; dynamic duplicators are absent.

**Arrow Calculus as a practical surface.**
The core combinators

- `arr :: (x → y) → a x y`
- `(>>>) :: a x y → a y z → a x z`
- `first :: a x y → a (x,z) (y,z)`

together with staging operators `⟨e⟩` and `~e`, give a compositional language whose intermediate form is already a wiring diagram. `ArrowApply` recovers the expressive power of monads while, in an explicit presentation, preserving inspectability.

**Interaction Nets as Stage-1 reduction.**
Because fan-out and control decisions have been resolved, the Stage-1 net contains none of the classic runtime `dup`/`era` interaction rules for the specialised fragment.

### 2.2 SAE Intermediate Representation

```
Type T   ::= Primitive | Pair T T
Node V   ::= Arr f | Split | Merge | LoopBack | DispatchTable | DynamicIsland
Wire E   ::= (Source:Port, Target:Port, Type)
Graph G  ::= { inputs, outputs, nodes, memory-map }
```

`DispatchTable` nodes represent finite-branch-space `ArrowApply`: a pre-compiled pool of Stage-1 chunks selected at runtime by zero-allocation pointer switch (see §2.4). `DynamicIsland` nodes are the residual hybrid escape hatch: regions whose topology is not statically resolvable and that may perform limited runtime allocation or ordinary λ-evaluation. The surrounding graph remains static and zero-alloc.

### 2.3 Locating SAE in the Arrow-First / Language-First Kernel

SAE is not a standalone language. It is a backend for an Arrow-First kernel whose central thesis is: *the kernel constructs computations; handlers interpret them* (see [PAPER.md](./PAPER.md)). That kernel makes two things first-class — arrows (computations as values) and languages (the rules for constructing/interpreting computations as values, after Cimini's `λ^𝓛`). SAE occupies one cell in that matrix: it is the **spatial-execution handler** for the kernel's statically-resolvable fragment.

Three consequences follow from locating SAE here, rather than presenting it as a self-contained macro system:

1. **Stage 0 is language-value computation, not a macro system.** Desugaring Paterson `proc` syntax to an adjacency matrix is `lang.parse` → `lang.transform` → `lang.compile` over a first-class `Language` value. The surface syntax, desugaring rules, and optimizer are swappable first-class values, not a fixed compiler. This directly addresses the Stage 0 latency trade-off (§4.2): incremental compilation — caching lowerings of sub-arrows and recompiling only what changed — falls out of treating Stage 0 as a handler over an inspectable language value, rather than being a separate engineering effort.

2. **Backends are parametric arrows, not bespoke emitters.** Each Stage 1 target (Wasm, WGSL, Verilog) is a first-class arrow `Arrow<SAE-IR, Target>`, inspectable, cacheable, and composable. The SAE IR itself is a `Language` value, so its `format`/`visualize`/`debug` operations come from the generic tooling protocol (PAPER.md §4) — the Phase 3 visual tracer is a `debug` handler over the language value, not a per-backend debugger.

3. **The hybrid architecture (§4.1) is handler separation, not an escape hatch.** A standard VM handling HTTP/I/O alongside an SAE core is two handlers over one kernel (PAPER.md §3); a first-class language value routes fragments to the appropriate handler based on whether they fall in the spatial-executable fragment. The hybrid is the predictable consequence of handler-separated interpretation over a first-class language, not a workaround for a limitation.

A further endpoint the kernel enables but SAE does not require: **self-hosted Stage 0.** Because the kernel's own compiler and optimizer are arrows interpreted by the same kernel (PAPER.md §7), Stage 0 can be written in the language it compiles and run via a non-spatial handler, while Stage 1 remains the spatial handler. SAE's standalone framing cannot express this; the kernel framing makes it natural.

### 2.4 The Three-Way Split: Static Transparency vs. Runtime Transparency

`ArrowApply` (`app`) makes the dynamic continuation an explicit first-class arrow value (a traversable data structure), not a hidden function closure as in monadic `bind`. This is a real representational difference. But it does not, by itself, make the continuation statically known: if the arrow value is produced by runtime data, its shape is unknown at compile time and cannot be route-optimized. The transparency is *runtime* (a handler can inspect, sandbox, or rewrite a received arrow before interpreting it), not *Stage-0* (the arrow's shape is not known at compile time).

This distinction yields a three-way split of the kernel (mirroring PAPER.md §9.1), not a two-way "static vs. dynamic" one:

1. **Staged-resolvable fragment** (no `app`, or `app` whose target is fixed at Stage 0) → spatial handler: full Stage 0 route optimization, zero-allocation execution. The Stage-1 net contains no `dup`/`era` rules for this fragment.

2. **Finite-branch-space `app`** (`app` whose target set is provably finite and statically discoverable) → spatial handler with pre-compiled dispatch: Stage 0 pre-compiles every candidate sub-graph into a pre-allocated pool of flat Stage 1 chunks; runtime `app` becomes a zero-allocation pointer switch / indirect dispatch into the pool. This is the analogue of a jump table for arrow-valued data. Proving the branch space finite is the enabling analysis — trivial for first-order programs, potentially requiring abstract interpretation or type-based analysis when arrow values are computed by complex logic. Pre-compiling all branches multiplies the code-bloat trade-off (§4.3); chunk eviction reintroduces dynamic memory management and must be balanced against the zero-allocation goal.

3. **Unbounded-branch-space `app`** (target set not provably finite, or dependent on unbounded runtime data schemas) → dynamic handler: live Stage 0 re-staging (sub-millisecond graph build + compile) or a conventional VM, exploiting the runtime transparency for inspection and sandboxing. These are the `DynamicIsland` nodes of §2.2.

The boundary is *provable-finite-branch-space*, not `app`-presence. This moves a large class of `app` usage — most conditional branching, pattern matching, and finite dispatch — from the dynamic handler into the spatial handler, leaving only genuinely unbounded dynamic structure to the fallback.

| Profile | Pre-compiled dispatch (finite branch space) | Live re-staging (unbounded branch space) |
| --- | --- | --- |
| When | Branch choices known statically or a small predictable set. | Dynamic structure depends on unbounded runtime data schemas. |
| Runtime overhead | ≈ 0 ns (single pointer read / indirect dispatch). | Sub-millisecond (Stage 0 graph build + compile). |
| Memory | Pre-allocated slab with proactive chunk recycling. | Temporary graph workspace with immediate discard. |

## 3. Comparative Evaluation

| Axis | Ordinary λ / Opaque Monad | Pure Interaction Nets (dynamic) | SAE (explicit + staged) |
| --- | --- | --- | --- |
| Structure | Buried in binds | Explicit but runtime-mutable | Explicit and frozen after Stage 0 |
| Dynamic control | Runtime stack / jumps | Runtime dup/era | Resolved in Stage 0 (tiers 1–2); isolated islands or fallback (tier 3) |
| Memory | Stack + heap | Dynamic node heap | Pre-allocated contiguous buffers (tiers 1–2); bounded workspace (tier 3) |
| Spatial mapping | Complex analysis | Possible but bloated | Near 1:1 routing (tiers 1–2) |
| Residual dynamism | Everywhere | Everywhere | Isolated islands or fallback (tier 3 only) |
| `ArrowApply` handling | N/A (opaque bind) | N/A | Tier 2: pre-compiled dispatch table; tier 3: dynamic island |

## 4. Architectural Trade-offs and Mitigations

Three residual problems remain once the core explicit-structure claim is granted.

### 4.1 Highly data-dependent control flow

Fragments whose topology cannot be resolved at Stage 0 stay dynamic. Under the three-way split (§2.4), this is tier 3 only — `app` whose target set is not provably finite. Mitigation: handler separation (PAPER.md §3) — profile-guided or programmer-annotated stable cores are staged (tiers 1–2); the remainder runs on an ordinary λ or Interaction-Net engine under a separate dynamic handler, with cheap buffer hand-off. The hybrid is the predictable consequence of handler-separated interpretation over a first-class language, not a workaround for a limitation.

### 4.2 Stage 0 compilation latency

Graph construction is a compilation step. Mitigation: memoisation of specialised graphs, incremental re-staging of only the changed subgraphs (a direct consequence of treating Stage 0 as `lang.compile` over an inspectable first-class `Language` value, PAPER.md §4/§7), and background compilation for hot paths. The win appears when the same graph is executed many times.

### 4.3 Code-size growth from specialisation

Unrolling increases the static graph, and finite-branch-space pre-compilation (tier 2) multiplies it further — one chunk per branch target. Mitigation: common-subgraph sharing, bounded unrolling, compression of the routing tables, and proactive chunk eviction with re-compilation on demand. The engineering bet is that the elimination of pointer-chasing and allocator traffic outweighs the larger instruction footprint for the target workloads (streaming, DSP, inference kernels, etc.). Note that chunk eviction reintroduces dynamic memory management and must be balanced against the zero-allocation goal of tiers 1–2.

## 5. Implementation Roadmap

### Phase 1 – Reference interpreter

Formal operational semantics of the explicit IR; a host-language prototype that materialises the graph into a single contiguous buffer and executes it without allocation. Stage 0 is implemented as operations on a first-class `Language` value (PAPER.md §4): `lang.parse` ingests Paterson-style `proc` syntax, `lang.transform` desugars it, `lang.compile` emits an explicit Adjacency Matrix. The surface syntax, desugaring rules, and optimizer are first-class values, enabling incremental compilation (cache and recompile only changed sub-arrows) as a direct consequence.

### Phase 2 – WebAssembly / WebGPU backend

Emit Wasm for CPU and WGSL compute shaders for GPU. Each Stage 1 target is implemented as a first-class parametric arrow `Arrow<SAE-IR, Target>` (PAPER.md §4): one emitting WebAssembly for CPU execution, one emitting WGSL for GPU compute. Backends are inspectable, cacheable, and composable as first-class values, not bespoke emitters. Demonstrate lock-free ring-buffer pipelines with zero dynamic allocation on representative DSP and matrix kernels.

### Phase 3 – Tooling and spatial synthesis

Visual wire-level tracer that maps Stage-1 execution back to Stage-0 source; experimental Verilog/VHDL emission for FPGA netlists. The tracer is an instance of the kernel's generic `debug`/`visualize` tooling protocol (PAPER.md §4/§7) — a handler over the language value — rather than a bespoke per-backend debugger; conforming tools operate on the protocol, not on SAE's internal graph AST.

## 6. Conclusion

The Staged Arrow Engine rests on a simple observation: spatial hardware rewards computation whose routing is known in advance. The practical way to obtain that knowledge is to keep the computation structure explicit and traversable so that a staging phase can freeze it. Arrow Calculus supplies a pleasant, compositional syntax for such structure; freer-style representations demonstrate that the same information can be recovered by other means. Opaque bindings erase it.

Once structure is explicit, Stage 0 can eliminate runtime duplication and stack frames, Stage 1 becomes a pure spatial pipeline, and residual dynamism can be confined to well-delimited islands. The resulting architecture offers a realistic path from high-level compositional code to efficient, predictable execution on GPUs, FPGAs and future dataflow silicon.

The decisive distinction is not arrows versus monads, but explicit versus opaque. Within the Arrow-First Kernel, SAE is the spatial-execution handler for the statically-resolvable fragment — one interpretation among several the kernel permits, and one whose coverage extends through finite-branch-space `app` via pre-compiled dispatch, leaving only genuinely unbounded dynamism to a separate handler.
