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

Nominal subtyping itself (this ADR's §14) is implemented in the Handl-authored
typecheck model, `project/design/adr0103/typecheck_model.hnl` (crate
`handl-plugin-engine-typecheck-handl`, loaded via `include_str!`): the
`subtype` judgment op reads the `sym` parent chain (`sym_chain`) and succeeds
iff the sub chain contains the sup's own handle, falling back to unification
for legacy `__tag__` values. Runtime `instanceof` walks the same chain
(`sym.instanceof`, `VmPrimitivesModule` in `handl-runtime`), with a legacy
`__tag__` bridge until `type.hnl` finishes migrating to sym-parented
identities. Note: the original Rust implementation lived in the crate
`handl-plugin-engine-typecheck-nbe`; that crate was deleted on `main` during
integration (superseded by the Handl model per ADR-0105), so the rule now
lives in the model, not in any `*-nbe` crate.

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






## Phase 5 conformance note (no closures / serializable arrows)

This note records the state of the §6/§8 invariant — *arrows are objects with
no closures, no lexical environments, and no hidden captured state* — as
verified against the current tree (branch `design/object-arrow-kernel`). **The
invariant does not hold yet.** Arrows remain closures over a captured
environment and an AST index whose meaning is process-local. This section is
factual: it lists the hidden state and what must change. It does **not** claim
conformance.

### Audit — every place an arrow/function value carries hidden captured state

Value layer (`crates/handl-value/src/lib.rs`):

- `TaggedValue::Arrow` — `lib.rs:345-357`. Fields: `params: Rc<[String]>`,
  `rest: Option<Rc<str>>`, `body: usize`, `env: Rc<FxHashMap<Rc<str>,
  TaggedValue>>`, `home: Option<u64>`, `array_pattern: bool`.
  - `env` (`:353`) is the captured lexical environment — the load-bearing
    hidden state. It **is** JSON-serialized (as `"env"`), but its *contents*
    can be non-serializable (see `Resource` below), and it exists only to
    support closure capture, which §3 forbids.
  - `body` (`:352`) is an **AST index** (`AstRef`). It serializes as a bare
    integer, which is not sufficient to reconstruct the arrow: the AST it
    indexes is not in the value.
  - `home` (`:354`) is stamped when an arrow crosses a runtime boundary. It
    serializes as `"__home__"` (`:750-752`), but the *frame it names* lives in
    a process-local table (below).
- JSON encoders (`lib.rs:729-757`): always emit `__arrow__`, `params`,
  `body`, `env`; emit `rest`, `__home__`, `array_pattern` only when set. The
  shape itself is serializable data.
- JSON decoders (`lib.rs:576-610`): reconstruct the `Arrow` variant; `home`
  from `"__home__"` (`:597`).
- `TaggedValue::Resource` (`lib.rs:358-365`): JSON encoding
  `{"__resource__": {"ptr": <addr>}}` (`:758-764`) is **debug-only** — the
  finalizer (a function pointer) is dropped, and decoding yields
  `finalizer = None` (`:611-623`). A `Resource` captured in an arrow's `env`
  therefore round-trips **lossily** (pointer preserved, finalizer lost). Not
  serializable as-is.

Tree-walk engine (`crates/plugin-engine-exec-tree-walk/src/lib.rs`):

- Thread-local eval state: `CURRENT_AST`, `CURRENT_LANG`, `CURRENT_MODULES`,
  `CURRENT_GLOBALS` and `HOME_FRAMES` (`:120-134`). `HOME_FRAMES` is a
  `HashMap<u64, EvalFrame>` — the process-local home-frame registry.
- `EvalFrame { ast: Rc<Ast>, lang: Rc<Language>, modules: Option<Rc<dyn
  ModuleLookup>>, globals: Option<Rc<FxHashMap<Rc<str>, TaggedValue>>> }`
  (`:152-162`). This is the state a `__home__` id resolves to; it is **not**
  serializable and **not** part of the arrow value. `home_frame` /
  `register_frame_at` (`:189-200`).
- `ScopeStack { scopes: Vec<FxHashMap<Rc<str>, TaggedValue>>, captured:
  Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>, globals: Rc<FxHashMap<...>> }`
  (`:291-302`), with `lookup` (`:338`), `define` (`:352`), `assign` (`:373`),
  `push_scope` (`:394`), `pop_scope` (`:399`), `capture` (`:417`). This is the
  mutable scope stack the closure substrate runs on.
- Closure mint (`:2328-2336`): `TaggedValue::Arrow { …, env: captured,
  home: None, … }`. Application threads `captured_env` back in
  (`apply_app`, `:3834-3975`; `:4007-4016`).


Bytecode engine (`crates/plugin-engine-exec-bytecode/src/vm.rs`):

- `ScopeStack` mirror (`:133-160`): `captured_root:
  Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>` (`:143`), `globals`
  (`:148`), `lookup` (`:180`), `push_scope` (`:232`), `pop_scope` (`:239`),
  `capture` (`:256`). Verbatim scope-stack + captured-env state.

Cranelift engine (`crates/plugin-engine-exec-cranelift/src/lib.rs`):

- `ScopeStack { scopes: Vec<Rc<FxHashMap<Rc<str>, TaggedValue>>>, … }`
  (`:613-637`), with `lookup` (`:643`), `push_scope` (`:695`), `pop_scope`
  (`:700`). Same captured-scope state.

Runtime VM primitives (`crates/handl-runtime/src/vm_primitives.rs`):

- `VmState { scopes: Vec<HashMap<String, Value>>, context }` behind
  `Arc<Mutex<..>>` (`:45-53`) — a host-side scope stack.
- `PushScopeFn` (`:72-78`); `MakeClosureFn` (`:141-156`) mints a
  `{"__closure__": true, "params", "body", "env"}` record — hidden capture as
  a JSON record; `CallFn` (`:165-204`) dispatches arrow values via
  `ctx.try_call_value`.

Boundary helpers:

- `crates/handl-language/src/lib.rs:303-319` reserves `__arrow__` and
  `__home__` as value keys (the hidden-state encodings leak into the value
  namespace).
- `crates/plugin-lib-handl/src/lib.rs:884-893` (`contains_unhomed_arrow`):
  arrows crossing a `serde_json` boundary are stamped with `__home__` so the
  receiving engine can route application back — evidence that the arrow value
  alone is insufficient to apply it.

### What is currently true (fenced by tests)

`crates/handl-runtime/tests/arrow_serializability.rs` (8 tests, all green)
pins the parts that *are* checkable through public APIs:

- the arrow JSON key universe is exactly `__arrow__`, `params`, `body`,
  `env`, `rest`, `__home__`, `array_pattern`; the bare arrow emits exactly
  `__arrow__`, `params`, `body`, `env` (silent additions fail loudly);
- an empty-`env` arrow round-trips JSON -> value -> JSON unchanged
  (idempotent);
- an `env`-captured nested arrow carrying a `home` id round-trips
  **exactly** at the value level (the id is data) — but the frame it names is
  not serialized;
- an `env`-captured `Resource` round-trips **lossily** (pointer kept,
  finalizer dropped; the decoded resource never finalizes);
- a composed arrow, represented as the `Seq` term
  (`{"__tag__": "Seq", "fst", "snd"}`), carries only its two components;
- the arrow the tree-walk engine mints for `x ~> x` uses only documented keys.

### TODO(ADR-0106) — what must change for §6/§8 to hold

1. **Drop `env` from the arrow value.** `TaggedValue::Arrow.env`
   (`handl-value/src/lib.rs:353`) and its encoder/decoder (`:729-757`,
   `:576-610`) must go; arrow inputs/outputs become explicit object fields.
   Consumers: tree-walk closure mint/apply
   (`plugin-engine-exec-tree-walk/src/lib.rs:2328-2336`, `:3834-3975`,
   `:4007-4016`), bytecode scope stack
   (`plugin-engine-exec-bytecode/src/vm.rs:133-256`), cranelift scope stack
   (`plugin-engine-exec-cranelift/src/lib.rs:613-700`), and the runtime VM
   primitives (`handl-runtime/src/vm_primitives.rs:45-78`, `:141-204`).
2. **Replace the `body` AST index with a serializable arrow graph.** Remove
   the process-local home-frame table
   (`plugin-engine-exec-tree-walk/src/lib.rs:120-134`, `:152-162`,
   `:189-200`) and the `home`/`__home__` mechanism
   (`handl-value/src/lib.rs:354`, `:750-752`;
   `handl-language/src/lib.rs:318-319`;
   `plugin-lib-handl/src/lib.rs:884-893`), so an arrow's body is data that
   survives a round-trip.
3. **Eliminate the scope stacks.** `push_scope`/`pop_scope`/`bind`/`lookup`/
   `capture` in all three engines (tree-walk `:291-403`; bytecode
   `vm.rs:133-256`; cranelift `lib.rs:613-700`) must be deleted in favour of
   explicit arrow-graph substitution, as must the
   `MakeClosureFn`/`CallFn` host primitives
   (`handl-runtime/src/vm_primitives.rs:141-204`).
4. **Close the `Resource`-in-`env` leak.** Either make arrow `env` incapable
   of holding non-serializable values (encode-time error) or carry opaque
   handles out-of-band; today they serialize as a finalizer-less pointer
   (`handl-value/src/lib.rs:758-764`) and leak on round-trip.
5. **Make `>>>` compose.** Surface `>>>` is currently *application*, not
   composition (`handl-kit-data-canonical/data/types.yaml:2158-2166`); the
   ADR-0106 §3 composed-arrow value does not exist yet, so "a composed arrow
   has no hidden state beyond its components" is only fenced for the `Seq`
   term. TODO: lower `>>>` to composition and add composed-arrow tests.


## Design: serializable arrow representation (item 2)

This section specifies the concrete representation that makes an arrow
**self-contained and serializable** — closing the gap the Phase 5 note
documents — and the migration order to get there. It is written against the
current tree and cites `file:line`. Only step 1 has been implemented so far
(see the "current truth" note at the end).

### Problem, precisely

An arrow today is `TaggedValue::Arrow { params, rest, body: usize, env:
Rc<FxHashMap<..>>, home: Option<u64>, array_pattern }`
(`crates/handl-value/src/lib.rs:345-357`). Two of its fields are meaningless
outside the process that minted it:

- `body: usize` is an `AstRef` index (`crates/handl-language/src/ast.rs:68`)
  into the producing engine's arena. The arena is never serialized with the
  arrow, so a decoded arrow's `body` has no target.
- `home: Option<u64>` names a frame in `HOME_FRAMES`, a process-local table
  (`crates/plugin-engine-exec-tree-walk/src/lib.rs:134`, `:152-162`). The
  frame (AST + `Language` + module chain) is not in the value.

`env` is the third piece of hidden state, but it is a *semantic* problem
(ADR-0106 §3/§4 forbid closures) rather than a *serializability* one, so the
design below replaces only the body representation first and leaves `env`
elimination to the larger migration (TODO items 1/3).

### The representation

Add one optional field to the arrow variant:

```rust
Arrow {
    params: Rc<[String]>,
    rest: Option<Rc<str>>,
    body: usize,                       // fallback: ambient/home AST index
    env: Rc<FxHashMap<Rc<str>, TaggedValue>>, // fallback (unchanged)
    home: Option<u64>,                 // fallback (unchanged)
    array_pattern: bool,
    body_def: Option<Rc<TaggedValue>>, // NEW: the body as explicit data
}
```

`body_def` carries the arrow's body as **explicit, serializable structure that
lives in the value**. It holds the *existing* language-level AST-value
encoding — a `TaggedValue::Obj` `{"__ast__": <serialized handl_language::Ast>}`
— the same shape `handl_language::ast_value` / `as_ast` already define for
vau-bound unevaluated expressions (`crates/handl-language/src/lib.rs:196-226`,
`crates/handl-language/src/lib.rs:208-211`). The AST fragment is produced with
the existing `Ast::subtree(root)` extractor
(`crates/handl-language/src/ast.rs:168-181`), which copies the body node plus
its transitively-referenced children into a fresh standalone arena.

Why this shape and not a new type in `handl-value`:

- `handl-value` is a low-level data crate with no dependency on
  `handl-language`; `body_def` is therefore typed as an *opaque*
  `TaggedValue`, not as an `Ast`. The `__ast__` convention is interpreted only
  by the engines (which do depend on `handl-language`), keeping the layering.
- `{"__ast__": <Ast>}` is already the serializable form of an expression the
  platform trusts and round-trips (`handl-language/src/lib.rs:196-226`), so no
  new serializer is introduced.
- `Option` keeps the change additive: an arrow with `body_def: None` behaves
  exactly as today (the `body`/`env`/`home` fallback).
- `body_def` is **derived data** (the same body, pre-extracted into a
  standalone arena), so it is not part of the value's *identity*: `eq` and
  `Hash` (`crates/handl-value/src/lib.rs:456-481`) both ignore it. Two arrows
  whose `params`/`rest`/`body`/`env`/`home`/`array_pattern` agree compare equal
  and hash identically whether or not either carries a definition — which also
  keeps the `Hash`/`Eq` contract intact.

### JSON shape

The `__arrow__` object gains exactly one optional key, emitted only when
`body_def` is `Some`:

```json
{
  "__arrow__": true,
  "params": ["x"],
  "body": 0,
  "env": {},
  "body_def": { "__ast__": { "nodes": [ { "kind": "…", "span": null, "payload": { … } } ] } }
}
```

- Encoder: `crates/handl-value/src/lib.rs:741-756` (add a `body_def` branch
  after `array_pattern`).
- Decoder: `crates/handl-value/src/lib.rs:576-610` (`o.get("body_def")`).
- The existing key universe is documented in the fence test
  (`crates/handl-runtime/tests/arrow_serializability.rs:57-65`) and must gain
  `body_def`.

### Call sites

**Mint** (arrow-value construction):

- tree-walk `crates/plugin-engine-exec-tree-walk/src/lib.rs:2329-2336`
  (`eval_inner_td`, `"closure"` arm) — the canonical mint. Here `ast` and the
  body index are in scope, so this site populates `body_def` via
  `ast.subtree(AstRef::new(body))` + `ast_value`.
- tree-walk `:3891-3905` / `:3929-3936` (`apply_app` re-entry),
  `:6117`/`:6122`: `body_def: None` (they re-wrap an existing body in the same
  arena — no new definition).
- tree-walk `:3308`, `:3390` (builtin arrow values): `body_def: None`.
- bytecode `crates/plugin-engine-exec-bytecode/src/vm.rs:1096`, `:1319`,
  `:3030`, `:3066`, `:752`, `:793`: `body_def: None` for now (the body there
  is an instruction index, not an AST node — a later step).
- cranelift `crates/plugin-engine-exec-cranelift/src/lib.rs:3843`, `:1949`,
  `:2542`: `body_def: None` for now (same reason).
- test/helper literals: `handl-value/src/lib.rs:1157,1165`,
  `handl-runtime/tests/arrow_serializability.rs:222`.

**Apply** (arrow-value consumption):

- tree-walk `apply_app` (`crates/plugin-engine-exec-tree-walk/src/lib.rs:3881`)
  and `apply_arrow_body` (`:4058`). The apply path currently derives
  `(ast, body_idx)` from the ambient arena or from `HOME_FRAMES[home]`
  (`:3912-3944`). A later step makes it **prefer** `body_def` when present:
  deserialize via `handl_language::as_ast`, then call `apply_arrow_body` with
  the standalone arena and root `AstRef::new(len-1)` (the Pratt post-order
  convention, `handl-language/src/ast.rs:156-181`).
- bytecode `vm.rs:1828`, `:2259` and cranelift `lib.rs:2857`, `:4623` consume
  arrows for `Choice`/predicate dispatch only; unaffected by step 1.

### Migration order

1. **(this item, done)** Value-layer field + encoder/decoder + fence tests;
   populate `body_def` at the tree-walk `"closure"` mint; apply path
   **unchanged** (still uses `body`/`env`/`home`). Nothing regresses.
2. Teach tree-walk `apply_app` to prefer `body_def` when present, falling back
   to `body`/`home`. This requires **(2a)** first: extend `Ast::subtree`
   (`crates/handl-language/src/ast.rs:168`) to remap *every* arena reference
   in a payload, not only the `children` array — today it remaps `children`
   only (`:197-201`), so nested absolute refs (a nested `closure` node's
   `payload["body"]`, an `extend_env` node's `arena_idx`) still point into the
   original arena. Until (2a) lands, `body_def` is a faithful copy for
   leaf-only bodies but not a complete standalone arena for every body, so it
   must **not** drive application.
3. Populate `body_def` at the bytecode and cranelift mint sites (requires an
   AST-level body representation there; the instruction index is not one).
4. Drop `env` and `home`/`__home__` (TODO items 1/2/3 of the Phase 5 note),
   leaving `body_def` (plus a serialized `Language` reference) as the arrow's
   whole definitional content.

### Known limitation recorded for step 2

`Ast::subtree` (`crates/handl-language/src/ast.rs:184-210`) remaps only
`payload["children"]`. Any payload field that carries an arena index under
another name (nested closure `body`; `extend_env` `arena_idx`) is copied
verbatim, so the extracted arena's such refs still denote the *original*
arena. That is acceptable for the meta `eval` use case the extractor was
written for, but it means step 1's `body_def` is a **structurally faithful,
self-contained body for bodies with no nested arena refs** and a partial graph
otherwise. Closing this is step (2a).

