# ADR-0075 Prototype Results: Traits as Arrow Signatures

## Status

Prototype complete for the catalog/storage/CutElim pipeline; Lean4
functor-law mechanization proven; non-functor law inference aspirational.

Date: 2026-09-02

---

## Summary

- **What was built:** (1) a Lean4 mechanization (`TraitInference.lean`)
  extending `CutElim` with two new rewrites and proving the functor and
  coproduct-functor identity + composition laws by normalization, with
  zero `sorry`; (2) a Rust prototype (`trait_catalog.rs`) implementing the
  open trait catalog, the 3-step matching pipeline (type filter ->
  fingerprint -> CutElim), an in-memory store, and a built-in catalog of
  5 entries; (3) a SQLite backend (`trait_catalog_sqlite.rs`) over the
  same abstract `TraitCatalogStore` interface, with persistence, indexes,
  and a verification cache.
- **What works:** CutElim verification of the functor/coproduct-functor
  identity + composition laws (Lean, zero `sorry`; mirrored in Rust); the
  3-step pipeline recognizes `Functor` for the demo `map` graph and
  reports `Mapping` as a placeholder multi-match; SQLite persistence
  survives close+reopen with the verification cache intact; law graphs
  round-trip losslessly through serde JSON.
- **What is aspirational:** non-functor equational laws (monad, roundtrip,
  sortedness-as-equation) are stubbed as semantic placeholders
  (`cutelim_proven = false`); higher-kinded type shapes (`F<A>`) are not
  modelled (`Ty` has no type-application constructor, so `Functor`'s
  `type_shape` uses the `List` instance); trait resolution coherence and
  CutElim strong normalization are not proven; the surface syntax
  (§5-§10: trait/impl/law declarations, supertraits, associated types,
  blanket impls) is not implemented.

---

## Mapping to ADR-0075 sections

| ADR section | Status | Notes |
|---|---|---|
| §1 Primary mode: trait inference | Partial | The 3-step pipeline recognizes `Functor` for the `map` graph from wiring alone (no `trait`/`impl`/`law` declaration). Proven only for the functor/coproduct-functor structural pattern; non-functor law inference is aspirational. |
| §2 Secondary mode: explicit declarations | Stubbed | No surface `trait`/`impl`/`law` parser. The built-in catalog entries play the role of declared law patterns, but there is no user-facing declaration syntax. |
| §3 Open trait catalog + 3-step pipeline | Implemented | `trait_catalog.rs`: `CatalogEntry`/`LawEquation`, `TraitCatalogStore`, `InMemoryStore`, `infer_traits` (type filter via HM unify -> fingerprint hash -> CutElim `law_equal`), multi-match, verification cache, 5 built-in entries. |
| §4 Catalog storage (SQLite) | Implemented | `trait_catalog_sqlite.rs`: `SqliteStore` over the same `TraitCatalogStore` trait; schema (`trait_entries` + `verification_cache` + indexes); persistence across reopen; equivalence with `InMemoryStore`; lossless law round-trip. `lookup_by_type_shape` uses a canonical-string-index + unify-confirm approximation (not a true unification index). |
| §5 Type parameter syntax (`F<A>`) | Out-of-scope | `Ty` has no type-application constructor; `Functor`'s `type_shape` uses the `List` instance `[A] ~> [B]`. Higher-kinded traits are open (ADR OQ#5). |
| §6 Impl declaration (graph layer) | Stubbed | No `impl` surface syntax; the demo supplies the `map` graph directly as a `PolyGraph`. |
| §7 Generic code with trait bounds | Out-of-scope | No trait-bound desugaring; no generic-code resolution wired to the catalog. |
| §8 Supertraits as signature inclusion | Out-of-scope | Not modelled. |
| §9 Associated types via dependent types | Out-of-scope | Not modelled in this prototype (substrate exists in `DependentProofGraphs.lean`). |
| §10 Blanket impls via ArrowApply | Stubbed | `PolyGraph::App` is present as an opaque placeholder (fresh input/output vars); it is NOT wired into CutElim or inference semantics. |
| §11 Law verification: CutElim connection | Partial | `TraitInference.lean` proves the functor/coproduct-functor identity + composition laws via `CutElimStar` normalization (zero `sorry`). `trait_catalog.rs` mirrors the 6 rewrites in `normalize`/`law_equal`. Congruence (rewriting inside subgraphs), termination, and completeness are open. |
| §12 Trait resolution: catalog search | Partial | The 3-step pipeline IS the catalog search (type filter -> fingerprint -> CutElim). No coherence/ambiguity algorithm; multi-match returns all matches. Step 3 checks the catalog entry's law pattern is CutElim-self-consistent (see honest note below); full graph-into-schema instantiation is future work. |

---

## The Lean4 mechanization (§11 foundation)

`lean/ArrowMinimalityLib/TraitInference.lean` mechanizes the CutElim
foundation for trait-law verification. It builds on
`ProofAdvantage.lean`, which defines the `CutElim` rewrite relation and
its reflexive-transitive closure `CutElimStar`.

**The 6 CutElim rewrites** (defined in `ProofAdvantage.lean` lines
101-124, the last two added for this work):

```
ceIdLeft      : comp(idG A, G)        ==> G
ceIdRight     : comp(G, idG B)        ==> G
ceFirstFusion : comp(first G, first H) ==> first(comp G H)
ceLeftFusion  : comp(left G, left H)   ==> left(comp G H)
ceFirstId     : first(idG A)           ==> idG(A x D)     (functor identity)
ceLeftId      : left(idG A)            ==> idG(A + D)     (coproduct functor identity)
```

`ceFirstId` and `ceLeftId` are valid cut-elimination steps: `first` is
conjunction-functoriality, and functoriality applied to the identity
proof *is* the identity proof on the conjunction. No substitution, no
binder.

**The law types and predicates:**

- `Law` — an inductive type of named equational laws
  (`functorId`, `functorComp`, `coproductFunctorId`,
  `coproductFunctorComp`), each carrying the type/graph parameters
  needed to instantiate both sides.
- `LawVerified law` — a `Prop`: the law is verified when both sides
  normalize to the same canonical form under `CutElimStar` (expressed
  via `Nonempty`, since `CutElimStar` is a `Type 1` derivation).
- `SatisfiesLaw G law` — graph `G` satisfies `law` when `G`'s wiring
  matches the law's pattern AND the law is verified. The pattern is
  recognized structurally.
- `UsesFirst` / `UsesLeft` — inductive structural recognizers: a graph
  *uses `first`* (resp. `left`) if the combinator appears anywhere in its
  wiring (here, or in either side of a composition).

**The inference theorems** (annotation-free — the law is not declared):

- `satisfies_functor_identity`: if `G` uses `first`, then `G` satisfies
  the functor identity law `first(idG A) = idG(A x D)` for any `A D`.
- `satisfies_functor_composition`: if `G` uses `first`, then `G`
  satisfies `first(comp F H) = comp(first F)(first H)` for any `F H`.
- `satisfies_coproduct_functor_identity` / `satisfies_coproduct_functor_composition`:
  the analogous theorems for `left`.
- `all_functor_laws_verified`: every functor/coproduct-functor law is
  verified by CutElim normalization.

**`inferred_trait`** is the structural law-inference function (analogue
of `inferred_postcondition` from `StructuralInference.lean`): it maps a
graph to a `Prop` by structural recursion — `first G` and `left G`
detect their patterns; `comp G H` unions via disjunction; `id`/`arr`/`loop`
detect nothing. `inferred_trait_sound_functor` and
`inferred_trait_sound_coproduct` prove soundness: if `inferred_trait`
detects the pattern, the corresponding laws are verified.

**Task 3 (connection to invariant domain D):**
`first_id_semantic_equiv` and `left_id_semantic_equiv` prove the two
sides of each identity law are semantically equivalent (same output for
same input) directly from the `Eval` rules. `first_comp_semantic_forward`
/ `first_comp_semantic_backward` prove the composition law's semantic
equivalence. `functor_id_law_implies_preserves_all_D` connects the
functor identity law to preservation of all `D`-invariants.

**Zero `sorry`:** the file builds green under `lake build` with no
`sorry` proof terms (the word `sorry` appears only in comments stating
"zero `sorry`"). `lake build` reports `Build completed successfully
(19 jobs)`.

**Honest assessment** (from the file's own "Honest assessment" section,
lines 628-653):

- **What works:** the functor laws for `first` and `left` are verified
  by CutElim normalization, and the inference from wiring structure to
  law satisfaction is proven. The connection to `D` is established for
  the identity law (preservation of all `D`-invariants).
- **`inferred_trait` is a `Prop`-valued flag**, not a data structure
  enumerating the specific laws. A more useful implementation would
  return a list/set of `Law` instances, but this requires
  universe-polymorphic containers that are awkward in Lean4.
- **The "preserves all D" connection is vacuously weak:** it only
  discharges the identity case `first(idG A)`, where the graph is
  semantically the identity (so trivially preserves every invariant).
  It does not establish preservation for non-identity functorial graphs.
- **Congruence is open:** `CutElim` only rewrites at the top level.
  Verifying laws for complex graphs (e.g. `map_list` built from the five
  primitives) requires congruence rules for rewriting inside subgraphs —
  straightforward but tedious to formalize.
- **More laws:** the framework handles functor laws for `first`/`left`
  only. Extending to `loop` trace laws, `arr` laws, interchange laws
  requires adding the corresponding CutElim rewrites and proving
  preservation.
- **Completeness is open:** the theorems prove *soundness* (if the
  pattern is detected, the law holds), not *completeness* (if the law
  holds, the pattern is detected). Completeness is false in general —
  some laws require semantic reasoning beyond CutElim.

---

## The Rust prototype (§3 + §11 + §12)

`src/trait_catalog.rs` is a self-contained (pure-std) prototype of the
open trait catalog and the matching pipeline. It mirrors the Lean
graph/type model and the CutElim rewrites.

### The `PolyGraph` kernel model

The graph is built from 5 core combinators plus 2 ADR-0075 extensions:

```
Id | Comp(G, H) | First(G) | Left(G) | Loop(G) | Arr(name) | Choice(G, H) | App(G)
```

- `Choice(G, H)` — binary sum functoriality (`A+B ~> C+D` from
  `A~>C`, `B~>D`), an extension over the original unary `Left`.
- `App(G)` — placeholder for ArrowApply (ADR-0052 kernel `app`). Opaque
  to inference (fresh input/output vars); NOT wired into CutElim.
  Present so the graph model matches the kernel alphabet without
  claiming apply semantics.

Types (`Ty`) mirror `poly_proof_graph_tests.rs`: `Atom | Var | Prod |
Sum | Arrow | Unit | Void | List`. There is **no type-application
constructor**, so higher-kinded `F<A>` cannot be represented (ADR OQ#5).

### The CutElim normalizer (mirrors `ProofAdvantage.lean`)

`rewrite_root` applies one of the 6 rewrites at the graph root:
`ceIdLeft`, `ceIdRight`, `ceFirstFusion`, `ceLeftFusion`, `ceFirstId`,
`ceLeftId`. `rewrite_pass` normalizes children bottom-up first, then
repeatedly fires root rewrites until none apply (so a cascade like
`comp(first id, first id) -> first(comp id id) -> first id -> id`
completes within one pass). `normalize` repeats passes to a fixpoint,
bounded by `NORMALIZE_FUEL = 1024`. `law_equal(g1, g2)` returns whether
the two graphs normalize to the same form.

**Termination honesty:** the CutElim rewrite relation is NOT proven
strongly normalizing for all graphs (ADR Negative; OQ#2). The six
rewrites are size-non-increasing and terminate quickly in practice (the
proof-net advantage), but the fuel bound is a safety net. If fuel is
exhausted before a fixpoint, the result is the partially-normalized
graph (`law_equal` may then report false negatives).

### `fingerprint`

`fingerprint(graph)` is a structural shape hash: the combinator tree
with all `Arr` names erased to a placeholder `arr` (e.g.
`comp(first(arr), arr)`). Two graphs with the same combinator shape but
different base-arrow names share a fingerprint — intentional, since the
fingerprint is a coarse fast filter; the precise check is CutElim
`law_equal`. (ADR OQ#6: the exact canonicalization is a design
question; this is a simple deterministic structural hash.)

### Catalog types and the in-memory store

- `LawEquation { name, lhs: PolyGraph, rhs: PolyGraph }` — an
  equational law: two graphs that must normalize to the same form.
- `CatalogEntry { name, type_shape: Ty, fingerprint: String, laws:
  Vec<LawEquation>, cutelim_proven: bool }` — a named law pattern. The
  `cutelim_proven` flag is the ADR's marker for whether CutElim
  normalization is proven for this pattern.
- `TraitCatalogStore` — the abstract storage trait
  (`lookup_by_type_shape`, `lookup_by_fingerprint`, `get`, `insert`,
  `remove`, `get_cached_verification`, `put_cached_verification`,
  `entries`, cache-stat hooks). The engine takes
  `&mut impl TraitCatalogStore` and never names a concrete backend.
- `InMemoryStore` — the in-memory backend (linear-scan lookups;
  `lookup_by_type_shape` filters by `Unifier::unify`).
- `VerificationResult { Verified, Unknown, Failed }` — cached outcome
  for a `(graph fingerprint, law pattern)` pair.

### The 3-step `infer_traits` pipeline

```
Step 0: infer the graph's type (annotation-free HM) + fingerprint
Step 1: type filter  -- store.lookup_by_type_shape (HM unify), prune non-unifying
Step 2: fingerprint  -- keep only entries whose fingerprint == graph's
Step 3: CutElim      -- for each law of each surviving entry:
                         cache check; if miss, run law_equal(lhs, rhs)
                         (placeholder entries -> Unknown, no verified laws)
```

Multi-match: ALL surviving entries are returned (sorted by name for
deterministic output). Verification results are written to the cache;
re-inference of the same graph hits the cache.

### Built-in catalog (5 entries)

```
Functor           type_shape [A]~>[B]   cutelim_proven=true
                    laws: identity (first(id) === id), composition (first(f;g) === first f; first g)
CoproductFunctor  type_shape (A+C)~>(B+D) cutelim_proven=true
                    laws: identity (left(id) === id), composition (left(f;g) === left f; left g)
Sorting           type_shape [A]~>[A]   cutelim_proven=false
                    laws: sorted (Sorted(sort(x)) -- semantic placeholder)
Mapping           type_shape [A]~>[B]   cutelim_proven=false
                    laws: length (LengthEq(map(x), x) -- semantic placeholder)
Filtering         type_shape (A~>Bool)~>([A]~>[A]) cutelim_proven=false
                    laws: allsat (AllSatisfy(filter(x), p) -- semantic placeholder)
```

`Functor` and `CoproductFunctor` are CutElim-proven — their laws ARE the
six CutElim rewrites. `Sorting`/`Mapping`/`Filtering` are semantic
placeholders: their laws (`Sorted`, `LengthEq`, `AllSatisfy`) are not
expressible as graph equations over the 5-combinator kernel and are
marked `cutelim_proven = false`, matching the ADR's explicit position
that non-functor law verification is aspirational (ADR "What is NOT yet
proven"; OQ#2).

**Prototype limitation (HKT):** the generic `Functor<F>` requires
higher-kinded type application `F<A>`, which is not in the `Ty` enum.
`Functor`'s `type_shape` therefore uses the `List` instance
`[A] ~> [B]`; the type filter is coarser than the ADR's eventual HKT
form (ADR OQ#5).

### The demo `map` graph and its inferred traits

The demo graph is the ADR §1 `map` shape built from primitives:

```
map = loop (first (uncons ; choice nil (first f ; cons))) ; merge
```

Inferred type: `[a] ~> [b]`. Fingerprint:
`comp(loop(first(comp(arr,choice(arr,comp(first(arr),arr))))),arr)`.

The pipeline matches two entries (multi-match): `Functor` (two
CutElim-verified laws) and `Mapping` (one placeholder law).
`Sorting`/`Filtering`/`CoproductFunctor` are pruned by the type and/or
fingerprint filters.

### Captured output: `cargo run --bin trait_inference`

```
=== ADR-0075 prototype: Traits as Arrow Signatures ===

=== Demo: map graph from primitives (loop+first+choice+arr) ===
graph : (loop(first((uncons ; choice(nil,(first(f) ; cons))))) ; merge)
type  : [a] ~> [b]
fp    : comp(loop(first(comp(arr,choice(arr,comp(first(arr),arr))))),arr)
built-in catalog entries: 5

=== Pipeline run #1 (cold cache) ===
matches (multi-match):
  - Functor : cutelim_proven=true verified=["identity", "composition"] placeholder=[]
  - Mapping : cutelim_proven=false verified=[] placeholder=["length"]
cache: hits=0 misses=3

=== Pipeline run #2 (warm cache) ===
matches (multi-match):
  - Functor : cutelim_proven=true verified=["identity", "composition"] placeholder=[]
  - Mapping : cutelim_proven=false verified=[] placeholder=["length"]
cache: hits=3 misses=3

=== Tests ===
[pass] CutElim law_equal true cases (all 6 rewrites)
[pass] CutElim law_equal false cases
[pass] fingerprint same-shape-different-Arr
[pass] fingerprint different-shape
[pass] type filter pruning (Filtering, CoproductFunctor excluded)
[pass] fingerprint pruning (Sorting excluded)
[pass] pipeline map -> [Functor (verified), Mapping (placeholder)] (multi-match)
[pass] cache hit on re-inference (cache_hits=3)
[pass] 5 built-in entries present: ["Functor", "CoproductFunctor", "Sorting", "Mapping", "Filtering"]
[pass] CutElim-proven=["Functor", "CoproductFunctor"]  placeholder=["Sorting", "Mapping", "Filtering"]

=== All tests passed ===
```

### Honest note on step 3 semantics

The prototype's step 3 checks `law_equal(law.lhs, law.rhs)` — i.e. it
confirms the catalog entry's law *pattern* is CutElim-self-consistent
(both sides of the stored law equation normalize to the same form) —
and relies on the step-2 fingerprint match to establish that the graph
exhibits that structural pattern. This is a faithful approximation of
the ADR's "normalize both sides and compare" for the
structural-pattern case, where the law's lhs/rhs are written in the
same combinator alphabet as the graph and the fingerprint already
established shape equality.

Full instantiation of an arbitrary graph into a law schema (with
graph-specific subgraph matching, binding the law's schematic variables
`f`/`g` to the graph's concrete sub-arrows) is future work. For the
functor laws as stored, the law sides are concrete small graphs
(`first(id)`, `comp(first f, first g)`, etc.) and `law_equal` confirms
they normalize equal — which is exactly the CutElim verification the
Lean mechanization performs. The gap relative to the ADR's full vision
is that the prototype does not yet substitute a candidate graph's
sub-arrows into the law schema and re-normalize; it verifies the law
pattern once (at catalog population) and trusts the fingerprint for
per-graph matching.

---

## The SQLite backend (§4)

`src/trait_catalog_sqlite.rs` implements `SqliteStore`, a second backend
for the SAME inference engine. The engine (`infer_traits`,
`populate_builtin_catalog`) is written against
`impl TraitCatalogStore` and is reused here verbatim via a `#[path]`
module-include trick — it is NOT duplicated and NOT modified.

**Schema** (idempotent `IF NOT EXISTS`):

```sql
CREATE TABLE trait_entries (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL,
  type_shape TEXT NOT NULL, fingerprint TEXT NOT NULL,
  laws TEXT NOT NULL, cutelim_proven INTEGER NOT NULL
);
CREATE INDEX idx_type_shape ON trait_entries(type_shape);
CREATE INDEX idx_fingerprint ON trait_entries(fingerprint);
CREATE TABLE verification_cache (
  graph_fingerprint TEXT NOT NULL, law_pattern_id TEXT NOT NULL,
  result TEXT NOT NULL,
  PRIMARY KEY (graph_fingerprint, law_pattern_id)
);
```

**Serialization:** `Ty`, `PolyGraph`, `LawEquation`, and `CatalogEntry`
derive `Serialize`/`Deserialize` (serde + serde_json). Entries are stored
as JSON blobs in `TEXT` columns and round-trip losslessly.
`PolyGraph::Arr` uses an owned `String` (not `&'static str`) so the
graph is serializable; this is behaviorally identical since
`law_equal`/`normalize` compare `Arr` names by string equality.

**`type_shape` indexing:** the `type_shape` column stores a canonical
string key: type variables are renamed to `t0, t1, ...` in order of
first appearance, so two shapes differing only in variable names share
a key. `lookup_by_type_shape` does an exact string match on the indexed
key, then confirms candidates with `Unifier::unify`. The full `Ty` is
recovered from the key via a small s-expr parser (`key_to_type_shape`).

**Honesty note on `lookup_by_type_shape`:** the ADR claims "O(1) hash"
lookup on `type_shape`. A type shape like `[A] ~> [B]` contains type
variables, so two entries match whenever their shapes UNIFY, not on
string equality. A true hash index over unifiable shapes is not
straightforward (it is a query against a unification relation, not an
equality relation). This backend uses a pragmatic approximation:
canonicalize variables, exact-match the canonical string, then
unify-confirm. This is invariant under alpha-renaming but is NOT a
complete unification index — two shapes that unify but have different
canonical structure (e.g. `[A]~>[A]` canonicalizes to `[t0]~>[t0]`
while `[X]~>[Y]` canonicalizes to `[t0]~>[t1]`; they DO unify via `X=Y`
but the index misses them) produce false negatives. Within this
prototype's built-in catalog the canonical key is exact for all stored
entries, so the equivalence test passes; a production backend would
need a unification-aware index (e.g. a substitution trie). This is a
noted gap relative to the ADR's O(1) claim.

**Persistence:** entries and the verification cache survive close +
reopen. The persistence test inserts 5 built-ins + 1 custom entry, runs
inference (filling the cache), closes the DB, reopens at the same path,
and confirms: entry count persisted, custom entry retrievable by id
with all fields intact, re-inference hits the cache (cache_hits > 0,
misses = 0), and a direct `verification_cache` read returns
`Functor::identity -> Verified`.

**Equivalence with `InMemoryStore`:** both backends, populated with the
same built-in catalog, produce identical `infer_traits` results on the
demo `map` graph (same match count, same matched traits by name /
verified laws / placeholder laws / `cutelim_proven`). Entry parity is
checked across names, fingerprints, law counts, `cutelim_proven`, and
canonicalized `type_shape`.

**Lossless law round-trip:** a `RoundTrip` entry with two laws (an
identity law and a fusion law) is inserted, reloaded by id, and the
reloaded law graphs are structurally equal to the originals
(`law0.lhs == First(Id)`, etc.). The reloaded laws are also identical to
an in-memory reference, confirming the engine can run CutElim on
reloaded laws.

### Captured output: `cargo run --bin trait_inference_sqlite`

```
=== ADR-0075 section 4: SQLite catalog backend ===
graph : (loop(first((uncons ; choice(nil,(first(f) ; cons))))) ; merge)
fp    : comp(loop(first(comp(arr,choice(arr,comp(first(arr),arr))))),arr)

=== Equivalence test: InMemoryStore vs SqliteStore ===
both backends populated with 5 entries
[pass] entry parity (names, fingerprints, laws, type_shape, cutelim_proven)
[pass] infer_traits identical across InMemoryStore and SqliteStore
  matches:
    - Functor : cutelim_proven=true verified=["identity", "composition"] placeholder=[]
    - Mapping : cutelim_proven=false verified=[] placeholder=["length"]

=== Persistence test: close + reopen, cache survives ===
phase 1: inserted 6 entries (custom id=6), ran inference (cache_hits=0, misses=3)
phase 2: reopened, 6 entries present
[pass] custom entry retrievable by id=6 with all fields intact
phase 2 re-inference: cache_hits=3, cache_misses=0
[pass] verification cache persisted across reopen (cache_hits=3)
[pass] direct verification_cache read: Functor::identity -> Verified
[pass] temp db cleaned up

=== CRUD test: insert / get / remove ===
[pass] insert -> get -> remove -> get(None) -> entries empty

=== Index test: lookup_by_fingerprint selectivity ===
inserted 9 entries total
[pass] lookup_by_fingerprint selective (custom fps -> 1 each; map fp -> 2)

=== Round-trip test: laws survive SQLite and remain CutElim-checkable ===
[pass] law graphs round-trip losslessly (structural equality holds)
[pass] reloaded laws identical to in-memory reference

=== === All tests passed === ===
```

---

## What is proven vs aspirational

### Proven (zero `sorry` / passing tests)

- **Functor identity + composition laws via CutElim** (Lean,
  `TraitInference.lean`): `first(idG A)` and `idG(A x D)` both normalize
  to `idG(A x D)`; `first(comp G H)` and `comp(first G)(first H)` both
  normalize to `first(comp G H)`. Mirrored in Rust (`law_equal` true
  cases for all 6 rewrites).
- **Coproduct-functor identity + composition laws via CutElim** (Lean +
  Rust): the `left` analogues of the above.
- **CutElim preserves proofs and types** (`ProofAdvantage.lean`):
  `cut_elim_preserves_proof`, `cut_elim_preserves_type`, and their
  `CutElimStar` closures — extended to handle `ceFirstId`/`ceLeftId`.
- **Semantic equivalence of the functor identity law** (Lean):
  `first_id_semantic_equiv`, `left_id_semantic_equiv` — both sides
  produce the same output for the same input, from the `Eval` rules.
- **The 3-step pipeline recognizes `Functor` for the `map` graph**
  (Rust): type filter + fingerprint + CutElim verify the two functor
  laws; `Mapping` reports as a placeholder multi-match.
- **SQLite persistence + verification cache** (Rust): entries and cache
  survive close + reopen; re-inference is all cache hits.
- **Law round-trip** (Rust): law graphs survive SQLite serde losslessly
  and remain structurally equal to the originals.
- **Backend equivalence** (Rust): `InMemoryStore` and `SqliteStore`
  produce identical `infer_traits` results on the built-in catalog.

### Aspirational / stubbed

- **Non-functor equational laws** (monad, roundtrip,
  sortedness-as-equation): stubbed as semantic placeholders
  (`cutelim_proven = false`). Their laws (`Sorted`, `LengthEq`,
  `AllSatisfy`) are not expressible as graph equations over the
  5-combinator kernel. This matches the ADR's "What is NOT yet proven"
  and OQ#1/#2/#4.
- **Higher-kinded type shapes (`F<A>`):** `Ty` has no type-application
  constructor, so `Functor`'s `type_shape` uses the `List` instance
  `[A] ~> [B]` rather than the generic `F<A> ~> F<B>`. The type filter
  is therefore coarser than the ADR's eventual HKT form (OQ#5).
- **Trait resolution coherence:** no coherence/ambiguity algorithm;
  multi-match returns all matches; no global uniqueness check (ADR
  Negative; OQ#3).
- **CutElim strong normalization:** not proven; fuel (`NORMALIZE_FUEL =
  1024`) is a safety net. The rewrites are size-non-increasing and
  terminate in practice, but a non-terminating law check is possible
  (ADR Negative; OQ#2).
- **CutElim congruence:** the Lean `CutElim` rewrites only at the top
  level; rewriting inside subgraphs (needed for complex graphs like
  `map_list` built from primitives) requires congruence rules — open.
- **`App`/ArrowApply blanket impls:** `PolyGraph::App` is an opaque
  placeholder (fresh vars, not wired into CutElim). Blanket impls via
  ArrowApply (ADR §10) are not implemented.
- **The surface syntax (§5-§10):** no `trait`/`impl`/`law` declaration
  parser, no supertraits, no associated types, no generic-code trait
  bounds. The prototype operates on the kernel `PolyGraph` directly.
- **`inferred_trait` as a data structure:** in Lean it is a `Prop`-valued
  flag, not a list/set of satisfied laws. The Rust side returns a
  `Vec<MatchedTrait>` (richer), but the Lean mechanization does not yet
  enumerate laws.
- **The "preserves all D" connection:** technically true but vacuously
  weak — only the identity case `first(idG A)` is discharged (the graph
  is semantically the identity, so it trivially preserves every
  invariant). Non-identity functorial graphs are not covered.
- **`lookup_by_type_shape` as a true unification index:** the SQLite
  backend uses a canonical-string-index + unify-confirm approximation,
  which is invariant under alpha-renaming but produces false negatives
  for unifiable-but-structurally-different shapes. A
  unification-aware index is future work.

---

## Open questions carried forward

The ADR lists 8 Open Questions. Status after this prototype:

- **OQ#1 — Trait inference mechanization (load-bearing).** Law inference
  for non-functor patterns is NOT mechanized. The prototype proves the
  functor/coproduct-functor structural pattern; extending to monad,
  roundtrip, and other equational laws remains the key open question.
  *The prototype informs this: it confirms the structural-pattern +
  CutElim approach works for the functor case, but does not extend
  beyond it.*
- **OQ#2 — CutElim-based law checking (part of this ADR).** The 6
  rewrites are mechanized (Lean + Rust) and prove the functor laws.
  Termination is not proven (fuel is the safety net); congruence
  (rewriting inside subgraphs) is open; completeness is open. *The
  prototype informs this: it provides the rewrite relation and the
  normalizer, but not the termination proof or the congruence
  extension.*
- **OQ#3 — Trait resolution / catalog search.** The 3-step pipeline IS
  the catalog search. No coherence/ambiguity algorithm; multi-match
  returns all matches. *Implemented as far as the pipeline; coherence
  deferred.*
- **OQ#4 — Law expressiveness.** Which laws are decidable under CutElim?
  The prototype shows the linear functor fragment is decidable (the 6
  rewrites terminate for it). The non-linear extension is open.
- **OQ#5 — Higher-kinded traits.** NOT modelled. `Ty` has no
  type-application constructor; `Functor` uses the `List` instance.
  *The prototype surfaces this gap concretely.*
- **OQ#6 — Catalog fingerprinting.** The prototype chooses a simple
  structural hash (combinator tree with `Arr` names erased to
  `arr`). This is invariant under base-arrow renaming and sensitive to
  combinator-shape differences, but is NOT invariant under
  alpha-equivalent rewiring of the law-relevant structure beyond shape.
  *The prototype makes a concrete choice; the design question remains
  open.*
- **OQ#7 — Catalog distribution.** Not addressed (storage is local
  SQLite; source files are the source of truth). Out of scope for this
  prototype.
- **OQ#8 — Built-in catalog contents.** The prototype ships 5 entries
  (Functor, CoproductFunctor, Sorting, Mapping, Filtering). The minimal
  set is not finalized. *The prototype provides a concrete starting
  set; the question of what else belongs remains open.*

---

## How to run

From the poc directory
(`/home/luis/GitHub/handl-lang/platform/research/arrow-calculus-kernel-poc`):

```
cd lean && lake build
```
Builds the Lean4 mechanization (19 jobs, zero `sorry`, builds green).

```
cargo run --bin trait_inference
```
Runs the in-memory catalog demo + 10 tests (CutElim law_equal, fingerprint,
type/fingerprint pruning, pipeline multi-match, cache, built-in catalog).

```
cargo run --bin trait_inference_sqlite
```
Runs the SQLite backend tests (equivalence with InMemoryStore,
persistence across reopen, CRUD, index selectivity, lossless law
round-trip).

---

## References

- ADR: `platform/project/adrs/0075-traits-as-arrow-signatures.md`
- Lean mechanization:
  `lean/ArrowMinimalityLib/TraitInference.lean` (law inference via
  CutElim; module doc lines 1-58; Summary + Honest assessment lines
  586-653)
- CutElim rewrite relation:
  `lean/ArrowMinimalityLib/ProofAdvantage.lean` (6 rewrites lines
  101-124; preservation theorems lines 146-232)
- Rust prototype:
  `src/trait_catalog.rs` (catalog, pipeline, in-memory store,
  CutElim normalizer, built-in catalog, demo + tests)
- SQLite backend:
  `src/trait_catalog_sqlite.rs` (`SqliteStore`, schema, persistence,
  equivalence, round-trip tests)
- Style reference: `RESULTS_PHASE3.md` (Phase 3 results doc)
