# ADR-0119: N-Dimensional Arrow Calculus

## Status

Proposed — design finalized during review; ready for implementation.

Refines [ADR-0052 Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md), [ADR-0100 Canonical Term Arrows](./0100-canonical-term-arrows.md), [ADR-0101 Frontier Calculus](./0101-frontier-calculus.md), [ADR-0102 Session Types / Grades](./0102-session-types-grades.md), and [ADR-0106 Object/Arrow Kernel](./0106-object-arrow-kernel.md).

## Context

The canonical arrow calculus already treats programs, rewrites, proofs, and
transformations as arrows. The pieces are in place but disconnected:

- Dimension-1 arrows: `function` / `graded_arrow` in `data/ast.yaml`, `Arr`,
  `Op`, `Seq`, `First`, `Choice`, `Loop`, `App` in `Term`.
- Dimension-2 arrows: a 2-cell is an arrow `f ~[2]~> g`. Rewrite rules live in
  `data/transforms.yaml` and are just `dim=2` arrows; no new surface keywords are
  added.
- Proof and path forms (`Equiv`, `Path`, `Witness`, `Refine`, `Univalence`, `Glue`, `Fibrant`, `JElim`) are represented as `Arr`/`Op` terms at the appropriate dimension.
- Sanada-style effects: `op`, `handler`, `with`, `handle` and `Term::Handle`.
- Dimensions: `f<X>` / `f<X, Y>` surface is application, not a native
  dimension-polymorphic layer (ADR-0106 §Phase 4 conformance note).
- Frontier cell combinators (`CellCompV`, `CellCompH`, `Whisker`, `CellId`) are
  redundant once every `Arr`/`Op` carries a dimension index.

This ADR unifies those pieces into one **N-dimensional arrow calculus**. The
goal is the principle stated in the design brief: *everything is an arrow, all
the way up*. No separate rule, proof, or metaprogramming language is needed;
each level uses the same arrow syntax and the same composition combinators.

## Decision

### 1. Remove the graded effect arrow

The `~[R]~>` graded effect-arrow syntax and the `EffectRow` tracking
mechanism are removed. The bracket is now the single **dimension** index, and
effect handling moves entirely into Sanada-style `op` and `handle`.

```handl
f : A ~[1]~> B      -- 1-cell: ordinary program
r : f ~[2]~> g      -- 2-cell: transformation / rule
m : r ~[3]~> s      -- 3-cell: meta-transformation
```

`A ~[N]~> B` is the only bracketed arrow form.

### 2. Dimension is a first-class object

`N` is an ordinary object in the `Obj` world (ADR-0106 §1). It may be a literal
natural number, a dimension variable, or any term that normalizes to a static
grade. The default `A ~> B` means `A ~[1]~> B`. This is the *programmable
dimensionality* requirement:

```handl
transform<N>(x: A ~[N]~> B)
```

is ordinary parameterization, exactly like `identity<T>`.

### 3. Add a `dim` field to `Term::Arr` and `Term::Op`

The dimension index is carried by the two primitive `Term` constructors:

```rust
pub enum Term {
    Arr { dim: Box<Term>, op: String, args: Vec<Term>, source: ArrSource },
    Op  { dim: Box<Term>, op: String, args: Vec<Term> },
    // ... existing combinators (Seq, First, Choice, Loop, App, Handle, ...)
}
```

`dim` is a `Term` so that it can be a literal, a variable, or a dimension
expression. Every other combinator (`Seq`, `First`, `Choice`, `Loop`, `App`,
`Handle`) is dimension-polymorphic: it composes sub-terms, and the primitive
leaves carry the dimension. This is the only calculus-vocabulary change in the
Rust kernel; everything else is a data change in YAML fragments.

### 4. Frontier cell constructors fold into generic combinators

`CellCompV`, `CellCompH`, `Whisker`, and `CellId` are removed from `Term`.
Their roles are subsumed by `Seq`, `First`, `Choice`, and the `dim` field:

```handl
r >>> s      -- Seq at the dimension of r and s
r ||| s      -- Choice at that dimension
first(r)     -- First at that dimension
id<N>        -- Arr { dim, op: "id", ... }
```

The proof and path forms (`Equiv`, `Path`, `Witness`, `Refine`, `Univalence`,
`Glue`, `Fibrant`, `JElim`) are folded into `Arr`/`Op` at the appropriate
dimension. A proof is an arrow: a path `p : a = b` is an `Arr` at the
dimension of the equality, and a witness `Witness(g, p)` is an `Arr`/`Op` whose
`dim` is the grade/dimension `g`.

### 5. Single calculus at every dimension

The same combinators operate at every dimension:

```handl
>>>                  -- vertical composition at the current dimension
&&&                  -- fan-out
|||                  -- choice
first(f)             -- carry context
```

At dimension 1 these are the existing arrow calculus combinators. At dimension
2 they are 2-cell vertical/horizontal composition. The same `Term` combinators
are reused; the only difference is the `dim` field on the `Arr` / `Op` leaves.

### 6. Effects and handlers at every dimension

`op` and `handle` are 1-cell forms today. At dimension `N` an operation is an
arrow from `(N-1)`-cells to `(N-1)`-cells, and a handler is an `(N+1)`-cell
that maps an effectful `N`-cell to a pure one.

A rewrite rule is therefore a typed 2-cell:

```handl
cache : (S.read >>> S.read) ~[2]~> S.read
```

It is an ordinary arrow at dimension 2, not a special compiler object.

### 7. Rules, laws, and proofs are the same data

2-cells and higher cells are ordinary arrows. No new `rule`, `law`, or `meta`
keywords are added beyond `op P` and `handle P with H`:

```handl
cache       := (S.read >>> S.read) ~[2]~> S.read
id_left     := id(A) >>> f ~[2]~> f
optimized   := cache ~[3]~> optimizedCache
```

Proof-carrying transformations are ordinary arrows. A witness is an `Arr`/
`Op` at the dimension `g` carrying the proof `p`; refinements, paths,
equivalences, and univalence are likewise `Arr`/`Op` terms with the appropriate
`dim`. The `lang.transforms` table (ADR-0094) is the runtime representation of
dimension-2 (and higher) programs: rewrite rules are data, not engine code.

### 8. Generic, dimension-polymorphic rewriting

Because transformations are programs, the generic `RewriteEngine` is a
dimension-1 interpreter for dimension-2 programs. A higher-dimensional rewrite
pass is simply another pass in `data/transforms.yaml` whose `node_kind`
dispatches on `dim_arrow` / `rule` forms. No engine branch on `kind ==` is
introduced.

### 9. Surface syntax

Promoted forms:

```handl
A ~[N]~> B          -- N-dimensional arrow
f ~[2]~> g          -- 2-cell transformation
id<N>(f)            -- N-cell identity
r >>> s             -- vertical composition at the current dimension
r &&& s             -- fan-out
r ||| s             -- choice
first(r)            -- carry context
```

The `~[N]~>` token replaces `~[R]~>` everywhere. `A ~> B` remains shorthand for
`A ~[1]~> B`.

## Consequences

### Positive

- A single calculus for programs, rules, proofs, and meta-transformations.
- The bracketed arrow now has one meaning (dimension) instead of two (effects
  vs dimensions).
- Dimension polymorphism is ordinary parameterization by object values.
- Existing `RewriteEngine` and `data/transforms.yaml` become a dimension-2
  interpreter without engine changes.
- The generic, data-driven engine policy (ADR-0044, ADR-0081) is preserved:
  the only Rust changes are a `dim` field on two `Term` variants and the removal
  of `CellCompV` / `CellCompH` / `Whisker` / `CellId` and all proof/path
  `Term` variants.
- Meta-optimization becomes a library: an `Optimizer -> Optimizer` 3-cell is a
  regular arrow.

### Negative

- Removing `~[R]~>` and `EffectRow` is a breaking change for existing canonical
  packages that use graded effect rows; they must migrate to `op` / `handle`.
- `Term::Arr` and `Term::Op` get a new field, and frontier / proof `Term`
  variants are removed — all are breaking serial-format changes for stored arrow
  data.
- `Handle` and the typechecker must cover the capability discipline previously
  enforced by `EffectRow`.
- Termination and decidability for dimension-polymorphic rewrites remain open
  research questions.

## Research Questions

1. What are the precise typing and boundary rules for `N`-cells when `N` is a
   variable?
2. Which dimension expressions are statically normalizable?
3. How are equality and rewriting represented and checked at each dimension?
4. How does Sanada's operational semantics lift through `N`-cell handlers?
5. Under what conditions can a higher-dimensional transformation safely rewrite
   a lower-dimensional program?
6. How can generic `N`-dimensional rewriting remain terminating and decidable?
7. What is the minimal set of `op`/`arr` names needed for proof and path forms
   once `CellComp`/`Whisker`/`CellId` and all proof `Term` variants are folded
   into generic combinators?

## Implementation plan

### Phase 0 — remove `~[R]~>` and add `~[N]~>` data

1. Remove `graded_arrow` from `crates/handl-kit-data-canonical/data/ast.yaml`,
   `data/grammar.yaml`, `data/lexer_rules.yaml`, `data/eval.yaml`,
   `data/types.yaml`, and `data/transforms.yaml`.
2. Remove `EffectRow` from `crates/handl-language/src/effects.rs` and from all
   typechecker / eval references.
3. Add `dim_arrow` to `data/ast.yaml` with roles `source`, `dim`, `target`.
4. Add `~[N]~>` grammar/lexer entries; reuse the existing `~[` and `]~>` token
   boundaries if possible.
5. Add `eval.dim_arrow` producing `Term::Arr { dim, ... }` or `Term::Op`.
6. Add `type.dim_arrow`; initial return can be `type.unknown`.
7. Update `transform.desugar.*` to lower `dim_arrow` to `Term`.

### Phase 1 — add `dim` field and fold frontier cells

1. Add `dim: Box<Term>` to `crates/plugin-engine/src/term.rs` for `Arr` and
   `Op`.
2. Remove `CellCompV`, `CellCompH`, `Whisker`, `CellId`, `Equiv`, `Path`,
   `Witness`, `Refine`, `Univalence`, `Glue`, `Fibrant`, and `JElim` from `Term`.
   Their surface forms become `Arr`/`Op` terms with the appropriate `dim`.
3. Update `ValueModel` and `TypeModel` to read `dim` and to map 2-cell
   composition to `Seq` / `First` / `Choice`.
4. Update serialization in `crates/handl-language/src/fragments/eval_scheme.rs`
   and `type_scheme.rs`.
5. Default `dim` to `Const(1)` for backward compatibility until a migration
   pass is written.

### Phase 2 — dimension-polymorphic composition

1. Surface `id<N>`, `r >>> s`, `r &&& s`, `r ||| s`, `first(r)` for `dim`
   terms.
2. Add `law` / `rewrite` at arbitrary dimension in `data/ast.yaml` and
   `data/eval.yaml`.

### Phase 3 — transforms and tests

1. Add a `meta-transform` pass in `data/transforms.yaml` that matches and
   rewrites `dim_arrow` / `rule` nodes.
2. Add test cases to `crates/handl-kit-data-canonical/tests/`.
3. Run `cargo check`, `cargo test --workspace`, `hnl fmt --check`, `hnl lint`,
   `hnl typecheck`, and the dogfood gate script.

### Phase 4 — type system integration

1. Replace `type.unknown` for `dim_arrow` with a real `~[N]~>` former.
2. Implement `Equal<N>` and dimension-indexed subtyping in `data/types.yaml` /
   the Handl type model.
3. Remove any residual `EffectRow` assumptions in the typechecker.

## Conclusion

The N-dimensional arrow calculus is not a new language. It is the existing
arrow calculus, exposed at every dimension. The changes are: a `dim` field on
`Term::Arr` and `Term::Op`, the removal of the `~[R]~>` grade surface and
`EffectRow`, the promotion of `~[N]~>` as the single dimension bracket, and the
folding of frontier cell combinators into the generic arrow combinators. The
rest is a data change — new `ast`, `grammar`, `lexer`, `eval`, `types`, and
`transforms` fragments — consistent with the data-driven language policy.
