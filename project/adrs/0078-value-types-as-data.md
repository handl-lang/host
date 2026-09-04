# ADR-0078: Value Types as Data — Composable Capabilities and Target Representations

## Status

Proposed. Extends [ADR-0044 — Language as Data and Runtime
Split](./0044-language-as-data-and-runtime-split.md) from the language
definition (tokens, grammar, eval schemes) to the **value type system**
(types, capabilities, target representations). Complements
[ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) and
[ADR-0076 — Decoupled Trait Layer](./0076-decoupled-trait-layer.md).
§6 introduces a per-`Language` `typecheck_mode` policy flag
(`optional` | `gate`) and designates the host language as statically
gated, building on the typecheck engine from
[ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md).

## Context

### What ADR-0044 fixed

ADR-0044 made the _language definition_ pure data: tokens, grammar rules,
AST shapes, and eval schemes are YAML files, not Rust trait impls. A
`Language` value is serializable, inspectable, authorable as text, and
loadable by a runtime with no compile-time knowledge of it. This enabled
self-hosting: a `Language` value can describe handl itself.

### What ADR-0044 did not address

**Value types are still hardcoded.** The platform uses
`serde_json::Value` everywhere — a fixed JSON type system
(`Null`/`Bool`/`Number`/`String`/`Array`/`Object`). This has three
problems:

1. **No user-defined types.** A handl program cannot define `Money`,
   `UUID`, or `UserId` as semantic types. Everything collapses to JSON
   primitives.

2. **No target-specific representations.** `Number` is always an
   `f64`-backed JSON number, even when the value is always an integer
   that could be a native `i64`. The compiler cannot unbox it because
   the type information is not in the data.

3. **No composable behavior.** The stdlib must hardcode knowledge of
   each type: `fn.sum` knows about numbers, `fn.concat` knows about
   strings, `collections.sort` knows about comparables. There is no
   `sum<T: Addable + Zero>` — every generic operation is reimplemented
   per type in Rust.

### The performance connection

The benchmark suite (`bench/`) shows handl is 1000x slower than Python on
`ackermann(3,3)` with the tree-walk engine, and ~44x slower than C even
with the cranelift JIT. The root cause is the value representation: every
integer is a heap-allocated `serde_json::Value::Number`, every pair is a
JSON object with a string key, every variable lookup is a `HashMap`
search. The cranelift backend generates native control flow but still
calls Rust helpers for every value operation because it has no type
information to specialize.

**This is not a backend problem — it is a type information problem.**
V8, LuaJIT, and PyPy all stay dynamically typed but specialize at runtime
using observed type information. The missing piece in handl is a type
system that can carry representation information to the compiler.

### The design principle

> **Types define semantics. Capabilities define reusable behavior.
> Targets define representations. The compiler connects them.**

This is ADR-0044's principle applied to types: just as `grammar.yaml` is
data that the parser engine consumes, `value_types.yaml` and
`capabilities.yaml` are data that the compiler engine consumes. No Rust
trait impls define what `Addable` means — the capability declaration
does, and the compiler resolves it to the target's concrete
implementation.

## Decision

### 1. Three new data fragment types

Extend the `Language` data schema (ADR-0044) with three new fragment
sets, each loaded from YAML:

| Fragment            | File                   | What it declares                    |
| ------------------- | ---------------------- | ----------------------------------- |
| `ValueTypeDef`      | `value_types.yaml`     | Semantic types and their fields     |
| `CapabilityDef`     | `capabilities.yaml`    | Behavioral contracts (operations)   |
| `RepresentationDef` | `representations.yaml` | Per-target concrete representations |

These merge into the `Language` value alongside the existing
`tokens`/`grammar`/`eval`/`ast`/`caps`/`options` fragment sets, using
the same merge policy (ADR-0044 §5: override vs collision vs union).

### 2. Value Types (`value_types.yaml`)

A value type declares:

- **Identity** — a stable id (e.g. `Int`, `String`, `Money`).
- **Structure** — fields with their types (for product types), or
  variants (for sum types). Scalar types have no fields.
- **Capabilities** — which capabilities this type implements
  (e.g. `Addable`, `Comparable`, `Hashable`).
- **Invariants** — optional assertions that must hold (e.g.
  `amount >= 0` for `Money`).
- **Conversions** — explicit, declared conversions to/from other types,
  with lossiness annotations.

```yaml
- id: Int
  description: 64-bit signed integer
  kind: scalar
  implements:
    [Addable, Subtractable, Multiplicable, Comparable, Hashable, Serializable, Displayable]

- id: Float
  description: 64-bit IEEE float
  kind: scalar
  implements:
    [Addable, Subtractable, Multiplicable, Divisible, Comparable, Serializable, Displayable]

- id: String
  description: UTF-8 string
  kind: scalar
  implements: [Concatenable, Comparable, Hashable, Serializable, Displayable, Iterable]

- id: Bool
  description: boolean
  kind: scalar
  implements: [Logical, Comparable, Hashable, Serializable, Displayable]

- id: Pair
  description: arrow-calculus pair (a, b)
  kind: product
  fields:
    - { name: fst, type: Any }
    - { name: snd, type: Any }
  implements: [Serializable]

- id: Either
  description: arrow-calculus sum type (left a | right b)
  kind: sum
  variants:
    - { name: left, type: Any }
    - { name: right, type: Any }
  implements: [Serializable]

- id: Array
  description: ordered sequence
  kind: parameterized
  type_params: [T]
  implements: [Iterable, Indexable, Lengthable]
  # Element type T is a type parameter, not a fixed type

- id: Arrow
  description: arrow-calculus closure value
  kind: product
  fields:
    - { name: params, type: Array<String> }
    - { name: body, type: Int } # AST node index
    - { name: env, type: Object } # captured environment
  implements: [Callable]
```

User-defined types follow the same schema:

```yaml
- id: Money
  description: monetary amount with currency
  kind: product
  fields:
    - { name: amount, type: Decimal }
    - { name: currency, type: Currency }
  implements: [Addable, Subtractable, Comparable, Hashable, Serializable, Displayable]
  invariants:
    - "amount >= 0"
  conversions:
    - from: Int
      to: Money
      lossiness: lossy # requires currency context
    - from: Money
      to: Int
      lossiness: lossy # truncates decimals
```

### 3. Capabilities (`capabilities.yaml`)

A capability declares:

- **Identity** — a stable id (e.g. `Addable`).
- **Operations** — the arrow signatures this capability requires.
  Each operation is an arrow type `A ~> B`.
- **Laws** — optional equational laws (links to ADR-0076's law system).
- **Type parameters** — generic parameters with bounds
  (e.g. `Addable<T>` requires `T: Addable`).

```yaml
- id: Addable
  description: values that can be added
  type_params: [T]
  operations:
    - { name: add, signature: "[T, T] ~> T" }
  laws:
    - commutative: "add(a, b) == add(b, a)"
    - associative: "add(add(a, b), c) == add(a, add(b, c))"

- id: Comparable
  description: values with a total order
  type_params: [T]
  operations:
    - { name: compare, signature: "[T, T] ~> Int" } # -1, 0, 1
    - { name: lt, signature: "[T, T] ~> Bool" }
    - { name: lte, signature: "[T, T] ~> Bool" }
    - { name: gt, signature: "[T, T] ~> Bool" }
    - { name: gte, signature: "[T, T] ~> Bool" }
  laws:
    - antisymmetric: "lt(a, b) && lt(b, a) == false"
    - total: "lt(a, b) || lt(b, a) || eq(a, b)"

- id: Hashable
  description: values that can be hashed
  type_params: [T]
  operations:
    - { name: hash, signature: "T ~> Int" }
  laws:
    - consistent: "eq(a, b) => hash(a) == hash(b)"

- id: Iterable
  description: values that can be iterated
  type_params: [T]
  operations:
    - { name: iterate, signature: "T ~> Array<Any>" }
    - { name: length, signature: "T ~> Int" }

- id: Serializable
  description: values that can be serialized to/from JSON
  type_params: [T]
  operations:
    - { name: serialize, signature: "T ~> Value" }
    - { name: deserialize, signature: "Value ~> T" }
  laws:
    - roundtrip: "deserialize(serialize(x)) == x"

- id: Callable
  description: values that can be invoked (arrow application)
  type_params: [A, B]
  operations:
    - { name: apply, signature: "[A, A~>B] ~> B" }
```

### 4. Target Representations (`representations.yaml`)

A representation declares how a value type is concretely represented
on a specific compilation target:

- **Target** — `native`, `js`, `wasm`, etc.
- **Type** — which value type this represents.
- **Layout** — the concrete representation (e.g. `i64`, `f64`,
  `Decimal128`, `BigInt`, `*mut Pair`).
- **Operations** — per-capability concrete implementations. Each
  operation maps to either:
  - an **inline** native expression (e.g. `add` → `a + b` for
    `Int` on `native`), or
  - a **foreign** function call (e.g. `add` → `Decimal.add(a, b)`
    for `Decimal` on `native`), or
  - a **lowered** scheme (an arrow-calculus scheme that the compiler
    inlines).
- **Conversions** — how to convert to/from the target's generic
  representation (e.g. `Int` on `js` → `Number` with checked range).

```yaml
# --- native target (cranelift/Rust) ---

- target: native
  type: Int
  layout: i64
  operations:
    Addable.add: inline "a + b"
    Subtractable.sub: inline "a - b"
    Multiplicable.mul: inline "a * b"
    Comparable.compare: inline "a.cmp(&b) as i64"
    Comparable.lt: inline "a < b"
    Hashable.hash: inline "a.hash()"
    Serializable.serialize: inline "Value::from(a)"
    Serializable.deserialize: inline "a.as_i64().ok_or(type_error)?"
    Displayable.display: inline "a.to_string()"

- target: native
  type: Float
  layout: f64
  operations:
    Addable.add: inline "a + b"
    Subtractable.sub: inline "a - b"
    Multiplicable.mul: inline "a * b"
    Divisible.div: inline "a / b"
    Comparable.lt: inline "a < b"
    Serializable.serialize: inline "Value::from(a)"
    Serializable.deserialize: inline "a.as_f64().ok_or(type_error)?"

- target: native
  type: String
  layout: "Rc<str>"
  operations:
    Concatenable.concat: inline "Rc::from(format!(\"{a}{b}\"))"
    Comparable.lt: inline "a < b"
    Hashable.hash: inline "a.hash()"
    Iterable.iterate: inline "a.chars().collect()"
    Iterable.length: inline "a.len() as i64"
    Serializable.serialize: inline "Value::from(a.as_ref())"
    Serializable.deserialize: inline "a.as_str().map(Rc::from).ok_or(type_error)?"

- target: native
  type: Pair
  layout: "Box<(TaggedValue, TaggedValue)>"
  operations:
    Serializable.serialize: lowered !Seq
      fst: !Arr { op: proj_child, args: [], source: Native, lit: 0 }
      snd: !Arr { op: proj_child, args: [], source: Native, lit: 1 }

- target: native
  type: Arrow
  layout: "Box<ArrowValue>"  # { params, body, env }
  operations:
    Callable.apply: foreign "arrow_apply"

# --- js target ---

- target: js
  type: Int
  layout: "Number"
  operations:
    Addable.add: inline "a + b"
    Subtractable.sub: inline "a - b"
    Multiplicable.mul: inline "a * b"
    Comparable.lt: inline "a < b"
    Hashable.hash: foreign "hashInt"
    Serializable.serialize: inline "a"
    Serializable.deserialize: inline "a"
  conversions:
    from_native:
      source: i64
      lossiness: checked  # may lose precision > 2^53
      guard: "a <= 9007199254740991 && a >= -9007199254740991"

- target: js
  type: String
  layout: "string"
  operations:
    Concatenable.concat: inline "a + b"
    Comparable.lt: inline "a < b"
    Hashable.hash: foreign "hashString"
    Iterable.iterate: inline "[...a]"
    Iterable.length: inline "a.length"
    Serializable.serialize: inline "a"
    Serializable.deserialize: inline "a"

# --- wasm target ---

- target: wasm
  type: Int
  layout: "i64"
  operations:
    Addable.add: inline "i64.add"
    Subtractable.sub: inline "i64.sub"
    Multiplicable.mul: inline "i64.mul"
    Comparable.lt: inline "i64.lt_s"
    Hashable.hash: foreign "hash_i64"
    Serializable.serialize: foreign "i64_to_json"
    Serializable.deserialize: foreign "json_to_i64"
```

### 5. How the compiler uses this

The compilation pipeline becomes:

```
Source
  ↓ (parse — ADR-0044)
Ast
  ↓ (typecheck — ADR-0054, using types.yaml rules)
TypedAst  ←  each node carries a ValueType
  ↓ (capability resolution — resolve each operation to a CapabilityDef)
ResolvedAst  ←  each operation node carries (Capability, Type, Target)
  ↓ (representation selection — look up representations.yaml for (type, target))
LoweredAst  ←  each operation is either inline/foreign/lowered
  ↓ (code generation — cranelift/LLVM/WASM/JS backend)
Target code
```

**Key property:** the compiler never hardcodes "Int + Int → i64 add". It
reads from `representations.yaml` that `Int` on `native` has layout `i64`
and `Addable.add` is `inline "a + b"`. A different language definition
could map `Int` to `BigInt` on `js` with a foreign call — same compiler,
different data.

### 6. Type inference: gradual + runtime profiling, gated by language policy

To specialize, the compiler needs to know the concrete type at each
operation. The typecheck engine (ADR-0054) is a full dependent
typechecker (NBE, unification, bidirectional check/synth) — more
powerful than what most "statically typed" languages have. Whether
typecheck is a _gate_ (programs that don't typecheck are rejected
before eval) or an _optimization_ (best-effort inference, unresolved
nodes fall back to dynamic) is **not a property of the engine but a
per-`Language` policy**.

#### 6.1 The `typecheck_mode` policy flag

A `Language` value (ADR-0044) declares its typing discipline via
`lang.options.typecheck_mode`:

| Mode                                 | Behavior                                                                                 | When typecheck runs     | Unresolved nodes                    |
| ------------------------------------ | ---------------------------------------------------------------------------------------- | ----------------------- | ----------------------------------- |
| `optional` (default for guest langs) | Typecheck is best-effort inference; eval/compile proceed regardless                      | As an optimization pass | Fall back to `TaggedValue` + tier 2 |
| `gate`                               | Typecheck is a hard gate; programs that don't typecheck are rejected before eval/compile | As a mandatory pre-pass | Compile error — no fallback         |

This keeps the engine generic: the same typecheck engine (ADR-0054)
and the same compiler (this ADR) serve both modes. A `Language` value
chooses its discipline, consistent with ADR-0044's "language as data"
principle. This is the TypeScript-to-JavaScript pattern: same engine,
different strictness policy.

#### 6.2 The host language is statically gated

The **host language** (the canonical kit that defines handl itself,
used for the platform's own stdlib, runtime, and tooling) declares
`typecheck_mode: gate`. Rationale:

- **Zero-cost abstraction in the hot path.** Every node synthesizes a
  type at compile time → the compiler unboxes to native representation
  (`i64`, `f64`, `Rc<str>`) everywhere, with no `TaggedValue` fallback,
  no type guards, no deopt paths. Monomorphization happens at compile
  time, not via runtime profiling.
- **Refactor safety.** Host-lang changes are caught at typecheck time,
  not at runtime — important for the platform's own correctness.
- **Proven practice.** Static typing for the implementation language
  while hosting dynamically-typed guest langs is the Rust/TypeScript
  pattern: a strong foundation that interoperates with weaker-typed
  guests.

Guest languages (plugins, embedded DSLs, user-authored kits) may
declare `typecheck_mode: optional` and use the full dynamic +
JIT-specialization path below.

#### 6.3 The two specialization tiers (used by `optional` mode)

For nodes in `optional`-mode languages (or at static/dynamic
boundaries in `gate`-mode languages), the engine uses a two-tier
strategy:

**Tier 1 — Static inference (gradual typing):**
The typecheck engine (ADR-0054) synthesizes types. Where it can prove
a node is always `Int`, the compiler uses the `native → i64`
representation directly. This covers literals, arithmetic chains, and
annotated bindings. No runtime overhead.

**Tier 2 — Runtime profiling + specialization (the JIT path):**
For nodes the typechecker cannot statically resolve (polymorphic
functions, dynamic dispatch), the engine:

1. Runs the bytecode/tree-walk interpreter with type profiling — each
   value records its `ValueType` on first observation.
2. When a function or loop body is hot (exceeds a threshold), the JIT
   recompiles it using the observed types, inserting **type guards**
   (runtime checks that fall back to the slow path if the type
   changes).
3. The specialized code uses the `representations.yaml` entry for the
   observed type — `Int` on `native` becomes unboxed `i64` arithmetic.

This is the V8 TurboFan / LuaJIT trace compilation model. The guest
language stays dynamically typed; the _compiler_ bets on observed
types and generates specialized native code with guards.

#### 6.4 Performance implications by mode

|                                 | `optional` (dynamic guest)    | `gate` (static host)                               |
| ------------------------------- | ----------------------------- | -------------------------------------------------- |
| Tier 1 (static inference)       | Best-effort, where provable   | Every node synthesizes                             |
| Tier 2 (JIT profiling + guards) | Required for unresolved nodes | Unnecessary for host-lang code                     |
| `TaggedValue` fallback          | Always present                | Gone for host-lang paths                           |
| Boxing/unboxing at boundaries   | Frequent                      | Only at FFI / dynamic-guest edges                  |
| Compile-time cost               | Low (typecheck optional)      | Higher (dependent typecheck runs on every program) |
| Runtime cost                    | Guards + fallback paths       | None in hot path                                   |

#### 6.5 Watch-out: dependent types as a hard gate

ADR-0054 commits handl to **full dependent types** ("a type is an
arbitrary expression evaluated at typecheck time"). Full dependent
typechecking is powerful but can be slow and fail opaquely — most
statically-typed languages (Rust, Haskell, OCaml) deliberately stop
short of it for this reason.

If `gate` mode proves painful for the host lang in practice, a future
ADR may introduce a third mode — e.g. `strict` (HM-style inference by
default, dependent types only where annotations request them) —
without changing the engine. The `typecheck_mode` flag is designed to
admit this extension.

### 7. Monomorphization

Yes — the compiler monomorphizes generic operations. When `sum<T:
Addable + Zero>` is called with `T = Int`, the compiler:

1. Looks up `Int` in `value_types.yaml` → confirms it implements
   `Addable` and `Zero` (via the `implements` list).
2. Looks up `(Int, native)` in `representations.yaml` → finds
   `Addable.add: inline "a + b"` and `Zero.zero: inline "0"`.
3. Generates a specialized `sum_Int` function with inlined `i64`
   arithmetic.

When called with `T = Money`, it generates `sum_Money` using
`Money`'s `Addable.add` implementation (which might be a foreign call
to `Decimal.add`). **Same generic stdlib function, different
specialized code per type.** No runtime capability dispatch in the hot
path.

### 8. Relationship to TaggedValue (the `handl-value` crate)

`TaggedValue` is the **dynamic fallback representation** — the
equivalent of V8's tagged pointer or CPython's `PyObject*`. It is what
the interpreter uses when the compiler has no type information. It is
NOT the final representation; it is the baseline that specialization
improves upon.

```
No type info  →  TaggedValue (boxed, tag-checked)     ← current work
Static type   →  unboxed native (i64, f64, Rc<str>)   ← tier 1
Profiled type →  unboxed native + guards              ← tier 2 (future)
```

`TaggedValue` is complementary to this ADR, not replaced by it:

- It gives the interpreter a fast boxed representation _today_ (5-10x
  over `serde_json::Value` on pair/either-heavy code).
- It defines the `Pair`/`Either`/`Arrow` variants that
  `value_types.yaml` declares as types.
- It is the fallback for type-guard failures in the JIT.
- It is the representation for dynamically-typed code that never
  specializes.

### 9. Relationship to ADR-0076 (Decoupled Trait Layer)

ADR-0076 decoupled traits into laws + bundles + handlers. This ADR
extends that model:

- **ADR-0076's traits** = this ADR's **capabilities** (named bundles of
  arrow operations + laws).
- **ADR-0076's impls** = this ADR's **representations** (concrete
  implementations per type per target).
- **ADR-0076's handlers** = this ADR's **compiler** (resolves
  capabilities to representations and generates target code).

The capability declarations in `capabilities.yaml` are the data form of
ADR-0076's trait declarations. The representation declarations in
`representations.yaml` are the data form of ADR-0076's impls. The
difference is that this ADR makes them **data** (YAML), consistent with
ADR-0044's principle, rather than Rust trait impls.

## Consequences

### Positive

- **User-defined types as first-class citizens.** `Money`, `UUID`,
  `UserId`, `Meters` — all declarable in YAML, all with target-specific
  representations.
- **Tiny, generic stdlib.** `sum<T: Addable + Zero>`,
  `sort<T: Comparable>`, `hash<T: Hashable>` — one implementation,
  monomorphized per type. No per-type Rust reimplementations.
- **No hardcoded knowledge of domain types.** The compiler reads
  `representations.yaml`; it does not know about `Money` or `Decimal`.
- **Target-specific representations.** `Int` is `i64` on native,
  `Number` (checked) on JS, `i64` on WASM. Same type, different
  concrete representation, declared in data.
- **Static specialization.** Where the typechecker proves a type, the
  compiler unboxes to native representation. Zero-cost abstraction.
- **Explicit, checkable cross-target conversions.** `UInt64 → JS Number`
  is declared as `lossiness: checked` with a guard. No silent overflow.
- **Plugins can introduce entire families of new types.** A plugin
  ships `value_types.yaml` + `capabilities.yaml` + `representations.yaml`
  fragments; the compiler merges them (ADR-0044 §5 merge policy).
- **The same capability model usable by libraries, frameworks, runtimes,
  and tooling.** LSP can use `Displayable` for hover, `Comparable` for
  sorting completions, `Hashable` for caching.
- **Per-`Language` typing discipline.** The `typecheck_mode` flag
  (`optional` | `gate`) lets each `Language` value choose dynamic or
  static typing without changing the engine — the TypeScript-to-JavaScript
  pattern, generic and data-driven (§6.1).
- **Host-lang zero-cost abstraction.** With the host language in `gate`
  mode, every node synthesizes a type at compile time → unboxed native
  representation everywhere, no `TaggedValue` fallback, no guards, no
  deopt paths in the hot path (§6.2, §6.4).
- **Refactor safety for the platform's own code.** Host-lang changes are
  caught at typecheck time, not at runtime — important for the stdlib,
  runtime, and tooling that the platform depends on.

### Negative

- **Three more YAML fragment types to maintain.** The canonical kit
  grows from 7 data files to 10. Each must be authored, tested, and
  kept consistent.
- **Compiler complexity increases.** The compiler must resolve
  capabilities, select representations, and monomorphize. This is real
  compiler work, not a weekend refactor.
- **Type profiling + JIT specialization is a large engineering effort.**
  Tier 2 (runtime profiling) is the hardest part and may not land
  initially. Tier 1 (static inference) covers the common case but
  requires the typechecker to be precise enough.
- **The dynamic fallback (TaggedValue) must coexist with unboxed
  representations.** The compiler must insert boxing/unboxing at
  representation boundaries (when a typed value flows into an
  untyped context). This is the standard "erasure" cost of gradual
  typing.
- **Capability laws are not automatically verified.** ADR-0076's law
  system can check them, but that is separate from this ADR. A type
  can declare `implements: [Addable]` without the compiler proving the
  laws hold — the representation's `inline` expression is trusted.
- **Host-lang compile-time cost increases.** With the host language in
  `gate` mode, the dependent typechecker (ADR-0054) runs on every
  program before eval/compile. Dependent typechecking is not cheap;
  this trades runtime cost for compile-time cost (§6.4).
- **Static/dynamic boundary cost.** When `gate`-mode host code calls
  `optional`-mode guest code (plugins, DSLs), the compiler must insert
  boxing/unboxing and `TaggedValue` bridging at the boundary. This is
  the standard erasure cost of gradual typing, paid only at boundaries.

### Risks

- **Over-engineering.** If handl programs rarely define custom types,
  the capability system is unused machinery. Mitigation: the canonical
  kit's built-in types (`Int`, `Float`, `String`, `Bool`, `Array`,
  `Pair`, `Either`, `Arrow`) are the first customers. User-defined
  types are a later phase.
- **Representation fragmentation.** Many targets × many types × many
  capabilities = a large `representations.yaml`. Mitigation: default
  representations (every type gets `Serializable` → `Value` on every
  target); explicit entries only for optimized paths.
- **JIT complexity.** Runtime profiling + guard insertion + deoptimization
  is the hardest part of a JIT. Mitigation: tier 1 (static inference)
  is the first deliverable; tier 2 is a follow-up.
- **Dependent types as a hard gate may prove painful.** ADR-0054 commits
  handl to full dependent types; running that on every host-lang program
  can be slow and fail opaquely (most statically-typed languages stop
  short of full dependence for this reason). Mitigation: the
  `typecheck_mode` flag is designed to admit a future `strict` mode
  (HM-style inference by default, dependent types only where annotations
  request them) without changing the engine (§6.5).

## Phasing

### Phase 1 — Data definitions (no compiler changes)

Add `ValueTypeDef`, `CapabilityDef`, `RepresentationDef` to
`handl-language` as new fragment types. Author the canonical
`value_types.yaml`, `capabilities.yaml`, `representations.yaml` for the
built-in types (`Int`, `Float`, `String`, `Bool`, `Null`, `Array`,
`Object`, `Pair`, `Either`, `Arrow`). Load and merge them into the
`Language` value. No compiler consumes them yet — this is pure data.

### Phase 2 — TaggedValue as the native representation

Complete the `handl-value` migration (in progress). The tree-walk,
bytecode, and cranelift engines use `TaggedValue` internally. The
`TaggedValue` variants match the `value_types.yaml` declarations
(`Int` → `TaggedValue::Int(i64)`, `Pair` → `TaggedValue::Pair`, etc.).
This is the dynamic fallback representation.

### Phase 3 — Static type inference → unboxing, `typecheck_mode` policy

Add the `typecheck_mode` flag to `lang.options` (`optional` | `gate`).
Extend the typechecker (ADR-0054) to carry `ValueType` information on
AST nodes. The cranelift backend reads `representations.yaml` for
`(type, native)` and generates unboxed arithmetic for statically-typed
`Int`/`Float` operations. This closes the gap to C on typed-integer
code.

Set the host language's canonical kit to `typecheck_mode: gate` so
host-lang code is statically gated: every node synthesizes a type at
compile time, no `TaggedValue` fallback in the hot path. Guest
languages remain `optional` by default.

### Phase 4 — Monomorphization of generic stdlib

Rewrite the stdlib functions (`fn.sum`, `fn.map`, `collections.sort`,
etc.) as generic operations over capabilities. The compiler
monomorphizes them per concrete type using the representation data.

### Phase 5 — Runtime profiling + JIT specialization

Add type profiling to the bytecode interpreter. When a function is hot,
recompile with cranelift using observed types + guards. This is the
V8/LuaJIT path and closes the gap on dynamically-typed code.

### Phase 6 — User-defined types

Allow handl programs to declare value types in `.hnl` source (desugared
to `value_types.yaml` fragments). User-defined types participate in the
same capability/representation system as built-in types.

## Status

Proposed. Phases 1-2 are the immediate work. Phases 3-4 are the
near-term goal (closing the perf gap to C/Rust on typed code). Phases
5-6 are the long-term vision.
