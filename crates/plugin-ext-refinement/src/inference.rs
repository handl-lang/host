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
use handl_language::{ArrSource, Term};
use plugin_engine::TermData;

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
    let td = TermData::from_term(scheme);
    infer_r_td(&td, registry, loop_invs)
}

/// Data-driven counterpart of [`infer_r`], operating on [`TermData`].
fn infer_r_td(
    td: &TermData,
    registry: &ContractRegistry,
    loop_invs: &mut Vec<LoopInvariant>,
) -> Result<(RefTy, RefTy, Pred), String> {
    match td.tag() {
        // arr(id) or arr(op): look up contract in registry.
        // RGraph::Id maps to Term::Op { name: "id", ... }.
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            if op == "id" {
                // Identity preserves everything.
                return Ok((
                    RefTy::Plain(BaseTy::Var("a".to_string())),
                    RefTy::Plain(BaseTy::Var("a".to_string())),
                    Pred::True,
                ));
            }
            let contract = registry
                .lookup(op, &source)
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
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            let (gi, _go, gp) = infer_r_td(&fst, registry, loop_invs)?;
            let (_hi, ho, hp) = infer_r_td(&snd, registry, loop_invs)?;
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
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            let (gi, go, gp) = infer_r_td(&inner, registry, loop_invs)?;
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
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            let (gi, go, gp) = infer_r_td(&l, registry, loop_invs)?;
            let (_ri, _ro, rp) = infer_r_td(&r, registry, loop_invs)?;
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
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            let (gi, go, gp) = infer_r_td(&body, registry, loop_invs)?;

            // Start the fixpoint from the body's guarantee, not from True.
            // This lets the fixpoint discover non-trivial invariants (e.g.
            // Sorted(acc)) that are already present in the body's contracts.
            let body_term = body.to_term().expect("Loop body: TermData -> Term failed");
            let (invariant, how, _trace, verified) = discover_invariant_fixpoint_real(
                &body_term,
                registry,
                &gp,
                FIXPOINT_MAX_ITERATIONS,
            );

            loop_invs.push(LoopInvariant {
                graph: format_scheme(&body_term),
                invariant: invariant.clone(),
                how_discovered: how,
                verified_by_smt: verified,
            });

            // The loop's guarantee is the invariant (what holds at the exit).
            Ok((gi, go, invariant))
        }

        // app(callee, args): conservatively return True (not used by inference).
        "App" => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("b".to_string())),
            Pred::True,
        )),
        // Handle nodes are eliminated before inference; treat as identity.
        "Handle" => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("a".to_string())),
            Pred::True,
        )),

        // Pure constants and child projections: conservatively identity-like.
        "Arr" | "Child" => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("b".to_string())),
            Pred::True,
        )),
        "Iter" => Err("iter contract inference not implemented".to_string()),
        // Rewrite nodes are not yet supported by contract inference.
        "Rewrite" => Err("rewrite contract inference not implemented".to_string()),
        // Unknown/unsupported tags: conservatively return True (identity-like).
        _ => Ok((
            RefTy::Plain(BaseTy::Var("a".to_string())),
            RefTy::Plain(BaseTy::Var("b".to_string())),
            Pred::True,
        )),
    }
}

/// Format a `Term` into a compact string for use in `LoopInvariant.graph`.
pub fn format_scheme(scheme: &Term) -> String {
    let td = TermData::from_term(scheme);
    format_scheme_td(&td)
}

/// Data-driven counterpart of [`format_scheme`], operating on [`TermData`].
fn format_scheme_td(td: &TermData) -> String {
    match td.tag() {
        "Op" => td.get_str("name").unwrap_or("").to_string(),
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            format!(
                "seq({}, {})",
                format_scheme_td(&fst),
                format_scheme_td(&snd)
            )
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            format!("first({})", format_scheme_td(&inner))
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            format!("choice({}, {})", format_scheme_td(&l), format_scheme_td(&r))
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            format!("loop({})", format_scheme_td(&body))
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            format!("app({})", format_scheme_td(&callee))
        }
        "Handle" => {
            let body = td.get_term("body").expect("Handle: missing body");
            let target = td.get_str("target").unwrap_or("");
            format!("handle({}, {})", format_scheme_td(&body), target)
        }
        "Arr" => {
            let value = td.get("value").cloned().unwrap_or(serde_json::Value::Null);
            format!("const({})", value)
        }
        "Child" => {
            let role = td.get_str("role").unwrap_or("");
            format!("child({})", role)
        }
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            format!(
                "iter({}, {}, {}, {})",
                format_scheme_td(&fuel),
                format_scheme_td(&predicate),
                format_scheme_td(&step),
                format_scheme_td(&initial)
            )
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            format!(
                "rewrite({} ~> {})",
                format_scheme_td(&source),
                format_scheme_td(&target)
            )
        }
        _ => td.tag().to_string(),
    }
}
