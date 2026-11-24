+++
id = "ADR-0205"
title = "ADR-0205: N-Dimensional Arrow Calculus"
status = "open"
priority = "med"
+++

---
title: N-Dimensional Arrow Calculus
type: adr
date: 2026-09-15
status: proposed
status_note: |-
  Proposed — design finalized during review; ready for implementation.

  Refines [ADR-0052 Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md), [ADR-0100 Canonical Term Arrows](./0100-canonical-term-arrows.md), [ADR-0202 Frontier Calculus](./0202-frontier-calculus.md), [ADR-0203 Session Types / Grades](./0203-session-types-grades.md), and [ADR-0106 Object/Arrow Kernel](./0106-object-arrow-kernel.md).
---


# ADR-0205: N-Dimensional Arrow Calculus

> Imported from `kit-data-prelude/project` — originally `ADR-0119` (global registry repo `lang`); renumbered `ADR-0205` in the host merge.

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
grade. The default `A ~> B` means `A ~[1]~> B`. This is the _programmable
dimensionality_ requirement:

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


## Why

The canonical arrow calculus already treats programs, rewrites, proofs, and transformations as arrows. The pieces are in place but disconnected:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- A single calculus for programs, rules, proofs, and meta-transformations.
- The bracketed arrow now has one meaning (dimension) instead of two (effects
- Dimension polymorphism is ordinary parameterization by object values.
- Existing `RewriteEngine` and `data/transforms.yaml` become a dimension-2
- The generic, data-driven engine policy (ADR-0044, ADR-0081) is preserved:

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0205-n-dimensional-arrow-calculus.md`](../design/0205-n-dimensional-arrow-calculus.md).
