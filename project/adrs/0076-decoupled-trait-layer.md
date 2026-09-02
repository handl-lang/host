# ADR-0076: Decoupled Trait Layer — Laws, Bundles, and Handlers

## Status

Proposed. Supersedes the bundled `trait`/`impl` syntax in ADR-0075 §2,
§6, §7, §8, §13. Preserves ADR-0075 §3 (catalog), §4 (storage), §11
(CutElim), §12 (catalog search as fallback).

## Context

### What ADR-0075 proposed

ADR-0075 introduced a single bundled construct: `trait` (a named schema
of arrow operations plus equational laws), `impl` (a graph assignment
for each operation), and `law` (an equation between arrow compositions,
declared inside a `trait`). The trait layer bundled eight features:

1. Named bundles of arrows.
2. Passing arrows as parameters.
3. Resolution (finding the right arrows for a type).
4. Named equational laws with compositional proof reuse.
5. First-class arrows (already in the kernel).
6. Effect handlers (already in the kernel, per ADR-0075 §13).
7. Dependent types / associated types (already in the kernel).
8. ArrowApply / blanket impls (already in the kernel).

### The analysis: 8 features, handl already has 6

Six of the eight features are already present in handl, independent of
any `trait` keyword:

- **Records of arrows** — Phase 3's `Language = { parse, eval, format,
  typecheck, lit }` is already a named bundle of arrows, with impls
  (Arithmetic, Boolean). No `trait` keyword needed.
- **First-class arrows** — the kernel (`arr`/`>>>`/`first`/`choice`/
  `loop`/`app`) already passes arrows as parameters.
- **Effect handlers** — `handle ... with` is already the resolution and
  injection mechanism (ADR-0052, ADR-0075 §13).
- **Dependent types** — `dPi`/`dSigma` (`DependentProofGraphs.lean`)
  already give associated types.
- **Signature inclusion** — supertraits are just signature union; no new
  mechanism.
- **ArrowApply** — already in the kernel; gives blanket impls.

The **two features handl lacks** are:

- **Equational laws** — the ability to *require* that an arrow satisfies
  an equation, and to verify that requirement.
- **Law proof reuse** — verify the law once, then inherit the proof by
  composition at every use site.

Everything else in the `trait`/`impl` bundle is sugar over what the
kernel already does. The bundle (the `trait` keyword) is only
load-bearing for **multi-operation laws** — laws that span two or more
arrows from the same impl. A roundtrip law needs both `serialize` and
`deserialize` from the same impl; a single-operation law like the
functor law attaches to one arrow and needs no bundle at all.

### The coherence problem

Rust and Haskell solve the "which impl?" question with global coherence:
at most one impl per trait+type, worldwide, enforced by orphan rules and
specificity algorithms. This is the most complex and most complained-about
part of both languages' trait systems.

ADR-0075 §13 already moved to explicit `handle ... with` as the primary
resolution mechanism, which avoids the coherence problem by lexical
scoping (nearest-wins, like exception handlers). This ADR makes that the
*only* resolution story — there is no implicit global resolution at all.
The coherence problem is not solved by an algorithm; it is eliminated by
design.

### The research foundation (unchanged)

The Lean4 mechanization and Rust prototype from ADR-0075 remain valid
and are the substrate for this ADR:

- **CutElim normalization verifies structural laws.**
  `TraitInference.lean` proves the functor laws (`first id ≡ id`,
  `first (f >>> g) ≡ first f >>> first g`) by CutElim normalization,
  with zero `sorry`. The recognition is purely from wiring structure
  (`UsesFirst` triggers the check). The "Honest assessment" (lines
  627-653) is clear: this works for the cases attempted; congruence and
  more laws are the obstacles for the general case.

- **Congruence + strong normalization.** `CutElimCongruence.lean` closes
  both gaps for the six-rewrite system: it adds congruence rules
  (rewriting inside subgraphs) and proves strong normalization — every
  rewrite sequence terminates in at most `graphSize G - 1` steps. This
  means CutElim-based law checking is decidable for the six-rewrite
  fragment.

- **Non-functor laws require semantic axioms.**
  `EquationalLawInference.lean` proves that laws like the roundtrip law
  (`serialize >>> deserialize ≡ id`) and semigroup associativity are
  *not* CutElim rewrites — they are semantic facts about specific base
  arrows. The mechanization models this honestly: the law's `sound`
  field is an explicit hypothesis (an axiom), not hidden behind a
  `sorry`. Structural pattern recognition still triggers the check; the
  axiom discharges it.

- **The Rust prototype** (`trait_catalog.rs`, `trait_catalog_sqlite.rs`)
  demonstrates the catalog + CutElim pipeline, including bottom-up
  congruence (`rewrite_pass`) and SQLite-backed storage.

None of this changes. This ADR changes only the *surface syntax and
semantic framing* of the trait layer — how the user writes laws,
bundles, and handlers — not the verification machinery underneath.

## Decision

Replace the bundled `trait`/`impl`/`law` construct with **three
independent, composable features**: laws, bundles, and handlers. No
`trait` keyword. No `impl` keyword. The "trait" pattern is an emergent
shape produced by combining the three, not a syntactic category.

### 1. Law on an arrow

A `law` is a standalone declaration that an arrow satisfies an
equation. It attaches to any arrow — no bundle required.

```handl
map : (A ~> B) ~> (F<A> ~> F<B>)
law map_id:    map id        = id
law map_comp:  map (f >>> g) = map f >>> map g
```

- `=` inside a `law` context means **extensional equality of arrows**:
  both sides produce the same output for all inputs. This is
  unambiguous because `law ...` is a distinct context. No special `≡`
  symbol is needed — `=` is easy to type and unambiguous in context.
- The law is verified by **CutElim normalization** (for structural laws
  like the functor laws, per `TraitInference.lean` and
  `CutElimCongruence.lean`) or by **semantic axioms** (for non-functor
  laws like roundtrip, per `EquationalLawInference.lean`).
- **Law proof reuse by composition.** If `migrate = deserialize >>>
  serialize` and both `serialize` and `deserialize` have verified
  roundtrip laws, then `migrate` inherits the lossless proof by
  composition — no separate proof. Laws compose because laws *are*
  arrow compositions.

A single-arrow law needs no bundle. The functor law attaches to `map`
alone. This is the common case.

### 2. Bundle

A **bundle** is a named record of arrows plus laws over them. It is
only needed when a law spans multiple operations that must come from the
same impl.

```handl
Serializer<F> = {
  serialize   : F ~> Text
  deserialize : Text ~> F
  law roundtrip: serialize >>> deserialize = id
}
```

- A bundle is a named record of arrows with laws attached. It is
  *syntactically* a record — no new keyword.
- The bundle guarantees the operations come from the same impl (same
  handler). This is what makes the roundtrip law meaningful: both
  `serialize` and `deserialize` are bound together.
- **Without laws, a bundle is just a record of arrows** — sugar, no
  semantic power. `Language = { parse, eval, format, typecheck, lit }`
  is a bundle with no laws; it is a named record, nothing more.
- No `trait` keyword. A bundle is just a record with laws attached.

### 3. Handler

A **handler** injects an arrow or bundle at a scope boundary. This is
dependency injection over effect handlers — the same machinery as
ADR-0052, now the single resolution mechanism.

```handl
-- inject a single arrow
handle sort_by intCmp list with map = map_list

-- inject a bundle (the "trait" case)
handle migrate json with Serializer<JSON> = json_impl
```

- **Lexically scoped, nearest-wins** — like exception handlers. An
  inner handler shadows an outer one for the same arrow or bundle.
- **No coherence problem** — scoping *is* resolution. There is no
  global uniqueness check, no orphan rule, no specificity algorithm.
- **Law checking runs once at the `handle` site.** When the handler is
  installed, the laws are verified (CutElim for structural laws, axioms
  for non-functor laws). If verification fails, the `handle` form is a
  compile error. Every inner use inherits the proof.
- **Static handler** (impl fixed at compile time) → laws verified, proof
  travels. This is the "trait" case.
- **Dynamic handler** (impl chosen at runtime) → no compile-time law
  guarantee. This is the effect-handler case.
- The bundle marks "this is static, law-checked." A bare operation
  handle is "this is dynamic, no law claim."

### How they compose

The three features are independent. No combination is mandatory. The
"trait" pattern emerges when all three are present.

| Combo | What you get |
|---|---|
| Law alone | Verified arrow |
| Bundle alone | Named record (sugar) |
| Handler alone | Effect injection (dynamic ok) |
| Law + Bundle | Verified multi-op contract |
| Law + Handler | Verified, injected arrow |
| Bundle + Handler | Injected contract |
| Law + Bundle + Handler | The "trait" pattern (all three) |

The "trait" pattern is an emergent shape, not a keyword. A user who
needs only a verified arrow writes a law. A user who needs a named
record writes a bundle. A user who needs runtime injection writes a
handler. A user who needs the full Rust/Haskell trait experience
combines all three.

### `=` in laws

Inside a `law` context, `=` means extensional equality of arrows (both
sides produce the same output for all inputs). This is unambiguous
because `law ...` is a distinct syntactic context — the parser knows it
is in a law, not in an expression. No special `≡` symbol is needed.

ADR-0075 used `≡` inside `law` declarations. This was inherited from
the Lean4 mechanization, where `≡` distinguishes graph equality from
definitional equality. At the surface syntax level, the `law` context
already disambiguates; `≡` is unnecessary ceremony. This ADR drops it.

### Resolution: handlers primary, catalog fallback

1. **Primary: explicit `handle ... with`** — names the impl, lexically
   scoped, no ambiguity, laws checked at installation. This is the safe
   default.
2. **Fallback: catalog search** — when no handler is installed, search
   the open trait catalog (ADR-0075 §3). One match = inject. Multiple
   matches = error: "ambiguous — N impls match, name one explicitly via
   `handle ... with`." The compiler never silently picks.
3. **No global coherence** — the problem is avoided by design (lexical
   scoping), not solved by algorithm.

The catalog (ADR-0075 §3) becomes the **ambient default handler set** —
handlers that are always installed unless overridden by an explicit
`handle ... with`. The catalog does not disappear; it is the fallback
layer. Catalog storage (ADR-0075 §4) is unchanged.

### What this replaces from ADR-0075

- The `trait` keyword → replaced by the optional **bundle** (a record
  with laws attached). No keyword.
- The `impl` keyword → replaced by `handle ... with` (handler
  installation). No keyword.
- The `law` keyword inside `trait` → `law` is now **standalone**,
  attachable to any arrow or bundle.
- Implicit catalog search as primary resolution → replaced by
  **explicit handlers as primary**, catalog search as fallback.
- The coherence problem → **eliminated** by lexical scoping of handlers.
- The `≡` symbol in laws → replaced by `=` (unambiguous in `law`
  context).

### What this preserves from ADR-0075

- **The open trait catalog (§3)** — becomes the ambient default handler
  set. Unchanged.
- **Catalog storage (§4)** — unchanged (abstract interface, SQLite
  default).
- **CutElim law verification (§11)** — unchanged, now applied at the
  `handle` site.
- **Catalog search (§12)** — demoted to fallback for the one-impl case.
  Unchanged in mechanism.
- **The research foundation** — CutElim rewrites, congruence, strong
  normalization, non-functor law axioms. Unchanged.
- **The prototype implementation** — the Rust/Lean code still
  demonstrates the catalog + CutElim pipeline. Unchanged.

## Consequences

### Positive

- **Simpler.** Three small features replace one large bundled construct.
  Each feature is understandable in isolation.
- **Composable.** The "trait" pattern is an emergent shape, not a
  keyword. Users take only what they need: a law, a bundle, a handler,
  or any combination.
- **No `trait` keyword.** The most complex syntactic category in
  Rust/Haskell is gone. A bundle is just a record with laws; an impl is
  just a handler.
- **No coherence problem.** Lexical scoping of handlers eliminates the
  hardest part of Rust/Haskell trait systems by design. The compiler
  never silently picks an impl.
- **One mechanism for traits and effects.** Trait resolution and effect
  handling use the same `handle ... with` machinery (ADR-0052). No
  separate resolution algorithm, no separate coherence checker.
- **`=` is easy to type.** No `≡` ceremony. The `law` context
  disambiguates.
- **Laws are proven, not tested** (for structural laws). CutElim
  normalization with congruence and strong normalization
  (`CutElimCongruence.lean`) gives decidable law checking for the
  six-rewrite fragment. Non-functor laws are discharged by explicit
  semantic axioms (`EquationalLawInference.lean`) — honest, not hidden.
- **Proof reuse by composition.** If `migrate = deserialize >>>
  serialize` and both arrows have verified roundtrip laws, `migrate`
  inherits the proof. Laws compose because laws are arrow compositions.

### Negative

- **More explicit.** Users must write `handle ... with` for the safe
  (law-checked) path. There is no implicit "the system finds the impl"
  by default — that is the fallback, and it errors on ambiguity. This
  is more verbose than Rust/Haskell for the common case.
- **No global uniqueness guarantee.** Because resolution is lexical,
  the same type can have different impls in different scopes. This is
  intentional (it is what eliminates coherence), but it means a type
  does not have a single canonical trait impl across a program.
- **Bundled-operations effect representation is open.** Effect handlers
  typically handle one operation. A bundle is a *record* of operations
  handled together. How the effect system represents a bundle as one
  handler is a design question for the effect-handler mechanism, not
  resolved here (see Open Questions).
- **Dynamic handlers cannot be law-checked.** A handler whose impl is
  chosen at runtime has no compile-time law guarantee. This is the
  static/dynamic split: static handlers are the "trait" case (laws
  verified); dynamic handlers are the effect-handler case (no law
  claim). There is no mechanism to verify laws for runtime-chosen impls.

### Neutral

- **Departure from Rust/Haskell.** Resolution is explicit and lexical,
  not implicit and global. Users familiar with Rust/Haskell traits will
  find a different mental model: handlers, not instances.
- **The "trait" pattern is emergent, not syntactic.** There is no
  syntactic marker that says "this is a trait." The pattern is
  recognized by the combination of law + bundle + handler. This is
  consistent with handl's annotation-free philosophy but requires users
  to see the shape rather than read a keyword.

## Open Questions

1. **Bundled-operations effect representation.** How does the effect
   system handle a bundle of operations as one handler? A `Serializer`
   bundle binds `serialize` and `deserialize` together; the effect
   system must treat them as a unit. This is a design question for the
   effect-handler mechanism, deferred from ADR-0075 §13.

2. **Dynamic handlers + laws.** Can we verify laws for runtime-chosen
   impls? Likely no — this is the static/dynamic split. A dynamic
   handler is the effect-handler case (no law claim); a static handler
   is the trait case (laws verified). Confirming this boundary is
   deferred.

3. **Specificity-by-verified-laws as a catalog search tiebreaker.**
   When catalog search returns multiple matches, the default is
   "multiple = error." A convenience tiebreaker (prefer the impl with
   more verified laws) is possible but deferred — the default is honest:
   ask the user to name one explicitly.

4. **Law syntax for higher-order laws.** Laws that quantify over arrows
   (e.g. `forall f g. map (f >>> g) = map f >>> map g`) need explicit
   quantifier syntax. The examples in this ADR use free variables
   (`f`, `g`) implicitly scoped by the law; the precise quantifier
   syntax is deferred.

5. **Whether single-arrow laws should be inferable or always declared.**
   ADR-0075's primary mode was trait *inference* — the system recognizes
   laws from wiring without declaration. This ADR defaults to declared
   laws (the secondary mode), because inference is proven only for
   functor laws (`TraitInference.lean`) and is not complete in general
   (the "Honest assessment" is explicit about this). Whether inference
   becomes the default for single-arrow laws is deferred pending
   further mechanization.

## References

- [ADR-0075 — Traits as Arrow Signatures](./0075-traits-as-arrow-signatures.md)
  (superseded: §2, §6, §7, §8, §13; preserved: §3, §4, §11, §12)
- [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md)
  (handlers: `arr`/`>>>`/`first`/`choice`/`loop`/`app`)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md)
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/TraitInference.lean`
  — functor law verification by CutElim; "Honest assessment" of
  obstacles for the general case.
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/EquationalLawInference.lean`
  — non-functor laws require semantic axioms; structural recognition +
  axiom discharge.
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/CutElimCongruence.lean`
  — congruence rules + strong normalization for the six-rewrite system.
- `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/ProofAdvantage.lean`
  — the CutElim rewrite relation (`idLeft`, `idRight`, `firstFusion`,
  `leftFusion`).
- `research/arrow-calculus-kernel-poc/src/trait_catalog.rs`,
  `trait_catalog_sqlite.rs` — the Rust prototype (catalog + CutElim
  pipeline, bottom-up congruence, SQLite storage).
- `research/arrow-calculus-kernel-poc/RESULTS_TRAITS.md` — prototype
  results.
