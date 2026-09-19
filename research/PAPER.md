# An N-Dimensional Arrow Calculus for Programmable Computation and Rewriting

## Abstract

We propose a programming language based on a single arrow calculus applied recursively across dimensions. Morphisms are the primitive computational objects: types are objects, and a morphism `A ~[1]~> B` is a transformation between them. Ordinary programs are 1-morphisms. Rewrite rules, laws, and proofs are 2-morphisms. Transformations between laws are 3-morphisms, and so on.

Two independent indices organize the calculus. The morphism dimension `n` in `~[n]~>` identifies what kind of cell is being constructed; the composition axis `k` in `>>>[k]` identifies along which direction cells compose. Higher-dimensional cells admit several composition directions — vertical, horizontal, and beyond — each with its own identity and interchange structure.

Rather than introducing separate languages for programming, rewriting, proofs, and metaprogramming, the language uses the same morphism mechanism at every dimension. This provides a uniform foundation for generic rewriting, proof-carrying transformations, and optimization of transformation systems themselves. Algebraic effects are expressed using Sanada-style `op` and `handle`; dependent structure is expressed through families with `Π`/`Σ` quantification. Dimension 1 is implicit; higher dimensions are explicitly marked.

## 1. Core Idea: Morphisms, All the Way Up

The fundamental construct is a morphism:

```ts
f : A ~[1]~> B
```

Types are objects; morphisms are transformations between them. Under a Curry–Howard reading, a proposition is an object and a proof is a morphism — the same judgment covers computation and evidence.

Dimension 1 is the default and may be elided:

```ts
f : A ~> B
```

Higher-dimensional morphisms connect parallel lower-dimensional cells:

```ts
f : A ~[1]~> B
g : A ~[1]~> B

α : f ~[2]~> g
```

A transformation is therefore not a special compiler object. It is simply a morphism at the next dimension.

(Earlier drafts wrote the dimension bracket `-[1]->`; we adopt the `~[n]~>` spelling of the object/arrow kernel.)

The same combinators apply at every dimension:

```ts
f >>> g
f &&& g
f ||| g
first(f)
```

This gives a recursive structure:

```text
[1] programs             (morphisms between objects)
 ↓
[2] transformations      (morphisms between 1-morphisms)
 ↓
[3] meta-transformations (morphisms between 2-morphisms)
 ↓
[4] ...
```

## 2. Polymorphic Dimension

The morphism formers are a single dimension-indexed family:

```ts
M[1]<A, B>    -- 1-morphisms between objects
M[2]<f, g>    -- 2-morphisms between parallel 1-morphisms
M[3]<α, β>    -- 3-morphisms between parallel 2-morphisms
```

The sugar `X ~[n]~> Y` denotes `M[n]<X, Y>`; the boundary of an n-morphism is always a pair of parallel (n−1)-morphisms.

Dimension is polymorphic: `n` is a parameter, not a fixed tier. Consequently the structural operations are defined once for all dimensions rather than per level.

Polymorphic identity:

```ts
id : M[n] → M[n]
```

`id` lifts any cell to the identity morphism on it; the dimension is inferred from context. There are no separate `id1`, `id2`, `id3`.

Dimension-polymorphic functoriality:

```ts
map : M<A, B> → M<F A, F B>
```

`map` operates at whatever morphism dimension is appropriate: on 1-morphisms it is the ordinary functor action; on 2-morphisms it is the induced transformation between transformed morphisms; and so on upward.

## 3. Composition Axes

Dimension and composition axis are distinct concepts:

- `~[n]~>` answers _what dimensional morphism is this?_
- `>>>[k]` answers _along which composition direction do we compose?_

At dimension 1 there is essentially one direction:

```ts
f >>>[0] g        -- plain `f >>> g` is shorthand for the default axis
```

At dimension 2 there are already two. Given `α : M[2]<F, F'>` and `β : M[2]<G, G'>`:

- **Horizontal** composition pairs transformations across composition of their boundary morphisms — `F ~[2]~> F'` together with `G ~[2]~> G'` yields a 2-morphism between the composites `F >>> G` and `F' >>> G'` — written `α >>>[0] β` (`∘₀`).
- **Vertical** composition stacks transformations along the morphism direction — `F ~[2]~> F' ~[2]~> F''` — written `α >>>[1] α'` (`∘₁`).

Spatially: `>>>[k]` pastes cells along their shared `k`-boundary — axis 0 composes cells _across_ a structure (meeting at an object), axis 1 composes them _along_ it (meeting at a morphism). Higher dimensions add further compositional directions, and the coherence conditions between them.

The general form is `compose[k](a, b)` or `a >>>[k] b`, with axes `Nat`-indexed: `>>>[0]`, `>>>[1]`, `>>>[2]`, .... Unannotated `>>>` is shorthand for the default, inferred axis.

## 4. Laws are Higher Morphisms

The structural laws of the calculus are not declarations in a separate rule language. They are 2-morphisms:

```ts
rightUnit : (f >>> id) ~[2]~> f
assoc     : ((f >>> g) >>> h) ~[2]~> (f >>> (g >>> h))
```

An inhabitant of `M[2]` is simultaneously a rewrite rule, a proof, and a program transformation; the intended reading is supplied by context and by the handler that consumes it. Coherence between laws lives one dimension up, as an `M[3]` cell:

```ts
ω : α ~[3]~> β
```

Consequently the language requires no separate `laws` subsystem: `~[1]~>`, `~[2]~>`, `~[3]~>` are 1-, 2-, and 3-morphisms of a single mechanism. Whether a given law is inhabited by a strict equality or by a genuine higher cell is a coherence choice, not a syntactic distinction (§13).

## 5. Families and Dependent Structure

Dependent structure is expressed through families — type-valued structure over an object:

```ts
P : Family<A>          -- ≈ A ↝ Type
Π (x:A), P(x)
Σ (x:A), P(x)
```

For a morphism `f : A ~[1]~> B`, reindexing (substitution) pulls families on `B` back to families on `A`, and its adjoints give dependent sum and product along `f`:

```ts
Σ_f ⊣ f* ⊣ Π_f
```

Dependent quantification is therefore not a separate type-theoretic subsystem. It is structure transported along the same morphisms that compute.

## 6. Effects

Effects use algebraic operations and handlers:

```ts
Store<K, V> := {
  op read: K -> V
  op write: (K, V) -> Unit
}
```

Implementations are ordinary typed values:

```ts
DatabaseStore: Store<UserId, User>;
```

Generic computations depend on the abstract operation algebra:

```ts
getUser<S: Store<UserId, User>> :=
  id => S.read(id)
```

An implementation is selected through a handler:

```ts
handle Store with DatabaseStore
```

Thus abstraction and implementation remain separate while the compiler retains the algebraic structure necessary for effect reasoning. Effect annotations are independent of morphism dimension — an effect-tracked computation is still a 1-morphism. The grade bracket (`~[R]~>` in the object/arrow kernel) and the dimension bracket share one slot: a grade is a _named_ dimension, and handler routing is dimension-typed (§12).

## 7. Rules as Programs

A rewrite rule is a 2-morphism:

```ts
cache : (S.read >>> S.read) ~[2]~> S.read
```

Its type expresses the transformation it performs: source and target are the parallel 1-morphisms forming its boundary.

Rules can themselves be composed, along whichever axis is appropriate:

```ts
cache >>>[0] otherRule
```

and transformations of rules occupy the next dimension:

```ts
metaRule : cache ~[3]~> optimizedCache
```

Consequently, the language requires no independent rule language.

## 8. Programmable Dimensions

The dimension index may be generalized from a fixed natural number to an expression:

```ts
f: M[G]<A, B>;
```

where `G` is itself represented in the calculus — dimensions are data. This makes dimensional structure programmable rather than merely fixed. Dimension-polymorphic abstractions can therefore operate uniformly over programs, transformations, and higher transformations.

Conceptually:

```ts
transform<N>(x : M[N]<A, B>)
```

can describe transformations independent of the particular dimensional level.

A statically evaluable dimension discipline preserves compile-time reasoning while allowing programs to compute structural information about other programs.

## 9. Generic Rewriting

Because transformations are themselves morphisms, rewriting can be dimension-polymorphic.

The same machinery can operate on:

```text
programs
rules
meta-rules
proofs
transformations of proofs
...
```

This enables **meta-optimization**.

An optimizer can optimize programs:

```text
Program -> Program
```

while another higher-dimensional transformation can optimize the optimizer itself:

```text
Optimizer -> Optimizer
```

No separate metaprogramming mechanism is fundamentally required.

## 10. Static Reasoning

Higher-dimensional morphisms can serve as typed evidence for transformations:

```ts
Equal (S.read >>> S.read) S.read
```

or directly as cells:

```ts
(S.read >>> S.read) ~[2]~> S.read
```

Normalization and proof-directed rewriting allow the compiler to reason about properties such as:

- effect equivalence
- idempotence
- independence
- parallelizability
- cacheability
- invalidation
- retryability
- optimization correctness

The compiler therefore becomes capable of **effect reasoning and verified transformation**, rather than merely effect tracking.

## 11. Central Principle

The language is built around one principle:

> **Everything is a morphism, all the way up.**

Computation, proofs, laws, transformations, and higher laws share one mechanism — the morphism. The resulting syntax is correspondingly small:

```ts
f : A ~[1]~> B
α : f ~[2]~> g
ω : α ~[3]~> β

f >>>[0] g
α >>>[1] β

P : Family<A>
Π x:A. P(x)
Σ x:A. P(x)
```

Dimensions do not introduce new programming languages. They identify the level at which a morphism operates; axes identify the direction in which morphisms compose. This creates a unified foundation for functional programming, algebraic effects, rewriting, metaprogramming, and higher-dimensional computation.

## 12. Implementation Alignment

The architecture is realized by the kernel: the calculus described above is the running implementation, not a target.

The repository's `Term` calculus (`crates/plugin-engine/src/term.rs`) embodies _dimension-as-expression_: the arrow formers `Arr` and `Op` carry `dim: Box<Term>` (defaulting to the constant `1`), so dimension is data inside the calculus rather than a static annotation. `Src`/`Tgt` project the boundary faces of a 2-cell, and `Seq{axis, fst, snd}` carries the composition axis. The law-inference pass (`crates/handl-language/src/law_inference.rs`) normalizes `Term` graphs to a fixpoint and decides law equality by comparing normal forms — the strict fast-path for 2-morphism checking.

The four engineering prerequisites previously identified have all landed:

1. **Arrows as values** — a `function` in value position lowers to a quoted `Arr{value: Term}`; `call(f, x)` lowers to `pair(quote f, x) >>> app`. An inner function that references enclosing parameters lowers to a bound-arrow record `{__term__, __bound__}` — a product encoding, not a closure. `desugar.data` compiles recursors to `loop` fold machines over explicit defunctionalized work stacks rather than self-application (`ast.loop`/`eval.loop` data shapes).
2. **Axis slot on `Seq`** — `Term::Seq` carries `axis` (serde-defaulted to `0`); `f >>> g` is `>>>[0]`. `>>>[k]` surface syntax is pure YAML: `ast.axis_flow` (`left`/`axis`/`right` roles), `eval.axis_flow`, `type.axis_flow`.
3. **Source/target tracking in types** — `A ~[N]~> B` synthesizes `Arrow{inputs, grade, output, dim}` (`synth_dim_arrow`); `cell_boundary_`/`cell_paste_` in the typecheck model project and recompose k-indexed globular boundaries, so `a >>>[k] b` enforces `tgt_k(a) = src_k(b)` (`synth_axis_flow`). At the value level `>>>[k]` for `k > 0` executes as globular pasting `∘ₖ` via `cell2_comp_axis` with the same `tgt_k = src_k` enforcement; `Term::globular_dim` gates `k < dim`, and all execution/compile backends are dimension-aware.
4. **One dimension concept** — the grade bracket and the dimension bracket are one slot. A grade is a _named_ dimension (`Const "S"`); handler routing is dimension-typed (`substitute_capability` matches dims). `ArrSource::Foreign` is strictly the host-function boundary — a module-registry key, not a dimension.

Beyond the prerequisites, the surface layer is implemented as well:

- **Laws as `M[2]` terms** — `f ~[2]~> g` mints a genuine dimension-2 `Op` and feeds `law_inference`/`quantified_laws` directly; there is no separate laws fragment. The former frontier ops are derived schemes over `Seq{axis}` in `eval.yaml`: `cell_comp_h`/`cell_comp_v` are `>>>[0]`/`>>>[1]` at dimension 2, `whisker_*` is `id₂ ∘₀ α`, and `cell_id` is `id`. Unrecognized derived operators fold to `Term::Opaque` tagged records, making the `Term`/surface conversion total.
- **Dependent structure** — `Family`/`Π`/`Σ`/`Forall` are recognized in type-expression position; `reindex` is ordinary composition (`reindex := (f, P) ~> f >>> P`); the `Σ_f ⊣ f* ⊣ Π_f` unit/counit laws are `~[2]~>` terms (`handl-packages/type/dependent.hnl`). `data` declarations bind `type.family`/`type.family_app`; constructor signatures (`type.ctor_sig`) instantiate per use, so `Succ(3)` is rejected; index terms normalize computationally (`Vec(1 + 1)` ≡ `Vec(2)`, commutative canonicalization, ring identities, linear solving); `match` patterns bind constructor fields.
- **Scheme-driven lowering** — `lower_via_scheme`/`instantiate_scheme` instantiate `EvalSchemeDef` `Term`s directly: YAML-declared constructs lower with zero Rust arms (`input_faithful` tracking, `seq_pastes` globular pasting). `pattern_role`/`pattern_role_is` drive kind dispatch in pattern matching and all execution engines via `AstShapeDef` roles.

The remaining gaps are documented approximations rather than missing machinery; they are enumerated in §13.

## 13. Open Questions

The design is settled architecturally and the core mechanics are implemented (§12); the open work is metatheory plus a small list of documented approximations.

**Cells and composition**

1. What exactly is `M[n]` — partially answered by the implementation: at the value level an n-morphism is a `Term` whose `dim` slot denotes `n` (a dimension-2 `Op`, or a `Cell2` tagged record for computed 2-cells); at the type level `M[n]<A, B>` is `Arrow{inputs, grade, output, dim}`. Whether that suffices as a family of categories is a metatheory question, not an engineering one.
2. What are the typing rules for `>>>[k]`? — answered: `tgt_k(a) = src_k(b)`, enforced statically by `synth_axis_flow` (`cell_boundary_`/`cell_paste_`) and dynamically by `cell2_comp_axis`.
3. Which axes exist for a given `n`? — answered operationally: `k < dim` is required and checked. The value model carries a single cell shape (`Cell2`, dimension 2) whose only representable boundary sits at level 1, so axes 0 and 1 execute meaningfully and `k ≥ 2` is rejected by the typechecker (and fails `k < dim` at the value level for ordinary operands). Generalizing the value model beyond 2-cells is open.
4. What identity and associativity structures exist at every level? — `id` is dimension-polymorphic through the `dim` slot and `cell_id` derives to `id`; higher associativity and unitor structure as inhabited cells remain open.
5. How are horizontal and vertical composition related, and which interchange and coherence laws hold between axes? — open.

**Strictness and shape**

6. Are higher laws strict equalities or genuine higher morphisms? — resolved: weak (proof-relevant). `~[2]~>` cells are first-class values composable via `>>>[k]`; `law_inference` remains as the strict fast-path where a canonical form suffices. `M[n]<A, B>` with `A, B ∈ M[n-1]` is the **globular** presentation, confirmed by the syntax.
7. What categorical structure results overall — strict, weak, globular, cubical? — resolved for the implemented fragment: weak globular. Cubical Kan structure (`hcomp`, `transport`, `Equiv` computing equality) is deliberately deferred and would arrive as a cube-diagram fragment rather than a rewrite of the directed calculus.

**Interaction with the rest of the language**

8. How do `Family`, `Π`, and `Σ` interact with higher morphisms? — the structural fragment is implemented (§12); the adjunction laws are `~[2]~>` terms. Still open: families indexed by `M[2]` cells, and dependent elimination — refining a scrutinee's family index per `match` arm. True dependent elimination needs an equality type/proof term (`v ≡ Ctor …`) to rewrite the index under each arm, machinery the structural fragment deliberately does not carry.
9. Is there a real distinction between computation cells and proof/law cells? — partially answered: one mechanism. Law cells are dimension-2 `Op`s and derived operators fold to `Term::Opaque` records; the computation/proof distinction lives in the consumers (eval engines vs `law_inference`), not in the calculus.
10. How does Sanada's operational semantics lift through dimensions? — partially answered: `op`/`handle` ride the unified dimension slot (a grade is a named dimension; routing is dimension-typed). The full dimensional lift of the operational semantics remains open.

**Metatheory**

11. Which dimension expressions are statically normalizable? — open; only literal axes and dimensions resolve today (a non-literal axis falls back to the 0-boundary with a warning).
12. Under what conditions can higher-dimensional transformations safely rewrite programs? — open.
13. How can generic rewriting remain terminating and decidable? — open.
14. What computational power and practical benefits arise from programmable dimensionality? — open.

**Documented approximations** (implementation debt, not design questions)

- `unify` on `type.function` is shallow (field-wise); dependent-aware unification lives in `unify_dependent_`, and a composite `f >>> P` of dependent operands flattens to `type.function` — no AST body exists to carry a `body_closure` across composition.
- `cell_boundary_` assumes globularity: the globe condition `src_j(inputs) = src_j(output)` is assumed, not verified.
- `AstShapeDef` declares a single `pattern_role` per shape; optional-role support for `match`/`assign` optional children is still needed — they carry bespoke handling.
- `desugar.data`/`desugar.match_recursor` remain semantic natives (per-constructor iteration, strict positivity, arena search) — genuinely semantic work awaiting a macro system rather than a rewrite step.
- The arrow lowerer's `match node.kind` table is reduced but not eliminated: the env/substitution constructs (`function`, `call`, `assign`, `flow`, …) keep Rust arms; only YAML-declared schemes lower generically.

The goal is a language in which **computation and reasoning about computation are expressed by the same calculus**, recursively and statically.
