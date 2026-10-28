# Denotational semantics — `Val` as the domain, `[[·]]` as the evaluator
- Refs: ADR-0106, ADR-0154, ADR-0158


Naming what the evaluator already computes. The semantic domain is not a
model built alongside the implementation — it *is* the implementation's
value space: `handl_value::nbe::Val`
(`kernel/crates/handl-value/src/nbe.rs:38`), shared by the kernel NbE
evaluator (`engine/src/nbe.rs:92`), the `Val`-domain checker
(`engine-typecheck-nbe/src/checker.rs`), and the judgment ops
(`engine-typecheck-nbe/src/judgments.rs`). `TaggedValue` is the boundary
encoding (serialization, FFI); `Val` is the semantics. This document
reads the variants denotationally and maps every surface form to its
denotation. Laws are stated as equations over `Val`; each one names the
operational witness that decides it. Adequacy (denotation = observable
behavior for all programs) is a doc-level claim — see §4.

Refs: ADR-0106 (object/arrow kernel), 0152 (`data` formers), 0153
(`~[n]~>`/`IO`), 0154 (identity/equality/universes/literals/erasure),
0158 (effects as algebra homomorphisms — the correspondence-table style
this doc extends language-wide), phase-1/2/3 specs.

## 1. The semantic domain

`D = Val`. Each variant's denotational reading:

| `Val` variant | Denotation |
|---|---|
| `Rigid{lvl, spine}` | A *free variable*: the de Bruijn-**level** `lvl` head applied to `spine` eliminations. Rigid under binders — a computation stuck on an unsolved binding. |
| `Flex{meta, spine}` | A *metavariable* `?m` applied to a spine — an open slot the equality relation solves (substitution lives in `MetaStore`). |
| `Former{head: Rc<Sym>, args}` | A *constructor application*: the nominal `head` applied to index arguments — `Vec(2, Int)`, `Hom(2, f, g)`, `IO({puts: Str})`, `cell.comp(k, a, b)`, `Pi(A, B)`, `Sigma(A, F)`, plus `stuck.*` heads for open terms. |
| `Nominal(Rc<Sym>)` | A *nominal atom*: a type object as a value (`Int`, `Nat`, `Type`-world names). Equality is sym identity (the handle), never structure. |
| `Arrow(Rc<ArrowVal>)` | A *morphism*: the defunctionalized closure `params + body_def + binds` — a closure spelled as serializable data, no host capture (ADR-0106 §3). |
| `Universe(i)` | `Type(i)` — the level-i universe (ADR-0154 §4). |
| `Const(TaggedValue)` | *Boundary data*: literals, records, pairs, `Either` — anything the domain does not structure stays tagged; equality is structural (0154 §3). |
| `Unfold{name, spine, value}` | A *suspended fold* of a top-level definition: name + spine with the unfolded value computed lazily (`OnceCell`). Conversion checks the name first, forces only on failure (Kovács-style controlled unfolding). |

Two auxiliaries carry the rest of the domain:

- **`Env`** — the environment model: a persistent cons list, `Rc`-shared
  tails (`nbe.rs:109`). Extend is O(1) and never mutates a prefix;
  lookup is by level offset. `ArrowVal.binds` is the same environment
  spelled as *data* inside an arrow value — the residual bindings an
  application still owes.
- **`MetaStore`** — the domain's mutable meta layer:
  `Vec<MetaEntry{solution, constraint}>` (`nbe.rs:193`). Solves are
  *substitutions*; `occurs` rejects cyclic solutions; `mark`/`truncate`
  give backtracking (`nbe.rs:1258`); `force_head`/`force_head_rc`
  substitute solved metas at the head — read-through equality. A
  `MetaConstraint::DescendantOf(sym)` constrains a meta to solve into
  `sym`'s nominal-descendant cone (literal typing, 0154 §5).
- **`Sym` seeding** — nominal identity is a hash of the qualified seed
  (`Sym::seeded("vec.Vec")`, `nbe.rs:394`): a declared name mints one
  identity per process *and* across processes/artifacts. A sym carries
  two links: `parent` (the `<:` edge) and `ty` (the membership `x : T`
  edge) — two relations, not one chain (0154 §2).

Convention: de Bruijn **indices** in terms, **levels** in values —
`Lvl` counts binders from the outside; `quote` converts level → index
on the way back to surface spelling (smalltt/Kovács).

## 2. The denotation map `[[·]]`

`[[t]] : Term → Val` is `nbe::eval`; on surface ASTs it is the
`synth_*`/`check_*` judgment ops of `dependent.rs`/`checker.rs`. Each
entry: surface form → its `Val`.

### 2.1 Data and binding

| Surface | `[[·]]` |
|---|---|
| literal `1`, `"s"`, `{…}`, `[…]` | `Const(TaggedValue)` — boundary data, structural equality |
| `x` | environment lookup — a bound var's `Val` (a `Rigid{lvl}` head for a checker binder; the definition's value for a Γ entry) |
| `x ~> body` (arrow literal) | `Arrow{params:[x], body_def:{"__ast__":…}, binds:ρ}` |
| `f(v)` (application) | `apply`: `Rigid`/`Flex`/`Unfold` extend the spine; `Former` extends `args`; `Arrow` applies via the engine `apply_arrow` hook, else the *residual closure* — `binds ∪ {param ↦ arg}`, `params` popped (`engine/src/nbe.rs:402`) |
| `first f` on `(a,b)` | `([[f]](a), b)` — the arrow `***`/`first` law |
| `choice l r` on `Either` | `[[l]]` on `Left v`, `[[r]]` on `Right v` |
| `f >>> g` (axis 0, data operands) | sequential threading — `[[g]]` sees `[[f]]`'s output |
| `f >>> g` on cell operands | `Former{cell.comp, [0, [[f]], [[g]]]}` or the `Arrow{first,second}`/`__dim__` composite — the ∘₀ paste (`compose_cell_vals`, `engine/src/nbe.rs:288`) |

### 2.2 Types as values

| Surface | `[[·]]` |
|---|---|
| `Type(i)` | `Universe(i)`; `Type(i) : Type(i+1)` (`Checker::infer`) |
| `Int`, `Nat`, … | `Nominal(seeded("type.Int"))` — nominal atom, identity equality |
| `a ~> b` as a *type* | `type.function{param,body}` in tagged coordinates; `Former{type.hom, [1, a, b]}` in domain coordinates (the 1-cell hom) |
| `a ~[n]~> b` | `Former{type.hom, [Const n, [[a]], [[b]]]}` — the n-cell type; literal `n`, a scope-bound dim var as a `var{…}` index, else a `Dim` meta (`synth_dim_arrow_judgment`, `dependent.rs:5973`) |
| `Vec<n, a>` / `F<i⃗>` | `Former{seeded("vec.Vec"), [n, a]}` — head equality = sym identity; indices convert structurally |
| `Pi(A, b ~> B)` / `forall b, T` | `Former{type.pi, [A, closure]}` / `type.pi{param, body_closure}`; level `max(dom, cod)` (`FormerSig::Pi`, `checker.rs:103`) |
| `Sigma(A, b ~> B)` / `exists b, T` | `type.sigma{param, body_closure}`; intro = `[witness, payload]` pair, destructure binds the witness *rigidly* (`var{w}`) and instantiates the closure at it (`bind_sigma_destructure`, `dependent.rs:5097`) |
| `IO(S)` | `Former{type.io, [sig]}`; bare `IO` = `IO(⊤)` (`FormerSig::Io`, `checker.rs:126`) |
| `Path{frm, to}` | the proof-relevant index-equality witness — bound as `eq_<name>` in `results` by branch refinement; `transport p v` rewrites `frm → to` through `v`'s type (`synth_transport_judgment`), `j` is the J eliminator |

### 2.3 `data` — free algebras

`data F(i⃗ : I⃗) :: C₁ : S₁ | … | Cₙ : Sₙ` denotes three declarations:

1. **Family former** — `F := Former{seeded("<mod>.F")}` with a
   `Telescope` signature `params[(i₁:I₁)…(iₖ:Iₖ)], result: Universe` —
   or `type.family{name, base}` in tagged coordinates
   (`synth_data_judgment`, `dependent.rs:3302`). `F(i⃗)` applications
   are constructor applications whose typing is the telescope.
2. **Constructor syms** — each `Cⱼ` gets a `type.ctor_sig` spec
   re-elaborated per use (`instantiate_ctor`): fresh index metas per
   family parameter; a ctor application `Cⱼ x⃗` denotes the family's
   saturated `Former{F, […indices]}` value — the generator.
3. **`F.rec`** — the dependent eliminator bound in the `__splice__`
   scope: `motive ~> c₁ ~> … ~> cₙ ~> i⃗ ~> F(i⃗) ~> motive(i⃗, scrut)` —
   the fold/homomorphism out of the free algebra. Strict positivity is
   checked on the ctor sigs at declaration.

`match s { Cⱼ p⃗ ⇒ eⱼ | absurd ⇒ … }` denotes the eliminator
application: the scrutinee's `Former{F, indices}` dispatches the arm
whose head ctor is `Cⱼ` (ctor dispatch on nominal identity); each arm
refines the family's indices by unifying the ctor's result indices
against the scrutinee's (`unify_index` — "index refinement is
unification", ADR-0152); the `Path{frm,to}` witness of each decided
equation binds `eq_<name>`. Coverage is a split tree over the family's
ctor census (`family_ctors`/`ctor_family`, WS-C). `absurd` arms
discharge the arms the index equations refute (e.g. `0 ≡ n+1` on a
`Nat`-sorted index is `Disproven`).

### 2.4 Effects — the algebra-homomorphism layer

| Surface | `[[·]]` | Reading |
|---|---|---|
| `perform(op, arg)` | `Former{type.io, [{op: [[arg]]}]}` | a *generator node*: the op `op` applied to `arg` in the free algebra; `synth_perform` (`dependent.rs:6417`) types it `IO{op: T_arg}` |
| `handle f with h` | the *fold*: `interpret_io_former` rewrites each sig entry `op ↦ h[op](entry)` in place; uncovered ops stay (`IO(S∖H)` residual) | the algebra homomorphism from the free algebra on `S` to `h`'s interpretation (`native_ops.rs:3559`, `handle_dyn_shell`:3613) |
| `handle f with h acc s0` | `h[op](arg, s) ↦ (res, s')` — the `acc` wire is the `loop`/trace combinator's `c` channel | the algebra's carrier — fold state as the trace wire, not a frame slot (ADR-0158) |
| `return: r ~> e` clause | applied to the body's pure result | the algebra's unit map |
| residual `IO{op: arg}` | the `Former{type.io}` value itself | the *trace*: "effects as data" — an inspectable term of the free algebra, interpretable by outer handlers or destructured by user code |

Handler *laws* (`law_state : put(v) >>> get() ~[2]~> pure(v)`) denote
`Former{type.hom, [2, lhs, rhs]}` — cell equations, decided by the
same machinery as every `~[2]~>`.

### 2.5 Cells

| Surface | `[[·]]` |
|---|---|
| `f >>>[k] g` | `Former{cell.comp, [Const k, [[f]], [[g]]]}` — the ∘ₖ composite cell value; its type is the boundary law `infer_comp_axis` (`checker.rs:790`) — `Dim`/`Mismatch` errors propagate, stuck operands store `Hom(d, a, b)` unchecked |
| `id_k x` | `Former{cell.id, [Const k, [[x]]]}`; its type is `Hom(k, x, x)` (`infer_id`, `checker.rs:950`; `synth_cell_id_judgment` mints `Hom(2, x, x)` for `id2`) |

### 2.6 Instances — dictionary search

`instance C(i⃗) = dict` denotes: `dict` synths as ordinary data; its
declared `type` link is the constraint former-app `C(i⃗)`; the entry
`(name: $instance:C:k, args)` registers in `ctx.instances[C]`
(`synth_instance_judgment`, `dependent.rs:4296`). Unbound index vars
generalize rigidly; open metas freeze to per-instance rigids; coherence
is a declaration-time no-overlap check (`unify_index` on index shapes).
An implicit `Pi` whose domain `C(i⃗)` is a registered head denotes an
*evidence* argument: `resolve_instance` searches candidates in
declaration order, unifying index args under a store mark —
dictionary search, not unification (`dependent.rs:4444`).

### 2.7 Op nodes and the dispatch seam

`Term::Op{name, args, lit, dim}` denotes whatever the installed
`OpHandler` says: the kernel forwards `(name, args, lit, dim, input)`
and knows no op meaning (`engine/src/nbe.rs:60`). The checker's
`JudgmentOps` installs the domain vocabulary: constructors
(`nominal`/`former`/`universe`/`pi`/`hom`/`meta`/`literal`/`lift`/
`quote`), judges (`infer`/`check`/`unify`/`subtype`/`bind`/`lookup`),
cell ops (`cell_id`/`cell_comp_axis`), and pure index arithmetic
(`add`/`sub`/`mul`/`div` over `Const` naturals) — `Vec(1+1) ≡ Vec(2)`
is ordinary conversion, not a bespoke solver.

## 3. Equations in the domain

- **`conv` = `=`**: nominal heads by sym identity, bodies/spines
  structurally, `Universe` by level, `Const` by `TaggedValue`
  structural equality, `Arrow` by `params`+`body_def`+`binds`
  structurally (`nbe.rs:648`). `conv(?m, v)` *solves* `?m := v` after
  the occurs check — equality is effective, not just a predicate.
  `Unfold ≡ Unfold` on name equality is the fast path; a name miss
  forces and retries. `type.unknown` is the dynamic top: `unknown ≡ T`
  for all `T` (`Checker::conv`, `checker.rs:370`).
- **`subtype` = `<:`**: `conv`, else the `parent` walk on nominal
  heads; `Universe(i) <: Universe(j)` for `i ≤ j` (cumulativity);
  `IO(sa) <: IO(sb)` by record-width on sigs (`sb <: sa`); `Former`
  args are *invariant* — `Vec(Nat)` is not a `Vec(Int)`
  (`checker.rs:383`).
- **Cell equality = `CellNf` normal form** (`nbe.rs:870`): the globular
  laws hold definitionally — same-axis chains flatten (associativity),
  `id_j` absorbs at axis `k` for `j ≤ k+1` (units; objects are
  degenerate `id` towers and absorb at every axis), uniform `j`-grids
  at `j < k` transpose (interchange — smaller axes sort outward).
  Fuel-bounded (`CELL_NF_FUEL = 4096`); open axes, non-uniform grids,
  and non-unit leaves stay stuck and compare structurally — never
  wrong-equal.
- **`~[2]~>` equations = path-equality**: `e : f ~[2]~> g` stores
  `Former{type.hom,[2,f,g]}`; `infer_hom` checks `f`/`g`'s `(n−2)`-faces
  convert at every literal `n ≥ 1` (`checker.rs:608`). Law endpoints are
  *pattern terms* — free names bind fresh metas for the check's
  duration (`bind_law_pattern_vars`, `dependent.rs:6070`).
- **Meta solves = substitutions**: `MetaStore` is the substitution
  environment; `mark`/`truncate` backtrack. Literal metas are
  constrained substitutions — `1`'s type is `Flex` constrained
  `DescendantOf(type.Number)`; unsolved at end it defaults to the
  anchor (`default_unsolved`, `checker.rs:1094`).
- **Index equality = evaluation**: `eval_index` normalizes index terms
  — ctor projections (`App{Succ, x} ↦ x+1`, `var{Zero} ↦ 0`), then
  commutative-ring normal form over `add`/`sub`/`mul`/`neg`; the
  `IndexSolver` seam returns `Proven | Disproven | Unknown`
  (`dependent.rs` index machinery).
- **`exists`/`Sigma` packing**: `[w, p] : Sigma(A, F)` checks
  `w : A`, then `p : F(w)` — the witness rigidifies to `var{w}` so the
  payload type instantiates at that atom (`dep_intro_sigma`:5035).
- **Iota**: `F.rec(m, c⃗, i⃗, Cⱼ x⃗) ⟶ cⱼ x⃗ (F.rec …)` — definitional
  (the eliminator computes), which is what makes `Vec(1+1) ≡ Vec(2)`
  ordinary conversion.

## 4. Operational correspondence

| Denotation | Executable witness |
|---|---|
| `[[·]]` | `nbe::eval : Term → Val` (`engine/src/nbe.rs:92`) over the pure fragment (`Arr`/`Const`/`Op`/`Seq`/`First`/`Choice`/`App`/`RecordProj`/`RecordWith`); `Handle`/`Rewrite`/`Tag`/`Cobind`/`Loop`/`Iter`/`Child`/`Opaque`/`Src`/`Tgt` return `NotImplemented` — runtime/effect positions outside the pure-eval boundary (ADR-0054). Surface `[[·]]` is the `synth_*`/`check_*` ops of `dependent.rs`. |
| readback | `quote : Val → TaggedValue` (`nbe.rs:1617`) — `__neutral__`/`__former__`/`__sym__`/`__type__`/`__meta__`/`__arrow__` spellings |
| `=` | `conv` (`nbe.rs:648`) + `dep_unify`/`unify_index` on the tagged side; `subtype` for `<:` |
| `Γ ⊢ v : T` | `Checker::infer`/`check` (`checker.rs:436/1013`), Γ = `Vec<(name, Val)>` level-indexed |
| the op seam | `JudgmentOps` (`judgments.rs`) — the `OpHandler` installed into `eval`; `RefCell<Checker>` because judging mutates |
| effects runtime | `PerformFn` mints `{"__former__": <type.io sym>, "indices": [sig]}` (`engine-exec/src/io.rs:43`); `handle_dyn_shell`/`interpret_io_former` + `handler_stack` are the homomorphism's operational witness (`native_ops.rs:3559`); compiled backends get the same meaning via `Term::eliminate_handlers` (a compile-time transform) or `handle_dyn` |
| cross-engine agreement | `scope_value` (env read, one per backend) + `host/crates/handl-runtime/tests/engine_parity.rs`: tree-walk (reference), bytecode, cranelift must produce the same value on the shared corpus — including `__former__`/`IO`/cell mints |

**Proven vs claimed.** Decided by tests: `conv` laws, meta
occurs/constraint solves, `CellNf` assoc/unit/interchange, `subtype`
cumulativity + `IO` sig-width + index invariance, seeded-sym identity
and link-upgrade semantics (`nbe.rs`/`checker.rs` unit tests), the
`dependent_structure`/`axis_flow` suites, and the three-engine parity
corpus. **Deferred metatheory** (doc-level claims): adequacy — that
`[[·]]` agrees with observable behavior for every program; progress/
preservation for the `FormerSig` judgment rules; that `conv` is a
congruence over the whole domain. The honest bound is stated the same
way ADR-0158 states it: same laws, operational witness — the domain is
executable data, not a promonad proof artifact.

## 5. Alignment notes

| External semantics | handl reading |
|---|---|
| Sanada: op `op : A ⇝ B` in signature Σ | `perform(op, x)` — a generator node `Former{type.io,[{op:x}]}` |
| Sanada: free arrow-algebra on Σ | arrow `A ~> IO<Σ>` — the residual trace is the free-algebra term itself |
| Sanada: handler = algebra homomorphism | `handle f with {op: impl, return: …}` — `interpret_io_former` is the fold |
| Sanada: algebra carrier / fold state | `acc s` — the `loop` trace wire |
| Sanada: strong promonad in **Prof** | `handler_stack` frames + `interpret_io_former` — operational witness, not denotational machinery (0158 §Deltas) |
| NbE convention (Kovács/smalltt) | `Val` *is* the domain: levels in values, indices in terms; `Rigid`/`Flex` neutrals = open terms; `quote` = readback; `Unfold` = controlled unfolding; `MetaStore` = the substitution |
| Initial algebra / `F`-algebra | `data F :: Cⱼ` — family former + ctor generators + `F.rec` fold; positivity checked at declaration; `match` = eliminator application |
| Globular n-category | `Hom(n,a,b)` = the type of n-cells; `>>>[k]`/`id_k` mint `cell.comp`/`cell.id` formers; `CellNf` makes associativity/units/interchange *definitional* — the coherence laws are `conv`, not proofs to carry |

The pattern throughout: each external law is honored by an executable
witness — `conv`, `interpret_io_former`, `F.rec`, `resolve_instance` —
not assumed at the meta-level. Where the witness is bounded (fuel,
`n ≥ 3` endpoints stored unchecked, one-shot resumption) the bound is
explicit in the code this document names.
