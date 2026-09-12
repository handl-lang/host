# ADR-0106: The Object/Arrow Kernel — `sym()`, `Obj`, and Native Arrow Calculus

## Status

Proposed.

## Context

Handl's semantic layers have grown as separate universes. Values live in
`crates/handl-value` (JSON-shaped tagged data), types live in
`handl-packages/type/type.hnl` as tagged objects (`Int :: { "__tag__": "int",
... }`), the eval kernel is an arrow calculus (ADR-0052, ADR-0103's frontier
calculus), and grades, effects, proofs, and capabilities each carry their own
machinery (ADR-0101, ADR-0102). Several awkward artifacts follow from the split:

- **Types are second-class.** `Int` and `Float` in `type.hnl` are plain tagged
  objects; there is no nominal identity primitive, so `Float <: Number` must be
  encoded by a parallel subtype relation rather than read off the objects
  themselves. `instanceof` at runtime and `<:` at static time are powered by
  two unrelated mechanisms.
- **Functions are not values.** Arrows are objects, but the binding model
  still leans on closures, lexical environments, and mutable scope stacks
  (`make_closure`, `bind`, `lookup` in
  `crates/handl-runtime/src/vm_primitives.rs`) — hidden captured state that is
  neither serializable nor inspectable.
- **Every new language feature is a new subsystem.** Interfaces, traits,
  effects, proofs, dependent types, generics each arrive with dedicated syntax
  and a dedicated checker, even though each is expressible as objects and
  arrows.

The object/arrow kernel collapses these universes into one. It refines
ADR-0052 (arrow calculus kernel), ADR-0101 (frontier calculus), ADR-0102
(grades as first-class objects), ADR-0075 (capability discipline), and ADR-0103
(Handl as its own metalanguage), and gives `handl-packages/type/type.hnl` a
real nominal substrate.

## Decision

### 1. One semantic world

Everything is an object; all computation is an arrow. There is no separate
type/value/function universe:

```
Obj : sym()
Type : Obj := sym()
```

`Type` is itself an object created by the same primitive as any other object.
Types are values, functions are values, grades are values, proofs are values.
A single world, a single serializer, a single inspector.

### 2. `sym()` / `sym(A)` — the nominal identity primitive

The VM exposes two primitives:

- `sym()` — create a fresh object with a unique nominal identity.
- `sym(A)` — create a fresh object whose nominal parent is `A`.

This one primitive yields the entire nominal type story with **no separate
subtype system**:

```handl
Float  := sym(Number)
Float == Float      -- true:  same nominal identity
Float != Number     -- true:  distinct identities
Number instanceof Obj
Float  instanceof Number   -- runtime: parent chain walk
-- static: Float <: Number  -- same parent chain, read statically
```

Runtime `instanceof` and the static `<:` relation are the *same walk over the
same nominal structure*. There is no parallel abstract type hierarchy and no
`Subtype<A, B>` subsystem: the `sym` parent graph *is* the subtyping relation.

### 3. Arrows as the fundamental computational primitive

The only computational form is the arrow `A ~> B`. Arrows are objects: they
are serializable, inspectable, composable, cacheable, and distributable. There
is **no lambda calculus**: no closures, no lexical environments, no mutable
bindings, no hidden captured state. A program is an explicit arrow graph, and
that is exactly what makes computation a value:

- store it, transmit it over the wire, inspect it, transform it,
- cache it, replay it, optimize it, distribute it.

### 4. No binding model

Parameter lists are product projections, not closures:

```handl
(x, y) ~> x      -- product projection of the first component
```

`(x, y) ~> x` is `fst` applied to a product. Nothing is captured; there is no
environment. This supersedes the `make_closure`/`bind`/`lookup` scope-stack
model in `crates/handl-runtime/src/vm_primitives.rs`: those primitives remain
bootstrap scaffolding but are not part of the kernel's semantics.

### 5. No mutation

State is data, threaded explicitly through arrows:

```handl
State ~> (State, Result)
State -[IO]~> (State, Result)
```

There is no mutation model, no assignment, no hidden store. A stateful
computation is an ordinary arrow whose output includes the next state.

### 6. Products

Products are ordinary objects:

```handl
(Int, Int)
Point := (Int, Int)
Grade := (Usage, Effect, Permission, Cost)
```

`Grade` is just a product of four objects — the same objects that appear as
grades in §10.

### 7. Composition and `do`

`>>>` is the single composition operator. Block syntax lowers to composition:

```handl
do { read(); parse(); validate(); save() }
-- lowers to:
read >>> parse >>> validate >>> save
```

Control flow is composition plus choice/loop (ADR-0052's `choice`/`loop`
combinators), not a statement language.

### 8. Dimensions `<>` — specialization over anything

Angle brackets are the single dimension mechanism. The dimension parameter can
be anything in the one world — a type, a value, an arrow, a proof, a grade, an
algebra, a composition:

```handl
identity<T> : T ~> T
identity<Int>(42)

f<(a >>> b)>        -- dimension is a composition
(f<a>) >>> b        -- dimension application binds tighter than >>>

map<T, U>
```

`f<...>` is ordinary application at the dimension level; `>>>` composes arrow
results. There is no separate generics system.

### 9. Generic objects via the same machinery

```handl
List<T> := T ~> sym()
List<Int> == List<Int>
List<Int> != List<Number>
```

A generic object is an arrow from the dimension parameter to a fresh symbol.
Equality of instantiations is equality of the resulting nominal identities —
the same `sym` discipline as §2, applied to generics.

### 10. Graded arrows

Grades annotate arrows and are themselves objects:

```handl
A -[R]~> B
Usage, Effect, Permission, Cost        -- grade objects
gradeCompose : (R, R) ~> R
```

Grades compose with `gradeCompose` and obey grade laws. This is the same grade
machinery as ADR-0102 (where session types are grades) — here it is grounded:
a grade is an object in the one world, so it is serializable, comparable, and
specializable like everything else.

### 11. Algebra, capabilities, ordering — no interface/trait/impl

Algebras are ordinary objects/arrows; capabilities are object/arrow structures:

```handl
Semiring                        -- an object: carrier + ops
NatSemiring                     -- an instance, built from objects

Ordering := (Less, Equal, Greater)
Comparable<T> := (T, T) ~> Ordering
```

There are no `interface`, `trait`, or `impl` constructs. A "trait instance" is
an object; a "method" is an arrow inside it. This concretizes ADR-0075's
capability discipline: a capability is a value you hold, pass, and lose.

### 12. Proofs and refinements as objects

```handl
x : Int
p : x > 0                      -- a proof object
positive<x, p> : Positive(Int)
```

A proof is an object; a refinement is a dimension parameterization carrying a
proof. There is no separate proof language, no separate refinement checker —
the same `<>` machinery and the same object world.

### 13. Dependent types as ordinary arrows

```handl
Vector : Nat ~> Type
buffer<N> : Vector(N)
buffer<3>
```

A dependent type is an arrow into `Type`; instantiation is dimension
application. No dependent-type subsystem — just arrows whose codomain is `Type`.

### 14. Polymorphism and Liskov via `sym`

The kernel distinguishes two moves that superficial syntax often conflates:

- `Float ~> Number` — **conversion**: an arrow from a `Float` to its parent
  type. Compositional, lossy or lossless per the arrow's definition.
- `Float := sym(Number)` — **nominal inheritance**: `Float` *is a* fresh
  identity whose parent is `Number`. Powers `instanceof`, `<:`, and Liskov
  substitution — a `Float` may be used wherever a `Number` is expected.

Polymorphism then falls out: writing code against `Number` accepts `Float`,
`Int`, or any other `sym(Number)` child.

### 15. Higher-order arrows

```handl
map<T, U> : (T ~> U) ~> (List(T) ~> List(U))
map<Int, String>(toString)
```

Arrows take arrows as dimension parameters. First-class function is not a
feature; it is the default.

### 16. Combinators

The usual arrow combinators are ordinary arrows in the world:

```handl
duplicate     : Int ~> (Int, Int)
both          := increment &&& double
pairTransform := increment *** double
```

### 17. Expected-type inference

The single-world type story makes bidirectional checking trivial:

```handl
x : Number := f()
```

The expected type `Number` is an object; `f()` is checked against it by the
same parent-chain walk that powers `<:` and `instanceof`.

### 18. Runtime/static convergence

One nominal structure (the `sym` parent graph) powers:

- runtime `instanceof`,
- static subtyping `<:`,
- expected-type checking,
- generic instantiation equality (`List<Int> == List<Int>`).

There is no parallel abstract type hierarchy, no reflection layer distinct
from the value layer, no "type universe" separate from the object world.

## The conceptual kernel

The entire language derives from ten forms:

| Kernel form  | Meaning                                     |
| ------------ | ------------------------------------------- |
| `Obj`        | the world                                   |
| `Type`       | `Type : Obj := sym()` — types are objects   |
| `sym()`      | fresh nominal identity                      |
| `sym(A)`     | fresh identity with parent `A`              |
| `A ~> B`     | arrow                                       |
| `a >>> b`    | composition                                 |
| `f(x)`       | application                                 |
| `f<...>`     | dimension specialization                    |
| `(A, B)`     | product                                     |
| `A -[R]~> B` | graded arrow                                |

And from these, everything else derives:

- **values** — objects; **types** — objects (`Type : Obj`);
- **nominal identity, inheritance, instanceof, subtyping** — the `sym` parent
  graph;
- **polymorphism, generics, higher-order** — dimension specialization `<...>`;
- **products, records, tuples** — products;
- **dependent types** — arrows into `Type`;
- **proofs, refinements** — proof objects carried in dimensions;
- **algebra, capabilities** — object/arrow structures;
- **effects, grades, state, control flow** — graded arrows over explicit
  state (`State ~> (State, Result)`) plus `choice`/`loop`.

## What the language does NOT have

- No lambda calculus.
- No function/object distinction — arrows are objects.
- No closures, no lexical environment, no captured state.
- No mutable binding, no mutation model.
- No special type universe — `Type : Obj`.
- No special subtype / interface / trait / impl systems — `sym(A)` and object
  structures.
- No special effect language, proof language, algebra system, or
  dependent-type language — all are objects, arrows, and dimensions.

## Architecture

```
                        ┌─────────────────────────────┐
                        │            OBJECT            │
                        │      (everything is Obj)     │
                        └──────────────┬──────────────┘
                                       │
             ┌─────────────────────────┴──────────────────────────┐
             │ sym() / sym(A)                                     │
             │  │                                                  │
             │  ├─► fresh nominal identity                         │
             │  │     │                                            │
             │  │     └─► sym(Number)  ─► VM inheritance chain     │
             │  │            │                       │             │
             │  │            │        ┌──────────────┴───────────┐ │
             │  │            │        ▼                          ▼ │
             │  │            │   runtime instanceof        static <:
             │  │            └────────────►  polymorphism / Liskov │
             │  ▼                                                  │
             │  ARROW  (arrows are objects too)                    │
             │   │                                                 │
             │   ├─► A ~> B                                        │
             │   │     ├─► >>>  composition (do-blocks lower here) │
             │   │     ├─► f(x) application                        │
             │   │     └─► A -[R]~> B   graded arrows              │
             │   └─► f<...> dimensions (types, arrows, proofs,     │
             │           grades, algebras, compositions)           │
             └─────────────────────────────────────────────────────┘
```

One world: `sym()` builds the nominal spine, arrows do all the computing, and
every feature above (generics, dependent types, proofs, effects, algebra,
capabilities) is an arrangement of these two.

## Consequences

### Positive

- **One mechanism instead of many.** Subtyping, instanceof, generics,
  dependent types, proofs, effects, and capabilities all reduce to `sym`,
  arrows, products, and dimensions. Each new feature is library work, not a
  new subsystem.
- **Computation is data.** Programs are serializable arrow graphs: cacheable,
  replayable, optimizable, distributable — consistent with ADR-0052 and
  ADR-0103.
- **No hidden state.** No closures or mutable environments means no aliasing
  surprises, trivially inspectable programs, and sound reasoning about
  resource usage (grades, ADR-0102).
- **Runtime/static convergence.** One nominal structure powers both worlds,
  eliminating a whole class of drift bugs between checker and VM.
- **Grades and capabilities become real values**, letting ADR-0102's session
  grades and ADR-0075's capability discipline compose like ordinary data.

### Negative

- **Bootstrap discontinuity.** `crates/handl-runtime/src/vm_primitives.rs`
  currently implements `make_closure`/`bind`/`lookup`; the kernel removes
  their semantic role, so the VM instruction set and the exec model
  (`exec_model.hnl`, ADR-0103) must be migrated.
- **`sym()` mutates the global identity space.** Fresh symbols are
  inherently serializable but identity equality across processes/restarts
  needs a stable naming or session-identity story (follow-up ADR).
- **Performance of explicit state threading.** `State ~> (State, Result)`
  must be optimized aggressively (deforestation of state pairs) to match
  imperative performance.
- **Surface syntax design pressure.** Everything rides on `<...>`, `~>`,
  `>>>`, and `-[R]~>`; precedence rules (e.g. `f<(a >>> b)>` vs
  `(f<a>) >>> b`) must be pinned early and documented.
- **Migration of `type.hnl`.** `Int`/`Number`/`Float` are currently tagged
  value objects; they must become `sym`-parented identities
  (`Float := sym(Number)`), which changes equality semantics for every
  consumer of the `type` package.

## Phase 4 conformance note (graded arrows and dimensions)

Verified against the current tree while implementing the nominal-subtyping
rule. Two surface deviations from the conceptual notation in this ADR are
recorded here as known gaps; neither changes the semantics.

- **Graded arrows.** This ADR writes the graded arrow as `A -[R]~> B`.
  The canonical grammar instead parses the ternary graded arrow `A ~G~> B`
  (`grammar.graded_arrow.infix` in `handl-kit-data-canonical/data/grammar.yaml`),
  with `grade`/`sort`/`grades` declarations carrying the algebra
  (ADR-0098/0100, ADR-0102; see `crates/kit-canonical/tests/graded_theory.rs`
  and `crates/plugin-engine-exec-tree-walk/tests/graded_e2e.rs`). The grade is
  still an object and grade composition is still an arrow, as required — only
  the bracket notation differs. TODO(ADR-0106): either add `-[R]~>` as sugar
  for `~R~>` or amend this ADR's notation to match the canonical grammar.
- **Dimensions.** `f<...>` specialization parses through the existing generic
  call rules (ADR-0060) and is not yet restricted to "anything" (values,
  arrows, proofs, grades, algebras) — the accepted dimension payloads are the
  ones the generic call dispatcher understands today. TODO(ADR-0106): widen
  dimension payloads and add explicit tests for `f<(a >>> b)>` versus
  `(f<a>) >>> b`.

Nominal subtyping itself (this ADR's §14) is implemented: `<:` is decided by
the `sym` parent chain in `check_subtype` (crate
`handl-plugin-engine-typecheck-nbe`), and runtime `instanceof` walks the same
chain (`sym.instanceof`, `VmPrimitivesModule` in `handl-runtime`), with a
legacy `__tag__` bridge until `type.hnl` finishes migrating to sym-parented
identities.

## Implementation notes

- `crates/handl-runtime/src/vm_primitives.rs` — today's VM primitives host
  functions (`push_scope`, `bind`, `make_closure`, `pattern_match`, `ffi`,
  `ctx`). The `sym`/arrow kernel replaces the scope-stack and closure
  primitives; `sym()`/`sym(A)` become the new irreducible identity
  instructions alongside `call`/`ffi`.
- `crates/handl-value` — the value model. Objects gain a nominal identity
  field (the `sym` parent chain) while remaining serializable tagged data.
- `crates/handl-language` — lexer/parser/desugar. Surface forms to parse and
  lower: `sym()`, `sym(A)`, `A ~> B`, `A -[R]~> B`, `f<...>`, `(A, B)`,
  `>>>`, and `do { ... }` lowering to `>>>` chains.
- `handl-packages/type/type.hnl` — type-as-values. `Int`, `Str`, `Bool`,
  `Null`, `Float`, `Number`, `Nat` are currently tagged objects
  (`{ "__tag__": "int", ... }`). They become `sym`-parented objects
  (`Float := sym(Number)`, `Int := sym(Number)`), which is what turns the
  package's kind predicates into real nominal `instanceof`/`<:` checks.

## References

- ADR-0052 — arrow calculus kernel (`arr`/`>>>`/`first`/`choice`/`loop`), the
  computational substrate this ADR grounds.
- ADR-0101 — frontier calculus `Term` variants and 2-cell/grade machinery.
- ADR-0102 — session types as grade morphisms; grades as first-class objects.
- ADR-0103 — Handl as its own metalanguage; engines as Handl arrows over the
  same kernel.
- ADR-0075 — capability discipline at the language level; capabilities as
  held object/arrow structures.





