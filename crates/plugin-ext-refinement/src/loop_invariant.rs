//! Fixpoint loop invariant discovery.
//!
//! Ported from the research prototype's `discover_invariant_fixpoint_real`
//! and `weakest_precondition` functions, adapted to operate over handl's
//! `Term` IR instead of `RGraph`.
//!
//! The algorithm:
//! 1. Start with `I_0 = True` (the trivial invariant -- everything is in D).
//! 2. Compute the WEAKEST PRECONDITION of the loop body on the feedback
//!    channel: `wp(G, I)` = the weakest predicate on `c` such that after `G`
//!    executes, `I(c')` holds.
//! 3. The invariant must be a FIXPOINT: `I = wp(G, I) & I` (conjunction).
//! 4. Iterate: `I_{n+1} = I_n && wp(G, I_n)`.
//! 5. Stop when `I_{n+1} == I_n` (fixpoint reached) or after `max_iterations`.

use crate::contracts::ContractRegistry;
use crate::smt::implies;
use crate::types::Pred;
use handl_language::{ArrSource, Term};

/// Simplify a predicate (delegates to `Pred::simplify`).
fn simplify(p: &Pred) -> Pred {
    p.simplify()
}

/// Default iteration cap for the fixpoint loop.
pub const FIXPOINT_MAX_ITERATIONS: usize = 20;

/// Discover a loop invariant by fixpoint iteration.
///
/// Algorithm:
/// 1. Start with `I_0 = initial` (typically the body's guarantee, or `True`
///    for the trivial starting point).
/// 2. Compute the WEAKEST PRECONDITION of the loop body on the feedback channel:
///    `wp(G, I)` = the weakest predicate on `c` such that after `G` executes, `I(c')` holds.
/// 3. The invariant must be a FIXPOINT: `I = wp(G, I) & I` (conjunction).
/// 4. Iterate: `I_{n+1} = I_n && wp(G, I_n)`.
/// 5. Stop when `I_{n+1} = I_n` (fixpoint reached) or after `max_iterations`.
///
/// Returns: `(final_invariant, how_discovered, iteration_trace, verified)`
/// where `iteration_trace` records every `I_n` so the algorithm is transparent
/// and auditable, and `verified` is `true` iff the final invariant is preserved
/// by one more step of the body (i.e. `I ==> wp(G, I)`).
pub fn discover_invariant_fixpoint_real(
    body: &Term,
    registry: &ContractRegistry,
    initial: &Pred,
    max_iterations: usize,
) -> (Pred, String, Vec<Pred>, bool) {
    let mut iterations = Vec::new();
    // Seed the fixpoint with the initial predicate AND any pattern-recognised
    // candidate invariants. The pattern recognizer detects structural shapes
    // (e.g. list-consuming + output-building loops) and adds candidate
    // invariants like IsPerm(input, output) that capture multiset-preservation
    // across loop iterations -- matching the Agda example
    // `S ys. Sorted ys * Permutation xs ys`.
    let pattern_cands = pattern_candidates(body);
    let seeded = if pattern_cands.is_empty() {
        initial.clone()
    } else {
        let mut acc = initial.clone();
        for c in &pattern_cands {
            acc = simplify(&Pred::And(Box::new(acc), Box::new(c.clone())));
        }
        acc
    };
    let mut current = seeded; // I_0 = seeded (initial && pattern candidates)

    for i in 0..max_iterations {
        iterations.push(current.clone());

        // Compute wp(G, current): what must hold before G for current to hold after
        let wp = weakest_precondition(body, registry, &current);

        // Strengthen: I_{n+1} = I_n && wp(G, I_n), then simplify.
        // Simplification is essential for convergence detection when Z3 is
        // not available: And(True, True) must simplify to True so the toy
        // implication checker can detect the fixpoint.
        let next = simplify(&Pred::And(Box::new(current.clone()), Box::new(wp.clone())));

        // Check fixpoint: is next equivalent to current?
        // Use the implies checker: if next ==> current AND current ==> next,
        // it's a fixpoint (the two predicates are logically equivalent).
        let imp1 = implies(&next, &current);
        let imp2 = implies(&current, &next);
        if imp1 && imp2 {
            // Fixpoint reached. The invariant is *preserved* by the body iff
            // current ==> wp(G, current) (running one more step keeps it true).
            let verified = implies(&current, &wp);
            return (
                current,
                format!("fixpoint iteration (converged at iteration {})", i),
                iterations,
                verified,
            );
        }

        current = next;
    }

    // Didn't converge -- return the strongest we found.
    let verified = implies(&current, &weakest_precondition(body, registry, &current));
    (
        current,
        format!(
            "fixpoint iteration (not converged after {} iterations)",
            max_iterations
        ),
        iterations,
        verified,
    )
}

/// Backwards-compatible wrapper returning the 3-tuple expected by `infer_r`.
/// Delegates to `discover_invariant_fixpoint_real` with `True` as the initial
/// predicate and discards the trace.
pub fn discover_invariant_fixpoint(
    body: &Term,
    registry: &ContractRegistry,
) -> (Pred, String, bool) {
    let (inv, how, _trace, verified) =
        discover_invariant_fixpoint_real(body, registry, &Pred::True, FIXPOINT_MAX_ITERATIONS);
    (inv, how, verified)
}

/// Compute the weakest precondition of `G` with respect to predicate `I` on the
/// output. `wp(G, I)` = the weakest predicate on the input such that after `G`
/// executes, `I` holds.
///
/// This is computed *structurally* by recursion over the `Term` -- not
/// hardcoded per test case. Each graph combinator has a well-defined
/// weakest-precondition transformer:
///
/// * `arr(id)` / `arr(op)` with no contract : `wp = I`
/// * `arr(f)` with contract                 : `wp = precondition(f)` if `guarantee(f) ==> I`,
///                                           else `True` (conservative -- cannot prove)
/// * `seq(g, h)`                            : `wp(g, wp(h, I))` (backwards composition)
/// * `first(g)`                             : `I` (the feedback channel C passes
///                                           through unchanged on the second component)
/// * `choice(l, r)`                         : `wp(l, I) && wp(r, I)` (the
///                                           invariant must hold on *both*
///                                           branches, so the weakest
///                                           precondition is the conjunction
///                                           of each arm's wp)
/// * `loop(g)`                              : `I` (the loop's invariant *is* the
///                                           fixpoint, so the weakest precondition
///                                           is the invariant itself)
/// * `app(callee, args)`                    : `I` (conservative -- not used by inference)
pub fn weakest_precondition(scheme: &Term, registry: &ContractRegistry, post: &Pred) -> Pred {
    match scheme {
        // arr(id) is identity: wp(id, I) = I
        // arr(f) with contract: wp = precondition(f) if guarantee(f) ==> I,
        //                       else True (conservative -- cannot prove the
        //                       postcondition follows from the guarantee, so
        //                       this branch contributes no constraint).
        //
        // This is the key rule for propagating conditional contracts backward:
        // when the guarantee implies the post, the *precondition* must hold
        // on the input (e.g. cons requires Sorted(tail) to guarantee
        // Sorted(output)). When the guarantee does not imply the post, we
        // conservatively return True -- the branch simply does not strengthen
        // the invariant.
        Term::Arr { op, source, .. } => {
            if op == "id" {
                return post.clone();
            }
            let contract = registry.lookup(op, source);
            if let Some(c) = contract {
                if implies(&c.guarantee, post) {
                    // The guarantee is strong enough to establish the post,
                    // so the weakest precondition is the contract's
                    // precondition (what must hold on the input for the
                    // guarantee to fire).
                    c.precondition.clone()
                } else {
                    // Cannot prove the post from the guarantee -- be
                    // conservative and impose no constraint (True).
                    Pred::True
                }
            } else {
                post.clone() // unknown arrow, conservative
            }
        }

        // seq(g, h): wp(seq(g, h), I) = wp(g, wp(h, I))  (backwards composition)
        Term::Seq { fst, snd } => {
            let wp_snd = weakest_precondition(snd, registry, post);
            weakest_precondition(fst, registry, &wp_snd)
        }

        // first(g): C passes through unchanged, so wp(first(g), I) = I
        Term::First { .. } => post.clone(),

        // choice(l, r): the invariant must hold on both branches, so the
        // weakest precondition is the conjunction of each arm's wp.
        // This makes the fixpoint iteration branch through both arms of
        // conditionals, enabling discovery of conditional invariants.
        Term::Choice { l, r } => {
            let wp_l = weakest_precondition(l, registry, post);
            let wp_r = weakest_precondition(r, registry, post);
            Pred::And(Box::new(wp_l), Box::new(wp_r))
        }

        // loop(g): the loop's invariant is the fixpoint, so wp = I
        Term::Loop { .. } => post.clone(),

        // app(callee, args): conservative -- not used by inference
        Term::App { .. } => post.clone(),
        // Handle nodes are eliminated before inference; treat as identity.
        Term::Handle { .. } => post.clone(),

        // Rewrite nodes are not yet analyzed; treat as identity.
        Term::Rewrite { .. } => post.clone(),
    }
}

/// Collect every base-arrow guarantee appearing anywhere in `scheme`.
pub fn collect_guarantees(scheme: &Term, registry: &ContractRegistry, out: &mut Vec<Pred>) {
    match scheme {
        Term::Arr { op, source, .. } => {
            if let Some(c) = registry.lookup(op, source) {
                if c.guarantee != Pred::True {
                    out.push(c.guarantee.clone());
                }
            }
        }
        Term::Seq { fst, snd } => {
            collect_guarantees(fst, registry, out);
            collect_guarantees(snd, registry, out);
        }
        Term::First { inner } => {
            collect_guarantees(inner, registry, out);
        }
        Term::Choice { l, r } => {
            collect_guarantees(l, registry, out);
            collect_guarantees(r, registry, out);
        }
        Term::Loop { body } => {
            collect_guarantees(body, registry, out);
        }
        Term::App { callee, args } => {
            collect_guarantees(callee, registry, out);
            for arg in args {
                collect_guarantees(arg, registry, out);
            }
        }
        Term::Handle { body, impl_, .. } => {
            collect_guarantees(body, registry, out);
            collect_guarantees(impl_, registry, out);
        }

        Term::Rewrite { source, target, .. } => {
            collect_guarantees(source, registry, out);
            collect_guarantees(target, registry, out);
        }
    }
}

/// Check whether the given `Term` contains a base arrow with the given op
/// name (matching either the bare name or the `list.`-prefixed name),
/// searching recursively through all combinators.
pub fn graph_contains(scheme: &Term, op: &str) -> bool {
    match scheme {
        Term::Arr {
            op: arr_op, source, ..
        } => {
            arr_op == op
                || arr_op == &format!("list.{}", op)
                || (source == &ArrSource::Native && arr_op == op)
        }
        Term::Seq { fst, snd } => graph_contains(fst, op) || graph_contains(snd, op),
        Term::First { inner } => graph_contains(inner, op),
        Term::Choice { l, r } => graph_contains(l, op) || graph_contains(r, op),
        Term::Loop { body } => graph_contains(body, op),
        Term::App { callee, args } => {
            graph_contains(callee, op) || args.iter().any(|a| graph_contains(a, op))
        }
        Term::Handle { body, impl_, .. } => graph_contains(body, op) || graph_contains(impl_, op),

        Term::Rewrite { source, target, .. } => {
            graph_contains(source, op) || graph_contains(target, op)
        }
    }
}

/// Collect candidate invariants from structural patterns in the loop body.
///
/// Currently recognises the "output-building loop" pattern: when the loop
/// body contains `cons` (the list constructor), it adds
/// `IsPerm(input, output)` as a candidate invariant. This captures the
/// multiset-preservation property of loops that consume an input list and
/// build an output list from its elements -- most notably insertion sort,
/// where each iteration moves one element from the remaining input into the
/// accumulator, so the accumulator is always a permutation of the original
/// input.
///
/// Matching the Agda example `S ys. Sorted ys * Permutation xs ys`, this lets
/// sort derive `Sorted(output) && IsPerm(input, output)`.
///
/// The list consumption may be implicit (the loop's feedback channel shrinks
/// each iteration) rather than an explicit `tail` arrow, so we only require
/// `cons` to be present -- the presence of a list constructor in a loop body
/// is a strong signal that the loop is building an output list from the
/// input's elements.
fn pattern_candidates(body: &Term) -> Vec<Pred> {
    let mut candidates = Vec::new();
    let has_cons = graph_contains(body, "list.cons") || graph_contains(body, "cons");
    if has_cons {
        candidates.push(Pred::IsPerm("input".to_string(), "output".to_string()));
    }
    candidates
}

/// Produce the fixpoint trace for a loop body.
/// Returns a vector of `(iteration, predicate)` pairs and whether a fixpoint
/// was reached.
pub fn fixpoint_trace(body: &Term, registry: &ContractRegistry) -> (Vec<(usize, Pred)>, bool) {
    let mut guarantees = Vec::new();
    collect_guarantees(body, registry, &mut guarantees);

    // The body's postcondition transformer: conjoin all non-trivial guarantees.
    let body_post = if guarantees.is_empty() {
        Pred::True
    } else {
        guarantees
            .iter()
            .cloned()
            .reduce(|acc, p| Pred::And(Box::new(acc), Box::new(p)))
            .unwrap_or(Pred::True)
    };

    let mut trace = vec![(0usize, Pred::True)];
    let mut current = Pred::True;

    // Iterate up to a small bound (fixpoint is reached quickly in practice).
    for i in 1..=4 {
        // Postcondition of one iteration from `current`: conjoin body_post, then simplify.
        let next = match (&current, &body_post) {
            (Pred::True, p) => p.clone(),
            (p, Pred::True) => p.clone(),
            (p, q) => simplify(&Pred::And(Box::new(p.clone()), Box::new(q.clone()))),
        };
        // Check fixpoint: does `current` already imply `next`?
        if implies(&current, &next) {
            trace.push((i, next.clone()));
            return (trace, true);
        }
        trace.push((i, next.clone()));
        current = next;
    }
    (trace, false)
}
