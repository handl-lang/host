# ADR-0052: Arrow Calculus Kernel

## Status

Accepted — Implemented (ADR-0052 big-bang cutover complete: v1 `Scheme`
removed, v2 arrow-calculus kernel is the sole default, `eval.yaml` rewritten
to v2 schemes, all engines and tests migrated; workspace builds and all
tests pass).

Refines [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
(§6 "Structured by default, vau as typed escape") and [ADR-0044 — Language as Data
and Runtime Split](./0044-language-as-data-and-runtime-split.md) (§4 "Evaluation
schemes as data, engines as interpreters"). **Supersedes [ADR-0045 — Vau as a
Typed Escape](./0045-vau-as-a-typed-escape.md)**: `vau` is removed from the
kernel — it addresses control over evaluation of unevaluated expressions and is
not required by the Arrow model. The kernel becomes purely arrow calculus
(`arr`/`>>>`/`first`/`choice`/`loop`/`app`); the semantic foundation for
interpreting effectful arrows is Sanada's _Algebraic Effects and Handlers for
Arrows_ — execution is one possible handler interpretation, rather than a
primitive property of arrows. Supersedes the lambda-flavored `Scheme`
combinator set defined in ADR-0044 §4.

## Context

ADR-0044 §4 defined the eval kernel as a `Scheme` combinator set. The set as
shipped (`crates/handl-language/src/scheme.rs`) is **lambda-calculus-flavored**,
not arrow-calculus-flavored, despite the surrounding architecture being
described in arrow terms:

| `Scheme` variant today       | Calculus lineage                                                           |
| ---------------------------- | -------------------------------------------------------------------------- |
| `Lit`, `Var`, `Child`        | lambda term / projection                                                   |
| `Bind { name, value, body }` | `let` / λ-abstraction                                                      |
| `App { callee, args }`       | lambda application (becomes `>>>` for static callee, `app` for dynamic)    |
| `Branch`, `Prim`, `Foreign`  | structured extras (become `choice`, `arr`)                                 |
| `Vau { params, body }`       | fexpr/operative — **removed** (not a calculus combinator; handler concern) |

ADR-0041 §6 and ADR-0045 describe the design as "structured by default, vau as
an explicitly-typed opt-in escape, tracked by the type system. Pure/structured
code compiles; vau code interprets. You opt in, the way you opt into
`ArrowApply`." This ADR accepts part of that framing and corrects the rest:

1. **`vau` is not arrow calculus — it is removed from the kernel.** `vau` is
   a fexpr/operative — it controls _how_ (and whether) child expressions are
   evaluated. That is a concern of the evaluator/handler, not of the
   calculus. Mixing `vau` into the kernel conflates "what the computation
   is" (the arrow) with "how the evaluator runs it" (the vau). The kernel
   describes computations; the handler interprets them. If fexpr-style
   behaviors are needed, they live in the handler layer.

2. **`app`/`ArrowApply` is kept — but distinguished from external
   interpretation.** `app` is a real calculus operation: it lets an arrow
   dynamically invoke another arrow it received as runtime data. This is
   required for higher-order arrows. What `app` is _not_ is external
   interpretation — running, compiling, or analyzing an entire arrow is the
   handler's job (Sanada's model), not a calculus operation. The clean
   three-way separation: `>>>` constructs, `app` invokes dynamically
   (within the calculus), handlers interpret externally.

### The `loop` correction

A prior reading of `ArrowLoop` treated `loop` as iteration ("`loop` executes
repeatedly"). The standard formulation (Paterson, _A New Notation for
Arrows_; Haskell `Control.Arrow`) corrects this:

```
loop : Arrow (A, S) (B, S) → Arrow A B
```

`loop f` takes an arrow `f : (A, S) ~> (B, S)` and feeds the second output `S`
back into the second input `S`; the first component becomes the externally
visible output. **`loop` is feedback, not iteration.** Repetition is a
consequence of the interpreted arrow's semantics, not the combinator's
definition. A state machine is the canonical shape:

```
step  : (Event, State) ~> (Result, State)
machine = loop step        -- Event ~> Result, State hidden in feedback
```

`while`/`for`/`untilA` are _patterns_ over `loop` + `ArrowChoice` (feedback +
routing), not primitives. GHC's arrow notation documents exactly this
derivation (`untilA` defined recursively via arrows + `ArrowChoice`).

This reframes recursion for an arrow-first language:

> **Feedback is primitive; recursion is one possible semantic interpretation or
> derived form of feedback.**

The language exposes `loop` (feedback) and derives `fix`/recursive functions
and `while`/`for` from it, rather than exposing `fix`/Y-combinator and
deriving feedback. This is a meaningful departure from lambda calculus and the
load-bearing design principle of this ADR.

### Why move at all

Three pressures converge:

1. **Faithfulness.** The platform's stated identity is arrow-calculus-based
   (ADR-0041 §"The arrow calculus solves the unpredictable-compute-graph
   problem"; ADR-0044 §4 "denotational semantics as a future interpretation …
   small enough to map to a category"). The kernel should match the literature
   it cites. A lambda-flavored kernel with an arrow veneer is a drift risk.

2. **The kernel should describe computation, not execute it.** Today `App`
   (execution) and `Vau` (evaluation control) are _in_ the kernel, mixing
   "what the computation is" with "how it runs." The arrow-calculus kernel
   separates these: `arr`/`>>>`/`first`/`choice`/`loop` describe
   computation; `app` allows an arrow to dynamically invoke another arrow
   (a real calculus operation, distinct from external interpretation);
   interpretation is delegated to handlers (Sanada's model — see Decision).
   `Vau` is removed: it controls evaluation of unevaluated expressions, which
   is not a calculus concern. `>>>` composes arrows; it does not execute
   them.

3. **`loop` as feedback gives a cleaner foundation for state, iteration, and
   recursion than `fix`.** State machines, reactive loops, and recursive
   definitions all lower to one combinator (`loop`) plus routing (`choice`),
   instead of three separate language constructs (`while`, `for`, `rec`).

### Lineage: from arrows to a kernel

This ADR does not invent a calculus; it adopts a mature lineage and takes it
one step further. The intellectual heritage:

| Work                                                                                                       | Contribution                                                                                                                                                                                                                                 | Relevance to our design                  |
| ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| **Moggi** — _Notions of Computation and Monads_ (1991)                                                     | Computational effects via monads; the monadic metalanguage. The precursor that Hughes generalizes.                                                                                                                                           | Computational effects foundation         |
| **Hughes (2000)** — _Generalising Monads to Arrows_                                                        | Introduced **arrows** as a generalization of monads; the minimal `arr`, `>>>`, `first`.                                                                                                                                                      | **Computational abstraction**            |
| **Paterson (2001)** — _A New Notation for Arrows_                                                          | Added practical **arrow notation** (`proc`), making point-free arrow programs readable and compositional; introduced **`loop`** (feedback) and `returnA`.                                                                                    | **Surface syntax + feedback**            |
| **Lindley, Wadler, Yallop (2008/2010)** — _The Arrow Calculus_                                             | Made arrows into a genuine **Arrow Calculus**: a typed metalanguage for manipulating Hughes's arrows, closely related to Moggi's metalanguage and Paterson's notation. Four constructs, five laws, proved complete.                          | **Calculus / language foundation**       |
| **Lindley, Wadler, Yallop (2011)** — _Idioms are Oblivious, Arrows are Meticulous, Monads are Promiscuous_ | Placed arrows in the landscape of monads/idioms/arrows; showed classic arrows and the Arrow Calculus are equationally corresponding.                                                                                                         | **Algebraic grounding**                  |
| **Lindley (2014)** — _Algebraic Effects and Effect Handlers for Idioms and Arrows_                         | Generalized algebraic effects and handlers to idioms and arrows; established that handlers aren't inherently tied to monadic computation.                                                                                                    | **Effects + handlers for arrows**        |
| **Sanada (2024)** — _Algebraic Effects and Handlers for Arrows_                                            | Formalized **Arrow Calculus with operations and handlers**, with operational + denotational semantics; handlers correspond to homomorphisms of arrow algebras; proves soundness and adequacy.                                                | **Semantic foundation for our handlers** |
| **Cimini (2018/2022)** — _A Calculus for Multi-language Operational Semantics_                             | Presents `λ^𝓛`, a typed λ-calculus where **languages are first-class citizens**: bound to variables, passed to functions, returned from functions, used to execute programs. Languages as runtime values; dynamic creation and modification. | **Languages as first-class values**      |

The conceptual genealogy:

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

What we take from each:

| Source            | We take                                               |
| ----------------- | ----------------------------------------------------- |
| **Moggi**         | Computational effects as a structured concept         |
| **Hughes**        | Arrow as a general computational abstraction          |
| **Paterson**      | Arrow-oriented programming syntax + `loop`            |
| **LWY 2008/2010** | A formal Arrow Calculus                               |
| **LWY 2011**      | Algebraic relationship between arrows, idioms, monads |
| **Lindley 2014**  | Effects and handlers generalized to arrows            |
| **Sanada**        | Formal semantics for Arrow operations + handlers      |
| **Cimini**        | Languages as first-class, manipulable values          |

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

That last step is the novel architectural idea: the language's own definition
— lex, parse, evaluate, format, lint, typecheck, compile, optimize, ... — is
part of the computational universe, not host-language infrastructure. And
crucially, **those operations can themselves be arrows**.

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

Those aren't fundamentally host-language features. They are computations
over a first-class `Language` value — and in our architecture, they are
arrows, interpreted by the same handlers as any other computation.

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
conventional functional language**. We're making the Arrow Calculus itself the
computational substrate, with Sanada's handler semantics providing the
foundation for interpretation. This is the core architectural thesis of this
ADR: the kernel _is_ the Arrow Calculus (six primitives), not a lambda
calculus with arrows bolted on; interpretation _is_ Sanada's handler model,
not an ad-hoc evaluator; the language _is_ a first-class value (Cimini's
insight), not host infrastructure. Every other decision in this ADR follows
from that thesis.

## Arrow Handlers

We adopt the handler model of Sanada's _Algebraic Effects and Handlers for
Arrows_ as the semantic foundation for interpreting effectful arrows.

An arrow is a first-class computational description:

```text
A ~> B
```

Operations represent effectful primitives, while handlers assign semantics to
those operations and their continuations.

Conceptually:

```text
program
  |
  | handle H
  v
interpreted computation
```

A handler contains:

```text
return clause
operation clauses
```

An operation handler receives the operation's argument and its continuation,
allowing it to define how the operation is interpreted and how computation
resumes.

This gives the language a clean separation:

```text
Arrow       = what computation is described
Operation   = what effect is requested
Handler     = how that effect is interpreted
```

Execution is therefore one possible handler interpretation, rather than a
primitive property of arrows.

Sanada provides operational semantics for this model and a denotational
semantics based on algebras of arrows, with handlers interpreted as
homomorphisms between the corresponding algebras. The operational semantics is
proved sound and adequate with respect to the denotational semantics.

We therefore treat Sanada's arrow-handler calculus as the semantic reference
model for this language, while adapting its syntax and kernel to our
Arrow-first design.

## Decision: Arrow Kernel

We adopt the following Arrow extensions as first-class language primitives:

```text
arr
>>>
first
choice
loop
app
```

These correspond to the Arrow hierarchy:

| Primitive | Role                                            |
| --------- | ----------------------------------------------- |
| `arr`     | Lift a primitive/pure computation into an arrow |
| `>>>`     | Sequential arrow composition                    |
| `first`   | Lift computation over product structure         |
| `choice`  | Branch over sum/control-flow structure          |
| `loop`    | Feedback/stateful computation                   |
| `app`     | Dynamic/higher-order arrow application          |

The first three form the **minimal Arrow** interface. `choice`, `loop`, and
`app` are deliberate extensions required by a general-purpose language.

### `app` and Interpretation

`app` is distinct from external interpretation.

Given:

```text
f : A ~> B
x : A
```

`app` allows an arrow to dynamically invoke another arrow:

```text
app(f, x)
```

This is required for higher-order arrows, where an arrow can receive another
arrow as runtime data.

By contrast, interpreting an entire arrow remains external:

```text
handle Runtime program
handle Compiler program
handle Analyzer program
```

Therefore:

```text
>>>       constructs computation
app       dynamically invokes computation
handler   interprets computation externally
```

`Vau` is not part of the language kernel. It addresses control over
evaluation of unevaluated expressions and is not required by the Arrow model.

### Semantic Principle

The language is Arrow-first rather than Lambda-first.

The fundamental computational object is:

```text
A ~> B
```

Functions are not a separate foundational computational category in this
language. This is a **language-design choice inspired by the Arrow theory**,
not a claim established by the literature: Hughes shows that ordinary
functions _can_ form an Arrow instance (`arr f = f`, with composition and
`first` defined naturally), and the Arrow Calculus (Lindley–Wadler–Yallop)
is itself formulated by extending simply typed lambda calculus with arrow
constructs. The literature supports that functions _can_ be an Arrow
instance; this language chooses to make arrows primary and treat functions
as the degenerate pure case.

Arrows are first-class computational descriptions; handlers provide
interpretations of those descriptions.

Feedback (`loop`), branching (`choice`), and higher-order invocation (`app`)
extend the minimal Arrow structure into the computational kernel required by
the language. `app` in particular is an `ArrowApply` extension, not part of
the minimal Arrow calculus; it is included because the language requires
higher-order arrows — arrows that can dynamically invoke other arrows
received as runtime data. Whether this is necessary is a language-design
choice, not something established by the Arrow literature.

### Kernel representation

The `Scheme` enum encodes the six primitives as data (per ADR-0044's
"language as data" model). `Arr` carries an `ArrSource` field distinguishing
native pure ops from foreign (host) calls, keeping the capability boundary
structural (matched on by the typechecker and load-time capability audit),
not a string convention to typo.

```rust
pub enum Scheme {
    /// `arr f` — lift a named pure function over the arrow's input.
    /// Sub-schemes feed the function's arguments. Replaces Lit, Var, Child,
    /// Prim, Foreign. `arr` describes a pure step; interpretation is the
    /// handler's job.
    Arr { op: String, args: Vec<Scheme>, source: ArrSource },

    /// `>>>` — sequential composition. Constructs a new arrow whose output
    /// of `fst` feeds `snd`. This *composes* arrows; it does **not execute**
    /// them. The composed arrow is a description; a handler interprets it.
    Seq { fst: Box<Scheme>, snd: Box<Scheme> },

    /// `first` — fan-out: run `inner` on the first component of a pair,
    /// carry the second component through unchanged. Constructs a new
    /// arrow; does not execute.
    First { inner: Box<Scheme> },

    /// `ArrowChoice` — route on `Either`: `l` handles `Left`, `r` handles
    /// `Right`. Replaces Branch. Constructs a routing arrow; does not
    /// execute.
    Choice { l: Box<Scheme>, r: Box<Scheme> },

    /// `ArrowLoop` — feedback (NOT iteration). `body` has shape
    /// `(In, State) ~> (Out, State)`; `Loop` hides `State` in a feedback
    /// channel, exposing `In ~> Out`. Recursion, `while`, `for`, state
    /// machines are derived patterns over `Loop` + `Choice`.
    ///
    /// The feedback semantics (causal/strict vs lazy/fixpoint) are
    /// specified separately (see §Feedback semantics) — `Loop` in the
    /// kernel is the *description* of a feedback channel; the
    /// interpretation strategy is the handler's concern.
    Loop { body: Box<Scheme> },

    /// `app` / `ArrowApply` — dynamic/higher-order arrow application.
    /// Given `f : A ~> B` and `x : A`, `app(f, x)` invokes `f` on `x`.
    /// Required for higher-order arrows (an arrow received as runtime data).
    /// Distinct from external interpretation: `app` is a calculus operation
    /// *within* an arrow; `handle H program` interprets an entire arrow
    /// from outside.
    App { callee: Box<Scheme>, args: Vec<Scheme> },
}

/// Whether an `Arr` op is a native pure function or a foreign host call.
/// `Foreign(name)` keeps the capability boundary structural: the typechecker
/// and load-time capability audit match on `source`, not on a string prefix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ArrSource {
    /// A native pure op (`add`, `proj_env`, `const`, `id`, …).
    Native,
    /// A foreign (host) function call — capability-tracked. The `name` is
    /// the registered host-function name (e.g. `"fs.read"`).
    Foreign(String),
}
```

**Six primitives.** `ArrSource` is metadata on `Arr`, not a seventh primitive
— the six are the calculus combinators; `source` is the capability discipline
on the `arr` combinator.

### Lowering table (current → arrow)

| Current `Scheme`                            | Arrow-calculus form                                                                                                               |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `Lit(v)`                                    | `Arr { op: "const", args: [v], source: Native }`                                                                                  |
| `Var(n)`                                    | `Arr { op: "proj_env", args: [n], source: Native }`                                                                               |
| `Child(i)`                                  | `Arr { op: "proj_child", args: [i], source: Native }`                                                                             |
| `Prim { op, args }`                         | `Arr { op, args, source: Native }` (sub-schemes feed the pure op)                                                                 |
| `Foreign { name, args }`                    | `Arr { op, args, source: Foreign(name) }` (capability-tracked via `source`, not a string prefix)                                  |
| `Bind { name, value, body }`                | `value >>> arr (extend env with name) >>> body` (the `x = y` desugar; `first` carries env)                                        |
| `App { callee, args }` — **static callee**  | `args >>> callee` (composition with the known arrow — compiles by construction)                                                   |
| `App { callee, args }` — **dynamic callee** | `App { callee, args }` (the `app` primitive — dynamic invocation within the calculus)                                             |
| `Branch { c, t, e }`                        | `c >>> arr tag >>> Choice(t, e)`                                                                                                  |
| `Vau`                                       | **removed from the kernel** — `vau` addresses control over evaluation of unevaluated expressions, not required by the Arrow model |
| `eval.assign` (reassignment)                | **deleted** — no mutation primitive; state via `Loop`                                                                             |
| `eval.stmt.let` / `eval.stmt.const`         | **folded into `x = y`** — one binding form, immutable                                                                             |

The `App` row: a static callee (a known arrow in scope at desugar time) is
composition — `args >>> callee` — which is wiring, not `app`. A dynamic
callee (a received/computed arrow) is `App` — the `app` primitive, a calculus
operation that dynamically invokes a received arrow. This is distinct from
external interpretation: `app` is _within_ the calculus; `handle H program`
interprets an entire arrow from outside.

### Derived forms (not primitive)

Expressed via the six primitives, not added to the kernel:

- `id` = `Arr { op: "id" }`
- `second` = `arr swap >>> first >>> arr swap`
- `&&&` / `Fan` (parallel pair) = `arr dup >>> first >>> first`-style wiring
- `Branch { c, t, e }` = `c >>> arr (λ b → if b then Left else Right) >>> Choice(t, e)`
- `while cond body` = library pattern over `Loop` + `Choice` (feedback + routing)
- `for` / `untilA` = library patterns over `Loop` + `Choice`
- `fix` / recursive functions = derived semantic interpretation of `Loop`

`while`/`for`/`untilA`/`fix` are **not** kernel constructs and **not** grammar
rules. They live in a `lib-*` crate as derived patterns. This is the concrete
payoff of "feedback is primitive; iteration is derived."

### `proc` notation and `x = y` binding as the authoring surface

Add a `proc x -> ...` surface form (arrow do-notation, GHC-style) with a
grammar rule and a desugar to `arr`/`>>>`/`first`/`Loop`/`App`. This is the
ergonomic authoring layer for arrow calculus; authors write `proc` and the
desugar produces the six-combinator kernel.

**Binding is `x = y`, not `let`/`const`/`assign`.** The lambda-calculus
keywords (`let`/`const`) are λ-abstraction ceremony with no meaning in arrow
calculus — binding is just `Seq` + env extension. The surface reflects this:
a name is introduced with `x = y` and lowered to `value >>> arr (extend_env
"x") >>> body`. One binding form, no keywords.

`x = y` is **immutable**: a name is bound once and cannot be redefined in
the same scope. Redefining `x` where `x` is already in scope is a static
error, not a shadow. There is no mutation primitive in the kernel and no
reassignment surface. Observable mutable state goes through `Loop` with a
state channel — the arrow-calculus-honest way — or not at all.

This collapses three current node kinds into one:

| Today                                  | Arrow surface | Kernel                                                    |
| -------------------------------------- | ------------- | --------------------------------------------------------- |
| `let x = y` / `stmt.let`               | `x = y`       | `Seq { y, arr(extend_env "x") } >>> body`                 |
| `const x = y` / `stmt.const`           | `x = y`       | (same — all bindings are immutable, `const` is redundant) |
| `x = y` / `eval.assign` (reassignment) | **deleted**   | no mutation primitive; state via `Loop`                   |

`eval.assign`, `eval.stmt.let`, and `eval.stmt.const` are removed from
`eval.yaml`. One binding form, one scheme, no mutation. The kernel surface
shrinks; the discipline of the arrow substrate (no mutation, state is
feedback) is reflected in the surface, not just the kernel.

The existing lambda-flavored surface (`function`/`call`) becomes **desugar
rules** lowering to arrow combinators per the lowering table. `let`/`const`
do not survive — they are replaced by `x = y` everywhere, including in the
bootstrap rewrite (Phase D). `function`/`call` become desugar-only
compatibility rules; a `call` with a static callee desugars to composition,
a `call` with a dynamic callee desugars to `App` (`app`). `proc` is the
canonical surface (see §Scope decision).

### Big-bang migration

A coordinated single change across kernel + data + all three engines. The
current `Scheme` (v1) is kept behind a feature flag until parity is proven
across the workspace test suite, then deleted.

**Phase A — Kernel swap (`handl-language`).**
Replace the `Scheme` enum with the six arrow primitives (no `Vau`). Update
`DesugarRule` to produce arrow combinators. Rewrite the ~50 schemes in
`crates/handl-kit-data-canonical/data/eval.yaml` per the lowering table
(mechanical). Bump `scheme_version` to `"v2"` (ADR-0044 §7 already provisions
versioning; v1 `Language` values run on a v2 engine backward-compatibly, v2
on v1 is a compile-time error).

**Phase B — Engines (one coordinated pass, all three).**

- `plugin-engine-exec-tree-walk`: interpret `Arr`/`Seq`/`First`/`Choice`/
  `Loop`/`App`. The engine is a _handler_ (Sanada's model) — it interprets
  arrow descriptions, dispatching operations to their handler clauses. `App`
  is interpreted as dynamic invocation of a received arrow. `Loop` is
  feedback; the interpretation strategy (causal/strict vs lazy/fixpoint) is
  specified in §Feedback semantics.
- `plugin-engine-exec-bytecode`: emit instructions for the six combinators.
  `Loop` → a back-edge / feedback instruction per §Feedback semantics.
- `plugin-engine-exec-cranelift`: lower to native; `Loop` → a loop with a
  state slot per §Feedback semantics.
- The closure representation (`{"__closure__": ...}`) is reconsidered:
  closures are now _arrows as values_ (descriptions, invokable via `app`).
  The `closure` prim becomes "package an arrow as a value"; `app` invokes a
  received arrow-value.

**Phase C — `proc` notation surface + desugar.**
New `proc x -> ...` grammar rule + desugar to `arr`/`>>>`/`first`/`Loop`/
`App`. `proc` is the canonical authoring surface. Lambda-flavored surface
(`function`/`call`, with `x = y` replacing `let`/`const`) becomes
desugar-only compatibility rules per the lowering table.

**Phase D — Bootstrap rewrite (`handl-defining-handl`).**
Rewrite `data/handl-defining-handl.hnl` and the `Language` value in arrow
calculus / `proc` notation. Rebuild the self-hosting corpus
(`crates/handl-defining-handl/src/lib.rs`) over arrow combinators; all three
engines must pass. Re-pass the Futamura projection tests
(`crates/handl-defining-handl/tests/futamura.rs`) on the arrow kernel.

**Phase E — `while`/`for`/iteration as library.**
Implement `while`/`for`/`untilA` as derived patterns over `Loop` + `Choice`
in a `lib-*` crate. Not kernel, not grammar. The concrete demonstration that
"feedback is primitive; iteration is derived."

### Handlers are outside the calculus; execution is one interpretation

The kernel describes computations (arrows) and includes `app` for dynamic
invocation _within_ the calculus. Interpreting an entire arrow — running it,
compiling it, analyzing it — is external, performed by a handler (Sanada's
model):

```text
Arrow<A,B>            (a description — the kernel's product)
      │
      ├── handle Runtime     (tree-walk handler: produces values)
      ├── handle Compiler    (bytecode/cranelift handler: produces native code)
      ├── handle Analyzer    (typechecker, linter, formatter: produces facts)
      └── ...                (any other handler of the same description)
```

This is the clean separation: the calculus composes descriptions (`>>>`
constructs, `arr` lifts, `first`/`choice`/`loop` shape, `app` invokes
dynamically); the handler interprets. `>>>` does not execute — it builds a
larger arrow from two smaller ones. A runtime handler walks that arrow to
produce values; a compiler handler lowers it to native code; an analyzer
handler inspects it without running it. All three consume the _same_
description.

`app` is _not_ a handler — it is a calculus operation that an arrow uses to
invoke another arrow it received as runtime data. The handler interprets
`app` (along with every other combinator) when it interprets the whole
arrow. The distinction:

```text
>>>       constructs computation
app       dynamically invokes computation (within the calculus)
handler   interprets computation externally
```

**`Vau` is not part of the language kernel.** It addresses control over
evaluation of unevaluated expressions and is not required by the Arrow model.
If/when fexpr-style behaviors (short-circuiting `and`, pattern matching,
macros) are needed, they are an _evaluator/handler_ concern, not a calculus
combinator — they live in the handler layer, not in the `Scheme` enum. The
kernel stays pure arrow calculus; the handler can be extended with operative
behaviors without touching the calculus. This is the correct factoring: the
calculus describes what a computation is; the handler decides how to run it.

### Feedback semantics: causal/strict vs lazy/fixpoint — specified separately

The kernel describes a feedback channel via `Loop`; the interpretation
strategy is the handler's concern. Two feedback semantics are standard, and
they disagree on read-before-write:

- **Causal/strict semantics.** The `State` output of one step is the `State`
  input of the _next_ step. A read of `State` in step _n_ sees the value
  written in step _n−1_. A read-before-write in the _same_ step reads the
  prior step's output (or an initial seed) — well-defined, not bottom. This
  is the state-machine reading: `step : (Event, State) ~> (Result, State)`,
  `loop step` hides `State` in a feedback channel that carries the previous
  step's output forward. Read-before-write is the normal case (you read the
  incoming state, then write the outgoing state).

- **Lazy/fixpoint semantics.** The `State` is a fixpoint: `loop f` solves
  `s = snd(f(a, s))` for the state `s`, then produces `fst(f(a, s))`. Under
  this reading, a read-before-write in the same "iteration" is a demand for
  `s` before it is defined — ill-founded (bottom) unless the fixpoint is
  productive (coinductive/final-coalgebra semantics, where a read can be
  satisfied by a _partial_ result that the write produces lazily). This is
  the Haskell `ArrowLoop` / `fix` reading.

These are _different interpretations of the same `Loop` description_. The
kernel does not choose; the handler does. A runtime handler that wants state
machines implements causal/strict semantics (read-before-write is normal, no
fixpoint machinery). A runtime handler that wants fixpoint recursion
implements lazy/fixpoint semantics (read-before-write may be productive or
ill-founded, detected at runtime as a demand cycle). A compiler handler
lowers according to whichever semantics the `Language` declares.

**What the kernel fixes:** `Loop { body }` is a feedback channel — `body` has
shape `(In, State) ~> (Out, State)`, `Loop` hides `State`, exposing `In ~> Out`.

**What the handler fixes:** the interpretation strategy — causal/strict
(state carries forward across steps) or lazy/fixpoint (state is a fixpoint).
This is declared in the `Language` or runtime config, not encoded in the
`Scheme`.

**What is NOT done:** equating "read before write" with ill-foundedness and
rejecting it at scheme-load time. That was a conflation of one interpretation
strategy (lazy/fixpoint) with the kernel. Under causal/strict semantics,
read-before-write is the normal pattern and must not be rejected. The
well-foundedness question is real _under lazy/fixpoint semantics_, but it is
the handler's concern (detected as a demand cycle at runtime, or analyzed by
a handler-specific static pass), not a kernel-level scheme-load gate.

This is an open question (see below), to be resolved in Phase B with a
reference implementation of each semantics + tests before the compiled
handlers follow.

### Scope decision: kernel + bootstrap + canonical `proc` surface

This ADR commits to **kernel + bootstrap lang + canonical `proc` surface**:
swap the kernel, rewrite the bootstrap in arrow calculus / `proc`, and make
`proc` the canonical authoring surface. The lambda-flavored surface
(`function`/`call`) becomes a desugar-only compatibility layer; `let`/`const`
are replaced by `x = y` everywhere. `proc` is not deferred — it is the
surface of the arrow-calculus language, and the bootstrap (Phase D) is
authorable in it.

## Consequences

### Positive

- **The kernel is faithfully arrow calculus.** Six primitives (`arr`/`>>>`/
  `first`/`choice`/`loop`/`app`), matching the Arrow hierarchy
  (minimal Arrow + `ArrowChoice` + `ArrowLoop` + `ArrowApply`). The
  platform's stated identity and its kernel align.
- **Sanada's arrow handlers give a formal semantic foundation.** Execution
  is one possible handler interpretation, not a primitive property of
  arrows. The calculus describes; handlers interpret. Operational and
  denotational semantics (handlers as homomorphisms between algebras of
  arrows) are proved sound and adequate in the literature.
- **`app` is in the calculus; interpretation is external.** The clean
  three-way separation: `>>>` constructs computation, `app` dynamically
  invokes computation (within the calculus), handlers interpret computation
  externally. Higher-order arrows work without conflating description with
  execution.
- **`Vau` is removed from the kernel.** It addresses control over
  evaluation of unevaluated expressions — not required by the Arrow model.
  Fexpr-style behaviors, if needed, are a handler/evaluator concern, not a
  calculus combinator. The kernel stays pure arrow calculus.
- **`loop` as feedback gives a cleaner foundation for state, iteration, and
  recursion.** State machines, reactive loops, and recursive definitions all
  lower to one combinator plus routing, instead of three separate language
  constructs. `while`/`for`/`fix` become library, not language.
- **Denotational semantics targets a category of arrows.** ADR-0044 §4's
  "future interpretation" (denotational engine mapping schemes to morphisms)
  now targets Sanada's algebras-of-arrows semantics (handlers as
  homomorphisms) — a formal foundation, not just an aspiration.
- **`proc` notation gives authors an ergonomic arrow surface.** Authors write
  `proc`, not raw combinators; the desugar produces the small kernel.
- **`x = y` replaces `let`/`const`/`assign`; binding is immutable.** The
  lambda-calculus keywords are gone — binding is `Seq` + env extension, and
  the surface says so. Three node kinds (`stmt.let`/`stmt.const`/`assign`)
  collapse into one. No mutation primitive, no reassignment surface; mutable
  state is `Loop` with a state channel. The arrow substrate's discipline
  (no mutation, state is feedback) is reflected in the surface, not just the
  kernel.

### Negative

- **Big-bang is high-risk.** All three engines + ~50 eval schemes + bootstrap
  - corpus change in one coordinated step. Mitigation: keep v1 `Scheme`
    behind a feature flag until the 2900-test suite passes on v2, then delete.
    The flag is a safety net, not a permanent state — ADR-0044 §6's "emitters
    as scaffold, data files as end state" discipline applies: the scaffold is
    removed once parity is proven.
- **`app` in the kernel requires handler-level interpretation.** `app` is a
  calculus operation, but interpreting it (actually invoking the received
  arrow) is the handler's job. A handler that cannot interpret `app` (e.g. a
  pure static compiler) must reject `App`-containing arrows or lower them to
  a runtime call. This is the standard `ArrowApply` cost: it makes the arrow
  no longer statically inspectable past the `app` boundary. The compile/
  interpret distinction moves to the handler layer — a compiler handles what
  it can lower; a runtime handler handles `app` and the rest.
- **`Loop` feedback semantics are not fixed by the kernel.** The kernel
  describes a feedback channel; the interpretation strategy (causal/strict
  vs lazy/fixpoint) is the handler's concern (§Feedback semantics). This is
  correct factoring, but it means `Loop` has no single semantics until a
  handler implements one — and the two standard semantics disagree on
  read-before-write. Resolving this (which semantics the reference handler
  implements, how a `Language` declares which it wants) is open work for
  Phase B, and getting it wrong affects state machines, recursion, and
  iteration simultaneously.
- **`Foreign` capability boundary is structural, not conventional.** Folding
  `Foreign` into `Arr { op, args, source: Foreign(name) }` keeps the
  capability boundary as a tagged field (`ArrSource`), matched on by the
  typechecker and the load-time capability audit — not a string prefix
  convention to typo. A non-foreign op cannot accidentally bypass capability
  tracking by being named `foreign:foo`. The cost is one extra field on
  `Arr`; the six-primitive claim is about calculus combinators, and
  `source` is metadata on the `arr` combinator, not a seventh primitive.
- **`function`/`call` as desugar-only compatibility.** `proc` is canonical;
  `function`/`call` survive as desugar rules lowering to the same kernel, not
  as a co-equal surface. This means two syntaxes for the same semantics during
  the transition, but `proc` is the intended authoring surface and the
  bootstrap (Phase D) is written in it.

### Neutral

- **Arrow calculus + language-as-value lineage.** This is Hughes's arrows
  - Paterson's `loop` and arrow notation + Lindley/Wadler/Yallop's calculus
  - `ArrowChoice` + `ArrowApply`, lifted into a typed, data-driven language,
    with Sanada's arrow handlers as the semantic foundation for interpretation,
    and Cimini's languages-as-first-class-values as the foundation for the
    `Language` value. The key departures from the literature:
    `loop` is exposed as the primitive (feedback), with recursion derived;
    `vau` is removed (it controls evaluation, which is a handler concern, not
    a calculus combinator); the kernel is serializable data (ADR-0044), not a
    Rust trait graph; interpretation is delegated to handlers (Sanada's model),
    not built into the calculus; the language itself is a first-class value
    (Cimini's insight) whose tooling operations are themselves arrows.

## Open questions

1. **`Loop` feedback semantics: causal/strict vs lazy/fixpoint.** The kernel
   describes a feedback channel; the handler chooses the interpretation
   strategy (§Feedback semantics). The reference tree-walk handler should
   implement causal/strict (state-machine reading) as the default, with
   lazy/fixpoint as an opt-in. How a `Language` declares which it wants is
   to be resolved in Phase B with a reference implementation + tests before
   the compiled handlers follow.
2. **`vau` in the handler layer.** If/when fexpr-style behaviors
   (short-circuit `and`, pattern matching, macros) are needed, `vau` lives in
   the handler/evaluator, not the `Scheme` enum. What does that
   handler-layer interface look like, and how does it interact with the
   pure-arrow-calculus kernel? Deferred until a concrete fexpr use case
   arises.

## References

- [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
  — §6 "Structured by default, vau as typed escape"; §"The arrow calculus
  solves the unpredictable-compute-graph problem".
- [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
  — §4 "Evaluation schemes as data, engines as interpreters"; §7 "Scheme
  combinator versioning".
- [ADR-0045 — Vau as a Typed Escape](./0045-vau-as-a-typed-escape.md) —
  **superseded by this ADR**: `vau` is removed from the kernel; it addresses
  evaluation of unevaluated expressions, which is a handler concern, not a
  calculus combinator.
- `crates/handl-language/src/scheme.rs` — the current lambda-flavored
  `Scheme` combinator set this ADR replaces.
- `crates/handl-kit-data-canonical/data/eval.yaml` — the ~50 eval schemes
  rewritten in Phase A.
- `crates/plugin-engine-exec-tree-walk/src/lib.rs`,
  `crates/plugin-engine-exec-bytecode/src/`,
  `crates/plugin-engine-exec-cranelift/src/` — the three engines migrated in
  Phase B (become handlers under Sanada's model).
- `crates/handl-defining-handl/` — the bootstrap rewritten in Phase D.
- Sanada — _Algebraic Effects and Handlers for Arrows_ (2024) — the
  semantic reference model: arrow handlers, operations, continuations;
  operational semantics (sound, adequate) and denotational semantics
  (handlers as homomorphisms between algebras of arrows).
- Cimini — _A Calculus for Multi-language Operational Semantics_ (2022,
  Springer; vision paper 2018) — `λ^𝓛`, a typed λ-calculus where
  languages are first-class citizens: bound to variables, passed to
  functions, returned from functions, used to execute programs. The
  theoretical foundation for languages as first-class values.
- Lindley — _Algebraic Effects and Effect Handlers for Idioms and Arrows_
  (2014) — generalized algebraic effects/handlers to idioms and arrows;
  the bridge from effects/handlers to arrow-style computation.
- Lindley, Wadler, Yallop — _Idioms are Oblivious, Arrows are Meticulous,
  Monads are Promiscuous_ (2011) — arrows in the monad/applicative/arrow
  landscape; classic arrows and the Arrow Calculus are equationally
  corresponding.
- Lindley, Wadler, Yallop — _The Arrow Calculus_ (2008/2010, JFP) — arrows
  as a genuine typed metalanguage; the calculus foundation.
- Paterson — _A New Notation for Arrows_ (2001) — `proc` notation;
  introduced `loop` (feedback) and `returnA`.
- Hughes — _Generalising Monads to Arrows_ (2000) — arrows as a
  generalization of monads; the computational abstraction.
- Moggi — _Notions of Computation and Monads_ (1991) — computational
  effects via monads; the monadic metalanguage. The precursor.
- Haskell `Control.Arrow` — `ArrowLoop` as feedback (`loop : Arrow (A,S)
(B,S) → Arrow A B`); `ArrowApply` / `app`; `untilA` as a derived
  `while`-pattern over `loop` + `ArrowChoice`.
- GHC User's Guide — arrow notation, `rec` construct, recursive arrows.
