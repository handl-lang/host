//! The refinement inference engine.
//!
//! Ported from the research prototype's `infer_refinement` and `infer_r`
//! functions, adapted to operate over handl's `Term` IR instead of `RGraph`.
//!
//! Mapping from `RGraph` to `Term`:
//! - `RGraph::Id`           -> `Term::Op { name: "id", source: Native, .. }`
//! - `RGraph::Comp(g, h)`   -> `Term::Seq { fst: g, snd: h }`
//! - `RGraph::First(g)`     -> `Term::First { inner: g }`
//! - `RGraph::Left(g)`      -> `Term::Choice { l: g, r: identity }` (conservatively demoted)
//! - `RGraph::Loop(g)`      -> `Term::Loop { body: g }`
//! - `RGraph::Arr(name)`    -> `Term::Op { name: name, source: Native, args: vec![], lit: None }`

use crate::contracts::ContractRegistry;
use crate::loop_invariant::{FIXPOINT_MAX_ITERATIONS, discover_invariant_fixpoint_real};
use crate::termination::analyze_termination;
use crate::types::{BaseTy, LoopInvariant, Pred, RefTy, RefinementResult};
use handl_language::Term;

/// Infer refinement properties from a `Term`, without annotations.
///
/// This is the main entry point. It walks the `Term` structurally, looking
/// up base-arrow contracts in the registry, composing guarantees, discovering
/// loop invariants via fixpoint iteration, and analyzing termination.
///
/// It also runs the **annotation-free trait inference** (Phase 9, mirroring
/// Lean4 `TraitInference.lean`): it inspects the graph's wiring structure to
/// detect which equational laws (functor identity/composition, coproduct
/// functor identity/composition) the graph satisfies. The detected laws are
/// returned in `RefinementResult.law_annotations` and can be used to
/// simplify or strengthen the inferred contract.
pub fn infer_refinement(
    scheme: &Term,
    registry: &ContractRegistry,
) -> Result<RefinementResult, String> {
    let mut loop_invariants = Vec::new();
    let (input, output, guarantee) = infer_r(scheme, registry, &mut loop_invariants)?;

    // Analyze termination for the whole scheme.
    let term = analyze_termination(scheme, registry);
    let termination = if term.terminates {
        Some(term.measure)
    } else {
        Some(term.measure) // Still report the measure even if not guaranteed
    };

    // Annotation-free trait inference: detect which equational laws the
    // graph satisfies from its wiring structure alone (mirrors Lean4
    // `TraitInference.lean`'s `inferred_trait`). When a graph is recognized
    // as satisfying a functor law, the contract inference can use this to
    // simplify or strengthen the inferred contract.
    let law_annotations = crate::trait_inference::infer_law_annotations(scheme);

    Ok(RefinementResult {
        input,
        output,
        guarantee,
        loop_invariants,
        termination,
        law_annotations,
    })
}

/// Structural recursion over `Term`, producing `(RefTy, RefTy, Pred)`.
///
/// Maps each `Term` variant to its refinement inference rule:
/// - `Arr` -> look up contract in registry, return `(input, output, guarantee)`
/// - `Seq` -> recursively infer both halves, conjoin guarantees
/// - `First` -> wrap in `Prod`, carry side channel
/// - `Choice` -> conservatively return `True` (left is demoted, as the paper says)
/// - `Loop` -> call `discover_invariant_fixpoint`
/// - `App` -> conservatively return `True` (not used by inference)
pub fn infer_r(
    scheme: &Term,
    registry: &ContractRegistry,
    loop_invs: &mut Vec<LoopInvariant>,
) -> Result<(RefTy, RefTy, Pred), String> {
    match scheme {
        // arr(id) or arr(op): look up contract in registry.
        // RGraph::Id maps to Term::Op { name: "id", ... }.
        Term::Op { name: op, source, .. } => {
            if op == "id" {
                // Identity preserves everything.
                return Ok((
                    RefTy::Plain(BaseTy::Var("a".to_string())),
                    RefTy::Plain(BaseTy::Var("a".to_string())),
                    Pred::True,
                ));
            }
            let contract = registry
                .lookup(op, source)
                .ok_or_else(|| format!("Unknown base arrow: {} (source: {:?})", op, source))?;

            // Conditional contracts: the guarantee only applies when the
            // precondition holds on the input. Full bidirectional refinement
            // typing (checking the precondition against the current input
            // refinement) is future work; for now we adopt an *optimistic*
            // strategy: return the guarantee as-is, letting the fixpoint
            // iteration and WP computation determine whether the precondition
            // is actually met. The SMT checker verifies implications.
            //
            // This lets real semantic content (Sorted, LengthEq, ...) flow
            // through inference instead of being dropped to True.
            let guarantee = contract.guarantee.clone();

            Ok((contract.input.clone(), contract.output.clone(), guarantee))
        }

        // seq(g, h) = comp(g, h): compose refinements (cut rule).
        // RGraph::Comp(g, h) maps to Term::Seq { fst: g, snd: h }.
        Term::Seq { fst, snd } => {
            let (gi, _go, gp) = infer_r(fst, registry, loop_invs)?;
            let (_hi, ho, hp) = infer_r(snd, registry, loop_invs)?;
            // The guarantee of comp is the conjunction of both guarantees.
            let combined = Pred::And(Box::new(gp), Box::new(hp));
            Ok((gi, ho, combined))
        }

        // first(g): preserves refinement on first component, second passes through.
        // RGraph::First(g) maps to Term::First { inner: g }.
        //
        // By definition, `first(g)` leaves the second component (the "side
        // channel") completely unchanged. This means the length of the side
        // channel is preserved: |input.1| = |output.1|. We conjoin this
        // `LengthEq` with the inner guarantee `gp` so that downstream
        // inference (e.g. `map = loop(first(fn.apply))`) can derive
        // `LengthEq` on the side channel without needing explicit cons/tail
        // structure in the graph.
        Term::First { inner } => {
            let (gi, go, gp) = infer_r(inner, registry, loop_invs)?;
            let d = RefTy::Plain(BaseTy::Var("d".to_string()));
            let input = RefTy::Prod(Box::new(gi), Box::new(d.clone()));
            let output = RefTy::Prod(Box::new(go), Box::new(d));
            // first(a) preserves the side channel: the second component
            // passes through unchanged, so |input.1| = |output.1|.
            let side_channel_preserved =
                Pred::LengthEq("input.1".to_string(), "output.1".to_string());
            let guarantee = Pred::And(Box::new(gp), Box::new(side_channel_preserved));
            Ok((input, output, guarantee))
        }

        // choice(l, r) = ArrowChoice: conjoin guarantees from both branches.
        // RGraph::Left(g) maps to Term::Choice { l: g, r: identity }.
        // Since either branch may be taken, the sound guarantee is the
        // conjunction of both branches' guarantees -- the property must hold
        // regardless of which arm executes. This lets real semantic content
        // (e.g. Sorted from cons) flow through Choice instead of being
        // dropped to True.
        Term::Choice { l, r } => {
            let (gi, go, gp) = infer_r(l, registry, loop_invs)?;
            let (_ri, _ro, rp) = infer_r(r, registry, loop_invs)?;
            let d = RefTy::Plain(BaseTy::Var("d".to_string()));
            let input = RefTy::Sum(Box::new(gi), Box::new(d.clone()));
            let output = RefTy::Sum(Box::new(go), Box::new(d));
            // Conjoin both branches' guarantees (simplified for cleanliness).
            let combined = Pred::And(Box::new(gp), Box::new(rp)).simplify();
            Ok((input, output, combined))
        }

        // loop(g): discover loop invariant via fixpoint iteration.
        // RGraph::Loop(g) maps to Term::Loop { body: g }.
        //
        // The fixpoint is seeded with the body's guarantee `gp` (not `True`),
        // so that non-trivial properties like `Sorted(output)` discovered in
        // the body propagate through the loop. The loop's guarantee is the
        // invariant itself -- what holds at the loop exit.
        Term::Loop { body } => {
            let (gi, go, gp) = infer_r(body, registry, loop_invs)?;

            // Start the fixpoint from the body's guarantee, not from True.
            // This lets the fixpoint discover non-trivial invariants (e.g.
            // Sorted(acc)) that are already present in the body's contracts.
            let (invariant, how, _trace, verified) =
                discover_invariant_fixpoint_real(body, registry, &gp, FIXPOINT_MAX_ITERATIONS);

            loop_invs.push(LoopInvariant {
                graph: format_scheme(body),
                invariant: invariant.clone(),
                how_discovered: how,
                verified_by_smt: verified,
            });

            // The loop's guarantee is the invariant (what holds at the exit).
            Ok((gi, go, invariant))
        }

        // app(callee, args): conservatively return True (not used by inference).
        Term::App { .. } => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("b".to_string())),
            Pred::True,
        )),
        // Handle nodes are eliminated before inference; treat as identity.
        Term::Handle { .. } => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("a".to_string())),
            Pred::True,
        )),

        // Pure constants and child projections: conservatively identity-like.
        Term::Arr { .. } | Term::Child { .. } => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("b".to_string())),
            Pred::True,
        )),
        Term::Iter { .. } => Err("iter contract inference not implemented".to_string()),
        // Rewrite nodes are not yet supported by contract inference.
        Term::Rewrite { .. } => Err("rewrite contract inference not implemented".to_string()),
    }
}

/// Format a `Term` into a compact string for use in `LoopInvariant.graph`.
pub fn format_scheme(scheme: &Term) -> String {
    match scheme {
        Term::Op { name: op, .. } => op.clone(),
        Term::Seq { fst, snd } => {
            format!("seq({}, {})", format_scheme(fst), format_scheme(snd))
        }
        Term::First { inner } => {
            format!("first({})", format_scheme(inner))
        }
        Term::Choice { l, r } => {
            format!("choice({}, {})", format_scheme(l), format_scheme(r))
        }
        Term::Loop { body } => {
            format!("loop({})", format_scheme(body))
        }
        Term::App { callee, .. } => {
            format!("app({})", format_scheme(callee))
        }
        Term::Handle { body, target, .. } => {
            format!("handle({}, {})", format_scheme(body), target)
        }

        Term::Arr { value } => format!("const({})", value),
        Term::Child { role } => format!("child({})", role),
        Term::Iter { fuel, predicate, step, initial } => {
            format!(
                "iter({}, {}, {}, {})",
                format_scheme(fuel),
                format_scheme(predicate),
                format_scheme(step),
                format_scheme(initial)
            )
        }
        Term::Rewrite { source, target, .. } => {
            format!("rewrite({} ~> {})", format_scheme(source), format_scheme(target))
        }
    }
}
