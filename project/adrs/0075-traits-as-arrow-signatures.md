# ADR-0075: Traits as Arrow Signatures — Graph-Based Interfaces and Contracts

## Status

Proposed. **Partially superseded by [ADR-0076 — Decoupled Trait Layer](./0076-decoupled-trait-layer.md):**
the bundled `trait`/`impl`/`law` syntax (§2, §6, §7, §8, §13) is
superseded by three independent composable features (laws, bundles,
handlers). The research foundation (§3 catalog, §4 storage, §11 CutElim,
§12 catalog search as fallback) is preserved and remains the substrate
for ADR-0076. Prototype implemented in the arrow-calculus PoC.
Awaiting:

- Lean4 mechanization of trait inference for law patterns (in progress;
  functor laws proven, non-functor laws require semantic axioms — see
  `EquationalLawInference.lean`).
- CutElim law checking mechanization (part of this ADR, §11;
  congruence + strong normalization for the 6-rewrite fragment proven
  — see `CutElimCongruence.lean`).
- Handler-based resolution implementation (§13) — design complete,
  implementation pending.
- Catalog storage interface + SQLite implementation (done — see
  `trait_catalog_sqlite.rs`).
- Built-in catalog population (done — functor, coproduct functor,
  sorting, mapping, filtering).
- Prototype implementation in the arrow-calculus PoC (done — see
  `RESULTS_TRAITS.md`).

Builds on [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md)
(`arr`/`>>>`/`first`/`choice`/`loop`/`app` as the kernel),
[ADR-0068 — Predicates as Types](./0068-predicates-as-types.md) (types as
predicates, dependent typing without a sublanguage),
[ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md) (modules are
arrows), and [ADR-0074 — Arrow Surface Syntax v0.1](./0074-arrow-syntax-v0_1-surface-redesign.md)
(human-facing syntax over the kernel). Extends the kernel with a
**trait/impl** layer grounded in arrow signatures and graph assignments,
without introducing a separate interface sublanguage.

## Context

### What handl already has

ADR-0052 fixed the kernel: programs are arrow graphs built from
`arr`/`>>>`/`first`/`choice`/`loop`/`app`. ADR-0068 made types into
predicates — ordinary values that return `bool` — with dependent typing
falling out without a `total` sublanguage. ADR-0069 made modules into
arrows. ADR-0074 fixed the surface syntax.

What is missing is the **interface/contract layer**: the ability to name a
bundle of related arrow signatures, declare equational laws over them, and
verify that a graph satisfies those laws. In Rust/Haskell this is
`trait`/`impl` (or `typeclass`/`instance`); in TypeScript it is
`interface`. handl has no equivalent.

### The research foundation

The arrow-calculus PoC research (`research/arrow-calculus-kernel-poc/`)
establishes the substrate:

1. **The signature `Σ` is already a trait declaration.** In
   `FORMALISM_PROOF_GRAPHS.md` §1.1, an arrow graph is built over "a
   signature of base arrows `φ : A → B`." This is precisely a trait: a set
   of named operations with typed interfaces. The `T-Arr` rule
   `φ : A → B ∈ Σ ⊢ arr(φ) : A ~> B` is the rule that says "an impl
   provides this method."

2. **The `Language` protocol is already a value-level trait with impls.**
   `RESULTS_PHASE3.md` shows `Language = { parse, eval, format, typecheck,
   lit }` — a record of arrows. Arithmetic and Boolean are two impls. The
   research states: *"A compiler/tooling plugin interface is not a
   separate language feature — it is arrow composition over a record of
   arrows."* That *is* a trait + two impls, at the value level. This ADR
   lifts it to the type level.

3. **Curry-Howard gives law proofs for free.** The correspondence
   `TyOf G ⟺ PrOf G` (mechanized in `lean/ArrowMinimalityLib/ProofGraphs.lean`,
   round-trip bijection proven) means: if an impl graph `G` type-checks at
   the trait's type, the typing derivation *is* a proof. Laws that fall
   out of structure (compositionality, functoriality of `first`,
   distributivity) are proven by the graph's wiring — no separate proof
   term.

4. **Annotation-free checking of impls against traits.**
   `poly_proof_graph_tests.rs` shows HM inference over graphs with zero
   annotations: `comp(first(id), id) : (α × β) ~> (α × β)` is inferred from
   wiring alone. So an impl graph is checked against a trait signature
   automatically — the user writes the graph, the system derives whether
   it satisfies the trait.

5. **Dependent types give associated types.**
   `lean/ArrowMinimalityLib/DependentProofGraphs.lean` extends the proof
   system with `dPi`/`dSigma`, where the output type depends on the input
   value. This is the substrate for `trait Iterator { type Item; ... }` —
   the associated `Item` is a dependent type indexed by the impl.

6. **ArrowApply gives blanket impls.** Challenge #11 establishes that
   ArrowApply (in the explicit/freer style) gives monadic power without
   opacity. A blanket impl `impl<T: U> T for [T]` is a higher-order arrow
   producing graphs from graphs — exactly ArrowApply.

### The tension with "annotation-free"

The invariant-inference work (`PAPER_INVARIANT_INFERENCE.md`) makes a
strong claim: the user writes only the graph, and types/proofs fall out.
Traits reintroduce a declared layer — the trait signature is an
annotation, not inferred from the graph.

This is **already true of `Σ`**: the base-arrow types are declared, not
inferred. The "annotation-free" claim is about the *program graph*, not
the *signature*. Traits are the same: the trait declaration is a
signature-level annotation; the *impl* (the graph) remains
annotation-free. The proof that the impl satisfies the trait is still in
the wiring.

The honest position: **traits sit at the signature layer (declared),
impls sit at the graph layer (annotation-free, proof-carrying).** This is
consistent with the existing framing — it does not break the
annotation-free property of programs, it just names the signature layer
explicitly.

### What is NOT yet proven (and how this ADR handles it)

1. **Equational law verification on arbitrary impls.** The current
   mechanization proves *subject reduction* (type soundness) and the
   *typing⇔proof bijection*. It does **not** prove that a specific impl
   graph satisfies an arbitrary equational law like `f >>> id ≡ f` or the
   functor law. The `FORMALISM.md` "Arrow Laws" section *states* these
   laws but they are not discharged as proof obligations on impl graphs.

   The right tool exists: `CutElim` (`ProofAdvantage.lean`) is a
   rewrite relation (`idLeft`, `idRight`, `firstFusion`, `leftFusion`)
   that normalizes graphs. Two graphs are law-equal iff they normalize
   to the same form. This is the proof-net advantage
   (`SYNTHESIS_PROOF_NETS_TO_PL.md`) — local, binder-free cut
   elimination. It just has not been connected to trait-law checking yet.

2. **Trait resolution / coherence.** When multiple impls could satisfy a
   trait, Rust/Haskell resolve via coherence. In the arrow calculus this
   is *program synthesis*: given a target type `A ~> B`, find a graph.
   This is undecidable in general. The structural transparency of arrow
   graphs makes heuristic search more tractable than over λ-terms (the
   dependency graph is explicit, fanout is visible), but there is no
   result here yet.

This ADR proposes the **surface** and **semantics** of the trait/impl
layer, and explicitly marks law verification and trait resolution as
**deferred to follow-on ADRs** pending the cut-elimination and synthesis
work.

### What traits add vs what is sugar

It is worth being precise about what the trait/impl layer contributes
beyond the bare kernel, because most of it is sugar:

1. **Named bundles of arrows** — sugar for a record of arrows. Phase 3's
   `Language` protocol already does this without trait syntax:
   `Language = { parse, eval, format, typecheck, lit }`. A trait is just
   a named, typed version of the same record.

2. **Passing arrows as parameters** — sugar for first-class arrows,
   already in the kernel. Without traits:
   ```handl
   sort_by = map => cmp => arr cmp >>> map
   ```
   The trait bound `Functor<F> =>` is sugar for "expect a `map` arrow as
   an implicit parameter."

3. **Resolution** — sugar for finding the right arrows for a type.
   Without traits, the caller supplies them explicitly. With traits, the
   system finds them. **Revised in §13:** the primary resolution
   mechanism is explicit handler installation (`handle ... with`),
   which is dependency injection over effect handlers — the caller
   wires the impl at a scope boundary. Catalog search (§12) is the
   fallback when no handler is installed.

4. **Named equational laws** — **NOT sugar.** This is the one thing the
   bare calculus cannot express. A `law` line requires that two arrow
   compositions normalize to the same graph, and lets that requirement
   be **reused** at every call site of the trait. Without the trait
   layer, you can write both sides of an equation, but you cannot
   *require* that an arbitrary impl satisfies it, or *compose* the proof
   that it does.

   This is what makes `migrate = deserialize >>> serialize` **proven
   lossless** rather than merely hoped-correct: each impl's roundtrip
   law was verified once, and `migrate` inherits the proof by
   composition. Without the trait layer, this would need re-verification
   at every call site.

**Summary:** traits are sugar for bundling, parameter passing, and
resolution. The non-sugar part — law declarations with compositional
proof reuse — is the real addition. Everything else is convenience over
what the kernel already does.

### The contingency on law inference

The usefulness of the trait layer **depends on extending invariant
inference to equational law patterns**. The invariant-inference
machinery is demonstrated for output properties (sortedness,
permutation, length) with 100% accuracy on 72 benchmark cells, but has
not yet been extended to equational laws (functor, monad, roundtrip).
Three outcomes:

- **If trait inference extends cleanly to law patterns** (the primary
  mode): the trait layer becomes a genuine advantage over Rust/Haskell
  — laws are inferred from wiring, not declared, and proofs compose
  with `>>>`. The user writes only the graph; the system reports which
  laws it satisfies. Explicit trait declarations become optional
  documentation.
- **If inference does not extend but `CutElim`-based law checking works**
  (the secondary mode): laws are verified when explicitly declared,
  giving proven-but-declared traits — still an advantage over
  Rust/Haskell (proofs, not just types), but requiring declaration.
- **If neither works for realistic laws**: the trait layer reduces to
  sugar (named bundles + implicit parameters) without a `law` keyword
  — useful, but no more powerful than Rust/Haskell traits.

This ADR commits to the primary mode (inference) as the default, with
the secondary mode (explicit declarations + `CutElim`) as fallback. The
Lean4 mechanization of trait inference is in progress.

### The primary mode: trait inference

The invariant-inference machinery (demonstrated in
`PAPER_INVARIANT_INFERENCE.md` for semantic properties like sortedness,
permutation, and length) extends naturally from output properties to
**structural law patterns**. The same structural pattern recognition
that infers `Sorted(output)` from a sort graph can recognize that a map
graph satisfies the functor laws — `map id ≡ id` and
`map (f >>> g) ≡ map f >>> map g` — **without any trait declaration**.

This makes **trait inference the primary mode** of the trait layer:

- The user writes a graph.
- The system recognizes which equational laws the graph satisfies, by
  structural pattern recognition + `CutElim` normalization.
- The system reports: *"this graph satisfies the functor laws, the
  serializer roundtrip law, the semigroup associativity law."*

At this point, explicit trait declarations become **optional** — names
for law patterns the system already detects, used for documentation and
explicit intent, not as a semantic requirement:

- `trait Functor` = a name for "graphs satisfying `map id ≡ id` and
  `map (f >>> g) ≡ map f >>> map g`"
- `impl Functor<List>` = "this graph happens to satisfy those laws"
  (recognized, not declared)

The laws are in the wiring; the system reads them off. This is
consistent with the invariant-inference paper's central claim: the user
writes one thing (the graph) and gets the type, the proof, and now the
trait conformance — all for free.

### What is proven vs aspirational

The invariant-inference machinery is demonstrated for **output
properties** (sortedness, permutation, length) with 100% accuracy on
72 benchmark cells (`PAPER_INVARIANT_INFERENCE.md`). Extending it to
**equational law patterns** (functor, monad, roundtrip) is the subject
of active Lean4 mechanization. The `CutElim` rewrite relation
(`ProofAdvantage.lean`) provides the normalization foundation; the
structural pattern recognition extension is in progress.

This ADR commits to trait inference as the primary mode on the strength
of the research direction. If the mechanization does not extend cleanly
to law patterns, the ADR falls back to explicit declarations with
`CutElim`-based law checking (the secondary mode below).

## Decision

**Trait inference is the primary mode. The system recognizes which
equational laws a graph satisfies, without declaration. Explicit trait
declarations are optional — names for law patterns the system already
detects.**

No new calculus primitive is introduced. Trait inference is a
**recognition layer** over the existing kernel, extending the
invariant-inference machinery from output properties to equational law
patterns.

### 1. Primary mode: trait inference (default)

The user writes a graph. The system derives:

1. **Type** — by HM inference over the graph (annotation-free, per
   `poly_proof_graph_tests.rs`).
2. **Semantic properties** — by invariant inference (sortedness,
   permutation, length, per `PAPER_INVARIANT_INFERENCE.md`).
3. **Equational laws satisfied** — by structural pattern recognition +
   `CutElim` normalization (the extension this ADR commits to).

```handl
-- the user writes only this:
map = loop (arr uncons >>> choice
              (arr nil)
              (first f >>> arr cons))

-- the system reports:
--   map : (α ~> β) ~> ([α] ~> [β])
--   satisfies: functor identity law      (map id ≡ id)
--   satisfies: functor composition law   (map (f >>> g) ≡ map f >>> map g)
```

No `trait` declaration. No `impl` declaration. No `law` declaration.
The system reads the laws off the wiring.

### 2. Secondary mode: explicit declarations (optional)

When the user wants to **name** a law pattern — for documentation,
explicit intent, or to constrain generic code — they may declare a
trait explicitly. This is sugar over the inference:

A trait is a named schema of typed arrow operations plus equational laws
between arrow compositions:

```handl
trait Functor<F> {
  map : (A ~> B) ~> (F<A> ~> F<B>)

  law identity:    map id        ≡ id
  law composition: map (f >>> g) ≡ map f >>> map g
}
```

- `F` is a **type parameter** of the trait (a type constructor variable).
- `A`, `B` are universally quantified over the trait's scope.
- Each operation is a typed arrow signature `φ : A ~> B` — an entry in the
  named sub-signature `Σ_T`.
- Each `law` is an **equation between arrow compositions** — two graphs
  that must normalize to the same form under `CutElim`. The `law`
  keyword is a first-class part of the trait syntax: it makes the
  equational requirement explicit and readable, and it is what gets
  indexed in the trait catalog (§3) for inference.

The trait declaration is a **signature-level annotation** — a name for
a law pattern the system can already recognize. It is optional: the
system infers trait conformance without it. The declaration is for
documentation, explicit intent, and constraining generic code.

Declaring a trait also **adds it to the open catalog** — the indexed
library of known law patterns that the inference engine checks against
(see §3 below). Without the declaration somewhere in the catalog, the
system cannot recognize the pattern. The laws must be written once — by
whoever adds the pattern to the catalog. After that, every graph
matching it is recognized automatically.

### 3. The open trait catalog

Trait inference does not magically derive unknown laws from nothing. It
**matches a graph's structure against known law patterns** in an open,
indexed catalog. This is the same approach the existing invariant
inference uses (`LoopInvariantDiscovery.lean` recognizes side-channel,
accumulator, and filter patterns from a hardcoded set) — generalized
from hardcoded patterns to an open, user-extensible library.

#### What the catalog stores

Each catalog entry is a named law pattern:

```
entry = {
  name:       "Functor",
  type_shape: (A ~> B) ~> (F<A> ~> F<B>),    -- for type filtering
  fingerprint: loop(comp(arr, choice(arr, first))),  -- structural shape hash
  laws: [
    { name: "identity",    lhs: map id,        rhs: id },
    { name: "composition", lhs: map (f >>> g), rhs: map f >>> map g },
  ],
  cutelim_proven: true,    -- whether CutElim normalization is proven for this pattern
}
```

#### The matching pipeline (fast to slow)

```
Step 1: Type filter (instant, O(1) hash lookup)
  - HM infers the graph's type
  - Each catalog entry has a type shape
  - Prune by unification: keep only entries whose type shape unifies
  - Most of the catalog eliminated here

Step 2: Structural fingerprint (fast, hash compare)
  - Precompute a shape hash for the graph (combinator tree as canonical hash)
  - Each catalog law pattern has a fingerprint
  - Match by hash; collision → proceed to step 3; no collision → skip
  - O(1) per remaining candidate

Step 3: CutElim normalization (slow, only on candidates)
  - Normalize both sides of each law equation
  - Compare canonical forms
  - Only runs on graphs that passed both fast filters
```

Most graphs hit "skip" at step 1 or 2. Only genuine candidates reach the
expensive CutElim step.

#### Why arrow graphs make this fast

The graph is built from **5 combinators** (`arr`/`comp`/`first`/`left`/
`loop`) — a small, fixed alphabet. The combinator tree is shallow (most
graphs are 5-10 nodes deep). This means:

- Fingerprints are small and cheap to compute
- The combinator tree can be indexed as a **trie** — prefix-match the
  tree shape
- The catalog can be pre-indexed by `(type shape, combinator
  fingerprint)` — a 2D hash table

#### Multi-match: a graph can satisfy multiple traits

The inference returns a **set** of all matched traits, not a single
best match:

```
graph: sort
matches: [Sorting, Permutation]    -- sorted AND a permutation of input

graph: someArrow
matches: [Serializer<JSON>, Functor<Tree>]   -- satisfies both patterns
```

Generic code can require any subset:

```handl
-- requires both
migrate : Serializer<F1> & Serializer<F2> => F1 ~> F2
```

Multi-match is just set intersection at the type-filter step — already
fast.

#### Replacing hardcoded refinement contracts — fully catalog-driven

Today the refinement inference has hardcoded base arrow contracts:

```
sort   → Sorted(output)            hardcoded
map    → LengthEq(input, output)   hardcoded
filter → AllSatisfy(output, p)     hardcoded
```

**Decision: go fully catalog-driven.** All hardcoded contracts are
removed from the engine and migrated to catalog entries:

```handl
trait Sorting<A>   { sort : [A] ~> [A],   law sorted:  Sorted(sort(x)) }
trait Mapping<F>   { map : ...,           law length:  LengthEq(map(x), x) }
trait Filtering<A> { filter : ...,        law allsat:  AllSatisfy(filter(x), p) }
```

The engine ships with a **built-in catalog** containing these standard
entries (functor, sorting, mapping, filtering, etc.). This is not a
hardcoded fallback — it is a default catalog that users can extend,
override, or replace. The engine itself has zero hardcoded contracts;
all refinement inference goes through the catalog.

This means:
- **No engine changes needed to add new refinement patterns** — add a
  trait to the catalog
- **The built-in catalog is versioned with the engine** — standard
  patterns are always available
- **Users can override built-in entries** — a user-defined `Sorting`
  with additional laws takes precedence
- **The engine is fully catalog-driven** — zero hardcoded refinement
  contracts

### 4. Catalog storage: abstract interface, SQLite implementation

The trait catalog is persisted in a storage backend. The storage
interface is abstract; SQLite is the default implementation. The
abstraction is key: storage is an implementation detail that can change
later without affecting the trait inference engine.

**Source files are local; the SQLite DB is a local cache.** Trait
declarations live in source files (`.hnl` or catalog files). The engine
indexes them into a local SQLite database for fast lookup and
verification caching. The source files are the source of truth; the
database is a derived artifact — rebuildable from source, cacheable
across runs.

#### Why persistent storage

- **Indexable:** type shapes and fingerprints are pre-indexed; lookup
  is O(1) hash + O(candidates) CutElim, not O(|catalog|) scan
- **Cacheable:** CutElim normalization results are cached per
  `(graph fingerprint, law pattern)` pair — re-verification of the same
  shape is instant
- **Incremental:** adding a trait to the catalog doesn't require
  re-checking all graphs; only graphs whose type+fingerprint match the
  new entry are re-checked
- **Shareable:** the catalog is a file (or files) that can be versioned,
  shared across projects, and distributed as a library

#### Abstract storage interface

```
interface TraitCatalogStore {
  -- query
  lookupByTypeShape(typeShape) -> [CatalogEntry]
  lookupByFingerprint(fingerprint) -> [CatalogEntry]
  get(entryId) -> CatalogEntry
  
  -- mutation
  insert(entry: CatalogEntry) -> entryId
  update(entryId, entry: CatalogEntry)
  remove(entryId)
  
  -- caching
  getCachedVerification(graphFingerprint, lawPatternId) -> Option<VerificationResult>
  putCachedVerification(graphFingerprint, lawPatternId, result)
}
```

#### SQLite implementation (default)

SQLite is the default backend because:

- **Zero configuration** — embedded, no server
- **Indexable** — B-tree indexes on `type_shape` and `fingerprint`
  columns give O(log n) lookup
- **Cacheable** — a `verification_cache` table maps
  `(graph_fingerprint, law_pattern_id) → result`
- **Portable** — the catalog is a single `.db` file, versionable and
  shareable
- **Transactional** — adding/removing traits is atomic

Schema sketch:

```sql
CREATE TABLE trait_entries (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  type_shape   TEXT NOT NULL,        -- serialized type schema
  fingerprint  TEXT NOT NULL,        -- combinator tree hash
  laws         TEXT NOT NULL,        -- serialized law equations
  cutelim_proven BOOLEAN NOT NULL DEFAULT 0
);

CREATE INDEX idx_type_shape  ON trait_entries(type_shape);
CREATE INDEX idx_fingerprint ON trait_entries(fingerprint);

CREATE TABLE verification_cache (
  graph_fingerprint  TEXT NOT NULL,
  law_pattern_id     INTEGER NOT NULL,
  result             TEXT NOT NULL,   -- "verified" | "unknown" | "failed"
  PRIMARY KEY (graph_fingerprint, law_pattern_id)
);
```

#### Alternative implementations

The abstract interface allows other backends:

- **In-memory** — for development and testing; no persistence
- **PostgreSQL** — for shared/team catalogs on a server
- **Git-tracked JSON** — for small catalogs that benefit from
  human-readable diffs

The SQLite implementation is the default; others are optional.

### 5. Type parameter syntax: angle brackets, not square brackets

Type application uses **angle brackets** `F<A>`, not square brackets
`[A]`.

Rationale: `[τ]` is already taken in the calculus for the **list type**
(`FORMALISM.md`: `τ, σ ∈ Type ::= ... | [τ]`). Overloading `[...]` for
both type application and list construction would create a real syntactic
clash, not just a stylistic one.

```handl
-- list type (existing kernel syntax)
map : (Int ~> Bool) ~> ([Int] ~> [Bool])     -- [Int] = list of Int

-- type application (this ADR)
sort_by : Functor<F> => (A ~> A) ~> (F<A> ~> F<A>)  -- F<A> = type application
```

Angle brackets also visually echo the arrow type family `A ~> B`,
reinforcing that type application is parameterization within the same
calculus.

### 6. Impl declaration (graph layer — annotation-free)

An impl is a graph assignment for each operation in the trait's
signature. The user writes **only the graphs**. No types, no proofs.

```handl
impl Functor<List> {
  map = loop (arr uncons >>> choice
                (arr nil)
                (first f >>> arr cons))
}
```

The system derives automatically:

1. **HM inference over the graph** — derives `map : (α ~> β) ~> ([α] ~> [β])`
   with zero annotations (per `poly_proof_graph_tests.rs`).
2. **Trait conformance check** — unifies the inferred type against the
   trait's declared signature: `F := List, A := α, B := β`.
3. **Law verification via `CutElim` normalization** — normalizes both
   sides of each law equation and checks equality:
   - `map id ⟹* id` ✓
   - `map (f >>> g) ⟹* map f >>> map g` ✓
4. **The typing derivation is the proof** (by `TyOf ⟺ PrOf`) — no
   separate proof term is written.

### 7. Generic code with trait bounds

A function may declare trait bounds on its type parameters. The bound is
a **capability declaration**: the named trait's operations are unhandled
effects within the function body — someone must provide them (§13).

```handl
sort_by : Functor<F> => (A ~> A) ~> (F<A> ~> F<A>)
sort_by = arr cmp >>> map       -- map is an unhandled operation
```

This desugars to: `sort_by` is an arrow whose body references `map` as
an abstract operation. The `Functor<F> =>` bound declares the capability
need; it does not resolve it. Resolution happens at the call site via
either an explicit handler (§13, primary) or catalog search (§12,
fallback). The bound is not a runtime check — it is a **compile-time
requirement that an impl is available at the use site**.

### 8. Supertraits as signature inclusion

A supertrait bound `trait T : U` means `Σ_T` requires `Σ_U` to be
satisfied first. This is **signature inclusion**: the impl must provide
graphs for both `Σ_U` and `Σ_T`.

```handl
trait Ord<A> : Eq<A> {
  compare : A ~> A ~> Ordering
}

impl Ord<Int> : Eq<Int> {
  -- must provide both Eq's eq and Ord's compare
  eq      = arr intEq
  compare = arr intCompare
}
```

No new mechanism — just signature union.

### 9. Associated types via dependent types

A trait may declare an associated type using the dependent-type machinery
from `DependentProofGraphs.lean` (`dSigma`/`dPi`):

```handl
trait Iterator<Coll> {
  type Item
  next : Coll ~> (Item × Coll) + ⊥
}
```

The associated `Item` is a **dependent type indexed by the impl**: the
impl's `next` graph has output type `(Item × Coll) + ⊥` where `Item` is
determined by the impl's graph structure (inferred from wiring, per the
annotation-free HM result).

```handl
impl Iterator<[A]> {
  next = arr uncons >>> choice (arr none) (arr some)
  -- Item inferred as A from the wiring
}
```

### 10. Blanket impls via ArrowApply

A blanket impl `impl<T: U> T for G<T>` is a **higher-order arrow
producing graphs from graphs** — exactly ArrowApply (Challenge #11). The
impl is itself an arrow: given an impl of `U[T]`, produce an impl of
`T[G<T>]`.

```handl
impl<T: Ord> Ord<[T]> {
  compare = arr lexCompare   -- uses Ord<T>.compare via app
}
```

This requires `app` (ArrowApply) in the kernel — already present per
ADR-0052.

### 11. Law verification: the `CutElim` connection

Law verification is the **open mechanization gap**. The plan:

- Each law is an equation `G₁ ≡ G₂` between two arrow compositions.
- `CutElim` (`ProofAdvantage.lean`) defines a rewrite relation that
  normalizes graphs: `idLeft` (absorb identity), `idRight`, `firstFusion`
  (`first (f >>> g) → first f >>> first g`), `leftFusion`.
- A law holds iff both sides normalize to the same canonical form under
  `CutElim`.
- The normalization is **local and binder-free** (the proof-net
  advantage, per `SYNTHESIS_PROOF_NETS_TO_PL.md`) — no substitution
  lemma, no α-equivalence.

This is not yet connected to trait-law checking in the mechanization.
**The CutElim-based law checker is part of this ADR, not a separate
ADR.** The trait system is not complete without law checking — keeping
it in the same ADR ensures the trait layer is not accepted without its
load-bearing piece. The mechanization is in progress; if this section
grows large enough to warrant splitting, it can be extracted into a
follow-on ADR later.

### 12. Trait resolution: catalog search (fallback)

When generic code uses a trait-bound type parameter and **no explicit
handler is installed** (§13), the system searches the trait catalog for
graphs satisfying the trait at the requested type. This is the
**fallback** resolution mechanism — the convenience mode that works
when there's exactly one match.

1. **Type-filter the catalog** — find entries whose type shape unifies
   with the requested type.
2. **Fingerprint match** — among candidates, find graphs whose
   combinator fingerprint matches the trait's law patterns.
3. **CutElim verify** — confirm the candidate graph satisfies the
   trait's laws.

This is **program synthesis over arrow graphs** — undecidable in
general, but the 3-step pipeline prunes the search space drastically.
The structural transparency of arrow graphs (explicit dependency graph,
visible fanout, traversable structure) makes heuristic search more
tractable than over λ-terms.

**Count-based disambiguation.** If the catalog search returns exactly
one match, the system injects it (convenience mode). If it returns
multiple matches, the system **errors**: "ambiguous — N impls match
Functor<List>, name one explicitly via `handle ... with`." There is no
global coherence check, no specificity algorithm, no orphan rule. The
rule is: one match = inject, multiple = ask the user to be explicit.
This is decidable (concrete types at the call site) and honest (the
compiler never silently picks).

### 13. Trait resolution: explicit handlers (primary)

**Revised from the original §12 design.** The primary resolution
mechanism is **explicit handler installation** — dependency injection
over effect handlers. A trait bound declares a capability need (an
unhandled operation in the body); a handler at an outer scope provides
the impl. This unifies trait resolution with handl's existing effect
handler machinery (ADR-0052).

```handl
-- inner: declares the capability, never names the impl
sort_by : Functor<F> => (A ~> A) ~> (F<A> ~> F<A>)
sort_by = arr cmp >>> map       -- map is an unhandled operation

-- outer: handles it by providing the impl
handle sort_by intCmp [3,1,2]
  with Functor<List> = map_list
```

The `handle ... with` form installs a handler that intercepts the
abstract `map` operation and supplies the concrete `map_list` graph.
The inner code stays abstract; the outer scope wires the impl.

#### Why handlers, not implicit search

- **No coherence problem.** Handlers are lexically scoped and
  nearest-wins — same as exception handlers. Two handlers for
  `Functor<List>`? The innermost applies. No ambiguity, no global
  uniqueness check, no orphan rule. The scoping *is* the resolution.
- **Law checking is localized.** CutElim verification runs once at the
  `handle` site, where the concrete graph is bound. Every inner use
  inherits the proof. This is the ADR's "proof reuse by composition"
  made explicit and local — no catalog search needed for the common
  case.
- **One mechanism, not two.** Trait resolution and effect handling use
  the same machinery (ADR-0052 handlers). No separate resolution
  algorithm.
- **Explicit is the safe default.** The handler names the impl. The
  compiler never silently picks. Catalog search (§12) is the
  convenience fallback for the common one-impl case.

#### Handler semantics

- A `handle body with Trait<Type> = impl` form installs a handler for
  the trait's operations within `body`.
- Handlers nest: an inner handler shadows an outer one for the same
  trait + type (nearest wins, like exception handlers).
- The handler's impl graph is law-checked once at installation: the
  trait's laws are verified via CutElim (for structural laws) or
  semantic axioms (for non-functor laws, per `EquationalLawInference`).
  If verification fails, the `handle` form is a compile error.
- Multiple traits can be handled in one form:
  `handle body with Functor<List> = map_list, Eq<Int> = intEq`.

#### The catalog as ambient default handlers

The built-in catalog (§3) serves as the **ambient default handler set**
— handlers that are always installed unless overridden. When no
explicit `handle ... with` is present, the system falls back to catalog
search (§12). When an explicit handler is present, it shadows the
catalog entry for that trait + type within its scope. The catalog does
not disappear; it becomes the default, and explicit handlers are the
scoped override.

#### What this replaces from the original design

The original §12 made catalog search the primary (and only) resolution
mechanism, with "user disambiguation + specificity" for the ambiguous
case. This revision:

- Makes explicit `handle ... with` the primary mechanism (safe,
  scoped, no ambiguity).
- Demotes catalog search to the fallback for the one-impl case.
- Replaces the "specificity by law count" tiebreaker with a hard rule:
  multiple matches = error, name one explicitly. (Specificity-by-laws
  can be added later as a convenience, but the default is honest: ask.)
- Eliminates the coherence problem entirely — lexical scoping resolves
  it, no algorithm needed.

#### Open: bundled operations

Effect handlers typically handle one operation (`Raise`, `Get`). A
trait is a *bundle* of operations (`Functor` = `map` + laws). Two
options: (a) one effect per operation (granular, loses law-bundling),
or (b) a "trait effect" that is a record of operations handled together.
Option (b) is cleaner but requires the effect system to support bundled
operations. This is a design question for the effect handler
mechanism, not a separate trait-layer concern.

### Migration policy

Traits are a tool for specific situations, not a style to apply
everywhere. The bare kernel is already annotation-free and
proof-carrying — most code doesn't need more.

**Now: do not migrate.** The law checker does not exist yet. Migrating
today means declaring laws that aren't verified — Rust traits with no
payoff over the bare kernel.

**After the law checker lands, migrate selectively:**

- **Public contracts between modules** — where module A requires module B
  to satisfy an equational law, a trait makes the requirement explicit
  and verified rather than implicit.
- **Generic libraries** — `sort_by` over `Functor<F>` is more reusable
  than `sort_by` taking an explicit `map` arrow, because the law proof
  travels with the trait bound. Callers don't re-verify.
- **Correctness-critical code with real laws** — serializers, parsers,
  numeric primitives, collection operations. Places where a broken law
  means silent data corruption.

**Never migrate:**

- **Internal wiring** — a graph used in one place, never passed around,
  never composed generically. Wrapping it in a trait adds ceremony with
  no payoff; the law is trivially checkable by reading the graph.
- **One-off arrows** — most arrows in a program are one-off. They don't
  need a named contract.
- **Law-free code** — not every arrow satisfies a clean equation.
  Forcing a trait with no laws is just a named record — sugar, fine if
  it helps readability, but not a migration priority.

## Consequences

### Positive

- **No new calculus primitive.** Trait inference is a recognition layer
  over the existing kernel. The calculus remains
  `arr`/`>>>`/`first`/`choice`/`loop`/`app`.
- **Trait inference is annotation-free.** The user writes only the
  graph; the system recognizes which laws it satisfies. This extends the
  invariant-inference paper's central claim — "the user writes one
  thing and gets the type and the proof for free" — to trait
  conformance. No `trait`, `impl`, or `law` declaration needed in the
  primary mode.
- **Laws are proven, not tested** *(contingent on law inference
  mechanization — see Negative)*. The `TyOf ⟺ PrOf` correspondence
  means the typing derivation of the impl graph *is* the proof. No
  separate proof terms, no test suite for laws.
- **Proof reuse by composition.** If `migrate = deserialize >>> serialize`
  and both graphs are recognized as satisfying the roundtrip law, then
  `migrate` is proven lossless by one-line composition of the two laws
  — no separate proof. Laws compose because laws *are* arrow
  compositions.
- **Explicit declarations are optional sugar.** When the user wants to
  name a law pattern (for documentation, explicit intent, or generic
  code constraints), they may declare a trait. This is sugar over the
  inference — a name for what the system already detects.
- **Associated types for free.** Dependent types (`dSigma`/`dPi`) already
  in the mechanization give associated types without new machinery.
- **Blanket impls for free.** ArrowApply already in the kernel gives
  higher-order impls without new machinery.
- **Consistent with `Language` protocol.** The Phase 3 `Language` record
  is already a value-level trait with impls; this ADR lifts the same
  pattern to the type level — and makes the declaration optional.
- **Open, extensible catalog.** The trait catalog is not hardcoded —
  users add law patterns to the indexed library, and the inference
  engine recognizes them everywhere. This generalizes the existing
  hardcoded pattern recognition (`LoopInvariantDiscovery.lean`) to an
  open, user-extensible system.
- **Fast matching pipeline.** Type filter (O(1) hash) → structural
  fingerprint (O(1) hash) → CutElim normalization (only on candidates).
  Most graphs are pruned before the expensive step. The 5-combinator
  alphabet makes fingerprints small and shallow.
- **Cached and indexable.** CutElim verification results are cached per
  `(graph fingerprint, law pattern)` pair. Re-verification of the same
  shape is instant. The catalog is pre-indexed by type shape and
  fingerprint.
- **Hardcoded refinement contracts become catalog entries.** The
  existing hardcoded base arrow contracts (`sort → Sorted`, `map →
  LengthEq`, `filter → AllSatisfy`) migrate to the open catalog,
  making the refinement engine fully user-extensible.
- **Multi-match.** A graph can satisfy multiple traits (e.g. `sort`
  satisfies both `Sorting` and `Permutation`). The inference returns a
  set; generic code can require any subset via intersection.
- **No coherence problem (§13).** The primary resolution mechanism is
  explicit `handle ... with` — lexically scoped, nearest-wins, like
  exception handlers. The compiler never silently picks an impl. The
  coherence problem that dominates Rust/Haskell trait systems is
  avoided by design, not solved by a global algorithm. Catalog search
  (§12) is the fallback: one match = inject, multiple = error.
- **One mechanism for traits and effects.** Trait resolution and effect
  handling use the same `handle ... with` machinery (ADR-0052). No
  separate resolution algorithm, no separate coherence checker.

### Negative

- **Trait inference for law patterns is not yet mechanized.** The
  invariant-inference machinery is demonstrated for output properties
  (sortedness, permutation, length) but not for equational law
  patterns (functor, monad, roundtrip). The `CutElim` rewrite relation
  provides the normalization foundation, but the structural pattern
  recognition extension is in progress. Until it lands, the primary
  mode (inference) is aspirational and the secondary mode (explicit
  declarations with `CutElim`-based law checking) is the fallback.
- **Trait resolution via catalog search is the fallback, not the
  primary.** The primary mechanism is explicit `handle ... with`
  (§13), which has no coherence problem (lexical scoping). The catalog
  search fallback (§12) uses count-based disambiguation (one match =
  inject, multiple = error). There is no global coherence algorithm,
  no specificity ordering, no orphan rule. This is simpler than
  Rust/Haskell and deliberately so — the coherence problem is avoided
  by design, not solved by algorithm.
- **`CutElim` normalization may not terminate.** The rewrite relation is
  not known to be strongly normalizing for all graphs. For laws that
  hold, normalization is expected to terminate quickly (the proof-net
  advantage), but a non-terminating law check is possible. A timeout or
  fuel limit will be needed in practice.
- **The trait layer's distinctive value is contingent on the law
  inference mechanization.** If extending invariant inference to law
  patterns fails, the primary mode (inference) does not work, and the
  ADR falls back to the secondary mode (explicit declarations with
  `CutElim`-based law checking). If that also fails to handle realistic
  laws, the trait layer reduces to sugar (named bundles + implicit
  parameters) without a `law` keyword — useful, but no more powerful
  than Rust/Haskell traits. See "The contingency on law verification"
  above.

### Neutral

- **Angle brackets for type application.** This fixes a syntactic clash
  with list types (`[τ]`) and aligns visually with the arrow type
  family, but diverges from Rust/Haskell convention (`<T>` is the same,
  but `[T]` for type application is not used in those languages anyway).
- **Traits are structural, not nominal.** A graph satisfies a trait if
  it type-checks and its laws hold — there is no "impl declaration
  required" check beyond being in scope. This is closer to TypeScript
  structural subtyping than to Rust nominal traits. A future ADR may
  add nominal branding if needed.
- **Resolution is handler-based, not search-based (§13).** The primary
  mechanism is explicit `handle ... with` (dependency injection), not
  implicit catalog search. This is a deliberate departure from
  Rust/Haskell, where implicit resolution is the default. The trade:
  more explicit code, no coherence problem, localized law checking.
  Catalog search remains as a fallback convenience.

## Open Questions

1. **Trait inference mechanization (load-bearing).** Extend the
   invariant-inference machinery from output properties (sortedness,
   permutation, length) to equational law patterns (functor, monad,
   roundtrip). This is the primary mode of the trait layer and the key
   open question. Active Lean4 mechanization in progress.
2. **`CutElim`-based law checking (part of this ADR).** Extend
   `CutElim` to a complete law-checking procedure, prove termination
   for a useful fragment, and connect it to the trait checker. Folded
   into this ADR (§11) — not a separate ADR.
3. **Trait resolution — handlers + catalog search (part of this ADR).**
   The primary mechanism is explicit `handle ... with` (§13) —
   dependency injection over effect handlers, lexically scoped, no
   coherence problem. The fallback is catalog search (§12) with
   count-based disambiguation (one match = inject, multiple = error).
   No global coherence algorithm is planned — the problem is avoided
   by design. Open: the bundled-operations effect representation (§13
   "Open: bundled operations") and whether specificity-by-verified-laws
   is worth adding as a convenience tiebreaker.
4. **Law expressiveness.** Which equational laws are decidable under
   `CutElim` normalization? The proof-net literature suggests the
   linear fragment is decidable; the non-linear extension (this
   calculus) is open.
5. **Higher-kinded traits.** `trait Functor<F>` requires `F` to be a
   type constructor. How are higher-kinded parameters represented in
   the dependent-type substrate? `dPi` over types may suffice, but this
   is not yet worked out.
6. **Catalog fingerprinting.** What is the canonical hash for a
   combinator tree? Structural fingerprinting must be invariant under
   alpha-equivalent rewiring but sensitive to law-relevant structural
   differences. The 5-combinator alphabet makes this tractable but the
   exact hash function needs design.
7. **Catalog distribution.** Source files are local; SQLite is a local
   cache (decided). Open question: how are catalogs shared across
   projects — git-tracked source files, a package registry, or both?
   The storage abstraction (§4) allows changing this later.
8. **Built-in catalog contents.** The engine ships with a built-in
   catalog (decided: fully catalog-driven, no hardcoded fallback). Open
   question: what is the minimal set of standard traits in the built-in
   catalog? (Functor, Sorting, Mapping, Filtering are obvious; what
   else?)

## References

- [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md)
- [ADR-0074 — Arrow Surface Syntax v0.1](./0074-arrow-syntax-v0_1-surface-redesign.md)
- `research/arrow-calculus-kernel-poc/FORMALISM_PROOF_GRAPHS.md` — `Σ` as
  signature, typing rules
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/ProofGraphs.lean`
  — `TyOf ⟺ PrOf` bijection, subject reduction
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/ProofAdvantage.lean`
  — `CutElim` rewrite relation
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/DependentProofGraphs.lean`
  — `dSigma`/`dPi` for associated types
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/LoopInvariantDiscovery.lean`
  — structural pattern recognition (side-channel, accumulator, filter) —
  the existing approach the open catalog generalizes
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/TraitInference.lean`
  — functor/coproduct-functor law inference via CutElim (zero sorry)
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/EquationalLawInference.lean`
  — non-functor law inference (roundtrip, associativity) with semantic
  axioms; proves CutElim alone cannot discharge non-functor laws
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/CutElimCongruence.lean`
  — congruence rules for CutElim + strong normalization for the
  6-rewrite fragment
- `research/arrow-calculus-kernel-poc/src/poly_proof_graph_tests.rs` —
  annotation-free HM inference over graphs
- `research/arrow-calculus-kernel-poc/src/trait_catalog.rs` — Rust
  prototype: catalog, 3-step pipeline, CutElim normalizer, in-memory
  store, built-in catalog
- `research/arrow-calculus-kernel-poc/src/trait_catalog_sqlite.rs` —
  SQLite catalog backend with persistence + verification cache
- `research/arrow-calculus-kernel-poc/RESULTS_TRAITS.md` — prototype
  results mapping to ADR sections
- `research/arrow-calculus-kernel-poc/RESULTS_PHASE3.md` — `Language`
  protocol as value-level trait with impls
- `research/arrow-calculus-kernel-poc/SYNTHESIS_PROOF_NETS_TO_PL.md` —
  local, binder-free cut elimination as the proof-net advantage
- `research/arrow-calculus-kernel-poc/PAPER_INVARIANT_INFERENCE.md` —
  invariant inference with 100% accuracy on 72 benchmark cells; the
  foundation the trait catalog extends
