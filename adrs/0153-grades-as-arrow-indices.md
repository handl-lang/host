# ADR-0153: `~[n]~>` is a dimension index — n-cells as one indexed former

- Status: Accepted
- Date: 2026-10-22 (corrects the first draft, which conflated
  dimensions with coeffect grades)
- Repos: `kernel` (`handl-value`, `handl-language`), `kit-prelude-lang`,
  `engine-typecheck-nbe`, `lib-type`

## Context

`A ~[N]~> B` parses today as `dim_arrow` (ADR-0119: "N-dimensional
arrow"), `>>>[k]` is axis-parameterized composition (ADR-0120 Phase 2),
and `f ~[2]~> g` mints n-cell records at runtime. Two readings of the
bracket have coexisted: ADR-0106 §10 called it a *graded* arrow
(semiring grades, `gradeCompose`), while the grammar and `>>>[k]`
machinery treat it as a *dimension* — the level of the cell being
formed. Those are different axes; a coeffect grade ("use twice") and a
cell dimension ("equation between arrows") are not the same thing and
must not share one notation.

**This ADR resolves it: the bracket is the dimension.** `~[n]~>` forms
the type of n-cells — homs at level n. Semiring-style coeffect/usage
tracking (QTT quantities, erasure) is a separate, orthogonal axis —
deliberately out of the bracket (see §6).

The literature's shape applies cleanly once the conflation is removed:
indexed families (`Vec(n, a)`, ADR-0152) already attach type-level
data to a former; a cell dimension is the same device applied to the
hom former itself.

## Decision

### 1. `~[n]~>` is sugar for the dimension-indexed hom former

```handl
Hom : (n : Dim, src : Cell(n-1), tgt : Cell(n-1)) ~> Type
```

| surface | former | meaning |
|---|---|---|
| `A` | object | a 0-cell — an object/type/value |
| `A ~[1]~> B` (or `A ~> B`) | `Hom(1, A, B)` | a 1-cell — an arrow |
| `f ~[2]~> g` | `Hom(2, f, g)` | a 2-cell — an equation/rewrite between arrows |
| `e ~[3]~> e'` | `Hom(3, e, e')` | a 3-cell — an equation between 2-cells |

`Hom` is a nominal former per ADR-0152 §1: `Hom(2, f, g)` is the
canonical application `{ "__former__": homSym, "indices": [2, f, g] }`.
Dimensions are *indices* — type-level data, never present in values —
checked by the same machinery as `Vec`'s `n`.

### 2. Composition is `>>>[k]`, per dimension

`f >>>[k] g` composes cells along their shared k-boundary — already in
the grammar (`axis_flow`). The n-categorical structure is ordinary
declared data: each `>>>[k]`'s typing rule, identity `id_k`, and
interchange law are stated once for all levels, not baked per
dimension:

```handl
e    : f ~[2]~> g
eInv : g ~[2]~> f
e >>>[2] eInv ~[3]~> id(f)      -- coherence: an equation between 2-cells
```

`Path(a, b)` is then definable — an invertible cell is data, not a
former:

```handl
Path(a,b) := Sigma(e : Hom(2, a, b),
             Sigma(eInv : Hom(2, b, a),
               Hom(3, e >>>[2] eInv, id(a)) × Hom(3, eInv >>>[2] e, id(b))))
```

### 3. Dimension polymorphism is ordinary index binding

```handl
id<d : Dim>   : A ~[d]~> A
comp<d : Dim> : (f ~[d]~> g) ~> (g ~[d]~> h) ~> (f ~[d]~> h)
```

`d` is a `Pi`-bound index — the same mechanism as `List<T>`'s `T`
(ADR-0106 §8 dimension specialization, finally given semantics). One
`id`/`comp`/`refl` serves every cell level — no repeated definitions
per dimension.

### 4. Effects are objects in the codomain — never in the bracket

```handl
IO    : Type ~> Type          -- an effect former (or plain object)
print : String ~> IO(Unit)    -- a 1-cell producing an effect value
echo  := read >>> print       -- ordinary composition
handle echo with Console      -- ops interpreted by a handler
```

An "effectful arrow" is just an arrow whose target is an effect
object; `handle ... with` interprets it as today. No effect marks in
the index — the bracket is reserved for cell dimension.

### 5. Usage/erasure is a *mode*, not a dimension — out of scope here

QTT-style quantities (erased/typelevel vs runtime) are a different
axis again — MTT separates **modes** (which judgment/world a term
lives in) from cell structure within a mode. Erasure = membership in
the typelevel mode, tracked on the binder/context, orthogonal to
`~[n]~>`. Left to a follow-up ADR; the only commitment made here is
that it does *not* enter the bracket.

### 6. Retired readings

- The semiring-grade reading of `~[R]~>` (ADR-0106 §10,
  `gradeCompose` on the bracket) is withdrawn: the bracket forms
  n-cell types. Usage grades, if pursued, live in the mode axis (§5).
- `sort`/`grade`/`op` declaration forms remain, but their graded-arrow
  interpretation moves to modes; as type-level declarations they
  elaborate as ordinary formers.

## Consequences

- **One indexing mechanism**: `Vec(n,a)`, `Hom(k,f,g)`, `Arr(r,A,B)`
  (if usage grades are ever added) — every bracket is an index on a
  nominal former. The checker has one notion to implement.
- **Notation coherent**: `~[n]~>` (the n-cell type) and `>>>[k]` (the
  k-composition) now mean the same dimension `n`/`k`.
- **`Path` decomposes**: invertible equality needs no primitive — Hom
  + inverse + coherence cells, per §2.
- **Research honesty**: `>>>[k]` typing rules and interchange laws for
  general `k` are implemented for the levels exercised (k ≤ 2);
  arbitrary-k coherence remains aspirational per ADR-0100's directed
  limit — the former shape is dimension-generic, the checked laws are
  not yet.
- **Migration**: `~[2]~>` surface unchanged; `dim_arrow`'s payload is
  read as dimension index rather than grade expression; ADR-0106 §10's
  "graded arrows" wording needs a conformance amendment.

## References

- ADR-0106 §8 (dimensions `f<…>`), §10 (graded arrows — superseded reading)
- ADR-0119 (`~[N]~>` surface), ADR-0120 Phase 2 (`>>>[k]`)
- ADR-0152 (nominal inductive families — the indexing machinery)
- Riehl–Shulman simplicial type theory / Rzk (directed homs);
  Gratzer–Kavvos–Nuyts–Birkedal, MTT (the mode/grade/dimension split)
