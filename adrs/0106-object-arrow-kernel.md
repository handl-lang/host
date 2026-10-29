---
title: The Object/Arrow Kernel — `sym()`, `Obj`, and Native Arrow Calculus
type: adr
date: 2026-09-11
status: accepted
status_note: |-
  Accepted. The kernel design (one object world, `sym()`/`sym(A)` nominal
  identity, native arrow calculus, products, `>>>`, `~[R]~>`, `f<...>`) is the
  normative design. Implementation status is tracked in the conformance notes
  below, and is **partial** — each note states precisely what holds and what
  remains:

  - `sym()`/`sym(A)` and `instanceof`: implemented, with nominal `<:` decided by
    the parent chain (Phase 4); the legacy `__tag__` bridge remains until
    `type.hnl` finishes migrating.
  - Serializable arrows: implemented. `TaggedValue::Arrow` is now
    `{params, rest, array_pattern, body_def, binds, handlers}` — `body`,
    `env`, and `home` are gone from the value. All three engines mint and
    apply the self-contained form, and a JSON-round-tripped arrow remains
    applicable. See the "Current truth" note in the representation section.
  - Per-call argument binding (env-removal for the `HandlFunction::call` seam):
    implemented — `Runtime::eval_shared_with_call_args` binds `__arg_N` directly
    on the call frame's `ScopeStack` globals (excluded from `capture()`), instead
    of injecting per-call engine globals via `with_global` cloning (zero engine
    clones per call).
  - Graded arrows `A ~[R]~> B`: implemented (canonical spelling; legacy
    `~R~>` accepted).
  - Dimensions `f<...>`: syntax implemented; specialization semantics remain.

  Accepted as the design of record; the open TODOs are implementation work, not
  unresolved design questions.
---

# ADR-0106: The Object/Arrow Kernel — `sym()`, `Obj`, and Native Arrow Calculus

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

Runtime `instanceof` and the static `<:` relation are the _same walk over the
same nominal structure_. There is no parallel abstract type hierarchy and no
`Subtype<A, B>` subsystem: the `sym` parent graph _is_ the subtyping relation.

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
A ~[R]~> B
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
- `Float := sym(Number)` — **nominal inheritance**: `Float` _is a_ fresh
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

| Kernel form  | Meaning                                   |
| ------------ | ----------------------------------------- |
| `Obj`        | the world                                 |
| `Type`       | `Type : Obj := sym()` — types are objects |
| `sym()`      | fresh nominal identity                    |
| `sym(A)`     | fresh identity with parent `A`            |
| `A ~> B`     | arrow                                     |
| `a >>> b`    | composition                               |
| `f(x)`       | application                               |
| `f<...>`     | dimension specialization                  |
| `(A, B)`     | product                                   |
| `A ~[R]~> B` | graded arrow                              |

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
             │   │     └─► A ~[R]~> B   graded arrows              │
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
  `>>>`, and `~[R]~>`; precedence rules (e.g. `f<(a >>> b)>` vs
  `(f<a>) >>> b`) must be pinned early and documented.
- **Migration of `type.hnl`.** `Int`/`Number`/`Float` are currently tagged
  value objects; they must become `sym`-parented identities
  (`Float := sym(Number)`), which changes equality semantics for every
  consumer of the `type` package.

## Phase 4 conformance note (graded arrows and dimensions)

Verified against the current tree while implementing the nominal-subtyping
rule.

- **Graded arrows — conformant.** The canonical surface is the bracketed
  ternary graded arrow

  ```
  A ~[R]~> B
  ```

  (the legacy shorthand `A ~R~> B` is still accepted). Both spellings parse
  to the same `graded_arrow` node in `crates/plugin-engine-parse-pratt`
  (which special-cases `~` because the declarative grammar cannot express an
  infix operator whose closing delimiter is a different token). The bracket
  holds the grade expression — the grade list for multi-grade arrows — and an
  empty list (`A ~[]~> B`) is rejected. Grade/sort declarations carry the
  algebra as before (ADR-0098/0100, ADR-0102; `graded_theory.rs`,
  `graded_e2e.rs`). ADR notation and implementation now agree.

- **Dimensions — syntax + lowering implemented; specialization is
  application.** The `f<...>` surface exists: the Pratt engine recognizes the
  bracketed form and produces a `dimension` node (children `[target, args…]`,
  shape `ast.dimension`). Because `<` is also the comparison operator, the
  bracket is only recognized on the unambiguous spelling: it must be
  **written adjacent** to a name-like target (`var`, `field-access`, `call`,
  `dimension`) and must close with a `>` at balanced depth — so `f < a`
  (spaced) and `f<a` (unclosed) remain comparisons, and `(f<a>) >>> b` is a
  flow whose left operand is the dimension. `f<>` is rejected; there is no
  `>>` split to worry about (the lexer has no shift token, so `f<g<x>>`
  lexes as two `>` and nests naturally).

  A dimension **evaluates as application**: the desugar pass lowers
  `dimension` to a `call` (`transform.desugar.dimension`, children preserved),
  so `f<X>` means `f(X)` — exactly the reading §12 gives the generic form
  (`List<T> := T ~> sym()`), and it reuses the proven `call` machinery
  (host dispatch + arrow application) rather than a parallel path. An
  `eval.dimension` scheme with identical semantics also exists for paths that
  evaluate a raw (undesugared) AST. Evidence: `graded_theory.rs` asserts
  `inc<41>` evaluates to `42` and that `inc<41>` / `add<1, 2>` evaluate
  _identically_ to `inc(41)` / `add(1, 2)`.

  TODO(ADR-0106): true _specialization semantics_ (a dimension selecting a
  type/value/grade instantiation rather than an argument, plus
  expected-type/dimension inference) are not implemented; the payload is
  parsed as a general expression and applied.

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
  lower: `sym()`, `sym(A)`, `A ~> B`, `A ~[R]~> B`, `f<...>`, `(A, B)`,
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

This note records the state of the §6/§8 invariant — _arrows are objects with
no closures, no lexical environments, and no hidden captured state_ — as
verified against the tree. **The invariant is partially satisfied.** As of the
"serializable arrow representation (item 2)" work, an arrow minted by the
tree-walk engine carries its body as explicit, serializable structure
(`body_def`) and is applicable after a JSON round-trip without its minting
frame (proven end-to-end by `round_tripped_arrow_is_applicable_without_its_frame`).
What remains non-conformant: `env` is still a captured lexical environment,
and the bytecode/cranelift engines mint arrows whose body is an instruction
index rather than an explicit definition. This section is factual: it lists
the remaining hidden state and what must change. It does **not** claim full
conformance.

### Audit — every place an arrow/function value carries hidden captured state

Value layer (`crates/handl-value/src/lib.rs`):

- `TaggedValue::Arrow` — `lib.rs:345-357`. Fields: `params: Rc<[String]>`,
  `rest: Option<Rc<str>>`, `body: usize`, `env: Rc<FxHashMap<Rc<str>,
TaggedValue>>`, `home: Option<u64>`, `array_pattern: bool`.
  - `env` (`:353`) is the captured lexical environment — the load-bearing
    hidden state. It **is** JSON-serialized (as `"env"`), but its _contents_
    can be non-serializable (see `Resource` below), and it exists only to
    support closure capture, which §3 forbids.
  - `body` (`:352`) is an **AST index** (`AstRef`). It serializes as a bare
    integer, which is not sufficient to reconstruct the arrow: the AST it
    indexes is not in the value.
  - `home` (`:354`) is stamped when an arrow crosses a runtime boundary. It
    serializes as `"__home__"` (`:750-752`), but the _frame it names_ lives in
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
pins the parts that _are_ checkable through public APIs:

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

1. **(done)** `env` dropped from the arrow value. `TaggedValue::Arrow` no
   longer carries a captured lexical environment — see "Current truth"
   below for the shipped `body_def` + `binds` + `handlers` shape.
2. **(done)** The `body` AST index and the process-local home-frame table
   (`HOME_FRAMES`, `home`/`__home__`, `stamp_home`) are removed; an arrow's
   body is the serialized `body_def` arena and survives a round-trip.
3. **(partially done)** Scope stacks still exist as eval-time machinery in
   all three engines (`push_scope`/`pop_scope`/`bind`/`lookup`/`capture`),
   but they no longer back arrow _values_ — an arrow's visible bindings are
   its own `binds` map plus its call-time scopes. Fully eliminating the
   stacks in favour of pure arrow-graph substitution remains open; the
   `MakeClosureFn`/`CallFn` host primitives were replaced by per-call
   `__arg_N` binding on the frame's globals.
4. **(addressed at the boundary)** The `Resource` leak is no longer hidden
   in an `env`: resources sit in the explicit `binds` map and still encode
   as a finalizer-less `{"__resource__": {"ptr": …}}` — but
   `handl_value::check_portable` now rejects `__resource__` pointers (along
   with `__envref__` markers and dangling `__bindsref__` references) in any
   document that crosses a process/foreign boundary, so a leak can no
   longer silently cross the wire. In-process tagged-value round-trips
   remain self-contained. Portability is deliberately **not** achieved by
   making `Resource` a serializable value: a resource is a capability (a
   machine pointer plus finalizer), not data. The design direction is
   instead to model resource access as effect operations — an arrow
   performs `op`s (`zlib.write`, `zlib.close`, …) and a `handle`
   installation owns the concrete handles. What crosses a boundary is then
   ordinary data (op names, arguments, integer handle-ids); the live
   handle table lives in the *handler's dynamic state*, which is runtime
   machinery and is allowed to be non-portable — unlike values.
   Ownership/finalization moves off the value's `dropped` flag (whose
   independent per-copy flags can alias a freed pointer) into explicit
   handler state, so `close` is an op and aliasing is a handler logic bug
   rather than a memory-safety hole. `check_portable` rejecting
   `__resource__` stays correct permanently — a bare pointer should never
   be a wire value. `ForeignResource` remains the substrate-internal FFI
   representation.
5. **Make `>>>` compose.** Surface `>>>` is currently _application_, not
   composition (`handl-kit-data-canonical/data/types.yaml:2158-2166`); the
   ADR-0106 §3 composed-arrow value does not exist yet, so "a composed arrow
   has no hidden state beyond its components" is only fenced for the `Seq`
   term. TODO: lower `>>>` to composition and add composed-arrow tests.

## Design: serializable arrow representation (item 2)

This section specifies the concrete representation that makes an arrow
**self-contained and serializable** — closing the gap the Phase 5 note
documents — and the migration order to get there. It is written against the
current tree and cites `file:line`. Steps 1, 2a and 2 are implemented (see
the "current truth" note at the end); steps 3 and 4 remain.

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

`env` is the third piece of hidden state, but it is a _semantic_ problem
(ADR-0106 §3/§4 forbid closures) rather than a _serializability_ one, so the
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
lives in the value**. It holds the _existing_ language-level AST-value
encoding — a `TaggedValue::Obj` `{"__ast__": <serialized handl_language::Ast>}`
— the same shape `handl_language::ast_value` / `as_ast` already define for
vau-bound unevaluated expressions (`crates/handl-language/src/lib.rs:196-226`,
`crates/handl-language/src/lib.rs:208-211`). The AST fragment is produced with
the existing `Ast::subtree(root)` extractor
(`crates/handl-language/src/ast.rs:168-181`), which copies the body node plus
its transitively-referenced children into a fresh standalone arena.

Why this shape and not a new type in `handl-value`:

- `handl-value` is a low-level data crate with no dependency on
  `handl-language`; `body_def` is therefore typed as an _opaque_
  `TaggedValue`, not as an `Ast`. The `__ast__` convention is interpreted only
  by the engines (which do depend on `handl-language`), keeping the layering.
- `{"__ast__": <Ast>}` is already the serializable form of an expression the
  platform trusts and round-trips (`handl-language/src/lib.rs:196-226`), so no
  new serializer is introduced.
- `Option` keeps the change additive: an arrow with `body_def: None` behaves
  exactly as today (the `body`/`env`/`home` fallback).
- `body_def` is **derived data** (the same body, pre-extracted into a
  standalone arena), so it is not part of the value's _identity_: `eq` and
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

1. **(done)** Value-layer field + encoder/decoder + fence tests; populate
   `body_def` at the tree-walk `"closure"` mint.
2. **(done, with 2a)** `apply_app` now **prefers** `body_def` when the callee
   carries one: the standalone arena is deserialized with
   `handl_language::as_ast` and applied at its root (`len`-1, the post-order
   convention), with `home` treated as absent. `body`/`home` remain the
   fallback when there is no definition (bytecode/cranelift arrows,
   hand-built values) or when the definition fails to deserialize.
   **(2a) done**: `Ast::subtree` now remaps _named_ arena references as well
   as positional children, driven by the audited list
   [`ARENA_REF_PAYLOAD_KEYS`] (`crates/handl-language/src/ast.rs`), which
   currently contains `body`. Only in-range integer values are treated as
   references, so non-arena payload numbers are never rewritten and
   out-of-range values are left untouched. This makes the extracted arena
   genuinely standalone for any body.
3. **(done)** `body_def` is minted at all three engines' closure sites.
   Bytecode `Instr::Closure` and the shared `closure` op mint the
   substituted definition; cranelift applies `body_def` arenas by
   delegating to the tree-walk (`Cranelift` has no AST interpreter of its
   own — `body_def` evaluation is an interpretive act).
4. **(done)** `env` and `home`/`__home__` are gone from
   `TaggedValue::Arrow`. The representation that shipped differs from the
   additive plan above and supersedes it — see "Current truth".

### Current truth — self-contained arrows (steps 1–4 landed)

`TaggedValue::Arrow` is now exactly:

```rust
Arrow {
    params: Rc<[String]>,
    rest: Option<Rc<str>>,
    array_pattern: bool,
    body_def: Rc<TaggedValue>,                  // substituted body arena
    binds: Rc<FxHashMap<Rc<str>, TaggedValue>>, // residual bindings
    handlers: Rc<[TaggedValue]>,                // carried handlers
}
```

The mint (shared `closure` op, `plugin-engine-exec/src/native_ops.rs`, and
`Instr::Closure` in bytecode) collects the definition-site bindings via
`env_capture`, filters them to the body's free-variable candidates
(`subtree_name_candidates`), excludes the arrow's own parameter names, and
splits the rest two ways:

- **Substituted into `body_def`** (`Ast::instantiate`,
  `crates/handl-language/src/instantiate.rs`): arrow-free, serializable
  values are baked into the copied body arena as `lit` nodes. Substitution
  is binder-aware — it does not descend under shadowing `params`/`pattern`/
  `target`/`subject` roles — and literal payload positions
  (`field-access.field`, `entry.key`) are never touched.
- **Kept as residual `binds`**: active `eval_rec` binders (substituting a
  rec name would embed a stale current-pass value and break fixpoint
  convergence), `Resource`s (unserializable finalizers), and other arrows
  (substituting an arrow would expand its `binds`/`body_def` transitively
  at every mint — exponential on the helper-diamond graphs of real
  modules). `binds` shares the `Rc`, so nested arrows are O(1) to mint.

At apply time the engines install `binds` as the scope stack's
`captured_root`, bind the params, and evaluate the `body_def` arena —
memoized per definition value (`cached_def_arena`, keyed by `Rc` pointer).
Bytecode resolves `body_def` nodes through a per-application definition
arena slot; a lexical `floor` on the `ScopeStack` keeps a callee's
`lookup`/`assign` from reaching below the call boundary. `handlers` are
pushed onto the ambient handler stack for the body's duration.

The arrow is therefore **self-contained serializable data**: a JSON
round-trip preserves `params`/`body_def`/`binds`/`handlers` and the decoded
value applies identically — `round_tripped_arrow_is_applicable_without_its_frame`
(`crates/handl-runtime/tests/arrow_serializability.rs`) exercises exactly
this. `Eq`/`Hash` compare the full data payload (`params`, `body_def`,
`binds`, `handlers`, `rest`, `array_pattern`): an arrow's identity is its
data. `eval_rec`'s convergence check (`rec_value_eq`) compares only
`params` + `body_def`, since `binds` legitimately differ between fixpoint
passes.

Two honest edges remain:

- A `Resource` in `binds` still encodes as `{"__resource__": {"ptr": …}}`,
  but `check_portable` rejects such documents at process/foreign
  boundaries — the leak is explicit data that cannot silently cross the
  wire (TODO item 4).
- The engines still run scope stacks internally as evaluation machinery;
  they no longer back arrow _values_ (TODO item 3's value-level goal is
  met; deleting the stacks themselves is orthogonal).

**Capture reduction — landed.** The `closure` mint now filters the
definition-site bindings to the body's free-variable candidates
(`subtree_name_candidates`, a syntax-directed pass that finds every name a
subtree can reference — including names read through scheme ops like
`proj_env`/`lookup`, not just `var` nodes). `None` (unresolvable subtree)
keeps the full binding set: over-collection is safe, under-collection is
not. The regression oracle is
`import_module_with_many_closures_does_not_explode`
(`crates/kit-canonical/tests/import_export.rs`), which a module of ~30
sibling closures could not survive unfiltered.

**The pre-existing `fanout` bug behind an earlier attempt.** While checking
a prior capture-reduction attempt,
`handl-kit-canonical`'s `fp_integration::fanout_eager_pair_from_source` was
found to **stack-overflow** (`fanout_eager_pair_from_source`, and the same
class of abort in `fanout_with_expressions` — the whole `&&&` fanout path
recurses without a depth bound), and a bisect confirmed this is
**pre-existing**:
it aborts identically on clean `main` (`4dc13554`) in a detached worktree, with
none of this work applied. So the overflow is a real, unfixed bug in the
`fanout` evaluation path, and it also means the `handl-kit-canonical` suite is
**already red on main** (the abort kills the test binary), which made it a poor
oracle for judging the capture reduction.

**Fanout diagnosis recorded (follow-up).** A focused investigation of the
overflow (branch `doc/adr0106-fanout-diagnosis`-sibling work, since dropped)
established the following evidence:

- **It is a stack-threshold + huge-evaluation problem, not an infinite loop.**
  With the default 8 MiB thread stack the test binary aborts; with
  `RUST_MIN_STACK ≈ 128 MiB` it _passes_ in ~20 s and returns the correct
  `[1, 2]`. An instrumentation of every `eval_node_inner` visit showed the
  program terminating at the `fanout` node.
- **The cost is disproportionate.** The trivial source `1 &&& 2` drives on the
  order of **4 million AST-node visits** and recurses ~10⁵ frames deep — far
  beyond what two operands should require.
- **The host is wired correctly.** `fn.fanout_fn` is registered via
  `canonical_fp_host_functions` (`crates/kit-canonical/src/lib.rs`), and
  `&&&` has `eval.fanout` → `fn.fanout_fn(proj_child 0, proj_child 1)` with no
  desugar rule — so the blowup is _inside_ the eval path, not a rewrite miss.
- **Open question — loader vs evaluator.** The `kit()` build installs the
  self-hosted `HandlLoader` (which runs stdlib transforms on the **bytecode**
  engine), and the instrumented node mix (`match`/`if`/`index`/`binary.eq`)
  does not match a bare `1 &&& 2`, so it is not yet clear whether the depth is
  the tree-walk eval of the `&&&` lowering or the loader's bootstrap work.
  Next step: run `1 &&& 2` through a **loader-free minimal kit** to isolate
  which engine/path is responsible, then either bound the recursion with a
  clean `LangError` or fix the algorithmic re-evaluation.

Until that isolation is done, the fix is deliberately **not** shipped: the
overflow is genuine and pre-existing, and a speculative depth guard was not
validated against a clean oracle (`fp_integration` is red because of exactly
this bug).

**Cost of step 2 (resolved).** Preferring `body_def` initially deserialized
the standalone arena on _every_ application (`tagged_to_value` + `as_ast`),
which allocates per call. `plugin-engine-exec-tree-walk` now memoizes the
arena per definition value (`cached_def_arena`): the table is keyed by the
`Rc<TaggedValue>` pointer of the `body_def`, and each entry holds an
`Rc::clone` of that definition, so the address cannot be recycled while the
entry — and a reused address can never hit a stale entry (the same argument
`HOME_FRAMES` relies on). Two structurally identical but distinct definitions
therefore have distinct entries. Growth is bounded by clearing the table at
`ARROW_ARENA_CACHE_CAP` (64) entries, at a cost of at most one
re-deserialization per live arrow afterwards. Evidence:
`arrow_arena_cache_tests` in `crates/plugin-engine-exec-tree-walk/src/lib.rs`
asserts that a second lookup returns the _same_ allocation
(`Rc::ptr_eq`) — a test that fails if the memo is removed — that equal-but-
distinct definitions do not collide, that malformed/empty definitions are
rejected (falling back to `body`/`home`), and that the table stays within its
cap while remaining correct after a clear.

## ADR-0153/0154 conformance amendment (dimensions, identity, equality)

**Status: Accepted — applied.** This amendment is landed: the decisions
below are normative over §10's "graded arrows" wording and phase-1
implementation work follows them.

Amends §10 and adds the decisions that close the semantics. Normative
over §10's "graded arrows" wording:

- **`~[n]~>` is a dimension index, not a coeffect grade.** The bracket
  forms the n-cell type `Hom(n, src, tgt)` (ADR-0153): 0 = objects,
  1 = arrows, 2 = equations between arrows, 3 = coherence. The §10
  reading (semiring grade `R`, `gradeCompose` on the bracket) is
  withdrawn; usage/erasure is a mode axis, deliberately out of the
  bracket (ADR-0154 §6).
- **`f<...>` specialization semantics** = application of an
  index-bound arrow: `id<d : Dim>` binds a dimension index, the same
  mechanism as `List<T>`'s `T` (0153 §3). This discharges the §8/§12
  "specialization semantics remain" TODO.
- **`>>>` must be real composition** (TODO item 5 promoted to a
  phase-1 blocker): cell-law typing (`f >>>[k] g`) and reindexing
  (`f >>> P`) assume `>>>` composes; today it applies.
- **Identity**: declared formers mint `sym` deterministically —
  seeded by module-qualified name / content hash; plain `sym()` is
  fresh-per-eval for locals only (0154 §1).
- **Membership vs subtyping**: two links — values carry `type` →
  type sym; types carry `parent` → supertype sym. `x : T` reads
  `type`; `<:` walks `parent` (0154 §2). The §2/§14 "one walk powers
  both" claim stands with this split.
- **Equality law**: `==` is structural over immutable data; `sym` is
  the only opt-out. Nominal heads, structural bodies (0154 §3).
- **Universes**: `Type` is an indexed former `Type(i) : Type(i+1)`;
  bare `Type` = `Type(0)`; `Obj` is the world, not a type (0154 §4).
