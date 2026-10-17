//! Shared canonical AST to arrow-calculus `Term` lowerer (ADR-0100).
//!
//! Data-driven lowering (ADR-0105/ADR-0120 §"Known debt"): `lower_node`
//! keeps Rust arms only for kinds whose lowering is compile-time
//! binding/env machinery the `lang.eval` schemes do not carry — env/param
//! resolution (`var`), quoting (`function`), `App` disambiguation
//! (`call`), env threading (`assign`/`block`/`bind`/`rec`), signature and
//! handler machinery (`op`/`handle`/`arr`/`graded_arrow`), and the
//! arrow-position forms (`flow`/`axis_flow`/`loop`/`if`) whose eval
//! schemes are value-level approximations. Every other kind delegates to
//! [`lower_via_scheme`], which instantiates the `EvalSchemeDef` `Term`
//! from `lang.eval[kind]` over the node's lowered children — a language
//! construct added via a YAML `eval.*` scheme lowers with no Rust arm.

use std::collections::{HashMap, HashSet};

use handl_language::{ArrSource, Ast, AstNode, AstRef, Language, Term, Value};

/// A compile-time env binding: the lowered `Term` plus call metadata.
///
/// - `arity`: how many call arguments the bound arrow consumes as a
///   tuple — a function literal's own parameter count; `1` for
///   non-function bindings.
/// - `arrow_value`: the term *evaluates to* an arrow value — a quoted
///   `Arr` or a `{__term__, __bound__}` bound-arrow record — and may be
///   used as a `Term::App` callee directly. Scheme-kind bindings
///   (ops, compositions) are quoted at the application site instead.
#[derive(Clone)]
struct EnvEntry {
    term: Term,
    arity: usize,
    arrow_value: bool,
}

/// Lower a canonical AST rooted at `root` to a `Term` arrow.
pub fn lower_ast(ast: &Ast, lang: &Language, root: AstRef) -> Result<Term, String> {
    lower_node(ast, lang, root.index(), &HashMap::new(), &[])
}

fn lower_node(
    ast: &Ast,
    lang: &Language,
    idx: usize,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let node = ast
        .get(AstRef::new(idx))
        .ok_or_else(|| format!("AST index {} out of bounds", idx))?;

    match node.kind.as_str() {
        "value" | "lit" => Ok(Term::const_value(node.payload.clone())),

        // `json` template node (ADR-0060): child 0 is the template leaf
        // (`{"tag": "Succ", "pred": $0}`), children 1..N are the args that
        // substitute `$i` holes. A leaf `json` parses its payload directly.
        "json" => lower_json(ast, lang, node, env, params),

        "var" => {
            let name = node
                .payload
                .as_str()
                .ok_or_else(|| format!("var at {} has non-string payload", idx))?;

            if let Some(pos) = params.iter().position(|p| p == name) {
                if params.len() == 1 {
                    // Unary function: the parameter *is* the incoming value.
                    Ok(Term::op_native("id", vec![]))
                } else {
                    // Multi-parameter function: project the n-th component.
                    Ok(Term::op_native_lit("elem", vec![], pos as i64))
                }
            } else if let Some(entry) = env.get(name) {
                Ok(entry.term.clone())
            } else if let Some(global) = lang.globals.get(name) {
                // Ambient global (`true`/`false` from `data/globals.yaml`):
                // `proj_env` resolves env scopes then globals, so a global
                // name that no binding shadows lowers to its value.
                Ok(Term::const_value(global.clone()))
            } else {
                // Unbound name: treat as a native op reference (e.g. a host fn).
                Ok(Term::op_native(name, vec![]))
            }
        }

        "function" => {
            // A function used as a value lowers to a *quoted arrow*
            // (ADR-0120 Phase 1): the lowered body `Term`, serialized into
            // `Arr.value`. `Term::App` deserializes it back when the arrow
            // is applied; composition sites (`flow`, `loop`, handler impls)
            // unquote it to recover the body arrow. There is no closure or
            // captured runtime environment — parameters become `id`/
            // `proj_child` projections over the arrow's input.
            let own_params = function_params(ast, lang, node)?;
            let body_idx = function_body_index(lang, node)?;
            // Capture only the enclosing parameters the body actually
            // references (and that the function's own params do not
            // shadow). A nested function that uses no outer params is a
            // plain quoted arrow.
            let mut body_vars = std::collections::HashSet::new();
            collect_var_names(ast, body_idx, &mut body_vars);
            let captured: Vec<(usize, String)> = params
                .iter()
                .enumerate()
                .filter(|(_, p)| body_vars.contains(*p) && !own_params.contains(*p))
                .map(|(i, p)| (i, p.clone()))
                .collect();
            if captured.is_empty() {
                let body = lower_node(ast, lang, body_idx, env, &own_params)?;
                quote_term(body)
            } else {
                // Capturing nested function: the inner arrow's full input
                // tuple is `captured ++ own_params`. Emit a bound-arrow
                // record `{"__term__": quote(body), "__bound__": [projs]}`
                // so `x ~> y ~> x - y` applied as `g(1)(5)` feeds
                // `__in = [x, y]` — products, not captured environments.
                let mut full_params: Vec<String> =
                    captured.iter().map(|(_, p)| p.clone()).collect();
                full_params.extend(own_params.iter().cloned());
                let body = lower_node(ast, lang, body_idx, env, &full_params)?;
                let bound_terms: Vec<Term> = captured
                    .iter()
                    .map(|(outer_pos, _)| {
                        if params.len() == 1 {
                            Term::op_native("id", vec![])
                        } else {
                            Term::op_native_lit("elem", vec![], *outer_pos as i64)
                        }
                    })
                    .collect();
                let bound_arr = Term::op_native("array", bound_terms);
                Ok(Term::Op {
                    dim: Term::default_dim(),
                    name: "record".to_string(),
                    args: vec![quote_term(body)?, bound_arr],
                    source: ArrSource::Native,
                    lit: Some(Value::Array(vec![
                        Value::from("__term__"),
                        Value::from("__bound__"),
                    ])),
                })
            }
        }

        "call" => lower_call(ast, lang, node, env, params),

        // `flow`/`axis_flow`/`loop`/`if` keep Rust arms: their eval
        // schemes are value-level approximations for the env-based
        // engines, not the arrow-dialect lowering.
        //
        // `eval.flow` is `Op{flow, [proj_child 0, proj_child 1]}` — the
        // `flow` native op composes two *evaluated* arrow values. The
        // arrow lowering instead composes the operand `Term`s directly
        // (`Seq`), unquoting arrow values back into body schemes.
        "flow" => {
            // `x >>> f` is sequential composition `seq(x, f)`. Both sides
            // sit in arrow position, so quoted arrow values are unquoted
            // back into their body schemes.
            let (l, r) = lower_two_children(ast, lang, node, env, params)?;
            Ok(Term::seq(unquote_term(l), unquote_term(r)))
        }

        // `eval.axis_flow` routes axis 0 through `flow` for the canonical
        // engines and `k > 0` through `Seq{axis}` over dimension-2
        // operands; the arrow dialect instead materializes `Term::Seq`
        // with the axis slot populated — which is this arm — letting the
        // operands keep their own declared dimensions.
        "axis_flow" => {
            // `x >>>[k] f` — composition along axis `k` (ADR-0120 Phases 2/5).
            // The axis is an ordinary expression position lowered like any
            // other expression; the operands sit in arrow position and
            // unquote like `flow`.
            let l_idx = child_at_role_or_pos(node, lang, "left", 0)
                .ok_or_else(|| format!("axis_flow at {idx} missing left child"))?;
            let axis_idx = child_at_role_or_pos(node, lang, "axis", 1)
                .ok_or_else(|| format!("axis_flow at {idx} missing axis child"))?;
            let r_idx = child_at_role_or_pos(node, lang, "right", 2)
                .ok_or_else(|| format!("axis_flow at {idx} missing right child"))?;
            let l = lower_node(ast, lang, l_idx, env, params)?;
            let axis = lower_node(ast, lang, axis_idx, env, params)?;
            let r = lower_node(ast, lang, r_idx, env, params)?;
            Ok(Term::seq_axis(axis, unquote_term(l), unquote_term(r)))
        }

        // `eval.array`/`eval.object`/`eval.json`/`eval.paren` are
        // zero-placeholder op stubs: the tree-walk `array`/`object`/
        // `json`/`paren` ops re-read `payload.children` through the
        // `EvalNode` engine service. Instantiated naively they would drop
        // the children (or, for `array`, silently evaluate to `[]`), so
        // the payload/children packing stays in Rust.
        "array" => {
            // Array literal — two payload shapes (mirrors `prim array`):
            // `{"elements": [v1, v2, ...]}` holds literal values;
            // `{"children": [idx, ...]}` holds arena indices to lower.
            // The native `array` op packs the evaluated element schemes
            // into a single array value (eval.array).
            if let Some(elements) = node.payload.get("elements").and_then(|e| e.as_array()) {
                let terms = elements
                    .iter()
                    .map(|v| Term::const_value(v.clone()))
                    .collect();
                return Ok(Term::op_native("array", terms));
            }
            let terms = children(node)
                .iter()
                .map(|&i| lower_node(ast, lang, i, env, params))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Term::op_native("array", terms))
        }

        "object" => lower_object(ast, lang, node, env, params),
        "entry" => {
            // Entries are handled inside lower_object; this arm is a fallback.
            let value_idx = child_at_role_or_pos(node, lang, "value", 1)
                .ok_or_else(|| format!("entry at {} missing value child", idx))?;
            lower_node(ast, lang, value_idx, env, params)
        }

        // `eval.field-access` passes the field child as a second op
        // *argument*; the arrow `field_access` op convention carries the
        // field/index in `lit` instead, so this stays in Rust.
        "field-access" => lower_field_access(ast, lang, node, env, params),

        "block" => lower_block(ast, lang, node, env, params),

        "assign" => {
            // Let-form `assign(target, value, body)` (3+ children): bind
            // the target name in a private compile-time env and lower the
            // body under it — the same substitution `bind_stmt` performs
            // for `block`/`rec` children. `body` is an `optional` role
            // (ADR-0081), so its presence resolves arity-aware; positional
            // index 2 is the role-less fallback.
            if let Some(body_idx) = child_at_role_or_pos(node, lang, "body", 2) {
                let mut env = env.clone();
                let mut local = HashSet::new();
                bind_stmt(ast, lang, idx, &mut env, &mut local, params)?;
                return lower_node(ast, lang, body_idx, &env, params);
            }
            // An `assign` as an expression lowers to the value being bound.
            // The binding itself is dropped by the substitution model, so
            // a target that resolves to an existing binding or parameter
            // is an update-in-place the calculus cannot express — decline
            // and let the AST engine handle it.
            let value_idx = child_at_role_or_pos(node, lang, "value", 1)
                .ok_or_else(|| format!("assign at {} missing value child", idx))?;
            if let Some(target_idx) = child_at_role_or_pos(node, lang, "target", 0) {
                if let Some(name) = ast
                    .get(AstRef::new(target_idx))
                    .and_then(|t| t.payload.as_str())
                {
                    if params.iter().any(|p| p == name) || env.contains_key(name) {
                        return Err(format!(
                            "lower: `{name}` assigns to an existing binding (update-in-place is not expressible)"
                        ));
                    }
                }
            }
            lower_node(ast, lang, value_idx, env, params)
        }

        "paren" => {
            let inner = child_at_role_or_pos(node, lang, "body", 0)
                .ok_or_else(|| format!("paren at {} missing body child", idx))?;
            lower_node(ast, lang, inner, env, params)
        }

        "op" => lower_op(ast, lang, node, env, params),
        "handle" => lower_handle(ast, lang, node, env, params),
        "arr" => lower_arr(ast, lang, node, env, params),
        "graded_arrow" => lower_graded_arrow(ast, lang, node, env, params),
        "bind" => lower_bind(ast, lang, node, env, params),

        "assert" => {
            // Keyword assertion (ADR-0089/0092). In the arrow/compiled
            // model this is a side-channel op request: the AST eval engine
            // (tree-walk) keeps the assert failure side-channel, so the
            // lowerer only needs to emit the op and let the runtime fall
            // back to the AST engine.
            Ok(Term::op_native("assert", vec![]))
        }

        // `eval.if` is `cond >>> tag >>> Choice(then, else_or_null)`: the
        // `tag` Either wraps the *condition value* and `Choice` feeds it
        // to the branch sub-schemes — correct for the env-based engines,
        // where `proj_child` placeholders ignore their incoming value.
        // The arrow lowering must instead feed the branches the
        // *original input* (branch bodies project `id`/`proj_child`
        // parameters), hence the `product`/`pair_tag` shape below.
        "if" => lower_if(ast, lang, node, env, params),

        // ADR-0074/ADR-0105: `rec` region — scoped bindings ending in the
        // self-application produced by `desugar.data`'s recursor.
        "rec" => lower_rec(ast, lang, node, env, params),

        // `eval.loop` runs the `fold_step` value protocol over an
        // *evaluated* step arrow; the arrow lowering needs the step as a
        // `Term` — unquoted — plus the bound-arrow-record case, so it
        // stays in Rust.
        // `loop` node (ADR-0120 Phase 1): children `[input, step]` lower to
        // `seq(input, Loop(unquote step))`. `step` is the feedback body —
        // an arrow `(in, state) -> Either(result, next_state)` — so a
        // quoted step function is unquoted back into its body scheme.
        "loop" => lower_loop(ast, lang, node, env, params),

        // `eval.while` is `Seq[proj_child 0, Loop[loop_step(proj_child 1)]]`
        // — the step placeholder sits in `Loop.body`, a non-input
        // position, so the scheme declines. Lowered here instead
        // (ADR-0147): `while init step` → `Seq[init, Loop[body]]` where
        // `body` reproduces the `loop_step` protocol in combinators.
        "while" => lower_while(ast, lang, node, env, params),

        // `eval.ternary`/`eval.nullish` — `Seq[…, tag] >>> Choice` shapes
        // whose `Choice` arms hold child placeholders fed the *condition*
        // value, not the node input: non-input positions, they decline.
        // The `product`/`pair_tag` plumbing (`lower_if` precedent) hands
        // the arms the input instead.
        "ternary" => lower_ternary(ast, lang, node, env, params),
        "nullish" => lower_nullish(ast, lang, node, env, params),

        // `eval.type-annot` is `Seq[Seq[proj_child 1, extend_env], proj_child 0]`:
        // the env engines record the annotation in the env, then yield the
        // subject. On the arrow path annotations erase — the subject term
        // alone is the lowering (the `extend_env` bookkeeping has no
        // compile-time counterpart). The subject placeholder sits in
        // `Seq.snd` — a non-input position — hence this arm, same pattern
        // as `lower_if`/`lower_while` (ADR-0147/0148).
        "type-annot" => {
            let subject_idx = child_at_role_or_pos(node, lang, "subject", 0)
                .ok_or_else(|| format!("type-annot at {idx} missing subject child"))?;
            lower_node(ast, lang, subject_idx, env, params)
        }

        // `eval.dimension` is the `eval.call` routing shape verbatim
        // (`f<X>` applies the target to its dimension args, children laid
        // out `[target, args…]` like `call`'s `[callee, args…]`) — same
        // decline, same lowering.
        "dimension" => lower_call(ast, lang, node, env, params),

        // `eval.match` (`Op{match}`) routes through the env engines'
        // `match_shell`, which the arrow interpreter cannot run. Structural
        // patterns (literals, `_`/binders, `[]`/`[h, …t]`, `__tag__`
        // records) compile to nested `pair_tag`/`Choice` dispatch instead
        // (ADR-0147); richer patterns decline to the AST fallback.
        "match" => lower_match(ast, lang, node, env, params),

        // `return` is early-exit control flow the substitution model
        // cannot express. `eval.return` declares only the plain value
        // path (`proj_child 0`) — the early exit lives in `eval_rec`'s
        // interception of `return` children, which compile-time env
        // threading cannot reproduce. Decline so the runtime falls back
        // to the AST engine and keeps its interception semantics.
        "return" => Err(format!(
            "lower: `return` is early-exit control flow the substitution model cannot express"
        )),

        // ------------------------------------------------------------------
        // Data-driven lowering (ADR-0105/0120): every remaining kind
        // consults `lang.eval[kind]` — the `EvalSchemeDef` `Term` is a
        // template whose `proj_child`/`Child` placeholders are
        // instantiated with the node's lowered children. This covers the
        // `binary.*`/`product`/`fanout` op-argument shapes and the
        // ADR-0119 §7 frontier cells (`eval.cell_*`/`eval.dim_arrow`/
        // `eval.witness`/`eval.refine`), and any construct added to the
        // language purely via YAML eval schemes.
        _ => lower_via_scheme(ast, lang, node, idx, env, params),
    }
}

// ---------------------------------------------------------------------------
// Data-driven lowering — `lang.eval` scheme instantiation (ADR-0105/0120)
// ---------------------------------------------------------------------------

/// Instantiation context for [`lower_via_scheme`]: the node being lowered
/// plus the surrounding compile-time state shared with [`lower_node`].
struct SchemeCtx<'a> {
    ast: &'a Ast,
    lang: &'a Language,
    node: &'a AstNode,
    env: &'a HashMap<String, EnvEntry>,
    params: &'a [String],
}

/// Lower `node` by instantiating its `lang.eval[kind]` [`Term`] over the
/// node's lowered children.
///
/// The eval scheme is a `Term` template: `proj_child i` / `proj_child_or
/// i` / `proj_rest` ops and `Child{role}` terms are placeholders for the
/// node's children; `$payload` sentinels resolve to the node payload.
/// Everything else — op names, sources, `lit` operands, combinator
/// structure — is carried through verbatim, so the scheme's `Op`/`Seq`/
/// `Choice`/`App` shape *is* the lowered shape.
///
/// `Err` means the scheme cannot be instantiated honestly (a placeholder
/// in a non-input position, an unresolvable role, a missing child) or no
/// scheme exists — the caller propagates it so the runtime falls back to
/// the AST eval engine, preserving today's behavior.
fn lower_via_scheme(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    idx: usize,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let def = lang
        .eval
        .get_by_identity(&node.kind)
        .ok_or_else(|| format!("lower: unsupported node kind `{}` at {}", node.kind, idx))?;
    let ctx = SchemeCtx {
        ast,
        lang,
        node,
        env,
        params,
    };
    instantiate_scheme(&def.scheme, true, false, &ctx)
}

/// Instantiate a scheme subterm.
///
/// `input_faithful` tracks whether this position receives the node's own
/// input value when the term runs as an arrow. A child placeholder is
/// only honest where it does: in the eval engines a `proj_child`/`Child`
/// placeholder re-evaluates the AST child in the ambient env — it
/// *ignores* its incoming value. The lowered child, by contrast, is an
/// arrow that consumes its incoming value, so substituting it at a
/// position fed by a computed value (a sequential `Seq.snd`, a `Choice`
/// branch, `Loop` body, `First`, `Handle.impl_`, `Iter`, `Rewrite`)
/// would hand it a value the scheme never intended. Instantiation
/// declines there — which is exactly how the canonical `Seq`/`Choice`/
/// `Loop` eval schemes (`eval.if`, `eval.ternary`, `eval.nullish`,
/// `eval.while`, `eval.call`, `eval.axis_flow`, `eval.type-annot`,
/// `eval.dimension`) stay out of the arrow path: their placeholders all
/// sit in non-input positions. A `Seq.snd` that *pastes* (`k <
/// dim(fst)`, ADR-0120 Phase 5) is the exception — both `∘ₖ` operands
/// run on the same input, so the `eval.cell_*` frontier schemes do
/// instantiate.
fn instantiate_scheme(
    term: &Term,
    input_faithful: bool,
    either_input: bool,
    ctx: &SchemeCtx<'_>,
) -> Result<Term, String> {
    match term {
        Term::Arr { dim, value } => Ok(Term::Arr {
            dim: Box::new(instantiate_dim(dim, input_faithful, either_input, ctx)?),
            value: value.clone(),
        }),
        Term::Const { .. } | Term::Opaque { .. } => Ok(term.clone()),
        Term::Op {
            dim,
            name,
            args,
            source,
            lit,
        } => instantiate_op(
            dim,
            name,
            args,
            source,
            lit.as_ref(),
            input_faithful,
            either_input,
            ctx,
        ),
        Term::Seq { axis, fst, snd } => {
            // `>>>[k]` has two regimes (ADR-0120 Phase 5): `axis 0` on
            // 1-cells is sequential composition (`snd` is fed `fst`'s
            // output), while `k < dim(fst)` is globular pasting `∘ₖ`
            // (both operands run on the *same* incoming value). `snd`
            // placeholders are therefore input-faithful exactly when the
            // Seq pastes — this is what lets the `eval.cell_*` frontier
            // schemes instantiate: their dim-2 operands always take the
            // pasting branch.
            //
            // `Seq{pair_tag, snd}` is the second faithful-adjacent case:
            // `pair_tag` emits an `Either` whose payload is the node's own
            // input (the `product(cond, id)` contract), so a `Choice` in
            // `snd` hands its *arms* the node input — tracked by
            // `either_input` rather than `input_faithful` since `snd`
            // itself sees the Either, not the input.
            let snd_faithful = input_faithful && seq_pastes(axis, fst);
            let snd_either_input = is_pair_tag(fst);
            Ok(Term::Seq {
                // The axis is an ordinary expression position (ADR-0120
                // Phase 2), evaluated against the same incoming as `fst`.
                axis: Box::new(instantiate_scheme(axis, input_faithful, either_input, ctx)?),
                fst: Box::new(instantiate_scheme(fst, input_faithful, either_input, ctx)?),
                snd: Box::new(instantiate_scheme(
                    snd,
                    snd_faithful,
                    snd_either_input,
                    ctx,
                )?),
            })
        }
        Term::First { inner } => Ok(Term::First {
            inner: Box::new(instantiate_scheme(inner, false, false, ctx)?),
        }),
        Term::Choice { l, r } => Ok(Term::Choice {
            // An arm's incoming is the Either *payload*: the node input
            // when the upstream was `pair_tag` (`either_input`), the
            // tagged LHS otherwise.
            l: Box::new(instantiate_scheme(l, either_input, false, ctx)?),
            r: Box::new(instantiate_scheme(r, either_input, false, ctx)?),
        }),
        Term::Loop { body } => Ok(Term::Loop {
            body: Box::new(instantiate_scheme(body, false, false, ctx)?),
        }),
        Term::App { callee, args } => Ok(Term::App {
            callee: Box::new(instantiate_scheme(
                callee,
                input_faithful,
                either_input,
                ctx,
            )?),
            args: args
                .iter()
                .map(|a| instantiate_scheme(a, input_faithful, either_input, ctx))
                .collect::<Result<_, _>>()?,
        }),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => Ok(Term::Iter {
            // The iteration protocol threads state pairs, not the node
            // input — placeholders anywhere inside are not input-faithful.
            fuel: Box::new(instantiate_scheme(fuel, false, false, ctx)?),
            predicate: Box::new(instantiate_scheme(predicate, false, false, ctx)?),
            step: Box::new(instantiate_scheme(step, false, false, ctx)?),
            initial: Box::new(instantiate_scheme(initial, false, false, ctx)?),
        }),
        Term::Child { role } => {
            if !input_faithful {
                return Err(format!(
                    "lower: `Child` placeholder for role `{role}` sits in a non-input position (kind `{}`)",
                    ctx.node.kind
                ));
            }
            lower_role_child(role, ctx)
        }
        Term::Handle {
            body,
            target,
            impl_,
        } => Ok(Term::Handle {
            body: Box::new(instantiate_scheme(body, input_faithful, either_input, ctx)?),
            target: target.clone(),
            // `impl_` evaluates on a constant input, not the node input.
            impl_: Box::new(instantiate_scheme(impl_, false, false, ctx)?),
        }),
        Term::Rewrite {
            source,
            target,
            witness,
        } => Ok(Term::Rewrite {
            // Conservative: rewrite operands are not ordinary expression
            // positions the arrow model shares placeholder semantics with.
            source: Box::new(instantiate_scheme(source, false, false, ctx)?),
            target: Box::new(instantiate_scheme(target, false, false, ctx)?),
            witness: witness.clone(),
        }),
        Term::Src { cell } => Ok(Term::Src {
            cell: Box::new(instantiate_scheme(cell, input_faithful, either_input, ctx)?),
        }),
        Term::Tgt { cell } => Ok(Term::Tgt {
            cell: Box::new(instantiate_scheme(cell, input_faithful, either_input, ctx)?),
        }),
        Term::RecordProj { record, field } => Ok(Term::RecordProj {
            record: Box::new(instantiate_scheme(
                record,
                input_faithful,
                either_input,
                ctx,
            )?),
            field: field.clone(),
        }),
        Term::RecordWith {
            record,
            field,
            value,
        } => Ok(Term::RecordWith {
            record: Box::new(instantiate_scheme(
                record,
                input_faithful,
                either_input,
                ctx,
            )?),
            field: field.clone(),
            value: Box::new(instantiate_scheme(
                value,
                input_faithful,
                either_input,
                ctx,
            )?),
        }),
        Term::Tag { tag, value } => Ok(Term::Tag {
            tag: tag.clone(),
            value: Box::new(instantiate_scheme(
                value,
                input_faithful,
                either_input,
                ctx,
            )?),
        }),
        Term::Cobind { grade, body } => Ok(Term::Cobind {
            // The grade is a label/dimension position, like `Op.dim`.
            grade: Box::new(instantiate_dim(grade, input_faithful, either_input, ctx)?),
            body: Box::new(instantiate_scheme(body, input_faithful, either_input, ctx)?),
        }),
    }
}

/// Whether `Seq{axis, fst, _}` takes the globular-pasting branch —
/// `∘ₖ` on n-cells (n > k), where both operands run on the same incoming
/// value. Mirrors the exec backends' discriminator: `axis 0` pastes only
/// when `fst` is a higher-dimensional cell (`dim > 1`); `axis k > 0`
/// pastes when `k < dim(fst)`. A non-literal axis cannot be resolved at
/// instantiate time, so it conservatively reports no pasting.
fn seq_pastes(axis: &Term, fst: &Term) -> bool {
    let d = scheme_dim(fst);
    match axis.nat_literal() {
        Some(0) => d > 1,
        Some(k) => k < d,
        None => false,
    }
}

/// Whether `fst` is the `pair_tag` op — `Seq{pair_tag, snd}` produces an
/// `Either` whose payload is the node's own input (the `product(cond,
/// id)` contract used by `eval.binary.and/or` and the `if` lowering), so
/// a `Choice` in `snd` hands its arms the node input.
fn is_pair_tag(fst: &Term) -> bool {
    matches!(fst, Term::Op { name, .. } if name == "pair_tag")
}

/// The globular dimension of a scheme term (ADR-0120 Phase 5) — the same
/// structural estimate the exec backends use to pick the
/// sequential-vs-pasting `Seq` branch (`term_interpreter::term_dim`):
/// the `dim` slot when it is a numeric literal, otherwise the greatest
/// dimension among a composite's parts. Defaults to `1` — an ordinary
/// 1-cell.
fn scheme_dim(t: &Term) -> u64 {
    if let Term::Arr { dim, .. } | Term::Op { dim, .. } = t {
        if let Some(d) = dim.nat_literal() {
            return d.max(1);
        }
    }
    match t {
        Term::Seq { fst, snd, .. } => scheme_dim(fst).max(scheme_dim(snd)),
        Term::First { inner } => scheme_dim(inner),
        Term::Choice { l, r } => scheme_dim(l).max(scheme_dim(r)),
        Term::Loop { body } | Term::Handle { body, .. } | Term::Cobind { body, .. } => {
            scheme_dim(body)
        }
        Term::App { callee, .. } => scheme_dim(callee),
        Term::Iter { step, .. } => scheme_dim(step),
        Term::RecordProj { record, .. } | Term::RecordWith { record, .. } => scheme_dim(record),
        Term::Tag { value, .. } => scheme_dim(value),
        Term::Src { cell } | Term::Tgt { cell } => scheme_dim(cell),
        Term::Rewrite { target, .. } => scheme_dim(target),
        _ => 1,
    }
}

/// Instantiate an `Op` scheme node: child-projection ops are the
/// placeholders; any other op is copied with its `dim`, args, `name`,
/// and `lit` instantiated/resolved.
fn instantiate_op(
    dim: &Term,
    name: &str,
    args: &[Term],
    source: &ArrSource,
    lit: Option<&Value>,
    input_faithful: bool,
    either_input: bool,
    ctx: &SchemeCtx<'_>,
) -> Result<Term, String> {
    match name {
        "proj_child" | "proj_child_or" => {
            if !input_faithful {
                return Err(format!(
                    "lower: `{name}` placeholder sits in a non-input position (kind `{}`, child {})",
                    ctx.node.kind,
                    lit.and_then(Value::as_u64).map(|i| i.to_string()).unwrap_or_default()
                ));
            }
            let i = lit
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("lower: `{name}` missing index literal"))?
                as usize;
            let term = match children(ctx.node).get(i).copied() {
                Some(child_idx) => lower_node(ctx.ast, ctx.lang, child_idx, ctx.env, ctx.params)?,
                None if name == "proj_child_or" => Term::const_value(Value::Null),
                None => {
                    return Err(format!(
                        "lower: `{}` missing child {i} for `{name}`",
                        ctx.node.kind
                    ));
                }
            };
            lift_to_dim(term, dim, input_faithful, either_input, ctx)
        }
        "proj_rest" => {
            if !input_faithful {
                return Err(format!(
                    "lower: `proj_rest` placeholder sits in a non-input position (kind `{}`)",
                    ctx.node.kind
                ));
            }
            // Pack children 1..N into one array value, mirroring the
            // `proj_rest` eval op: each lowered child runs on the input
            // and `array` packs the results.
            let terms = children(ctx.node)
                .iter()
                .skip(1)
                .map(|&i| lower_node(ctx.ast, ctx.lang, i, ctx.env, ctx.params))
                .collect::<Result<Vec<_>, _>>()?;
            lift_to_dim(
                Term::op_native("array", terms),
                dim,
                input_faithful,
                either_input,
                ctx,
            )
        }
        _ => {
            let dim = instantiate_dim(dim, input_faithful, either_input, ctx)?;
            let name = if name == "$payload" {
                payload_sentinel(ctx.node)?
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("lower: `{}` `$payload` is not a name", ctx.node.kind))?
            } else {
                name.to_string()
            };
            let lit = match lit {
                Some(Value::String(s)) if s == "$payload" => Some(payload_sentinel(ctx.node)?),
                other => other.cloned(),
            };
            let args = args
                .iter()
                .map(|a| instantiate_scheme(a, input_faithful, either_input, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Term::Op {
                dim: Box::new(dim),
                name,
                args,
                source: source.clone(),
                lit,
            })
        }
    }
}

/// A child placeholder declared at dimension `dim` (`Op{dim, name:
/// "proj_child"}`) lifts the lowered child to that dimension: the
/// composite `child >>> id_{dim}` has `dim` as its dimension (the maximum
/// of its parts), so `Seq{axis: k}` sees it as a cell of dimension `dim`
/// — the same operand the eval engines' dimension-typed `proj_child`
/// produces for the ADR-0119 §7 frontier schemes (`eval.cell_comp_h`,
/// `eval.whisker_*`, `eval.witness`, …). A dimension-1 placeholder is an
/// ordinary child position and lifts to itself.
fn lift_to_dim(
    term: Term,
    dim: &Term,
    input_faithful: bool,
    either_input: bool,
    ctx: &SchemeCtx<'_>,
) -> Result<Term, String> {
    if dim.nat_literal() == Some(1) {
        return Ok(term);
    }
    Ok(Term::seq(
        term,
        Term::Op {
            dim: Box::new(instantiate_dim(dim, input_faithful, either_input, ctx)?),
            name: "id".to_string(),
            args: vec![],
            source: ArrSource::Native,
            lit: None,
        },
    ))
}

/// Instantiate a *dimension slot* (`Op.dim`, `Arr.dim`, `Cobind.grade`):
/// placeholders there denote dimension expressions, so a `var` child is
/// a *named* dimension label (`Const "S"`) and a literal is a numeric
/// dimension — [`lower_dim_term`]'s contract, matching how
/// `eval.dim_arrow`/`eval.witness` read these children as labels.
/// Non-placeholder dimension terms instantiate as ordinary expressions
/// (the `Seq.axis` slot is handled by [`instantiate_scheme`] itself).
fn instantiate_dim(
    term: &Term,
    input_faithful: bool,
    either_input: bool,
    ctx: &SchemeCtx<'_>,
) -> Result<Term, String> {
    match term {
        Term::Child { role } => {
            if !input_faithful {
                return Err(format!(
                    "lower: `Child` dim placeholder for role `{role}` sits in a non-input position"
                ));
            }
            let child_idx = role_child_index(role, ctx)?;
            lower_dim_term(ctx.ast, ctx.lang, child_idx, ctx.env, ctx.params)
        }
        Term::Op {
            name,
            lit: Some(lit),
            ..
        } if name == "proj_child" || name == "proj_child_or" => {
            if !input_faithful {
                return Err(format!(
                    "lower: `{name}` dim placeholder sits in a non-input position"
                ));
            }
            let i = lit
                .as_u64()
                .ok_or_else(|| format!("lower: `{name}` missing index literal"))?
                as usize;
            match children(ctx.node).get(i).copied() {
                Some(child_idx) => {
                    lower_dim_term(ctx.ast, ctx.lang, child_idx, ctx.env, ctx.params)
                }
                None if name == "proj_child_or" => Ok(Term::const_value(Value::Null)),
                None => Err(format!(
                    "lower: `{}` missing child {i} for `{name}`",
                    ctx.node.kind
                )),
            }
        }
        _ => instantiate_scheme(term, input_faithful, either_input, ctx),
    }
}

/// Instantiate a `Child{role}` placeholder in expression position.
///
/// Roles that name *label positions* — `dim`, `grade`, `name` — lower via
/// [`lower_dim_term`] (a `var` child denotes its name, matching how the
/// `eval.dim_arrow`/`eval.witness`/`eval.sort`-family schemes read these
/// children as label strings). Every other role is an ordinary
/// expression position and lowers via [`lower_node`].
fn lower_role_child(role: &str, ctx: &SchemeCtx<'_>) -> Result<Term, String> {
    let child_idx = role_child_index(role, ctx)?;
    if matches!(role, "dim" | "grade" | "name") {
        lower_dim_term(ctx.ast, ctx.lang, child_idx, ctx.env, ctx.params)
    } else {
        lower_node(ctx.ast, ctx.lang, child_idx, ctx.env, ctx.params)
    }
}

/// Resolve a `Child{role}` placeholder's arena index via the node's
/// `AstShapeDef` child roles (ADR-0081).
fn role_child_index(role: &str, ctx: &SchemeCtx<'_>) -> Result<usize, String> {
    let pos = ctx
        .lang
        .child_role_index(&ctx.node.kind, role)
        .map_err(|e| format!("lower: role `{role}` on `{}`: {e}", ctx.node.kind))?
        .ok_or_else(|| format!("lower: `{}` declares no `{role}` child role", ctx.node.kind))?;
    children(ctx.node).get(pos).copied().ok_or_else(|| {
        format!(
            "lower: `{}` missing `{role}` child at position {pos}",
            ctx.node.kind
        )
    })
}

/// Resolve the `$payload` sentinel used by eval schemes: a string payload
/// is itself the datum (the `proj_env "$payload"` convention); an object
/// payload's `name` field is (the `extend_env "$payload"` convention).
fn payload_sentinel(node: &AstNode) -> Result<Value, String> {
    if let Some(s) = node.payload.as_str() {
        return Ok(Value::String(s.to_string()));
    }
    node.payload
        .get("name")
        .filter(|v| v.is_string())
        .cloned()
        .ok_or_else(|| format!("lower: `{}` payload cannot resolve `$payload`", node.kind))
}

fn lower_block(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let mut env = env.clone();
    let mut local = HashSet::new();
    let statements = children(node);
    if statements.is_empty() {
        return Ok(Term::const_value(Value::Null));
    }

    // `prefix` accumulates mid-block non-binding statements as an
    // input-preserving effect sequence: each step is
    // `Seq(product(stmt, id), proj_child 1)` — `stmt` runs against the
    // block's input (its effects and errors happen), the pair keeps the
    // input, and `proj_child 1` restores it for the next statement.
    // Statements whose ops the `Term` interpreter does not implement
    // (`import`, `assert`, host ops) lower fine but fail at interpret
    // time, where `Runtime::eval` falls back to the AST eval engine —
    // the same end result as declining here, without log noise.
    let mut prefix: Option<Term> = None;
    let mut last = None;
    let count = statements.len();
    for (i, &idx) in statements.iter().enumerate() {
        match bind_stmt(ast, lang, idx, &mut env, &mut local, params)? {
            // `eval_block`: the block's value is the last statement's
            // value — a trailing `assign`/`bind`/`op` yields its bound
            // value, not null.
            Some(term) if i + 1 == count => last = Some(term),
            Some(_) => {}
            None if i + 1 == count => last = Some(lower_node(ast, lang, idx, &env, params)?),
            None => {
                let stmt = lower_node(ast, lang, idx, &env, params)?;
                let step = Term::seq(
                    Term::op_native("product", vec![stmt, Term::op_native("id", vec![])]),
                    Term::op_native_lit("elem", vec![], 1),
                );
                prefix = Some(match prefix {
                    Some(p) => Term::seq(p, step),
                    None => step,
                });
            }
        }
    }

    let last = last.ok_or_else(|| {
        format!(
            "block at {} produced no result expression",
            node_index(node)
        )
    })?;
    Ok(match prefix {
        Some(p) => Term::seq(p, last),
        None => last,
    })
}

/// `rec` region (ADR-0074): a scoped sequence of bindings produced by
/// `desugar.data` for the recursor's bound-arrow record (ADR-0120 Phase
/// 1 — `{__term__, __bound__}`, not self-application). All but the last
/// child bind into a private env; the node lowers to the last
/// statement's value.
fn lower_rec(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let stmts = children(node);
    let (last, init) = stmts
        .split_last()
        .ok_or_else(|| format!("rec at {} has no children", node_index(node)))?;
    let mut env = env.clone();
    let mut local = HashSet::new();
    for &idx in init {
        if bind_stmt(ast, lang, idx, &mut env, &mut local, params)?.is_none() {
            return Err(format!(
                "rec statement {} is not a binding; its effects cannot be \
                 sequenced in the pure arrow calculus (falling back to AST eval)",
                idx
            ));
        }
    }
    lower_node(ast, lang, *last, &env, params)
}

/// Bind a `block`/`rec` child statement into `env` when it is a binding
/// (`assign`, `bind`, `op`). Returns the bound value for bindings and
/// `None` for non-binding statements.
///
/// `local` tracks the names bound in *this* lexical scope (the block or
/// `rec` region currently being lowered). Rebinding a name that is
/// already bound but not in `local` is an update to an *enclosing*
/// binding — update-in-place mutation the pure arrow calculus cannot
/// express (there is no store). Declining with `Err` makes the runtime
/// fall back to the AST eval engine, which implements
/// update-or-define scope semantics. Sequential rebinding inside the
/// same scope is fine: later statements lower under the updated env.
fn bind_stmt(
    ast: &Ast,
    lang: &Language,
    idx: usize,
    env: &mut HashMap<String, EnvEntry>,
    local: &mut HashSet<String>,
    params: &[String],
) -> Result<Option<Term>, String> {
    let child = ast
        .get(AstRef::new(idx))
        .ok_or_else(|| format!("block child {} out of bounds", idx))?;
    // Binding dispatch is face-driven (ADR-0123): `binding` names the family.
    // The *layout* is then distinguished by the declared child roles, which is
    // where `assign` and `bind` actually differ — `assign` has `target`/`value`
    // roles; `bind` carries `payload.name` and one child. Reusing the role
    // metadata (rather than adding a second face) is what makes a language
    // that renames `assign` to `set` lower unchanged.
    //
    // `is_binding` keeps the canonical kind fallback for face-less shapes, so
    // a hand-built test language that declares no `faces` behaves as before.
    let is_binding = |kind: &str| -> bool {
        if lang.declares_faces(kind) {
            lang.shape_has_face(kind, "binding")
        } else {
            kind == "assign" || kind == "bind"
        }
    };
    // A binding whose shape declares a `target` role uses the assign layout.
    // Role-less shapes (hand-built test languages that declare no `children`)
    // fall back to the canonical `assign` kind for the layout, exactly as
    // `is_binding` falls back for the face.
    let is_assign_layout = lang
        .child_role_index(&child.kind, "target")
        .ok()
        .flatten()
        .is_some()
        || (!lang.declares_faces(&child.kind) && child.kind == "assign");
    if is_binding(&child.kind) && is_assign_layout {
        let target_idx = child_at_role_or_pos(child, lang, "target", 0)
            .ok_or_else(|| format!("assign at {} missing target child", idx))?;
        let target = ast
            .get(AstRef::new(target_idx))
            .ok_or_else(|| format!("assign target {} out of bounds", target_idx))?;
        let name = target
            .payload
            .as_str()
            .ok_or_else(|| format!("assign target at {} is not a string", target_idx))?
            .to_string();
        check_rebind(&name, env, local, params)?;
        let value_idx = child_at_role_or_pos(child, lang, "value", 1)
            .ok_or_else(|| format!("assign at {} missing value child", idx))?;
        decline_self_reference(ast, lang, &name, value_idx)?;
        let bound = if is_op_graded_arrow(ast, lang, value_idx)? {
            op_term_from_graded_arrow(ast, lang, &name, value_idx)?
        } else {
            lower_node(ast, lang, value_idx, env, params)?
        };
        let entry = env_entry(ast, lang, value_idx, bound);
        env.insert(name.clone(), entry.clone());
        local.insert(name);
        Ok(Some(entry.term))
    } else if is_binding(&child.kind) {
        let name = child
            .payload
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("bind at {} missing payload.name", idx))?
            .to_string();
        check_rebind(&name, env, local, params)?;
        let value_idx = child_at_role_or_pos(child, lang, "value", 0)
            .or_else(|| children(child).first().copied())
            .ok_or_else(|| format!("bind at {} missing value child", idx))?;
        decline_self_reference(ast, lang, &name, value_idx)?;
        let bound = if is_op_graded_arrow(ast, lang, value_idx)? {
            op_term_from_graded_arrow(ast, lang, &name, value_idx)?
        } else {
            lower_node(ast, lang, value_idx, env, params)?
        };
        let entry = env_entry(ast, lang, value_idx, bound);
        env.insert(name.clone(), entry.clone());
        local.insert(name);
        Ok(Some(entry.term))
    } else if child.kind == "op" {
        let name = child
            .payload
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} at {} missing payload.name", child.kind, idx))?
            .to_string();
        check_rebind(&name, env, local, params)?;
        // `subtree_name_candidates` on the `op` node itself would pick up
        // `payload.name` — the bound name — as a candidate and always
        // fire. Scan the op's arrow child instead.
        let arrow_idx = children(child)
            .first()
            .copied()
            .ok_or_else(|| format!("op at {} missing arrow child", idx))?;
        decline_self_reference(ast, lang, &name, arrow_idx)?;
        let bound = lower_node(ast, lang, idx, env, params)?;
        let entry = EnvEntry {
            term: bound.clone(),
            arity: 1,
            arrow_value: false,
        };
        env.insert(name.clone(), entry);
        local.insert(name);
        Ok(Some(bound))
    } else {
        Ok(None)
    }
}

/// A bound name *referenced* in its own RHS is a self-recursive call —
/// not a calculus construct (recursion is λ; the arrow calculus has
/// feedback via `Loop`, not self-application). `var f` would emit
/// `Op{f}` → `call_host` → "function 'f' not found" at runtime, so
/// decline: the env engines' `eval_rec` resolves it correctly instead.
/// The scan counts `var` nodes only — a bare payload string that merely
/// *spells* the name (the `x := "x"` bare-name import lowering, record
/// keys, op names) is a mention, not a reference.
fn decline_self_reference(
    ast: &Ast,
    lang: &Language,
    name: &str,
    value_idx: usize,
) -> Result<(), String> {
    if handl_language::subtree_var_names(
        |r| ast.get(r).cloned(),
        |n| {
            lang.pattern_role_is(&n.kind, "leaf")
                .unwrap_or(n.kind == "var")
        },
        AstRef::new(value_idx),
    )
    .is_some_and(|names| names.contains(name))
    {
        return Err(format!(
            "lower: self-recursive call to `{name}` — the arrow calculus has no \
             recursion; express it as a `while`/`loop` fold"
        ));
    }
    Ok(())
}

/// Reject a binding of `name` when it would mutate state the pure arrow
/// calculus cannot express: assigning to a function parameter, or
/// rebinding a name bound in an *enclosing* scope (`env` but not
/// `local`). The mutable engine semantics update the nearest existing
/// binding, but a compile-time substitution env can only shadow it — so
/// the write would be silently lost. Declining makes the runtime fall
/// back to the AST eval engine.
fn check_rebind(
    name: &str,
    env: &HashMap<String, EnvEntry>,
    local: &HashSet<String>,
    params: &[String],
) -> Result<(), String> {
    if params.iter().any(|p| p == name) {
        return Err(format!(
            "lower: `{name}` assigns to a function parameter (update-in-place is not expressible)"
        ));
    }
    if env.contains_key(name) && !local.contains(name) {
        return Err(format!(
            "lower: `{name}` rebinds an enclosing-scope binding (update-in-place is not expressible)"
        ));
    }
    Ok(())
}

/// Build the env binding for a bound value: function literals are
/// arrow *values* (quoted `Arr` or bound-arrow records) whose call arity
/// is their own parameter count; everything else is a scheme term that
/// is quoted at the application site.
fn env_entry(ast: &Ast, lang: &Language, value_idx: usize, term: Term) -> EnvEntry {
    let node = ast.get(AstRef::new(value_idx));
    match node {
        Some(n) if n.kind == "function" => {
            let arity = function_params(ast, lang, n)
                .map(|p| p.len().max(1))
                .unwrap_or(1);
            EnvEntry {
                term,
                arity,
                arrow_value: true,
            }
        }
        // `desugar.data` wraps the recursor's dispatch function in a `rec`
        // region; the region evaluates to the arrow it binds, so the call
        // arity is that function's parameter count.
        Some(n) if n.kind == "rec" => {
            let arity = rec_produced_arity(ast, lang, n);
            EnvEntry {
                term,
                arity: arity.unwrap_or(1),
                arrow_value: arity.is_some(),
            }
        }
        _ => EnvEntry {
            term,
            arity: 1,
            arrow_value: false,
        },
    }
}

/// Arity of the arrow a `rec` region produces: the last child is a `var`
/// bound by an earlier `assign` — if its value is a function, that's the
/// produced arrow's parameter count.
fn rec_produced_arity(ast: &Ast, lang: &Language, node: &AstNode) -> Option<usize> {
    let ch = children(node);
    let last = ast.get(AstRef::new(*ch.last()?))?;
    if last.kind != "var" {
        return None;
    }
    let name = last.payload.as_str()?;
    for &i in ch.iter().rev().skip(1) {
        let n = ast.get(AstRef::new(i))?;
        if n.kind != "assign" {
            continue;
        }
        let t_idx = child_at_role_or_pos(n, lang, "target", 0)?;
        let t = ast.get(AstRef::new(t_idx))?;
        if t.payload.as_str() != Some(name) {
            continue;
        }
        let v_idx = child_at_role_or_pos(n, lang, "value", 1)?;
        let v = ast.get(AstRef::new(v_idx))?;
        return if v.kind == "function" {
            function_params(ast, lang, v).ok().map(|p| p.len().max(1))
        } else {
            None
        };
    }
    None
}

/// `json` template (ADR-0060): children `[template, arg0..argN]` — child 0
/// is a leaf `json` node whose string payload is the template; the op
/// substitutes evaluated args into `$i` holes and parses the result.
fn lower_json(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let children = children(node);
    let (template, arg_indices): (String, &[usize]) = if children.len() >= 2 {
        let template_node = ast
            .get(AstRef::new(children[0]))
            .ok_or_else(|| format!("json template {} out of bounds", children[0]))?;
        let template = template_node
            .payload
            .as_str()
            .ok_or_else(|| format!("json template at {} is not a string", children[0]))?
            .to_string();
        (template, &children[1..])
    } else {
        let template = node
            .payload
            .as_str()
            .ok_or_else(|| format!("json at {} has non-string payload", node_index(node)))?
            .to_string();
        (template, &[][..])
    };
    let mut args = Vec::with_capacity(arg_indices.len());
    for &idx in arg_indices {
        args.push(lower_node(ast, lang, idx, env, params)?);
    }
    Ok(Term::op_native_lit("json", args, Value::String(template)))
}

fn lower_call(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let children = children(node);
    if children.is_empty() {
        return Err(format!("call at {} has no children", node_index(node)));
    }
    let callee_idx = child_at_role_or_pos(node, lang, "callee", 0).unwrap_or(children[0]);
    let callee = ast
        .get(AstRef::new(callee_idx))
        .ok_or_else(|| format!("call callee {} out of bounds", callee_idx))?;

    let arg_indices: Vec<usize> = if let Some(start) = child_at_role_or_pos(node, lang, "args", 1) {
        // `args` is the first variadic argument role; collect remaining children too.
        children
            .iter()
            .skip_while(|&&i| i != start)
            .copied()
            .collect()
    } else {
        // Positional: child 0 is callee, the rest are arguments.
        children.iter().skip(1).copied().collect()
    };

    let mut arg_terms = Vec::with_capacity(arg_indices.len());
    for &arg_idx in &arg_indices {
        arg_terms.push(lower_node(ast, lang, arg_idx, env, params)?);
    }

    if callee.kind == "var" {
        let name = callee
            .payload
            .as_str()
            .ok_or_else(|| format!("call callee at {} is not a string", callee_idx))?;

        if let Some(entry) = env.get(name) {
            if arg_terms.is_empty() {
                return Ok(entry.term.clone());
            }
            // Env-bound callee (ADR-0120 Phase 1): apply the bound term
            // via `Term::App` — `seq(pair(quote f, x), app)`. Arrow-value
            // bindings (quoted `Arr`s and bound-arrow records) are
            // already in callee position; scheme-kind bindings are
            // quoted at the application site.
            let callee_term = if entry.arrow_value {
                entry.term.clone()
            } else if binding_denotes_value(&entry.term) {
                // Value-producing terms (`App` → a bound-arrow record,
                // `Const`/plain `Arr` → constants) must be *evaluated*,
                // then the App dispatches on the resulting value:
                // quoting an `App` re-ran the application as an arrow
                // body and returned the bound-arrow record unapplied
                // (`h := c(a)(b); h(5)` yielded `h`, not `11`), and
                // quoting a constant made `x := 5; x(2)` silently return
                // `5` instead of erroring like the engines.
                entry.term.clone()
            } else {
                ensure_quoted(&entry.term)
            };
            let mut args = arg_terms;
            // A multi-parameter binding consumes its `arity` arguments as
            // one tuple application; surplus arguments apply curried —
            // one single-argument `App` each (the calculus's unary
            // `app`).
            if entry.arity > 1 && args.len() >= entry.arity {
                let head: Vec<Term> = args.drain(..entry.arity).collect();
                let mut app = Term::app(callee_term, head);
                for a in args {
                    app = Term::app(app, vec![a]);
                }
                return Ok(app);
            }
            return Ok(sequential_app(callee_term, args));
        }

        if let Some(pos) = params.iter().position(|p| p == name) {
            // Parameter callee (`self(self)`, `f(x)` where `f` is a
            // parameter): dynamic application — the callee value comes
            // from the input tuple. Sequential `App`s preserve curried
            // application for arrow-valued parameters.
            let callee_term = if params.len() == 1 {
                Term::op_native("id", vec![])
            } else {
                Term::op_native_lit("elem", vec![], pos as i64)
            };
            return Ok(sequential_app(callee_term, arg_terms));
        }

        // Unknown callee: treat as a native op call.
        return Ok(Term::op_native(name, arg_terms));
    }

    let callee_term = lower_node(ast, lang, callee_idx, env, params)?;
    // Call dispatch is face-driven (ADR-0123): `call` names the family, so a
    // language that spells its application differently still gets
    // `term_app` semantics. Face-less shapes keep the canonical fallback.
    let is_call = if lang.declares_faces(&callee.kind) {
        lang.shape_has_face(&callee.kind, "call")
    } else {
        callee.kind == "call"
    };
    if is_call {
        // A `call` callee produces a bound-arrow record (`{__term__,
        // __bound__}`) — e.g. `Type.rec(m, c0..)(params.., v)` or the
        // intermediate of a curried application. Tuple application
        // extends `__bound__` flatly, matching the engines' bound-record
        // semantics.
        return Ok(Term::app(callee_term, arg_terms));
    }
    // Other dynamic callees: sequential `App`s — `App(App(f, a), b)` —
    // preserve curried application for arrow-valued callees.
    Ok(sequential_app(callee_term, arg_terms))
}

/// Apply `callee` to each argument with a single-argument `Term::App` —
/// the calculus's unary `app`, so `f(a, b)` on a curried arrow is
/// `app(app(f, a), b)`. A multi-parameter arrow callee should instead be
/// emitted as one `App` with all args (the tuple application) — the
/// caller selects via the binding's `arity`.
fn sequential_app(callee: Term, args: Vec<Term>) -> Term {
    if args.is_empty() {
        return Term::app(callee, vec![]);
    }
    let mut it = args.into_iter();
    let mut app = Term::app(callee, vec![it.next().unwrap()]);
    for a in it {
        app = Term::app(app, vec![a]);
    }
    app
}

fn lower_object(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let mut keys = Vec::new();
    let mut values = Vec::new();

    // `{"entries": [{"key": "k", "value": v}, ...]}` — the rich payload
    // shape with literal values (mirrors `prim object`); each value
    // lowers to a constant.
    if let Some(entries) = node.payload.get("entries").and_then(|e| e.as_array()) {
        for entry in entries {
            let key = entry
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("object entry at {} missing key", node_index(node)))?;
            let value = entry
                .get("value")
                .ok_or_else(|| format!("object entry at {} missing value", node_index(node)))?;
            reject_reserved_object_key(key, node)?;
            keys.push(Value::String(key.to_string()));
            values.push(Term::const_value(value.clone()));
        }
        return Ok(Term::Op {
            dim: Term::default_dim(),
            name: "record".to_string(),
            args: values,
            source: handl_language::ArrSource::Native,
            lit: Some(Value::Array(keys)),
        });
    }

    let entry_indices = children(node);

    for &entry_idx in &entry_indices {
        let entry = ast
            .get(AstRef::new(entry_idx))
            .ok_or_else(|| format!("object entry {} out of bounds", entry_idx))?;

        let key_idx = child_at_role_or_pos(entry, lang, "key", 0)
            .ok_or_else(|| format!("entry at {} missing key child", entry_idx))?;
        let key_node = ast
            .get(AstRef::new(key_idx))
            .ok_or_else(|| format!("entry key {} out of bounds", key_idx))?;
        // A bare key parses as `var`; a *quoted* key parses as a `value`
        // holding the string (the `string` kind is the contract-dialect
        // spelling). Both are literals and lower; a computed key (any
        // other kind, e.g. a numeric `value`) is declined.
        let key = match key_node.kind.as_str() {
            "var" | "string" => key_node
                .payload
                .as_str()
                .ok_or_else(|| format!("entry key at {} is not a string", key_idx))?
                .to_string(),
            "value" => key_node
                .payload
                .as_str()
                .ok_or_else(|| format!("object entry key at {} is not a literal", key_idx))?
                .to_string(),
            _ => return Err(format!("object entry key at {} is not a literal", key_idx)),
        };

        let value_idx = child_at_role_or_pos(entry, lang, "value", 1)
            .ok_or_else(|| format!("entry at {} missing value child", entry_idx))?;
        let value = lower_node(ast, lang, value_idx, env, params)?;

        reject_reserved_object_key(&key, node)?;
        keys.push(Value::String(key));
        values.push(value);
    }

    Ok(Term::Op {
        dim: Term::default_dim(),
        name: "record".to_string(),
        args: values,
        source: handl_language::ArrSource::Native,
        lit: Some(Value::Array(keys)),
    })
}

/// Reject a reserved host-sentinel key (`__tag__`, `__registry__`, …) in an
/// object literal (ADR-0055 §"Negative — A reserved key", ADR-0054).
///
/// The evaluator's `object`/`record` ops enforce this at construction time so
/// a user object literal cannot forge a tagged value. The `Term` lowering
/// path builds records itself, so it must apply the same guard — otherwise a
/// lowered program would bypass the invariant that the AST engines enforce.
///
/// Returning `Err` declines the lowering; the caller then falls back to the
/// bound AST eval engine, whose own `object` guard reports the reserved-key
/// error to the user.
fn reject_reserved_object_key(key: &str, node: &AstNode) -> Result<(), String> {
    if handl_language::is_reserved_value_key(key) {
        return Err(format!(
            "object: key '{}' is reserved (node {})",
            key,
            node_index(node)
        ));
    }
    Ok(())
}

fn lower_op(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let name = node
        .payload
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op at {} missing payload.name", node_index(node)))?
        .to_string();

    // The signature's grade/dim child lowers into the `Op`'s `dim` slot —
    // the unified dimension/grade representation (ADR-0120 Phase 3). A
    // `dim_arrow`'s `dim` role sits at position 1, as does a legacy
    // `graded_arrow`'s `grade` child; a plain `function` signature
    // (`A ~> B`) has no dimension bracket and defaults to dimension 1.
    // A named grade is a named dimension (`Const "S"`); a numeric
    // bracket is a numeric dimension (`Const n`). `source` stays
    // `Native`: `Foreign` is the host-function boundary, not a
    // dimension.
    let dim = if let Some(signature_idx) = child_at_role_or_pos(node, lang, "signature", 1) {
        match ast.get(AstRef::new(signature_idx)) {
            Some(signature) if matches!(signature.kind.as_str(), "dim_arrow" | "graded_arrow") => {
                // `dim_arrow` declares the `dim` role at position 1; the
                // legacy `graded_arrow` keeps its `grade` child at the
                // same position.
                let dim_idx = child_at_role_or_pos(signature, lang, "dim", 1)
                    .ok_or_else(|| format!("{} signature missing dim child", signature.kind))?;
                lower_dim_term(ast, lang, dim_idx, env, params)?
            }
            _ => Term::constant(Value::from(1)),
        }
    } else {
        Term::constant(Value::from(1))
    };

    Ok(Term::Op {
        dim: Box::new(dim),
        name,
        args: vec![],
        source: ArrSource::Native,
        lit: None,
    })
}

/// Lower a signature `dim`/`grade` child into a dimension `Term` for an
/// `Op`'s `dim` slot (ADR-0120 Phase 3).
///
/// A bare `var` (`~[S]~>` / `~[Addable]~>`) is a *named* dimension —
/// `Const "S"`. A literal (`~[2]~>`) is a numeric dimension —
/// `Const 2`. Anything else is an ordinary dimension expression lowered
/// as a term.
fn lower_dim_term(
    ast: &Ast,
    lang: &Language,
    dim_idx: usize,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let dim_node = ast
        .get(AstRef::new(dim_idx))
        .ok_or_else(|| format!("dim child {} out of bounds", dim_idx))?;
    match dim_node.kind.as_str() {
        // Named dimension label (formerly a "grade name").
        "var" => {
            let name = dim_node
                .payload
                .as_str()
                .ok_or_else(|| format!("dim var at {} has non-string payload", dim_idx))?;
            Ok(Term::named_dim(name))
        }
        // Literal dimension (number, string, …).
        "value" | "lit" => Ok(Term::constant(dim_node.payload.clone())),
        _ => lower_node(ast, lang, dim_idx, env, params),
    }
}

fn lower_handle(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    // `handle <target> with <impl>`: child 0 is the handled expression,
    // child 1 is the handler record.
    let body_idx = child_at_role_or_pos(node, lang, "target", 0)
        .ok_or_else(|| format!("handle at {} missing target child", node_index(node)))?;
    let impl_idx = child_at_role_or_pos(node, lang, "impl", 1)
        .ok_or_else(|| format!("handle at {} missing impl child", node_index(node)))?;
    let target = if let Some(name) = node.payload.get("name").and_then(Value::as_str) {
        name.to_string()
    } else {
        let body = lower_node(ast, lang, body_idx, env, params)?;
        op_target_name(&body).unwrap_or_default().to_string()
    };
    let impl_ = unquote_term(lower_node(ast, lang, impl_idx, env, params)?);
    let body = lower_node(ast, lang, body_idx, env, params)?;
    Ok(Term::handle(body, target, impl_))
}

fn lower_arr(
    _ast: &Ast,
    _lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    _params: &[String],
) -> Result<Term, String> {
    let name = node
        .payload
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("arr at {} missing payload.name", node_index(node)))?
        .to_string();
    if let Some(entry) = env.get(&name) {
        Ok(entry.term.clone())
    } else {
        Ok(Term::Op {
            dim: Term::default_dim(),
            name,
            args: vec![],
            source: ArrSource::Native,
            lit: None,
        })
    }
}

fn lower_graded_arrow(
    _ast: &Ast,
    _lang: &Language,
    _node: &AstNode,
    _env: &HashMap<String, EnvEntry>,
    _params: &[String],
) -> Result<Term, String> {
    // A bare graded-arrow expression is a signature, not a runnable arrow.
    Ok(Term::const_value(Value::Null))
}

fn lower_bind(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let value_idx = child_at_role_or_pos(node, lang, "value", 0)
        .or_else(|| children(node).first().copied())
        .ok_or_else(|| format!("bind at {} missing value child", node_index(node)))?;
    // An expression-position `bind` evaluates to the bound value; the
    // binding itself is dropped. Rebinding an existing binding or a
    // parameter is update-in-place the calculus cannot express.
    if let Some(name) = node.payload.get("name").and_then(Value::as_str) {
        if params.iter().any(|p| p == name) || env.contains_key(name) {
            return Err(format!(
                "lower: `{name}` binds an existing binding (update-in-place is not expressible)"
            ));
        }
    }
    lower_node(ast, lang, value_idx, env, params)
}

fn lower_if(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let cond_idx = child_at_role_or_pos(node, lang, "condition", 0)
        .ok_or_else(|| format!("if at {} missing condition child", node_index(node)))?;
    let then_idx = child_at_role_or_pos(node, lang, "then", 1)
        .ok_or_else(|| format!("if at {} missing then child", node_index(node)))?;
    let cond = lower_node(ast, lang, cond_idx, env, params)?;
    let then_ = lower_node(ast, lang, then_idx, env, params)?;
    let else_ = match child_at_role_or_pos(node, lang, "else", 2) {
        Some(idx) => lower_node(ast, lang, idx, env, params)?,
        None => Term::const_value(Value::Null),
    };
    // `cond &&& id >>> pair_tag >>> Choice(then, else)`: `pair_tag` turns
    // `[cond, input]` into `left/right(input)`, so the selected branch
    // arrow sees the *original* input — branch bodies may project function
    // parameters (`id`/`proj_child`) like any other position.
    Ok(Term::seq(
        Term::op_native("product", vec![cond, Term::op_native("id", vec![])]),
        Term::seq(
            Term::op_native("pair_tag", vec![]),
            Term::choice(then_, else_),
        ),
    ))
}

fn lower_loop(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let children = children(node);
    if children.len() < 2 {
        return Err(format!(
            "loop at {} needs 2 children (input, step), got {}",
            node_index(node),
            children.len()
        ));
    }
    let input = lower_node(ast, lang, children[0], env, params)?;
    let step = lower_node(ast, lang, children[1], env, params)?;
    if let Term::Op {
        name,
        lit: Some(Value::Array(keys)),
        ..
    } = &step
    {
        if name == "record" && keys.iter().any(|k| k.as_str() == Some("__term__")) {
            // A nested function lowers to a bound-arrow record producer:
            // `__bound__` captures the *enclosing* arrow's input, so the
            // record must be materialized before the loop pairs the input
            // with the feedback state. `product(input, step)` evaluates
            // both on the enclosing input — yielding `[v0, record]` —
            // and each iteration applies the record to the loop's
            // `(in, state)` components, so `__term__` runs on
            // `__bound__ ++ [v0, state]` — the `fold_step` protocol.
            let get_record = Term::seq(
                Term::op_native_lit("elem", vec![], 0),
                Term::op_native_lit("elem", vec![], 1),
            );
            let arg_in = Term::seq(
                Term::op_native_lit("elem", vec![], 0),
                Term::op_native_lit("elem", vec![], 0),
            );
            let arg_st = Term::op_native_lit("elem", vec![], 1);
            let body = Term::app(get_record, vec![arg_in, arg_st]);
            return Ok(Term::seq(
                Term::op_native("product", vec![input, step]),
                Term::loop_feedback(body),
            ));
        }
    }
    Ok(Term::seq(input, Term::loop_feedback(unquote_term(step))))
}

/// `while init { step }` — the state-fold surface (ADR-0052). `eval.while`
/// (`Seq[proj_child 0, Loop[loop_step(proj_child 1)]]`) cannot
/// instantiate: the step placeholder sits in `Loop.body`, a non-input
/// position, so the scheme declines. The lowering spells the `loop_step`
/// protocol out in combinators instead (ADR-0147).
///
/// `Loop`'s body runs on the `[in, st]` pair (a 2-array — `proj_child`
/// projects it): `current = st == null ? in : st`, the step arrow is
/// applied to the state destructured by param name, and `loop_outcome`
/// maps `{state: v}` → right(v) / `{out: v}` → left(v) — exactly
/// `loop_step_outcome`.
///
/// A step that *captures* enclosing parameters (`p` in
/// `drop_where := (xs, p) ~> while .. { (xs, acc) ~> .. p(x) .. }`)
/// lowers to a bound-record producer whose `__bound__` projects the
/// *enclosing* input — so it is hoisted out of the loop via
/// `product(init, step)` (the `lower_loop` precedent) and the body
/// applies the materialized record instead.
fn lower_while(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let init_idx = child_at_role_or_pos(node, lang, "init", 0)
        .or_else(|| children(node).first().copied())
        .ok_or_else(|| format!("while at {} missing init child", node_index(node)))?;
    let step_idx = child_at_role_or_pos(node, lang, "step", 1)
        .or_else(|| children(node).get(1).copied())
        .ok_or_else(|| format!("while at {} missing step child", node_index(node)))?;
    let step_node = ast
        .get(AstRef::new(step_idx))
        .ok_or_else(|| format!("while step {} out of bounds", step_idx))?;
    if step_node.kind != "function" {
        return Err(format!(
            "lower: `while` step must be a function literal, got `{}`",
            step_node.kind
        ));
    }
    let step_params = function_params(ast, lang, step_node)?;
    let init_t = lower_node(ast, lang, init_idx, env, params)?;
    let step_t = lower_node(ast, lang, step_idx, env, params)?;

    let proj = |i: i64| Term::op_native_lit("elem", vec![], i);
    let id = Term::op_native("id", vec![]);
    let null = Term::const_value(Value::Null);
    // `pair -> (st == null)` — the Loop body's second component.
    let is_st_null = Term::op_native("eq", vec![proj(1), null.clone()]);
    // A field projection on the `Seq`-preceded input: `seq(t, field_access
    // p)` evaluates `t` then reads field `p` of its result.
    let field = |t: Term, name: &str| {
        Term::seq(
            t,
            Term::op_native_lit("field_access", vec![id.clone()], Value::from(name)),
        )
    };

    // Bound-arrow record producer (`{__term__, __bound__}`) — the step
    // captured enclosing parameters, so it must be materialized on the
    // *outer* input before the loop starts (`product(init, step)` → the
    // Loop input is `[init_v, rec]`).
    let captures = matches!(
        &step_t,
        Term::Op { name, lit: Some(Value::Array(keys)), .. }
            if name == "record" && keys.iter().any(|k| k.as_str() == Some("__term__"))
    );

    if captures {
        // Loop input = `[init_v, rec]`; body input = `[[init_v, rec], st]`.
        let rec = Term::seq(proj(0), proj(1));
        let init_v = Term::seq(proj(0), proj(0));
        let cur = Term::seq(
            Term::op_native("product", vec![is_st_null, id.clone()]),
            Term::seq(
                Term::op_native("pair_tag", vec![]),
                Term::choice(init_v, proj(1)),
            ),
        );
        // App args: one per own param — `cur` destructured by name for
        // multi-param, `cur` itself for a single param (the flat `__bound__
        // ++ args` convention on both apply paths).
        let arg_terms: Vec<Term> = if step_params.len() > 1 {
            step_params
                .iter()
                .map(|p| field(cur.clone(), p))
                .collect()
        } else {
            vec![cur]
        };
        let body = Term::seq(
            Term::app(rec, arg_terms),
            Term::op_native("loop_outcome", vec![]),
        );
        return Ok(Term::seq(
            Term::op_native("product", vec![init_t, step_t]),
            Term::loop_feedback(body),
        ));
    }

    // Non-capturing: unquote the step and run its body on the packed
    // argument tuple.
    let step_body = unquote_term(step_t);
    let cur = Term::seq(
        Term::op_native("product", vec![is_st_null, id.clone()]),
        Term::seq(
            Term::op_native("pair_tag", vec![]),
            Term::choice(proj(0), proj(1)),
        ),
    );
    let pack = if step_params.len() > 1 {
        Term::op_native(
            "array",
            step_params
                .iter()
                .map(|p| {
                    Term::op_native_lit(
                        "field_access",
                        vec![id.clone()],
                        Value::from(p.as_str()),
                    )
                })
                .collect(),
        )
    } else {
        id
    };
    let body = Term::seq(
        cur,
        Term::seq(
            pack,
            Term::seq(step_body, Term::op_native("loop_outcome", vec![])),
        ),
    );
    Ok(Term::seq(init_t, Term::loop_feedback(body)))
}

/// `cond ? then : else` — the same `product`/`pair_tag` shape as
/// `lower_if` (ADR-0147): `pair_tag` turns `[cond_v, input]` into
/// `Either input`, so the selected arm still sees the node input.
fn lower_ternary(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let ch = children(node);
    if ch.len() < 3 {
        return Err(format!(
            "ternary at {} needs 3 children, got {}",
            node_index(node),
            ch.len()
        ));
    }
    let cond = lower_node(ast, lang, ch[0], env, params)?;
    let then_ = lower_node(ast, lang, ch[1], env, params)?;
    let else_ = lower_node(ast, lang, ch[2], env, params)?;
    Ok(Term::seq(
        Term::op_native("product", vec![cond, Term::op_native("id", vec![])]),
        Term::seq(
            Term::op_native("pair_tag", vec![]),
            Term::choice(then_, else_),
        ),
    ))
}

/// `a ?? b` — left when `a` is null, `a` otherwise. `Seq[a, eq(id, null)]`
/// computes the discriminant while `id` keeps the input for the arms
/// (each arm re-evaluates its child, matching the interpreter's
/// `proj_child` semantics).
fn lower_nullish(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let ch = children(node);
    if ch.len() < 2 {
        return Err(format!(
            "nullish at {} needs 2 children, got {}",
            node_index(node),
            ch.len()
        ));
    }
    let a = lower_node(ast, lang, ch[0], env, params)?;
    let b = lower_node(ast, lang, ch[1], env, params)?;
    let is_null = Term::seq(
        a.clone(),
        Term::op_native(
            "eq",
            vec![Term::op_native("id", vec![]), Term::const_value(Value::Null)],
        ),
    );
    Ok(Term::seq(
        Term::op_native("product", vec![is_null, Term::op_native("id", vec![])]),
        Term::seq(
            Term::op_native("pair_tag", vec![]),
            Term::choice(b, a),
        ),
    ))
}

/// `match scrut { pat: body, … }` — compile the `match_shell` walk to
/// combinators (ADR-0147).
///
/// `Seq[product(scrut, id), D]` carries `[scrut_value, input]` (a 2-array
/// — `proj_child` projects it) into the dispatch. Each arm becomes
/// `Seq[product(Seq[p0, pred], id), pair_tag, Choice[body, next]]`:
/// `pair_tag` turns `[pred, pair]` into `Either pair`, so the arm body —
/// like every lowered child — still sees `[sv, in]`, the node input
/// preserved. A pattern whose predicate is the *negation* of a cheap test
/// (e.g. `[h, …t]` ⇔ non-empty) emits `Choice[next, body]` with the
/// empty-test as the discriminant. The final fallback is
/// `Op{match_fail}`, an error op mirroring `match_shell`'s
/// "no arm matched".
///
/// Arm bodies run under `params = []` with `env` rebased: enclosing
/// params project `snd` (`Seq[proj_child 1, proj_i]` — the `p1` rebase),
/// pattern binders project `fst` (`Seq[proj_child 0, <path>]`).
/// [`compile_pattern`] covers the `try_pattern` vocabulary — leaf
/// binders/wildcards/globals, literals, nested `array`/`object`
/// destructuring with `...rest`, `call` tag patterns, literal-eval
/// expression patterns, and statically-resolvable computed object keys:
/// predicates and binder paths are evaluated on the `[sv, in]` dispatch
/// pair, so `pat_env`'s `Seq[proj(1), T]` re-anchored bindings let a
/// pattern expression read the ambient env at any depth.
fn lower_match(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    // children = [scrutinee, (motive, optional), block]; the block's
    // children are `arm` nodes of [pattern, body].
    let ch = children(node);
    let scrut_idx = child_at_role_or_pos(node, lang, "scrutinee", 0)
        .or_else(|| ch.first().copied())
        .ok_or_else(|| format!("match at {} missing scrutinee", node_index(node)))?;
    let block_idx = child_at_role_or_pos(node, lang, "block", ch.len() - 1)
        .filter(|&b| b != scrut_idx)
        .ok_or_else(|| format!("match at {} missing block", node_index(node)))?;
    let block = ast
        .get(AstRef::new(block_idx))
        .ok_or_else(|| format!("match block {} out of bounds", block_idx))?;
    let arm_indices: Vec<usize> = children(block).to_vec();
    if arm_indices.is_empty() {
        return Err(format!("match at {} has no arms", node_index(node)));
    }

    let scrut_t = lower_node(ast, lang, scrut_idx, env, params)?;
    let proj = |i: i64| Term::op_native_lit("elem", vec![], i);
    let id = Term::op_native("id", vec![]);

    // Rebasing for arm bodies: env bindings were lowered to consume the
    // node's input `in`, but the dispatch body input is `[sv, in]`, so
    // every env term is re-anchored through `proj(1)` (`in`) — a binding
    // that references the input (e.g. `y :: xs`) would otherwise read the
    // pair instead of `in`. Enclosing param `params[i]` at position i
    // then reads `snd` of `[sv, in]` (the `p1` rebase); arm bodies see
    // `params = []` so only these env entries resolve their names.
    let mut arm_env = env.clone();
    for entry in arm_env.values_mut() {
        entry.term = Term::seq(proj(1), entry.term.clone());
    }
    for (i, name) in params.iter().enumerate() {
        arm_env.insert(
            name.clone(),
            EnvEntry {
                term: Term::seq(
                    proj(1),
                    if params.len() == 1 {
                        id.clone()
                    } else {
                        proj(i as i64)
                    },
                ),
                arity: 1,
                arrow_value: false,
            },
        );
    }

    // Collect arms; richer patterns decline the whole node.
    struct Arm {
        /// Discriminant term evaluated on the `[sv, in]` dispatch pair.
        pred: Term,
        /// Pattern-bound names → terms evaluated on `[sv, in]`.
        binds: Vec<(String, Term)>,
        body_idx: usize,
    }
    let mut arms: Vec<Arm> = Vec::new();
    for &arm_idx in &arm_indices {
        let arm = ast
            .get(AstRef::new(arm_idx))
            .ok_or_else(|| format!("match arm {} out of bounds", arm_idx))?;
        let ach = children(arm);
        let pat_idx = child_at_role_or_pos(arm, lang, "pattern", 0)
            .or_else(|| ach.first().copied())
            .ok_or_else(|| format!("match arm {} missing pattern", arm_idx))?;
        let body_idx = child_at_role_or_pos(arm, lang, "body", 1)
            .or_else(|| ach.get(1).copied())
            .ok_or_else(|| format!("match arm {} missing body", arm_idx))?;
        let (pred, binds) = compile_pattern(ast, lang, pat_idx, node, &arm_env)?;
        arms.push(Arm {
            pred,
            binds,
            body_idx,
        });
    }

    // Arm body env = arm_env + this arm's pattern binders.
    let mut dispatch = Term::op_native("match_fail", vec![]);
    for arm in arms.iter().rev() {
        let mut body_env = arm_env.clone();
        for (name, path) in &arm.binds {
            body_env.insert(
                name.clone(),
                EnvEntry {
                    term: path.clone(),
                    arity: 1,
                    arrow_value: false,
                },
            );
        }
        let body = lower_node(ast, lang, arm.body_idx, &body_env, &[])?;
        let decision = Term::seq(
            Term::op_native("product", vec![arm.pred.clone(), id.clone()]),
            Term::op_native("pair_tag", vec![]),
        );
        dispatch = Term::seq(decision, Term::choice(body, dispatch));
    }
    Ok(Term::seq(
        Term::op_native("product", vec![scrut_t, Term::op_native("id", vec![])]),
        dispatch,
    ))
}

/// Compile one `match` arm pattern to a discriminant term plus the
/// binders it introduces — **both evaluated on the `[v, in]` dispatch
/// pair**, where `v` is the (sub-)scrutinee value and `in` is the match
/// node's input. `pat_env` is [`lower_match`]'s rebased env
/// (`Seq[proj(1), T]` entries consuming that pair), so a literal-eval
/// expression lowered under it reads the ambient env — enclosing
/// params included — at any pattern depth.
///
/// The dispatch mirrors `pattern_match::try_pattern` (ADR-0061/0081):
/// leaf → sequence → entries → tag → literal-eval. Supported shapes:
/// - `var` leaf — `_`/`_*` wildcard (but not `__fresh_` desugar binders,
///   which bind), a bare binder (binds `v`), or a `Language::globals`
///   name → literal equality;
/// - `value`/`lit`/`number`/`string`/`bool` literal payloads → `eq`;
/// - `array` (`pattern_role: sequence`) — positional *sub-patterns*
///   (recursively: literals, nested arrays/objects/tags, binders,
///   literal-eval expressions) plus an optional `rest` binder; the
///   length check guards element projections via short-circuit `and`
///   chains so a non-match falls through instead of erroring;
/// - `object` (`pattern_role: entries`) — `entry` children whose keys
///   resolve *statically*: a declared literal key
///   (`literal_children`), or a computed key (`[expr]`/`(expr)` —
///   desugared to `paren`) that lowers to a compile-time constant such
///   as an env binding `k :: "a"`; nested sub-patterns per key,
///   `...rest` via `obj_except`; the empty `{}` pattern is the `is_obj`
///   test;
/// - `call` tag patterns `Tag(b0, b1, …)` (ADR-0077) — `has_field
///   "__tag__"` + tag equality; binders take the sorted non-`__tag__`
///   payload values positionally via `tag_fields`;
/// - `paren`/`record`/anything else — literal-eval (`try_pattern`'s
///   tail): the node lowers as an expression under `pat_env` and
///   `eq`-compares against `v`. `record` keeps its legacy
///   `_tag`/`__tag__` scan ahead of literal-eval.
///
/// Anything `lower_node` cannot express declines (`Err`) —
/// `match_shell` covers it via the AST fallback. A computed key that is
/// *not* a compile-time constant (a param, a call, a concat) still
/// declines: `has_field`/`field_access`/`obj_except` literals are
/// static strings, and there is no dynamic-key pattern op.
fn compile_pattern(
    ast: &Ast,
    lang: &Language,
    pat_idx: usize,
    node: &AstNode,
    pat_env: &HashMap<String, EnvEntry>,
) -> Result<(Term, Vec<(String, Term)>), String> {
    let pat = ast
        .get(AstRef::new(pat_idx))
        .ok_or_else(|| format!("match pattern {pat_idx} out of bounds"))?;
    // `[v, in]` accessors: `pv` is the current (sub-)scrutinee value,
    // `pin` the ambient input.
    let pv = || Term::op_native_lit("elem", vec![], 0);
    let pin = || Term::op_native_lit("elem", vec![], 1);
    let id = Term::op_native("id", vec![]);
    let true_t = || Term::const_value(Value::from(true));
    // `eq`/`field_access` evaluate their args on the input, so a
    // pair-level use is `op[pv, …]`; `has_field`/`is_obj`/`arr_len`/
    // `arr_drop`/`tag_fields`/`obj_except` read the input itself (their
    // `args` are inert), so they ride behind `Seq[pv, …]` (`on_v`).
    let eq_lit = |v: Value| Term::op_native("eq", vec![pv(), Term::const_value(v)]);
    let field = |name: &str| Term::op_native_lit("field_access", vec![pv()], Value::from(name));
    let elem_at = |i: usize| Term::op_native_lit("field_access", vec![pv()], Value::from(i as i64));
    let on_v = |t: Term| Term::seq(pv(), t);
    let has_field = |name: &str| {
        on_v(Term::op_native_lit(
            "has_field",
            vec![id.clone()],
            Value::from(name),
        ))
    };
    // Descend into a sub-pattern: `t` yields the sub-scrutinee on the
    // pair; `product(t, pin)` re-pairs it with `in` → `[v', in]`.
    let sub_pair = |t: Term| Term::op_native("product", vec![t, pin()]);
    // `try_pattern`'s tail: evaluate the node as an expression in the
    // ambient env and `eq`-compare against `v` — honest for anything
    // `lower_node` covers (`paren` bodies, env-bound vars, calls,
    // field accesses, object/array literals).
    let lit_eval = |expr_idx: usize| -> Result<(Term, Vec<(String, Term)>), String> {
        let e = lower_node(ast, lang, expr_idx, pat_env, &[])?;
        Ok((Term::op_native("eq", vec![pv(), e]), vec![]))
    };
    let role = |r: &str, kind: &str| {
        lang.pattern_role_is(&pat.kind, r)
            .unwrap_or(pat.kind == kind)
    };

    // 1. Leaf (`pattern_role: leaf`, canonical `var`) — wildcard, binder,
    //    or global value pattern.
    if role("leaf", "var") {
        return match pat.payload.as_str() {
            Some(name) if name.is_empty() || is_wildcard_name(name) => Ok((true_t(), vec![])),
            Some(name) => {
                if let Some(global) = lang.globals.get(name) {
                    return Ok((eq_lit(global.clone()), vec![]));
                }
                // Bare `var` binds the whole scrutinee; an env-bound name
                // still binds (binders shadow — `try_pattern` checks only
                // `ctx.global_value`).
                Ok((true_t(), vec![(name.to_string(), pv())]))
            }
            // A leaf without a string payload (e.g. `arr`) is a literal
            // in `try_pattern` — evaluate it in the ambient env and
            // `eq`-compare.
            None => lit_eval(pat_idx),
        };
    }

    // 2. Sequence (`pattern_role: sequence`, canonical `array`) —
    //    positional sub-patterns + optional `rest` tail binder.
    if role("sequence", "array") {
        let mut positional: Vec<usize> = Vec::new();
        let mut rest: Option<usize> = None;
        for &c in children(pat).iter() {
            let cn = ast
                .get(AstRef::new(c))
                .ok_or_else(|| format!("array pattern child {c} out of bounds"))?;
            let is_rest = lang
                .pattern_role_is(&cn.kind, "rest")
                .unwrap_or(cn.kind == "rest");
            if is_rest {
                if rest.is_some() {
                    return Err(format!(
                        "match at {}: multiple rest patterns",
                        node_index(node)
                    ));
                }
                rest = Some(c);
            } else {
                positional.push(c);
            }
        }
        let n = positional.len();
        // `arr_len` returns -1 on non-arrays, so a non-array scrutinee
        // falls through (matching `try_pattern`'s false) instead of
        // erroring mid-dispatch. `[]` uses structural equality so a
        // non-array also falls through.
        let mut pred = if n == 0 && rest.is_none() {
            eq_lit(Value::Array(vec![]))
        } else {
            Term::op_native(
                if rest.is_some() { "gte" } else { "eq" },
                vec![
                    on_v(Term::op_native("arr_len", vec![id.clone()])),
                    Term::const_value(Value::from(n as i64)),
                ],
            )
        };
        // Positional sub-patterns recurse on `[sv[i], in]` behind the
        // length check (short-circuit), so a too-short scrutinee cannot
        // error on `field_access i`.
        let mut binds: Vec<(String, Term)> = Vec::new();
        for (i, &p) in positional.iter().enumerate() {
            let (sub_pred, sub_binds) = compile_pattern(ast, lang, p, node, pat_env)?;
            if !is_const_true(&sub_pred) {
                pred = and_short(pred, Term::seq(sub_pair(elem_at(i)), sub_pred));
            }
            for (name, path) in sub_binds {
                binds.push((name, Term::seq(sub_pair(elem_at(i)), path)));
            }
        }
        if let Some(r) = rest {
            if let Some(name) = rest_binder_name(ast, lang, r, node)? {
                binds.push((
                    name,
                    on_v(Term::op_native_lit(
                        "arr_drop",
                        vec![id.clone()],
                        Value::from(n as i64),
                    )),
                ));
            }
        }
        return Ok((pred, binds));
    }

    // 3. Entries (`pattern_role: entries`, canonical `object`) —
    //    `{k0: p0, k1: p1, ..., ...rest}`. Each entry's key must resolve
    //    statically (a declared `literal_children` key, or a computed
    //    key that is a compile-time constant); its value is a
    //    sub-pattern matched against `sv.k`. `has_field k` is the shape
    //    guard: non-objects and missing keys fall through.
    if role("entries", "object") {
        let mut rest: Option<usize> = None;
        let mut entry_indices: Vec<usize> = Vec::new();
        for &c in children(pat).iter() {
            let en = ast
                .get(AstRef::new(c))
                .ok_or_else(|| format!("object pattern child {c} out of bounds"))?;
            let is_rest = lang
                .pattern_role_is(&en.kind, "rest")
                .unwrap_or(en.kind == "rest");
            let is_entry = lang
                .pattern_role_is(&en.kind, "entry")
                .unwrap_or(en.kind == "entry");
            if is_rest {
                if rest.is_some() {
                    return Err(format!(
                        "match at {}: multiple rest patterns",
                        node_index(node)
                    ));
                }
                rest = Some(c);
            } else if is_entry {
                entry_indices.push(c);
            }
            // Other children are ignored, mirroring `try_object_pattern`.
        }

        let mut binds: Vec<(String, Term)> = Vec::new();
        let mut keys: Vec<String> = Vec::new();
        let mut pred: Option<Term> = None;
        for &e in &entry_indices {
            let en = ast
                .get(AstRef::new(e))
                .ok_or_else(|| format!("object entry {e} out of bounds"))?;
            let key_idx = child_at_role_or_pos(en, lang, "key", 0)
                .ok_or_else(|| format!("object entry {e} missing key child"))?;
            let val_idx = child_at_role_or_pos(en, lang, "value", 1)
                .ok_or_else(|| format!("object entry {e} missing value child"))?;
            let key_node = ast
                .get(AstRef::new(key_idx))
                .ok_or_else(|| format!("entry key {key_idx} out of bounds"))?;
            // `literal_children` declares which key kinds carry the field
            // name (canonical: `var`/`string`); anything else is a
            // computed key the interpreter evaluates in the ambient env.
            // It lowers statically when the key expression is a
            // compile-time constant (a literal payload, a `paren` around
            // one, or an env-bound constant like `k :: "a"`); a
            // genuinely runtime key (a param, a call) yields no static
            // string for `has_field`/`obj_except` — decline.
            let is_literal = lang
                .literal_child(&en.kind, "key", &key_node.kind)
                .unwrap_or(key_node.kind == "var");
            let key = if is_literal {
                key_node.payload.as_str().unwrap_or("").to_string()
            } else {
                static_pattern_key(ast, lang, key_idx, pat_env).ok_or_else(|| {
                    format!(
                        "match at {}: computed object-pattern key kind `{}` is not a compile-time constant",
                        node_index(node),
                        key_node.kind
                    )
                })?
            };
            keys.push(key.clone());
            let (sub_pred, sub_binds) = compile_pattern(ast, lang, val_idx, node, pat_env)?;
            // `has_field k` first — it is the `sv is Obj && k present`
            // guard — then the sub-predicate on `sv.k`, short-circuited
            // so a missing key cannot error on `field_access`.
            let entry_pred = if is_const_true(&sub_pred) {
                has_field(&key)
            } else {
                and_short(has_field(&key), Term::seq(sub_pair(field(&key)), sub_pred))
            };
            pred = Some(match pred {
                Some(p) => and_short(p, entry_pred),
                None => entry_pred,
            });
            for (name, path) in sub_binds {
                binds.push((name, Term::seq(sub_pair(field(&key)), path)));
            }
        }
        // `{}` — any object matches (`try_object_pattern` checks only
        // that the scrutinee is an Obj).
        let pred = pred.unwrap_or_else(|| on_v(Term::op_native("is_obj", vec![id.clone()])));
        if let Some(r) = rest {
            if let Some(name) = rest_binder_name(ast, lang, r, node)? {
                let key_lits: Vec<Value> = keys.iter().map(|k| Value::from(k.as_str())).collect();
                binds.push((
                    name,
                    on_v(Term::op_native_lit(
                        "obj_except",
                        vec![id.clone()],
                        Value::Array(key_lits),
                    )),
                ));
            }
        }
        return Ok((pred, binds));
    }

    // 4. Tag pattern (`callee`-roled shape, canonical `call`;
    //    ADR-0077) — `Tag(b0, …)`: the scrutinee must be an object
    //    carrying `__tag__ == Tag`; binders take the sorted
    //    non-`__tag__` payload values positionally.
    let is_tag_pattern =
        matches!(lang.child_role_index(&pat.kind, "callee"), Ok(Some(_))) || pat.kind == "call";
    if is_tag_pattern {
        let ch = children(pat);
        let callee_idx = child_at_role_or_pos(pat, lang, "callee", 0)
            .ok_or_else(|| format!("match at {}: tag pattern missing callee", node_index(node)))?;
        let callee = ast
            .get(AstRef::new(callee_idx))
            .ok_or_else(|| format!("tag callee {callee_idx} out of bounds"))?;
        let tag_name = callee.payload.as_str().ok_or_else(|| {
            format!(
                "match at {}: tag pattern callee is not a var",
                node_index(node)
            )
        })?;
        // `has_field "__tag__"` covers both the non-object and the
        // untagged-object miss — `try_tag_pattern`'s `Ok(false)`.
        let pred = and_short(
            has_field("__tag__"),
            Term::op_native(
                "eq",
                vec![field("__tag__"), Term::const_value(Value::from(tag_name))],
            ),
        );
        // Binder positions start at the `args` role (index 1 on
        // canonical `call`); every position advances the payload index,
        // while non-`var` and wildcard args bind nothing — mirroring
        // `try_tag_pattern`'s positional `payload_fields` loop.
        let args_start = lang
            .child_role_index(&pat.kind, "args")
            .ok()
            .flatten()
            .unwrap_or(1);
        let mut binds: Vec<(String, Term)> = Vec::new();
        for (i, &b) in ch.iter().skip(args_start).enumerate() {
            let bn = ast
                .get(AstRef::new(b))
                .ok_or_else(|| format!("tag binder {b} out of bounds"))?;
            let is_leaf = lang
                .pattern_role_is(&bn.kind, "leaf")
                .unwrap_or(bn.kind == "var");
            if !is_leaf {
                continue;
            }
            let Some(name) = bn.payload.as_str() else {
                continue;
            };
            if is_wildcard_name(name) {
                continue;
            }
            binds.push((
                name.to_string(),
                Term::seq(
                    on_v(Term::op_native("tag_fields", vec![id.clone()])),
                    Term::op_native_lit("field_access", vec![id.clone()], Value::from(i as i64)),
                ),
            ));
        }
        return Ok((pred, binds));
    }

    match pat.kind.as_str() {
        // Literal values — `value`/`lit`/`number`/`string`/`bool` nodes
        // carry their payload directly.
        "value" | "lit" | "number" | "string" | "bool" => Ok((eq_lit(pat.payload.clone()), vec![])),
        // `paren` is a transparent wrapper — literal-eval of the inner
        // expression (`lower_node` unwraps the `body` child), which
        // covers literals, env-bound vars, and arbitrary expressions.
        "paren" => lit_eval(pat_idx),
        // `_tag`-keyed record patterns (`{_tag: "Some", value: x}` — the
        // `nullish`/`opt-chain` desugar shape) and `__tag__` sums on the
        // `record` kind (which has no `pattern_role` — it is a literal
        // in `try_pattern`, so only the legacy `_tag` scan applies).
        "record" => {
            let mut tag_child: Option<usize> = None;
            let mut value_child: Option<usize> = None;
            for c in children(pat).iter().copied() {
                let Some(en) = ast.get(AstRef::new(c)) else {
                    continue;
                };
                let ech = children(en);
                let key_idx = ech.first().copied();
                let val_idx = ech.get(1).copied();
                let key = key_idx
                    .and_then(|k| ast.get(AstRef::new(k)))
                    .and_then(|kn| {
                        kn.payload.as_str().map(String::from).or_else(|| {
                            kn.payload
                                .get("value")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                        })
                    })
                    .unwrap_or_else(|| en.kind.clone());
                if key == "_tag" || key == "__tag__" {
                    tag_child = val_idx.or(key_idx);
                } else if key == "value" {
                    value_child = val_idx.or(key_idx);
                }
            }
            let (tc, vc) = (tag_child, value_child);
            let Some(tc) = tc else {
                // No `_tag`/`__tag__` key — the interpreter treats a
                // `record` as a literal pattern, so evaluate it and
                // `eq`-compare (declines if it cannot lower).
                return lit_eval(pat_idx).map_err(|_| {
                    format!(
                        "match at {}: record pattern without a `_tag`/`__tag__` key",
                        node_index(node)
                    )
                });
            };
            let tag_node = ast
                .get(AstRef::new(tc))
                .ok_or_else(|| format!("tag pattern {tc} out of bounds"))?;
            let tag_name = tag_node
                .payload
                .as_str()
                .or_else(|| tag_node.payload.get("value").and_then(|v| v.as_str()))
                .ok_or_else(|| {
                    format!(
                        "match at {}: tag pattern value not literal",
                        node_index(node)
                    )
                })?;
            let pred = and_short(
                has_field("_tag"),
                Term::op_native(
                    "eq",
                    vec![field("_tag"), Term::const_value(Value::from(tag_name))],
                ),
            );
            let binds = vc
                .and_then(|v| ast.get(AstRef::new(v)))
                .filter(|vn| vn.kind == "var")
                .and_then(|vn| vn.payload.as_str().map(String::from))
                .filter(|name| !name.is_empty() && !is_wildcard_name(name))
                .map(|name| vec![(name, field("value"))])
                .unwrap_or_default();
            Ok((pred, binds))
        }
        // `try_pattern`'s literal-eval tail — any other expression node
        // (binary ops, field accesses, object/array literals, calls)
        // evaluates in the ambient env and `eq`-compares.
        other => lit_eval(pat_idx).map_err(|e| {
            format!(
                "match at {}: unsupported pattern kind `{other}` ({e})",
                node_index(node)
            )
        }),
    }
}

/// A computed object-pattern key's string when it resolves at compile
/// time: lower the key expression against the pattern env and take its
/// static value ([`static_term_value`]). Mirrors `try_object_pattern`'s
/// `key_val.as_str().unwrap_or("")` — a non-string static value keys
/// the empty field.
fn static_pattern_key(
    ast: &Ast,
    lang: &Language,
    key_idx: usize,
    pat_env: &HashMap<String, EnvEntry>,
) -> Option<String> {
    let term = lower_node(ast, lang, key_idx, pat_env, &[]).ok()?;
    let value = static_term_value(&term)?;
    Some(value.as_str().unwrap_or("").to_string())
}

/// The value a term produces regardless of input — the compile-time
/// constants (`Const`, `Arr`) plus one layer of the
/// `Seq[proj(0|1)|id, const]` re-anchor [`lower_match`] wraps env
/// bindings in (that projection cannot fail on the `[v, in]` dispatch
/// pair). Anything deeper — a projection of `in`, an op — may read the
/// input or error at runtime, so it is *not* static.
fn static_term_value(t: &Term) -> Option<Value> {
    match t {
        Term::Const { value } | Term::Arr { value, .. } => Some(value.clone()),
        Term::Seq { fst, snd, .. } => {
            let safe_proj = matches!(
                fst.as_ref(),
                Term::Op { name, lit, .. }
                    if name == "id"
                        || (name == "elem"
                            && matches!(lit, Some(Value::Number(n)) if n.as_u64() <= Some(1)))
            );
            if !safe_proj {
                return None;
            }
            match snd.as_ref() {
                Term::Const { value } | Term::Arr { value, .. } => Some(value.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A `...name` rest binder's name — mirrors
/// `pattern_match::rest_binder_name`: the `subject`-roled child (index 0)
/// must be a leaf (`var`); a wildcard subject binds nothing, any other
/// shape is a hard error the AST fallback surfaces.
fn rest_binder_name(
    ast: &Ast,
    lang: &Language,
    rest_idx: usize,
    node: &AstNode,
) -> Result<Option<String>, String> {
    let rest_node = ast
        .get(AstRef::new(rest_idx))
        .ok_or_else(|| format!("rest pattern {rest_idx} out of bounds"))?;
    let subject_idx = child_at_role_or_pos(rest_node, lang, "subject", 0)
        .or_else(|| children(rest_node).first().copied())
        .ok_or_else(|| {
            format!(
                "match at {}: rest pattern missing subject",
                node_index(node)
            )
        })?;
    let var_node = ast
        .get(AstRef::new(subject_idx))
        .ok_or_else(|| format!("rest binder {subject_idx} out of bounds"))?;
    let is_leaf = lang
        .pattern_role_is(&var_node.kind, "leaf")
        .unwrap_or(var_node.kind == "var");
    if is_leaf {
        if let Some(name) = var_node.payload.as_str() {
            if is_wildcard_name(name) {
                return Ok(None);
            }
            return Ok(Some(name.to_string()));
        }
    }
    Err(format!(
        "match at {}: rest pattern binder is not a var",
        node_index(node)
    ))
}

/// Whether `name` in pattern position binds nothing — `_`/`_*`, except
/// `__fresh_`-prefixed names, which the transform engine mints as
/// desugar-internal binders (e.g. `nullish`'s `__fresh_rec_v`) and which
/// *do* bind. Mirrors `pattern_match::is_wildcard_name`.
fn is_wildcard_name(name: &str) -> bool {
    name.starts_with('_') && !name.starts_with("__fresh_")
}

/// Whether `t` is the constant `true` — used to elide trivial
/// sub-predicates from `and_short` chains (a bare binder/wildcard
/// contributes no discriminant).
fn is_const_true(t: &Term) -> bool {
    matches!(
        t,
        Term::Const {
            value: Value::Bool(true)
        } | Term::Arr {
            value: Value::Bool(true),
            ..
        }
    )
}

/// `a && b` as a *short-circuit* on the same input — `b` runs only when
/// `a` is truthy (`pair_tag` hands the `Choice` arms the input, so `b`
/// sees it too). Pattern guards use this to run index/field projections
/// only behind the shape check that licenses them, so a non-matching
/// scrutinee falls through to the next arm instead of erroring.
fn and_short(a: Term, b: Term) -> Term {
    Term::seq(
        Term::op_native("product", vec![a, Term::op_native("id", vec![])]),
        Term::seq(
            Term::op_native("pair_tag", vec![]),
            Term::choice(b, Term::const_value(Value::from(false))),
        ),
    )
}

fn lower_field_access(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let target_idx = child_at_role_or_pos(node, lang, "target", 0)
        .ok_or_else(|| format!("field-access at {} missing target child", node_index(node)))?;
    let target = lower_node(ast, lang, target_idx, env, params)?;

    let field = if let Some(f) = node.payload.get("field").and_then(Value::as_str) {
        Value::String(f.to_string())
    } else {
        let field_idx = child_at_role_or_pos(node, lang, "field", 1)
            .ok_or_else(|| format!("field-access at {} missing field child", node_index(node)))?;
        let field_node = ast
            .get(AstRef::new(field_idx))
            .ok_or_else(|| format!("field child {} out of bounds", field_idx))?;
        if field_node.kind == "var" {
            Value::String(
                field_node
                    .payload
                    .as_str()
                    .ok_or_else(|| format!("field at {} is not a string", field_idx))?
                    .to_string(),
            )
        } else {
            field_node.payload.clone()
        }
    };

    Ok(Term::op_native_lit("field_access", vec![target], field))
}

fn lower_two_children(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<(Term, Term), String> {
    let children = children(node);
    if children.len() < 2 {
        return Err(format!(
            "{} at {} needs 2 children, got {}",
            node.kind,
            node_index(node),
            children.len()
        ));
    }
    let l = lower_node(ast, lang, children[0], env, params)?;
    let r = lower_node(ast, lang, children[1], env, params)?;
    Ok((l, r))
}

fn function_params(ast: &Ast, lang: &Language, node: &AstNode) -> Result<Vec<String>, String> {
    // Hand-built ASTs may store parameter names directly in the payload.
    if let Some(params) = node.payload.get("params").and_then(Value::as_array) {
        let names: Vec<String> = params
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        return Ok(names);
    }

    // Prefer the `params` child role; fall back to a single payload index.
    let params_idx = child_at_role_or_pos(node, lang, "params", 0)
        .or_else(|| {
            node.payload
                .get("params")
                .and_then(Value::as_u64)
                .map(|u| u as usize)
        })
        .ok_or_else(|| {
            format!(
                "function at {} missing params child/payload",
                node_index(node)
            )
        })?;

    let params_node = ast
        .get(AstRef::new(params_idx))
        .ok_or_else(|| format!("function params index {} out of bounds", params_idx))?;

    match params_node.kind.as_str() {
        "var" => {
            let name = params_node
                .payload
                .as_str()
                .ok_or_else(|| format!("function param at {} is not a string", params_idx))?
                .to_string();
            Ok(vec![name])
        }
        "paren" | "array" => {
            let mut names = Vec::new();
            for &child_idx in &children(params_node) {
                let child = ast
                    .get(AstRef::new(child_idx))
                    .ok_or_else(|| format!("param child {} out of bounds", child_idx))?;
                if child.kind == "var" {
                    names.push(
                        child
                            .payload
                            .as_str()
                            .ok_or_else(|| format!("param at {} is not a string", child_idx))?
                            .to_string(),
                    );
                } else if child.kind == "type-annot" {
                    // `(x: T)` — the annotation is compile-time
                    // metadata; the param binds the subject var.
                    let subject_idx =
                        child_at_role_or_pos(child, lang, "subject", 0).ok_or_else(|| {
                            format!("type-annot param at {} missing subject", child_idx)
                        })?;
                    let subject = ast
                        .get(AstRef::new(subject_idx))
                        .ok_or_else(|| format!("param subject {} out of bounds", subject_idx))?;
                    if subject.kind != "var" {
                        return Err(format!(
                            "unsupported parameter kind `type-annot({})` at {}",
                            subject.kind, child_idx
                        ));
                    }
                    names.push(
                        subject
                            .payload
                            .as_str()
                            .ok_or_else(|| format!("param at {} is not a string", subject_idx))?
                            .to_string(),
                    );
                } else {
                    return Err(format!(
                        "unsupported parameter kind `{}` at {}",
                        child.kind, child_idx
                    ));
                }
            }
            Ok(names)
        }
        other => Err(format!(
            "unsupported function params kind `{}` at {}",
            other, params_idx
        )),
    }
}

fn function_body_index(lang: &Language, node: &AstNode) -> Result<usize, String> {
    child_at_role_or_pos(node, lang, "body", 1)
        .or_else(|| {
            node.payload
                .get("body")
                .and_then(Value::as_u64)
                .map(|u| u as usize)
        })
        .ok_or_else(|| {
            format!(
                "function at {} missing body child/payload",
                node_index(node)
            )
        })
}

fn is_op_graded_arrow(ast: &Ast, lang: &Language, value_idx: usize) -> Result<bool, String> {
    let value = ast
        .get(AstRef::new(value_idx))
        .ok_or_else(|| format!("value node {} out of bounds", value_idx))?;
    if value.kind != "graded_arrow" {
        return Ok(false);
    }
    let grade_idx = child_at_role_or_pos(value, lang, "grade", 1)
        .ok_or_else(|| format!("graded_arrow at {} missing grade child", value_idx))?;
    Ok(grade_contains_op(ast, grade_idx))
}

fn op_term_from_graded_arrow(
    ast: &Ast,
    lang: &Language,
    name: &str,
    value_idx: usize,
) -> Result<Term, String> {
    let value = ast
        .get(AstRef::new(value_idx))
        .ok_or_else(|| format!("graded_arrow node {} out of bounds", value_idx))?;
    let grade_idx = child_at_role_or_pos(value, lang, "grade", 1)
        .ok_or_else(|| format!("graded_arrow at {} missing grade child", value_idx))?;
    // The first non-`Op` grade var is a *named* dimension (ADR-0120
    // Phase 3): it rides in the `dim` slot, not `ArrSource::Foreign` —
    // `Foreign` is the host-function boundary, not a dimension.
    let dim = first_non_op_grade(ast, lang, grade_idx)
        .map(Term::named_dim)
        .unwrap_or_else(|| Term::constant(Value::from(1)));
    Ok(Term::Op {
        dim: Box::new(dim),
        name: name.to_string(),
        args: vec![],
        source: ArrSource::Native,
        lit: None,
    })
}

fn grade_contains_op(ast: &Ast, grade_idx: usize) -> bool {
    if let Some(grade) = ast.get(AstRef::new(grade_idx)) {
        match grade.kind.as_str() {
            "var" => grade.payload.as_str() == Some("Op"),
            "product" | "paren" | "array" => children(grade)
                .iter()
                .any(|&child| grade_contains_op(ast, child)),
            _ => false,
        }
    } else {
        false
    }
}

fn first_non_op_grade(ast: &Ast, lang: &Language, grade_idx: usize) -> Option<String> {
    if let Some(grade) = ast.get(AstRef::new(grade_idx)) {
        match grade.kind.as_str() {
            "var" => {
                let name = grade.payload.as_str()?;
                if name != "Op" {
                    Some(name.to_string())
                } else {
                    None
                }
            }
            "product" | "paren" | "array" => {
                for &child in &children(grade) {
                    if let Some(name) = first_non_op_grade(ast, lang, child) {
                        return Some(name);
                    }
                }
                None
            }
            _ => None,
        }
    } else {
        None
    }
}

fn op_target_name(term: &Term) -> Option<&str> {
    match term {
        Term::Op {
            name,
            args,
            lit: None,
            ..
        } if args.is_empty() => Some(name),
        _ => None,
    }
}

fn children(node: &AstNode) -> Vec<usize> {
    node.payload
        .get("children")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|u| u as usize))
                .collect()
        })
        .unwrap_or_default()
}

fn child_at_role_or_pos(node: &AstNode, lang: &Language, role: &str, pos: usize) -> Option<usize> {
    // Arity-aware resolution (`child_role_index_for`): roles declared after
    // an `optional` slot shift when the slot is absent — e.g. `match`'s
    // `block`, `assign`'s trailing `body` (ADR-0081 optional roles).
    let index = lang
        .child_role_index_for(node, role)
        .ok()
        .flatten()
        .unwrap_or(pos);
    node.payload
        .get("children")
        .and_then(Value::as_array)
        .and_then(|arr| arr.get(index))
        .and_then(|v| v.as_u64())
        .map(|u| u as usize)
}

fn node_index(node: &AstNode) -> String {
    // Not an actual index; used only for diagnostics.
    format!("{}", node.kind)
}

/// Quote an arrow `Term` into an arrow *value* (ADR-0120 Phase 1): the
/// term is serialized to JSON inside `Arr.value`, the existing quoting
/// form. `Term::App` deserializes it back when the value is applied.
fn quote_term(term: Term) -> Result<Term, String> {
    let value =
        serde_json::to_value(&term).map_err(|e| format!("quote: serialize term failed: {e}"))?;
    Ok(Term::Arr {
        dim: Term::default_dim(),
        value,
    })
}

/// Unquote an arrow *value* back into its body `Term`: when `term` is an
/// `Arr` whose payload is a serialized `Term`, recover that term;
/// otherwise return `term` unchanged. Used at composition sites (`flow`,
/// `loop`, handler impls) where an arrow — not an arrow value — is needed.
fn unquote_term(term: Term) -> Term {
    if let Term::Arr { value, .. } = &term {
        if let Ok(inner) = serde_json::from_value::<Term>(value.clone()) {
            return inner;
        }
    }
    term
}

/// Whether a bound term denotes a *value* rather than an arrow scheme:
/// `Term::App` evaluates to a bound-arrow record and `Term::Const`/a
/// plain `Term::Arr` evaluate to constants — the `Term::App` callee
/// dispatch must run on the evaluated value, so these stay unquoted.
/// Arrow schemes (`Op`, `Seq`, `Choice`, `Loop`, `First`, `Iter`,
/// `Handle`) take the ambient input and are quoted into arrow values.
fn binding_denotes_value(term: &Term) -> bool {
    match term {
        Term::App { .. } | Term::Const { .. } => true,
        // An `Arr` holding a serialized `Term` is already an arrow
        // *value* (quoted); an `Arr` holding anything else is a
        // constant — evaluate it and let App dispatch on the value.
        Term::Arr { value, .. } => serde_json::from_value::<Term>(value.clone()).is_err(),
        _ => false,
    }
}

/// Ensure `term` denotes an arrow *value* suitable for `Term::App`: an
/// `Arr` whose payload already deserializes to a `Term` is returned as-is
/// (idempotent quoting); any other term is quoted.
fn ensure_quoted(term: &Term) -> Term {
    if let Term::Arr { value, .. } = term {
        if serde_json::from_value::<Term>(value.clone()).is_ok() {
            return term.clone();
        }
    }
    match quote_term(term.clone()) {
        Ok(quoted) => quoted,
        Err(_) => term.clone(),
    }
}

/// Collect every `var`/`arr` name referenced anywhere in the subtree
/// rooted at `idx` — used to decide which enclosing parameters a nested
/// function actually captures (bound-arrow `__bound__` positions).
fn collect_var_names(ast: &Ast, idx: usize, out: &mut std::collections::HashSet<String>) {
    let Some(node) = ast.get(AstRef::new(idx)) else {
        return;
    };
    match node.kind.as_str() {
        "var" => {
            if let Some(name) = node.payload.as_str() {
                out.insert(name.to_string());
            }
        }
        "arr" => {
            if let Some(name) = node.payload.get("name").and_then(Value::as_str) {
                out.insert(name.to_string());
            }
        }
        _ => {}
    }
    for &child in &children(node) {
        collect_var_names(ast, child, out);
    }
}
