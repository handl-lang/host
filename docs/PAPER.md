# Arrow-First Language Kernel

## Abstract

This paper proposes a programming language whose fundamental computational
abstraction is the **arrow**, rather than the function, and whose language
definition is itself a **first-class value**.

An arrow `A ~> B` is a first-class description of a computation from `A` to
`B`. Computations are constructed compositionally using an Arrow Calculus;
their meaning is supplied separately by **handlers**. The kernel is six
primitives — `arr`, `>>>`, `first`, `choice`, `loop`, `app` — the first
three forming the minimal Arrow structure, the latter three providing
branching, feedback, and higher-order computation.

Two principles organize the design:

> **The kernel constructs computations; handlers interpret them.**

Execution is therefore one possible interpretation of a program, not a
primitive property of it. The same arrow may be compiled, analyzed,
optimized, tested, or otherwise interpreted.

> **If programs are first-class arrows, then the language that defines those
> programs should itself be a first-class value.**

Making the language first-class has the same effect that making
functions/arrows first-class had: capabilities that previously lived
outside the language — lexing, parsing, typechecking, formatting, compiling,
debugging — become ordinary values that can be composed, passed around,
transformed, and interpreted.

---

## 1. Arrow as the Fundamental Computational Value

The fundamental computational type is:

```text
A ~> B
```

rather than:

```text
A => B
```

An arrow is not primarily a function to be called. It is a compositional
computational object.

For example:

```text
parse
>>> validate
>>> compile
```

constructs a larger arrow. Composition does not imply execution.

---

## 2. The Kernel

### `arr`

Introduces primitive/pure computation into the arrow calculus.

```text
arr f : A ~> B
```

### `>>>`

Sequentially composes arrows:

```text
a : A ~> B
b : B ~> C

a >>> b : A ~> C
```

It is the fundamental flow operation.

### `first`

Lifts an arrow over product structure:

```text
f : A ~> B

first f : (A, X) ~> (B, X)
```

This provides the basic mechanism for structured data flow.

### `choice`

Provides branching over alternatives:

```text
f : A ~> C
g : B ~> C

choice f g : A + B ~> C
```

This supports conditionals, pattern matching, error paths, and other
branching structures.

### `loop`

Provides feedback:

```text
f : (A, S) ~> (B, S)

loop f : A ~> B
```

The `S` channel is fed back into the computation. The fundamental concept
is **feedback**, not `while`. Iteration and state machines can be derived
from feedback.

### `app`

Provides dynamic/higher-order arrow application (`ArrowApply` in the Haskell
hierarchy). It allows an arrow to receive another arrow as data and invoke it
during computation. This is distinct from interpreting an entire arrow
externally — `app` is a calculus operation _within_ a computation; a handler
interprets the computation _from outside_.

`app` is not part of the minimal Arrow calculus (Lindley–Wadler–Yallop); it
is an extension. We include it because the language requires higher-order
arrows — arrows that can dynamically invoke other arrows received as runtime
data. Whether this is necessary is a language-design choice, not something
established by the Arrow literature.

---

## 3. Construction vs Interpretation

The kernel constructs arrow programs:

```text
program =
    parse
    >>> validate
    >>> compile
```

A handler gives the program an interpretation:

```text
handle Runtime program
handle Compiler program
handle Analyzer program
```

Thus:

```text
arrow       = computational description
handler     = interpretation
```

Execution is not a primitive property of arrows. A runtime handler performs
I/O; a test handler provides deterministic substitutes; a compiler handler
produces machine code; an analyzer inspects effects without executing them.
The computational description remains unchanged.

### Effects and handlers

Effectful primitives are operations in the arrow computation. A handler
determines their semantics. We adopt the handler model of Sanada's
_Algebraic Effects and Handlers for Arrows_ as the semantic foundation: an
operation handler receives the operation's argument and its continuation,
defining how the operation is interpreted and how computation resumes.
Sanada provides operational semantics (proved sound and adequate) and a
denotational semantics in which handlers are homomorphisms between algebras
of arrows.

```text
Arrow       = what computation is described
Operation   = what effect is requested
Handler     = how that effect is interpreted
```

---

## 4. First-Class Programs and Languages

### Programs are values

Because arrows are values, programs can themselves be transformed:

```text
optimize : Program ~> Program
analyze  : Program ~> Analysis
compile  : Program ~> MachineCode
```

This makes compiler passes, optimizers, analyzers, and language tooling
ordinary computations over computational objects. The distinction between
program, compiler, optimizer, and interpreter becomes less fundamental —
they are all computations that can themselves be represented and
manipulated.

### Languages are values

If programs are first-class arrows, then the language that defines those
programs should itself be a first-class value.

Making functions first-class turned functions from a compiler-internal
concept into values that could be passed, stored, composed, and returned.
Making arrows first-class turned computations from an execution-time
concept into values that can be inspected, transformed, and interpreted.
Making the **language** first-class turns the language itself — its lexer,
parser, typechecker, formatter, compiler, debugger — from external
infrastructure into values that can be composed, passed around, transformed,
and interpreted.

A language is a first-class value:

```text
Language
   │
   ├── lex
   ├── parse
   ├── evaluate
   ├── format
   ├── lint
   ├── typecheck
   ├── proofcheck
   ├── debug
   ├── profile
   ├── compile
   ├── optimize
   ├── transform
   └── visualize
```

These are not bespoke features of a host language. They are **generic
tooling protocols** that any language implementing the protocol can
participate in. A typechecker is not a privileged compiler pass — it is a
handler over a language value. A new language is not a new toolchain — it
is a new value of type `Language`, and every conforming tool works on it
immediately.

### The hierarchy

```text
values
   ↓
arrows              ← first-class computations
   ↓
programs            ← arrows describing programs
   ↓
languages           ← first-class values describing how programs work
   ↓
generic tooling     ← operates on any conforming language
```

Each level is a first-class value at the level below it. Arrows are values.
Programs are arrows. Languages are values describing how programs work.
Generic tooling operates on any conforming language.

This is the route to a self-hosting, reflective computational environment
where the distinction between program, compiler, DSL, and language
implementation is deliberately minimized — not by fiat, but because they
are all the same kind of thing: first-class values interpreted by handlers.

### Consequences

Four consequences follow from making both arrows and languages first-class:

- **Language becomes first-class.** `currentLanguage` is accessible inside
  programs; languages can be inspected, composed, extended, and used to
  lex/parse/evaluate themselves.
- **Tooling becomes generic.** Formatter, linter, typechecker, proofchecker,
  debugger, profiler, compiler, optimizer, transformer, visualizer, etc.
  operate through a common `Language` protocol — not as per-language
  infrastructure.
- **Functions become derived.** `A → B` is the pure/simplest case of
  `A ~> B`. Hughes shows that ordinary functions form an Arrow instance
  (`arr f = f`, with composition and `first` defined naturally), so one
  uniform model covers pure functions, effectful arrows, stateful arrows,
  branching arrows, and higher-order arrows without two foundational
  worlds needing bridges between them. This is a **language-design choice
  inspired by the Arrow theory**: the literature supports that functions
  _can_ be an Arrow instance, not that they _should_ be eliminated as a
  separate type. The Arrow Calculus itself (Lindley–Wadler–Yallop) is
  formulated by extending simply typed lambda calculus with arrow
  constructs. Our language chooses to make arrows primary and treat
  functions as the degenerate pure case.
- **Parametric arrows** become the abstraction for computations over
  computations:

  ```text
  Arrow<A, B> → Arrow<C, D>
  ```

  An optimizer, a compiler, an analyzer, or a language transformer is an
  arrow whose input and output are themselves arrows. This is the
  meta-level of the calculus — and it is not a separate meta-language; it
  is the same calculus, applied to arrow-valued data.

---

## 5. A Sketch of the Language

### Programs are data

A primitive arrow:

```text
add : (Int, Int) ~> Int
```

Arrow composition:

```text
double : Int ~> Int =
    x => x + x

program : Int ~> Int =
    double
    >>> add(10)
```

The important part is that **`program` is data**:

```text
program
// Arrow<Int, Int>
```

You can transform it:

```text
optimized = optimize(program)
formatted = format(program)
graph     = visualize(program)
```

### The language is also a value

```text
lang : Language
```

You can access its capabilities:

```text
tokens = lang.lex(source)
ast    = lang.parse(tokens)
result = lang.evaluate(ast)
pretty = lang.format(ast)
errors = lang.lint(ast)
types  = lang.typecheck(ast)
trace  = lang.debug(ast)
```

### Manipulating the language itself

You can manipulate the language itself:

```text
newLang =
    lang
    >>> addSyntax(...)
    >>> addPrimitive(...)
    >>> addType(...)
```

and then use the resulting language:

```text
ast    = newLang.parse(source)
result = newLang.evaluate(ast)
```

### The self-hosting loop

The language can be available **from inside programs written in that
language**:

```text
language = currentLanguage

ast  = language.parse(source)
ast2 = transform(ast)
language.evaluate(ast2)
```

### The full architecture

```text
                 Language
                    │
       ┌────────────┼────────────┐
       │            │            │
      lex          parse       evaluate
       │            │            │
       └────────────┼────────────┘
                    │
                   AST
                    │
       ┌────────────┼────────────┐
       │            │            │
    transform    optimize      format
       │            │            │
       └────────────┼────────────┘
                    │
                 Program
                    │
                  Arrow
                    │
                 handler
                    │
       ┌────────────┼────────────┐
       │            │            │
     runtime      compiler     analyzer
```

And **all of those are themselves values/computations**.

That is the distinctive part: not merely _first-class ASTs_, but
**first-class languages whose generic protocol exposes lexing, parsing,
evaluation, and tooling**.

The language is therefore not just the thing that runs the program.

**The language is another object the program can compute with.**

---

## 6. Lineage

This proposal does not invent a calculus; it adopts two mature lineages and
combines them.

| Work                                                                                                       | Contribution                                                                                                                                                                                  | Relevance                            |
| ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| **Moggi** — _Notions of Computation and Monads_ (1991)                                                     | Computational effects via monads; the monadic metalanguage. The precursor that Hughes generalizes.                                                                                            | Computational effects foundation     |
| **Hughes (2000)** — _Generalising Monads to Arrows_                                                        | Introduced **arrows** as a generalization of monads; the minimal `arr`, `>>>`, `first`.                                                                                                       | Computational abstraction            |
| **Paterson (2001)** — _A New Notation for Arrows_                                                          | Added practical **arrow notation** (`proc`), making point-free arrow programs readable and compositional; introduced **`loop`** (feedback) and `returnA`.                                     | Surface syntax + feedback            |
| **Lindley, Wadler, Yallop (2008/2010)** — _The Arrow Calculus_                                             | Made arrows into a genuine **Arrow Calculus**: a typed metalanguage for manipulating Hughes's arrows. Four constructs, five laws, proved complete.                                            | Calculus / language foundation       |
| **Lindley, Wadler, Yallop (2011)** — _Idioms are Oblivious, Arrows are Meticulous, Monads are Promiscuous_ | Placed arrows in the landscape of monads/idioms/arrows; showed classic arrows and the Arrow Calculus are equationally corresponding.                                                          | Algebraic grounding                  |
| **Lindley (2014)** — _Algebraic Effects and Effect Handlers for Idioms and Arrows_                         | Generalized algebraic effects and handlers to idioms and arrows; established that handlers aren't inherently tied to monadic computation.                                                     | Effects + handlers for arrows        |
| **Sanada (2024)** — _Algebraic Effects and Handlers for Arrows_                                            | Formalized **Arrow Calculus with operations and handlers**, with operational + denotational semantics; handlers correspond to homomorphisms of arrow algebras; proves soundness and adequacy. | Semantic foundation for our handlers |
| **Cimini (2018/2022)** — _A Calculus for Multi-language Operational Semantics_                             | Presents `λ^𝓛`, a typed λ-calculus where **languages are first-class citizens**: bound to variables, passed to functions, returned from functions, used to execute programs.                  | Languages as first-class values      |

```text
Moggi
  │
  │ computational effects / monadic metalanguage
  ▼
Hughes (2000)
  │
  │ arrows
  ▼
Paterson (2001)
  │
  │ arrow notation + feedback
  ▼
Lindley / Wadler / Yallop (2008–10)
  │
  │ Arrow Calculus
  ▼
Lindley / Wadler / Yallop (2011)
  │
  │ arrows in the monad/applicative/arrow landscape
  ▼
Lindley (2014)
  │
  │ effects + handlers for arrows
  ▼
Sanada (2024)
  │
  │ formal operational + denotational
  │ semantics for arrow handlers
  ▼
Cimini (2018/2022)
  │
  │ languages as first-class values
  ▼
Our language
  │
  │ Arrow-first + Language-first
  ▼
```

Our key step combines two lineages:

```text
Arrow lineage:                    Language-as-value lineage:

Hughes → Paterson → LWY           Cimini
  → Lindley → Sanada                languages as
  Arrow Calculus                    first-class values
  + handlers                        + operational semantics
       │                                 │
       └──────────────┬──────────────────┘
                      ▼
               Our language
               Arrow-first + Language-first
```

The precise formulation of our contribution:

> **Cimini makes languages first-class inside a general-purpose language.
> We make the Arrow Calculus the computational substrate of such a language,
> while treating the language itself as a first-class computational value.**

The three first-class transformations are symmetrical:

```text
Function → first-class
    ↓
computation becomes manipulable data

Arrow → first-class
    ↓
computation descriptions become manipulable data

Language → first-class
    ↓
the rules for constructing/interpreting
those computations become manipulable data
```

That last step is what makes the architecture substantially more powerful
than merely having a reflective AST API.

The important difference is that **we aren't embedding arrows inside a
conventional functional language**. We're making the Arrow Calculus itself
the computational substrate, with Sanada's handler semantics providing the
foundation for interpretation — and making the language itself a
first-class value (Cimini's insight) whose tooling operations are
themselves arrows.

---

## 7. Use Cases

The architectural properties of an Arrow Calculus kernel — programs as
first-class descriptions, interpretation delegated to handlers, languages as
first-class values, no serialization tax, structural capability tracking —
make it well-suited to several concrete use cases that are awkward in
conventional lambda-first languages.

### No serialization tax

In a conventional language, a program is an in-memory AST or closure graph.
To send it across a process boundary, store it, or hand it to a different
runtime, you serialize it (to JSON, protobuf, bytecode) and deserialize it
on the other side. This is a tax: every cross-boundary interaction pays a
formatting and parsing cost, and the deserialized form is a _different
object_ from the original — it has lost structure, identity, or both.

In an Arrow Calculus language, the arrow _is_ data. The kernel is six
serializable primitives; an arrow is a first-class description that can be
stored, transmitted, and loaded without a separate serialization layer. The
same value that the runtime interprets is the value that crosses the
boundary.

This matters for:

- **Web performance.** A server can send an arrow (a computation) to a
  client, and the client interprets it directly — no JSON parse, no AST
  rebuild, no separate "client runtime" that reconstructs the program. The
  arrow is the wire format. This eliminates a round of serialization on
  every server-to-client computation transfer, which is the dominant cost
  in data-heavy web applications.
- **Game modding performance.** A game engine can load a mod as an arrow
  description and interpret it directly, without compiling the mod to a
  separate bytecode and without a per-mod AST-to-bytecode pipeline. Mods
  load fast; the engine's handler interprets them; the same arrow can be
  inspected for capability/safety before interpretation.
- **Distributed computation.** A computation can migrate between nodes
  without a marshalling layer. The arrow is the portable form; each node's
  handler interprets it locally.

### Sandboxing

Because an arrow is a first-class description with structural capability
tracking (foreign operations are tagged, not a string convention), a host
can inspect an arrow _before_ interpreting it and decide whether to allow
it. Sandboxing is interpretation-time policy, not a property the program
must opt into.

This matters for:

- **Untrusted scripts.** A host receives an arrow from an untrusted source,
  inspects its operations (does it call `fs.read`? `net.connect`?
  `process.exit`?), and either rejects it, rewrites it, or interprets it
  under a restricted handler that refuses unauthorized operations. The
  program cannot bypass the sandbox because the sandbox _is_ the handler —
  there is no "escape" primitive in the kernel for the program to exploit.
- **Untrusted game mods.** A game engine loads a community mod as an arrow,
  inspects it for forbidden operations, and interprets it under a handler
  that enforces the policy. The mod author writes in the same language as
  the engine; the sandbox is a handler choice, not a separate "safe subset"
  of the language.
- **Plugin systems.** A host application accepts third-party plugins as
  arrows, inspects them for required capabilities, and interprets them
  under a handler that grants exactly those capabilities and no more.

### First-class languages and DSLs

Because a language is a first-class value that implements tooling protocols,
a host can define, extend, compose, and interpret domain-specific languages
without a separate meta-language or compiler infrastructure.

This matters for:

- **Embedded DSLs.** A DSL is a language value — a set of primitives plus
  the six combinators, plus implementations of the tooling protocols. Every
  conforming tool (formatter, linter, debugger) works on it immediately,
  because the tool operates on the protocol, not on a language-specific AST.
- **Programmable tools.** A linter, formatter, or typechecker is a handler
  over a language value. Tools are not external infrastructure; they are
  interpretations of the tooling protocols a language implements. A spatial
  backend's runtime tracer (e.g., mapping Stage 1 wire execution back to
  Stage 0 source blocks in the Staged Arrow Engine) is the same kind of
  object: a `debug`/`visualize` handler over the language value, not a
  bespoke debugger built per-backend.
- **Self-hosting.** The language's own compiler and optimizer are arrows,
  interpreted by the same kernel. The language's own tooling is a language
  value. This is the route to a self-hosting, reflective computational
  environment without a separate meta-language.

### Multi-target compilation

Because compilation is a handler interpretation, the same arrow description
can be lowered to multiple targets without changing the source program.

This matters for:

- **Cross-platform deployment.** One arrow, multiple backends (native,
  WASM, JavaScript, bytecode, and — per the Staged Arrow Engine proposal —
  WebGPU/WGSL compute shaders and FPGA netlists via static spatial graph
  lowering). The source is target-independent; the handler chooses the
  target.
- **Incremental compilation.** A compiler handler can cache lowerings of
  sub-arrows and recompile only what changed, because the arrow structure
  is inspectable and referentially stable.

---

## 8. Arrow Calculus vs Lambda Calculus

| Property               | Lambda Calculus                 | Arrow Calculus                                 |
| ---------------------- | ------------------------------- | ---------------------------------------------- |
| Primitive computation  | Function                        | Arrow                                          |
| Core operation         | Application                     | Composition                                    |
| Primary abstraction    | Lambda                          | Computational construction                     |
| Data flow              | Implicit                        | Explicit                                       |
| Program representation | Usually external                | Native first-class value                       |
| Interpretation         | Usually execution               | Explicit handler                               |
| Transformation         | Meta-level / common tooling     | Language-level operation                       |
| Compiler               | External infrastructure         | Possible arrow / handler                       |
| DSL representation     | Often AST / library             | Native computational object                    |
| Effects                | Usually layered                 | Intrinsic to arrows; handler-interpreted       |
| Recursion              | Primitive (`fix`, Y-combinator) | Derived from feedback (`loop`)                 |
| Language definition    | External / fixed                | First-class value                              |
| Tooling                | Per-language / bespoke          | Generic protocols over any conforming language |

The point is not that one calculus is universally superior. They optimize
for different conceptual centers: Lambda Calculus centers **abstraction and
substitution**; Arrow Calculus centers **construction and interpretation**.

Lambda-like functionality does not disappear — it becomes a derived
construct. Parameterized computations may still be expressed with syntax
resembling `x ~> x + 1`, but the semantic object produced is `Int ~> Int`,
an arrow, rather than a primitive function closure. Higher-order behavior
is expressed via `app`:

```text
app : ((A ~> B), A) ~> B
```

This is a language-design choice, not a claim established by the Arrow
literature. The Arrow Calculus (Lindley–Wadler–Yallop) is itself formulated
by extending simply typed lambda calculus with arrow constructs; the
literature shows that functions _can_ be an Arrow instance (Hughes), not
that a language _should_ eliminate functions as a separate type. Our
language chooses to make arrows the primary computational abstraction,
with pure functions represented as the degenerate/pure case.

---

## 9. Open Design Questions

### 9.1 Are arrows opaque or inspectable?

If arrows are fully inspectable, they resemble a typed intermediate
representation. If they are opaque, handlers must provide all meaningful
semantics. A hybrid model is likely preferable: the six combinators are
inspectable; `app` boundaries are **structurally transparent but not
statically known**.

`ArrowApply` (`app`) makes the dynamic continuation an explicit first-class
arrow value (a traversable data structure), not a hidden function closure as
in monadic `bind`. This is a real representational difference: a handler can
inspect, sandbox, or rewrite a received arrow _at runtime_ before
interpreting it. It is not, however, static transparency: if the arrow value
is produced by runtime data, its shape is unknown at compile time and cannot
be route-optimized. Note also that the opacity asymmetry is a representation
choice, not inherent to either calculus — a free monad `Free f a` is fully
inspectable data, and an arrow can be opaque if implemented as a closure.
`ArrowApply ≅ Monad` (Hughes, LWY) means they share the same transparency
_potential_; the structural-explicitness difference is what makes `app`'s
runtime inspectability cheaper to exploit.

For the Staged Arrow Engine backend (§9.6), the structural explicitness of
`app` enables a three-way split of the kernel, not a two-way one:

1. **Staged-resolvable fragment** (no `app`, or `app` whose target is fixed
   at Stage 0) → spatial handler: full Stage 0 route optimization,
   zero-allocation execution.
2. **Finite-branch-space `app`** (`app` whose target set is provably finite
   and statically discoverable) → spatial handler with pre-compiled dispatch:
   Stage 0 pre-compiles every candidate sub-graph into a pre-allocated pool
   of flat Stage 1 chunks; runtime `app` becomes a zero-allocation pointer
   switch / indirect dispatch into the pool. This is the analogue of a
   jump table for arrow-valued data. Proving the branch space finite is the
   enabling analysis — trivial for first-order programs, potentially
   requiring abstract interpretation or type-based analysis when arrow
   values are computed by complex logic. Pre-compiling all branches
   multiplies the code-bloat trade-off (§4.2 of the SAE proposal); chunk
   eviction reintroduces dynamic memory management and must be balanced
   against the zero-allocation goal.
3. **Unbounded-branch-space `app`** (target set not provably finite, or
   dependent on unbounded runtime data schemas) → dynamic handler: live
   Stage 0 re-staging (sub-millisecond graph build + compile) or a
   conventional VM, exploiting the runtime transparency for inspection and
   sandboxing.

The boundary is _provable-finite-branch-space_, not `app`-presence. This
moves a large class of `app` usage — most conditional branching, pattern
matching, and finite dispatch — from the dynamic handler into the spatial
handler, leaving only genuinely unbounded dynamic structure to the
fallback.

### 9.2 What is the feedback semantics of `loop`?

The kernel describes a feedback channel; the handler chooses the
interpretation strategy. Two are standard:

- **Causal/strict** — state carries forward across steps (state-machine
  reading; read-before-write is normal).
- **Lazy/fixpoint** — state is a fixpoint (Haskell `ArrowLoop` reading;
  read-before-write may be productive or ill-founded).

Which semantics the reference handler implements, and how a language
declares which it wants, is an implementation decision.

### 9.3 How are effects typed?

Effects could be encoded in the arrow's operations (the handler tracks
them), or represented explicitly in the type: `Arrow<Effects, A, B>`.

### 9.4 What are the tooling protocols?

If a language is a first-class value that participates in generic tooling,
the tooling protocols (lex, parse, format, lint, typecheck, compile, ...)
must be specified. What is the minimal protocol a language must implement
to participate? How are protocol extensions composed?

### 9.5 Are languages themselves arrows?

One attractive possibility is `Language<A, B>` being an arrow-producing
algebra rather than a fundamentally different kind of object. If successful,
this removes the distinction between the object language and the
meta-language entirely.

### 9.6 What is the runtime representation?

The source-level arrow might compile to graph, bytecode, closure, CPS,
machine code, a VM instruction sequence, or a **static spatial graph /
interaction-net reduction** (as in the Staged Arrow Engine proposal, where
Stage 0 staging resolves dynamic control flow into a fixed, zero-allocation
graph for stackless Stage 1 execution) — without changing the source
semantics. The SAE backend is one handler interpretation among those listed
above. Its coverage of the kernel is three-tiered (see §9.1): the
staged-resolvable fragment and finite-branch-space `app` go to the spatial
handler (the latter via pre-compiled dispatch tables, zero-allocation at
runtime); unbounded-branch-space `app` is deferred to a dynamic handler
(live re-staging or a conventional VM) that exploits `app`'s runtime
transparency for inspection and sandboxing.

---

## 10. Conclusion

The central object is `A ~> B`, an arrow representing a first-class
computation. The central composition operator is `>>>`, which constructs
larger computations without requiring immediate execution. The central
higher-order operator is `app`, which dynamically invokes a received arrow
within the calculus. The central semantic mechanism is the **handler**,
which interprets computational descriptions from outside.

The language is not Lambda Calculus with an arrow library. It is Arrow
Calculus plus effects, handlers, and first-class languages. The conceptual
shift is from _abstraction → application → substitution_ toward
_construction → composition → interpretation_ — and the second-order shift
is from a fixed language with external tooling toward a first-class language
value participating in generic tooling protocols.

The kernel's purpose is not to execute programs directly. It provides the
**algebra from which programs are constructed**. Handlers provide the
semantics. Languages are first-class values that describe how programs work
and participate in generic tooling.

The resulting tower is:

```text
arrows → programs → languages → generic tooling
```

The unifying idea: **make computation, then languages, first-class values,
so the same compositional machinery applies recursively at every level.**

The language is not just the thing that runs the program.

**The language is another object the program can compute with.**

---

## References

- Moggi — _Notions of Computation and Monads_ (1991). Computational
  effects via monads; the monadic metalanguage. The precursor.
- Hughes — _Generalising Monads to Arrows_ (2000). Arrows as a
  generalization of monads; the computational abstraction.
- Paterson — _A New Notation for Arrows_ (2001). `proc` notation;
  introduced `loop` (feedback) and `returnA`.
- Lindley, Wadler, Yallop — _The Arrow Calculus_ (2008/2010, JFP). Arrows
  as a genuine typed metalanguage; the calculus foundation.
- Lindley, Wadler, Yallop — _Idioms are Oblivious, Arrows are Meticulous,
  Monads are Promiscuous_ (2011). Arrows in the monad/applicative/arrow
  landscape; classic arrows and the Arrow Calculus are equationally
  corresponding.
- Lindley — _Algebraic Effects and Effect Handlers for Idioms and Arrows_
  (2014). Effects/handlers generalized to arrows; the bridge from
  algebraic effects to arrow-style computation.
- Sanada — _Algebraic Effects and Handlers for Arrows_ (2024). The semantic
  reference model: arrow handlers, operations, continuations; operational
  semantics (sound, adequate) and denotational semantics (handlers as
  homomorphisms between algebras of arrows).
- Cimini — _A Calculus for Multi-language Operational Semantics_ (2022,
  Springer; vision paper 2018). `λ^𝓛`, a typed λ-calculus where languages
  are first-class citizens. The theoretical foundation for languages as
  first-class values.
- Haskell `Control.Arrow` — `ArrowLoop` as feedback; `ArrowApply` / `app`;
  `untilA` as a derived `while`-pattern over `loop` + `ArrowChoice`.
- GHC User's Guide — arrow notation, `rec` construct, recursive arrows.
