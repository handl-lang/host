//! Termination analysis for `Term` loops.
//!
//! Ported from the research prototype's `analyze_termination`,
//! `analyze_loop_termination`, and `analyze_comp_loop_termination` functions,
//! adapted to operate over handl's `Term` IR.
//!
//! The `Loop` combinator `loop G : A ~> B` where `G : A*C ~> B*C` repeatedly
//! applies `G`, threading `C` through as feedback. Termination requires that
//! `C` has a well-founded measure that decreases each iteration.
//!
//! Cases handled:
//! 1. Side-channel loop (`loop(first(G))`) -- C unchanged -> NOT GUARANTEED
//! 2. List-consuming loop (`loop(seq(process, consume_one))`) -- |C| decreases -> GUARANTEED
//! 3. Counter loop (`loop(seq(step, decrement))`) -- C decreases -> GUARANTEED
//! 4. Fixed-point loop (`loop(seq(f, id))`) -- no decreasing measure -> NOT GUARANTEED
//! 5. Nested loop (`loop(seq(loop(inner), first(id)))`) -- depends on inner
//! 6. Nested loop with non-terminating inner -- NOT GUARANTEED
//! 7. Map (no loop) -- direct termination
//! 8. Sort via loop (`loop(seq(insert, remove_one))`) -- |C| decreases -> GUARANTEED

use crate::contracts::ContractRegistry;
use crate::types::Measure;
use handl_language::{ArrSource, Term};
use plugin_engine::TermData;

/// Termination analysis result for a `Term`.
#[derive(Debug, Clone)]
pub struct TerminationResult {
    /// Whether the scheme is guaranteed to terminate.
    pub terminates: bool,
    /// The discovered decreasing measure (if any).
    pub measure: Measure,
    /// Human-readable reason for the verdict.
    pub reason: String,
}

/// Analyze termination of a `Term` by discovering a decreasing measure on
/// any feedback channel `C` introduced by a `Loop` combinator.
///
/// For schemes with no loop, termination is direct (the scheme is a finite
/// composition of base arrows). For `loop G`, we structurally analyze `G` to
/// discover whether `C` has a well-founded decreasing measure.
pub fn analyze_termination(scheme: &Term, registry: &ContractRegistry) -> TerminationResult {
    let td = TermData::from_term(scheme);
    analyze_termination_td(&td, registry)
}

/// Data-driven counterpart of [`analyze_termination`], operating on
/// [`TermData`].
fn analyze_termination_td(td: &TermData, registry: &ContractRegistry) -> TerminationResult {
    match td.tag() {
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            analyze_loop_termination_td(&body, registry)
        }

        "Seq" => {
            // Composition: both parts must terminate.
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            let tg = analyze_termination_td(&fst, registry);
            let th = analyze_termination_td(&snd, registry);
            let terminates = tg.terminates && th.terminates;
            let measure = if tg.measure != Measure::None {
                tg.measure
            } else if th.measure != Measure::None {
                th.measure
            } else {
                Measure::None
            };
            let reason = if terminates {
                format!("composition terminates ({} && {})", tg.reason, th.reason)
            } else {
                format!(
                    "composition may not terminate ({} || {})",
                    tg.reason, th.reason
                )
            };
            TerminationResult {
                terminates,
                measure,
                reason,
            }
        }

        "First" => {
            // first(G): G terminates, the second component passes through.
            let inner = td.get_term("inner").expect("First: missing inner");
            let ti = analyze_termination_td(&inner, registry);
            TerminationResult {
                terminates: ti.terminates,
                measure: Measure::None,
                reason: format!("first -- {}", ti.reason),
            }
        }

        "Choice" => {
            // choice(l, r): both branches must terminate.
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            let tl = analyze_termination_td(&l, registry);
            let tr = analyze_termination_td(&r, registry);
            TerminationResult {
                terminates: tl.terminates && tr.terminates,
                measure: Measure::None,
                reason: format!("choice -- {} && {}", tl.reason, tr.reason),
            }
        }

        // Base arrows and identity: no loop, direct termination.
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            let known = registry.contains(op, &source);
            TerminationResult {
                terminates: true,
                measure: Measure::None,
                reason: if known {
                    format!("base arrow {} -- direct termination (no loop)", op)
                } else {
                    format!("base arrow {} (unknown) -- assumed direct termination", op)
                },
            }
        }

        "App" => TerminationResult {
            terminates: true,
            measure: Measure::None,
            reason: "app -- direct termination (no loop)".to_string(),
        },

        // Handle nodes are eliminated before analysis; treat as terminating.
        "Handle" => TerminationResult {
            terminates: true,
            measure: Measure::None,
            reason: "handle -- eliminated before analysis".to_string(),
        },

        // Pure constants and child projections: direct (no loop).
        "Arr" | "Child" => TerminationResult {
            terminates: true,
            measure: Measure::None,
            reason: "constant/child -- direct termination (no loop)".to_string(),
        },
        "Iter" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "iter -- termination not analyzed".to_string(),
        },
        // Rewrite nodes are not yet analyzed; treat as direct termination.
        "Rewrite" => TerminationResult {
            terminates: true,
            measure: Measure::None,
            reason: "rewrite -- direct termination (no loop)".to_string(),
        },
        _ => unreachable!(),
    }
}

/// Analyze termination of a `loop G` by discovering a decreasing measure on
/// the feedback channel `C`. The body `G` has type `A*C ~> B*C`; we examine
/// `G`'s structure to find what it does to `C`.
fn analyze_loop_termination_td(td: &TermData, registry: &ContractRegistry) -> TerminationResult {
    match td.tag() {
        // --- Case 1: Side-channel loop ---
        // loop(first(G)): G only touches A, C passes through unchanged.
        // Measure = constant -> does not decrease -> NOT GUARANTEED.
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            let inner_term = analyze_termination_td(&inner, registry);
            TerminationResult {
                terminates: false, // C unchanged -> no decreasing measure
                measure: Measure::Constant,
                reason: format!(
                    "side-channel: C unchanged by first, no decreasing measure on C{}",
                    if inner_term.terminates {
                        ""
                    } else {
                        " (inner also may not terminate)"
                    }
                ),
            }
        }

        // --- Case 2/3/4/5: Composition body ---
        // loop(seq(left, right)): `right` is what updates C. We inspect right
        // to discover the measure on C.
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            analyze_comp_loop_termination_td(&fst, &snd, registry)
        }

        // --- Case 5: Nested loop ---
        // loop(loop(inner)): the body is itself a loop. Termination of the
        // outer loop depends on termination of the inner loop.
        "Loop" => {
            let inner = td.get_term("body").expect("Loop: missing body");
            let inner_result = analyze_loop_termination_td(&inner, registry);
            TerminationResult {
                terminates: inner_result.terminates,
                measure: Measure::Nested(Box::new(inner_result.measure.clone())),
                reason: format!(
                    "nested: outer termination depends on inner ({})",
                    inner_result.reason
                ),
            }
        }

        // --- Direct base arrow as loop body ---
        // loop(arr): the arrow itself must decrease C. Check its contract.
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            let contract = registry.lookup(op, &source);
            if let Some(c) = contract {
                if let Some(ref effect) = c.termination_effect {
                    measure_from_effect(op, effect)
                } else {
                    TerminationResult {
                        terminates: false,
                        measure: Measure::None,
                        reason: format!(
                            "arrow {} has no termination effect -- no decreasing measure",
                            op
                        ),
                    }
                }
            } else {
                TerminationResult {
                    terminates: false,
                    measure: Measure::None,
                    reason: format!("unknown arrow {} -- no decreasing measure found", op),
                }
            }
        }

        // --- loop(app(...)): conservatively not guaranteed ---
        "App" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "loop(app(...)): no structural decreasing measure on C".to_string(),
        },

        // --- loop(choice(l, r)): conservatively not guaranteed ---
        "Choice" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "loop(choice(...)): no structural decreasing measure on C".to_string(),
        },

        // --- loop(handle(...)): eliminated before analysis ---
        "Handle" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "loop(handle(...)): handle eliminated before analysis".to_string(),
        },

        // --- loop(rewrite(...)): no structural decreasing measure ---
        "Rewrite" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "loop(rewrite(...)): no structural decreasing measure on C".to_string(),
        },
        // --- loop(constant/child): no decrease on C ---
        "Arr" | "Child" => TerminationResult {
            terminates: false,
            measure: Measure::Constant,
            reason: "loop(constant/child): C unchanged".to_string(),
        },
        "Iter" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "loop(iter): no structural decreasing measure on C".to_string(),
        },
        _ => unreachable!(),
    }
}

/// Analyze `loop(seq(left, right))`. The `right` component is what updates the
/// feedback channel `C`, so we inspect its structure to discover the measure.
fn analyze_comp_loop_termination_td(
    left_td: &TermData,
    right_td: &TermData,
    registry: &ContractRegistry,
) -> TerminationResult {
    match right_td.tag() {
        // right is arr(id): seq(G, id) -- id leaves C unchanged. The decrease
        // (if any) must come from `left`. So recurse into left as the loop body.
        // This is the "fixed-point / accumulator" pattern.
        // Note: id is encoded as Term::Op { name: "id", ... }
        // This arm must come BEFORE the general Arr arm.
        "Op" if right_td.get_str("name") == Some("id") => {
            let left_result = analyze_loop_termination_td(left_td, registry);
            TerminationResult {
                terminates: left_result.terminates,
                measure: left_result.measure,
                reason: format!(
                    "seq(_, id): id leaves C unchanged -- {}",
                    left_result.reason
                ),
            }
        }

        // right is a base arrow: its termination_effect tells us about C.
        "Op" => {
            let op = right_td.get_str("name").unwrap_or("");
            let source = right_td.arr_source().unwrap_or(ArrSource::Native);
            let contract = registry.lookup(op, &source);
            if let Some(c) = contract {
                if let Some(ref effect) = c.termination_effect {
                    let mut res = measure_from_effect(op, effect);
                    // The left part must also terminate (it may contain a nested loop).
                    let left_term = analyze_termination_td(left_td, registry);
                    if !left_term.terminates {
                        res.terminates = false;
                        res.reason = format!(
                            "{}; BUT left may not terminate: {}",
                            res.reason, left_term.reason
                        );
                    }
                    res
                } else {
                    TerminationResult {
                        terminates: false,
                        measure: Measure::None,
                        reason: format!(
                            "arrow {} has no termination effect -- no decreasing measure",
                            op
                        ),
                    }
                }
            } else {
                TerminationResult {
                    terminates: false,
                    measure: Measure::None,
                    reason: format!("unknown arrow {} -- no decreasing measure found", op),
                }
            }
        }

        // right is first(inner): seq(G, first(H)) -- first threads C through.
        // C is unchanged by first -> side-channel on C. The decrease (if any)
        // must come from `left`. We check both the loop-termination analysis
        // of `left` and a structural scan for any consuming arrow (e.g.
        // fold_step with `decrements_length`) buried inside `left`.
        "First" => {
            // first(H) leaves the second component (C) unchanged.
            // Check whether left provides a decrease (e.g. left is itself a
            // loop or a consuming arrow like fold_step).
            let left_result = analyze_termination_td(left_td, registry);
            let left_loop = analyze_loop_termination_td(left_td, registry);
            let fallback = find_decreasing_measure_in_td(left_td, registry);
            let measure = if left_loop.measure != Measure::None {
                left_loop.measure
            } else if let Some((m, _)) = &fallback {
                m.clone()
            } else {
                Measure::Constant
            };
            let terminates = (left_loop.terminates || fallback.is_some()) && left_result.terminates;
            TerminationResult {
                terminates,
                measure,
                reason: if terminates {
                    format!(
                        "seq(_, first): first leaves C unchanged, but left provides decreasing measure: {}",
                        left_loop.reason
                    )
                } else {
                    format!(
                        "seq(_, first): first leaves C unchanged, no decreasing measure on C from right"
                    )
                },
            }
        }

        // right is a nested loop: seq(G, loop(H)). The inner loop must terminate,
        // and it must decrease C. We analyze the inner loop's termination.
        "Loop" => {
            let inner_body = right_td.get_term("body").expect("Loop: missing body");
            let inner_result = analyze_loop_termination_td(&inner_body, registry);
            let left_term = analyze_termination_td(left_td, registry);
            let terminates = inner_result.terminates && left_term.terminates;
            TerminationResult {
                terminates,
                measure: Measure::Nested(Box::new(inner_result.measure.clone())),
                reason: format!(
                    "seq(_, loop): inner loop {} -- {}",
                    if inner_result.terminates {
                        "terminates"
                    } else {
                        "may not terminate"
                    },
                    inner_result.reason
                ),
            }
        }

        // right is a composition: seq(G, seq(h1, h2)). Recurse into the
        // right composition to find the measure on C.
        "Seq" => {
            let h1 = right_td.get_term("fst").expect("Seq: missing fst");
            let h2 = right_td.get_term("snd").expect("Seq: missing snd");
            // The rightmost part (h2) is closest to C. Analyze seq(h1, h2)
            // as a loop body's right side.
            let inner = analyze_comp_loop_termination_td(&h1, &h2, registry);
            let left_term = analyze_termination_td(left_td, registry);
            // If the structural recursion did not find a decreasing measure,
            // scan the ENTIRE right side for any consuming arrow (e.g.
            // consume_one nested inside first(...) within the composition).
            let (terminates, measure, reason) = if inner.terminates {
                (
                    inner.terminates && left_term.terminates,
                    inner.measure,
                    format!("seq(_, seq(_, _)): right composition -- {}", inner.reason),
                )
            } else if let Some((m, why)) = find_decreasing_measure_in_td(right_td, registry) {
                let t = left_term.terminates;
                (
                    t,
                    m,
                    format!(
                        "seq(_, seq(_, _)): decreasing measure found in right side -- {}",
                        why
                    ),
                )
            } else {
                (
                    false,
                    inner.measure,
                    format!("seq(_, seq(_, _)): right composition -- {}", inner.reason),
                )
            };
            TerminationResult {
                terminates,
                measure,
                reason,
            }
        }

        // right is choice(l, r): conservatively no decrease on C,
        // BUT scan for a consuming arrow before giving up.
        "Choice" => {
            let cl = right_td.get_term("l").expect("Choice: missing l");
            let cr = right_td.get_term("r").expect("Choice: missing r");
            let left_term = analyze_termination_td(left_td, registry);
            if let Some((m, why)) = find_decreasing_measure_in_td(left_td, registry)
                .or_else(|| find_decreasing_measure_in_td(&cl, registry))
                .or_else(|| find_decreasing_measure_in_td(&cr, registry))
            {
                TerminationResult {
                    terminates: left_term.terminates,
                    measure: m,
                    reason: format!(
                        "seq(_, choice): choice alone does not decrease C, but a consuming arrow was found -- {}",
                        why
                    ),
                }
            } else {
                TerminationResult {
                    terminates: false,
                    measure: Measure::None,
                    reason: "seq(_, choice): choice does not decrease C".to_string(),
                }
            }
        }

        // right is app: conservatively no decrease on C.
        "App" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "seq(_, app): app does not decrease C".to_string(),
        },

        // right is handle: eliminated before analysis.
        "Handle" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "seq(_, handle): handle eliminated before analysis".to_string(),
        },

        // right is a constant or child projection: no decrease on C.
        "Arr" | "Child" => TerminationResult {
            terminates: false,
            measure: Measure::Constant,
            reason: "seq(_, constant/child): C unchanged".to_string(),
        },
        "Iter" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "seq(_, iter): no structural decreasing measure on C".to_string(),
        },
        // right is rewrite: no structural decreasing measure.
        "Rewrite" => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: "seq(_, rewrite): rewrite does not decrease C".to_string(),
        },
        _ => unreachable!(),
    }
}

/// Walk a sub-scheme and return a decreasing `Measure` if ANY base arrow
/// appearing in it has a decreasing termination effect (even when nested
/// inside `first`, `seq`, `choice`, or `loop`). This lets the analyzer
/// discover termination when the consuming arrow (e.g. `consume_one`) is not
/// the rightmost leaf of the loop body's right side but is buried inside a
/// composition such as `seq(first(consume_one), append_to_output)`.
fn find_decreasing_measure_in_td(
    td: &TermData,
    registry: &ContractRegistry,
) -> Option<(Measure, String)> {
    match td.tag() {
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            let contract = registry.lookup(op, &source)?;
            let effect = contract.termination_effect.as_ref()?;
            match effect.as_str() {
                "decrements_length" => Some((
                    Measure::ListLength("c".to_string()),
                    format!(
                        "{} decrements |C| by 1 each iteration, bounded below by 0",
                        op
                    ),
                )),
                "decrements_counter" => Some((
                    Measure::Counter("c".to_string(), 1),
                    format!(
                        "{} decrements C by 1 each iteration, bounded below by 0",
                        op
                    ),
                )),
                _ => None,
            }
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            find_decreasing_measure_in_td(&fst, registry)
                .or_else(|| find_decreasing_measure_in_td(&snd, registry))
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            find_decreasing_measure_in_td(&inner, registry)
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            find_decreasing_measure_in_td(&l, registry)
                .or_else(|| find_decreasing_measure_in_td(&r, registry))
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            find_decreasing_measure_in_td(&body, registry)
        }
        "App" => None,
        "Handle" => {
            let body = td.get_term("body").expect("Handle: missing body");
            let impl_ = td.get_term("impl_").expect("Handle: missing impl_");
            find_decreasing_measure_in_td(&body, registry)
                .or_else(|| find_decreasing_measure_in_td(&impl_, registry))
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            find_decreasing_measure_in_td(&source, registry)
                .or_else(|| find_decreasing_measure_in_td(&target, registry))
        }
        "Arr" | "Child" => None,
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            find_decreasing_measure_in_td(&fuel, registry)
                .or_else(|| find_decreasing_measure_in_td(&predicate, registry))
                .or_else(|| find_decreasing_measure_in_td(&step, registry))
                .or_else(|| find_decreasing_measure_in_td(&initial, registry))
        }
        _ => unreachable!(),
    }
}

/// Build a `TerminationResult` from a base arrow's termination effect string.
/// This is where the measure is discovered from the arrow's declared effect
/// on the feedback channel C.
fn measure_from_effect(name: &str, effect: &str) -> TerminationResult {
    match effect {
        "decrements_length" => TerminationResult {
            terminates: true,
            measure: Measure::ListLength("c".to_string()),
            reason: format!(
                "{} decrements |C| by 1 each iteration, bounded below by 0",
                name
            ),
        },
        "decrements_counter" => TerminationResult {
            terminates: true,
            measure: Measure::Counter("c".to_string(), 1),
            reason: format!(
                "{} decrements C by 1 each iteration, bounded below by 0 (assuming C >= 0)",
                name
            ),
        },
        "constant" => TerminationResult {
            terminates: false,
            measure: Measure::Constant,
            reason: format!("{} leaves C unchanged, no decreasing measure", name),
        },
        _ => TerminationResult {
            terminates: false,
            measure: Measure::None,
            reason: format!("{} has no known decreasing measure on C", name),
        },
    }
}
