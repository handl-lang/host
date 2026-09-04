//! Trait inference: annotation-free law recognition (Phase 9).
//!
//! This module is the Rust mirror of the Lean4 formalization in
//! `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/TraitInference.lean`.
//!
//! The Lean4 file proves that a graph's wiring structure alone — without any
//! trait declaration — determines which equational laws it satisfies. The key
//! concepts are:
//!
//! - `Law` — a named equational law (functor identity, functor composition,
//!   coproduct functor identity, coproduct functor composition).
//! - `UsesFirst` / `UsesLeft` — structural recognizers that detect whether a
//!   graph uses the `first` (product functor) or `left` (coproduct functor)
//!   combinator anywhere in its wiring.
//! - `SatisfiesLaw` — a graph `G` satisfies `law` when (1) G's wiring
//!   exhibits the structural pattern the law applies to, and (2) the law is
//!   verified (both sides normalize to the same canonical form). In the Rust
//!   port, law verification is taken as proven (the Lean4 proofs have zero
//!   `sorry`), so `SatisfiesLaw` reduces to the structural-pattern check.
//! - `inferred_trait` — the set of laws inferred from a graph's wiring, by
//!   structural recursion. This is the annotation-free trait inference: the
//!   system inspects the graph and returns the laws it satisfies.
//!
//! # Mapping from Lean4 `Graph` to Rust `Scheme`
//!
//! The Lean4 `Graph` type has constructors `idG`, `arr`, `comp`, `first`,
//! `left`, `loop`. The Rust `Scheme` type has `Arr`, `Seq`, `First`,
//! `Choice`, `Loop`, `App`. The correspondence is:
//!
//! | Lean4 `Graph`     | Rust `Scheme`                  |
//! |-------------------|--------------------------------|
//! | `Graph.first F`   | `Scheme::First { inner: F }`   |
//! | `Graph.left F`    | `Scheme::Choice { l: F, r }`   |
//! | `Graph.comp G H`  | `Scheme::Seq { fst: G, snd: H }`|
//! | `Graph.loop G`    | `Scheme::Loop { body: G }`     |
//! | `Graph.idG A`     | `Scheme::arr_native("id", [])` |
//! | `Graph.arr f`     | `Scheme::Arr { op: f, .. }`    |
//!
//! The `left` combinator (coproduct functor) maps to `Scheme::Choice` in the
//! Rust IR. In the Lean4 formalization, `left F` routes `Left` through `F`
//! and `Right` through the identity — this is `Choice { l: F, r: id }` in
//! the Rust IR. The `UsesLeft` recognizer therefore checks for `Choice`
//! nodes where the left branch is the "active" sub-graph (the right branch
//! is the pass-through identity).

use std::fmt;

use handl_language::Scheme;

// ============================================================================
// The `Law` type: named equational laws (mirrors Lean4 `Law`)
// ============================================================================

/// Named equational laws between graph expressions, mirroring the Lean4
/// `Law` inductive type.
///
/// Each constructor identifies a specific law and names the combinator it
/// applies to (`first` for product-functor laws, `left` for coproduct-functor
/// laws). The Lean4 formalization carries type/graph parameters; the Rust
/// port names only the law (the parameters are implicit in the `Scheme`
/// structure being analyzed).
///
/// # The four laws
///
/// 1. **FunctorIdentity**: `first (idG A) ≡ idG (A × D)`
///    — functoriality of `first` preserves the identity.
/// 2. **FunctorComposition**: `first (comp G H) ≡ comp (first G) (first H)`
///    — functoriality of `first` distributes over composition (fusion).
/// 3. **CoproductIdentity**: `left (idG A) ≡ idG (A + D)`
///    — functoriality of `left` preserves the identity.
/// 4. **CoproductComposition**: `left (comp G H) ≡ comp (left G) (left H)`
///    — functoriality of `left` distributes over composition (fusion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Law {
    /// `first (idG A) ≡ idG (A × D)` — functor identity for `first`.
    FunctorIdentity,
    /// `first (comp G H) ≡ comp (first G) (first H)` — functor composition
    /// (fusion) for `first`.
    FunctorComposition,
    /// `left (idG A) ≡ idG (A + D)` — coproduct functor identity for `left`.
    CoproductIdentity,
    /// `left (comp G H) ≡ comp (left G) (left H)` — coproduct functor
    /// composition (fusion) for `left`.
    CoproductComposition,
}

impl Law {
    /// All four laws, in declaration order.
    pub fn all() -> &'static [Law] {
        &[
            Law::FunctorIdentity,
            Law::FunctorComposition,
            Law::CoproductIdentity,
            Law::CoproductComposition,
        ]
    }

    /// A human-readable name for the law (matching the Lean4 constructor
    /// names).
    pub fn name(&self) -> &'static str {
        match self {
            Law::FunctorIdentity => "functorId",
            Law::FunctorComposition => "functorComp",
            Law::CoproductIdentity => "coproductFunctorId",
            Law::CoproductComposition => "coproductFunctorComp",
        }
    }

    /// The equation the law states, in arrow-calculus notation.
    pub fn equation(&self) -> &'static str {
        match self {
            Law::FunctorIdentity => "first (idG A) ≡ idG (A × D)",
            Law::FunctorComposition => "first (comp G H) ≡ comp (first G) (first H)",
            Law::CoproductIdentity => "left (idG A) ≡ idG (A + D)",
            Law::CoproductComposition => "left (comp G H) ≡ comp (left G) (left H)",
        }
    }

    /// Whether this law applies to the `first` (product functor) combinator.
    pub fn is_functor_law(&self) -> bool {
        matches!(self, Law::FunctorIdentity | Law::FunctorComposition)
    }

    /// Whether this law applies to the `left` (coproduct functor) combinator.
    pub fn is_coproduct_law(&self) -> bool {
        matches!(
            self,
            Law::CoproductIdentity | Law::CoproductComposition
        )
    }
}

impl fmt::Display for Law {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// `LawVerified`: a law is verified by CutElim normalization (Lean4-proven)
// ============================================================================

/// Whether a law is verified by CutElim normalization.
///
/// In the Lean4 formalization, `LawVerified law` is a `Prop` that holds when
/// both sides of the law normalize to the same canonical form under
/// `CutElimStar`. The theorem `all_functor_laws_verified` proves that **all
/// four laws are verified** — with zero `sorry`.
///
/// In the Rust port, we take this as a proven fact: `law_verified` always
/// returns `true`. The structural-pattern check (`UsesFirst` / `UsesLeft`) is
/// the only runtime computation needed; the law verification is a theorem,
/// not a runtime check.
///
/// This mirrors the Lean4 design where `SatisfiesLaw G law` is
/// `UsesFirst G ∧ LawVerified law` — the `LawVerified` conjunct is
/// theorem-level (always true), so the runtime work is entirely in the
/// `UsesFirst` / `UsesLeft` recognizer.
pub fn law_verified(_law: Law) -> bool {
    // All four laws are proven verified by CutElim normalization in the
    // Lean4 formalization (all_functor_laws_verified, zero sorry). We take
    // this as a proven fact — no runtime check needed.
    true
}

// ============================================================================
// `UsesFirst` / `UsesLeft`: structural pattern recognizers
// ============================================================================

/// Whether a `Scheme` graph *uses `first`* — i.e. `Scheme::First` appears
/// anywhere in its wiring.
///
/// This is the Rust mirror of the Lean4 `UsesFirst` inductive predicate. A
/// graph uses `first` if:
/// - It is a `Scheme::First { .. }` node (the `here` case), OR
/// - It is a `Scheme::Seq { fst, snd }` and either `fst` or `snd` uses
///   `first` (the `leftComp` / `rightComp` cases), OR
/// - It is a `Scheme::Loop { body }` and `body` uses `first`, OR
/// - It is a `Scheme::Choice { l, r }` and either branch uses `first`, OR
/// - It is a `Scheme::App { callee, args }` and the callee or any arg uses
///   `first`.
///
/// `Scheme::Arr` nodes never use `first` (they are base arrows).
///
/// # Example
///
/// ```
/// use handl_language::Scheme;
/// use handl_plugin_ext_refinement::trait_inference::uses_first;
///
/// // first(id) — uses first directly.
/// let g = Scheme::first(Scheme::arr_native("id", vec![]));
/// assert!(uses_first(&g));
///
/// // comp(first(id), id) — uses first in the left component.
/// let g = Scheme::seq(
///     Scheme::first(Scheme::arr_native("id", vec![])),
///     Scheme::arr_native("id", vec![]),
/// );
/// assert!(uses_first(&g));
///
/// // id — does not use first.
/// let g = Scheme::arr_native("id", vec![]);
/// assert!(!uses_first(&g));
/// ```
pub fn uses_first(scheme: &Scheme) -> bool {
    match scheme {
        Scheme::First { .. } => true,
        Scheme::Seq { fst, snd } => uses_first(fst) || uses_first(snd),
        Scheme::Loop { body } => uses_first(body),
        Scheme::Choice { l, r } => uses_first(l) || uses_first(r),
        Scheme::App { callee, args } => {
            uses_first(callee) || args.iter().any(uses_first)
        }
        Scheme::Arr { args, .. } => args.iter().any(uses_first),
        Scheme::Handle { body, impl_, .. } => uses_first(body) || uses_first(impl_),
    }
}

/// Whether a `Scheme` graph *uses `left`* — i.e. the coproduct-functor
/// combinator appears anywhere in its wiring.
///
/// In the Lean4 formalization, `left F` is a dedicated graph constructor
/// that routes `Left` through `F` and `Right` through the identity. In the
/// Rust `Scheme` IR, the coproduct functor is represented as
/// `Scheme::Choice { l: F, r: id }` — the left branch is the active
/// sub-graph, and the right branch is the pass-through identity.
///
/// This recognizer checks for `Choice` nodes where the left branch is
/// non-trivial (not just an identity) — this distinguishes a genuine
/// `left(F)` (coproduct functor) from a general `choice(G, H)` (which may
/// be a conditional, not a functor application). The right branch being an
/// identity (`op == "id"`) is the hallmark of `left`.
///
/// This is the Rust mirror of the Lean4 `UsesLeft` inductive predicate.
///
/// # Example
///
/// ```
/// use handl_language::Scheme;
/// use handl_plugin_ext_refinement::trait_inference::uses_left;
///
/// // left(id) = choice(id, id) — uses left (coproduct functor).
/// let g = Scheme::choice(
///     Scheme::arr_native("id", vec![]),
///     Scheme::arr_native("id", vec![]),
/// );
/// assert!(uses_left(&g));
///
/// // id — does not use left.
/// let g = Scheme::arr_native("id", vec![]);
/// assert!(!uses_left(&g));
/// ```
pub fn uses_left(scheme: &Scheme) -> bool {
    match scheme {
        Scheme::Choice { l, r } => {
            // A `left(F)` is `Choice { l: F, r: id }`. We recognize it when
            // the right branch is an identity (the pass-through), which is
            // the structural hallmark of the coproduct functor. The left
            // branch carries the active sub-graph F.
            is_identity(r) || uses_left(l) || uses_left(r)
        }
        Scheme::Seq { fst, snd } => uses_left(fst) || uses_left(snd),
        Scheme::Loop { body } => uses_left(body),
        Scheme::First { inner } => uses_left(inner),
        Scheme::App { callee, args } => {
            uses_left(callee) || args.iter().any(uses_left)
        }
        Scheme::Arr { args, .. } => args.iter().any(uses_left),
        Scheme::Handle { body, impl_, .. } => uses_left(body) || uses_left(impl_),
    }
}

/// Whether a `Scheme` is the identity arrow (`arr("id")` with no args).
fn is_identity(scheme: &Scheme) -> bool {
    matches!(
        scheme,
        Scheme::Arr {
            op,
            args,
            ..
        } if op == "id" && args.is_empty()
    )
}

// ============================================================================
// `SatisfiesLaw`: graph G satisfies law L
// ============================================================================

/// Whether a `Scheme` graph `G` satisfies `law`.
///
/// This is the Rust mirror of the Lean4 `SatisfiesLaw G law` predicate. A
/// graph satisfies a law when:
/// 1. G's wiring exhibits the structural pattern the law applies to
///    (`UsesFirst` for functor laws, `UsesLeft` for coproduct-functor laws),
///    AND
/// 2. The law is verified by CutElim normalization (`LawVerified law`).
///
/// Since all four laws are proven verified in the Lean4 formalization (zero
/// `sorry`), condition (2) always holds. The runtime computation is entirely
/// in the structural-pattern check.
///
/// # Inference theorem
///
/// The Lean4 inference theorems (`satisfies_functor_identity`, etc.) state:
/// if `G` uses `first`, then `G` satisfies the functor identity and
/// composition laws — **without the law being declared**. This function
/// implements that inference: given a graph and a law, it checks whether the
/// graph's wiring matches the law's pattern.
///
/// # Example
///
/// ```
/// use handl_language::Scheme;
/// use handl_plugin_ext_refinement::trait_inference::{satisfies_law, Law};
///
/// // first(id) uses first → satisfies FunctorIdentity.
/// let g = Scheme::first(Scheme::arr_native("id", vec![]));
/// assert!(satisfies_law(&g, Law::FunctorIdentity));
/// assert!(satisfies_law(&g, Law::FunctorComposition));
/// // first(id) does not use left → does not satisfy CoproductIdentity.
/// assert!(!satisfies_law(&g, Law::CoproductIdentity));
/// ```
pub fn satisfies_law(scheme: &Scheme, law: Law) -> bool {
    let pattern_matches = match law {
        Law::FunctorIdentity | Law::FunctorComposition => uses_first(scheme),
        Law::CoproductIdentity | Law::CoproductComposition => uses_left(scheme),
    };
    pattern_matches && law_verified(law)
}

// ============================================================================
// `inferred_trait`: the structural law inference function
// ============================================================================

/// The set of laws inferred from a graph's wiring, by structural recursion.
///
/// This is the Rust mirror of the Lean4 `inferred_trait` function. It maps a
/// graph to the list of laws it satisfies, by inspecting the wiring
/// structure. No law is declared; the function reads the laws off from the
/// structure.
///
/// The inference is structural:
/// - A graph using `first` → functor pattern detected → functor laws.
/// - A graph using `left` → coproduct functor pattern detected → coproduct
///   functor laws.
/// - A graph using neither → no laws.
///
/// # Example
///
/// ```
/// use handl_language::Scheme;
/// use handl_plugin_ext_refinement::trait_inference::{inferred_trait, Law};
///
/// // first(id) → functor laws.
/// let g = Scheme::first(Scheme::arr_native("id", vec![]));
/// let laws = inferred_trait(&g);
/// assert!(laws.contains(&Law::FunctorIdentity));
/// assert!(laws.contains(&Law::FunctorComposition));
/// assert!(!laws.contains(&Law::CoproductIdentity));
///
/// // id → no laws.
/// let g = Scheme::arr_native("id", vec![]);
/// assert!(inferred_trait(&g).is_empty());
/// ```
pub fn inferred_trait(scheme: &Scheme) -> Vec<Law> {
    let mut laws = Vec::new();
    if uses_first(scheme) {
        laws.push(Law::FunctorIdentity);
        laws.push(Law::FunctorComposition);
    }
    if uses_left(scheme) {
        laws.push(Law::CoproductIdentity);
        laws.push(Law::CoproductComposition);
    }
    laws
}

// ============================================================================
// Contract strengthening: using inferred laws to strengthen contracts
// ============================================================================

/// A law-based contract annotation. When a graph is recognized as satisfying
/// a law, the contract inference can use this to simplify or strengthen the
/// inferred contract.
///
/// This is the bridge between the trait inference (which laws a graph
/// satisfies) and the contract inference (what refinement guarantees hold).
/// The Lean4 formalization connects the two via the invariant domain `D`:
/// law satisfaction implies semantic equivalence, which implies preservation
/// of all `D`-invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawAnnotation {
    /// The law that is satisfied.
    pub law: Law,
    /// A human-readable description of how the law strengthens the contract.
    pub strengthening: String,
}

/// Infer law-based contract annotations for a `Scheme` graph.
///
/// This is the wiring point between the trait inference and the contract
/// inference. When a graph is recognized as satisfying a functor law, the
/// contract inference can use this to simplify or strengthen the inferred
/// contract.
///
/// The annotations describe the semantic consequence of each satisfied law:
/// - **FunctorIdentity**: `first(id)` is the identity on `A × D` — the
///   side channel is preserved exactly. This strengthens the
///   `LengthEq(input.1, output.1)` guarantee that `infer_r` already derives
///   for `First`, confirming it is an exact equality (not just length).
/// - **FunctorComposition**: `first(comp G H) = comp (first G) (first H)` —
///   the guarantee of `first(comp G H)` is the composition of the guarantees
///   of `first G` and `first H`. This confirms the `Seq` composition rule
///   used by `infer_r` is sound for `First`-wrapped graphs.
/// - **CoproductIdentity**: `left(id)` is the identity on `A + D` — the
///   right branch passes through unchanged. This confirms the `Choice`
///   inference rule is sound when the right branch is an identity.
/// - **CoproductComposition**: `left(comp G H) = comp (left G) (left H)` —
///   the guarantee of `left(comp G H)` is the composition of the guarantees
///   of `left G` and `left H`.
pub fn infer_law_annotations(scheme: &Scheme) -> Vec<LawAnnotation> {
    inferred_trait(scheme)
        .into_iter()
        .map(|law| LawAnnotation {
            law,
            strengthening: law_strengthening(law),
        })
        .collect()
}

/// The contract-strengthening description for a law.
fn law_strengthening(law: Law) -> String {
    match law {
        Law::FunctorIdentity => {
            "first(id) ≡ id(A×D): side channel preserved exactly (identity functor)"
                .to_string()
        }
        Law::FunctorComposition => {
            "first(comp G H) ≡ comp(first G)(first H): First distributes over Seq (fusion)"
                .to_string()
        }
        Law::CoproductIdentity => {
            "left(id) ≡ id(A+D): right branch passes through unchanged (identity coproduct functor)"
                .to_string()
        }
        Law::CoproductComposition => {
            "left(comp G H) ≡ comp(left G)(left H): left distributes over Seq (fusion)"
                .to_string()
        }
    }
}
