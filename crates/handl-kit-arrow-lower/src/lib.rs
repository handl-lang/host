//! Shared canonical AST to arrow-calculus `Term` lowerer (ADR-0100).
//!
//! TECH DEBT (ADR-0105): `lower_node` still `match`es on canonical kind
//! strings (`value`, `var`, `assign`, `bind`, `op`, `flow`, …) to pick the
//! lowering combinator. The data-driven replacement is a `lower` scheme
//! fragment per `AstShapeDef` — the same shape `lang.eval`/`lang.types`
//! already use — so a renamed kind keeps lowering without an engine edit.
//! Until that fragment exists, this `match` is the Arrow dialect's lowering
//! table and new canonical constructs must add an arm here.

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
                    Ok(Term::op_native_lit("proj_child", vec![], pos as i64))
                }
            } else if let Some(entry) = env.get(name) {
                Ok(entry.term.clone())
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
                            Term::op_native_lit("proj_child", vec![], *outer_pos as i64)
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

        "binary.add" => lower_binary(ast, lang, node, env, params, "add"),
        "binary.mul" => lower_binary(ast, lang, node, env, params, "mul"),
        "binary.sub" => lower_binary(ast, lang, node, env, params, "sub"),
        "binary.div" => lower_binary(ast, lang, node, env, params, "div"),

        "product" => lower_pair_combinator(ast, lang, node, env, params, "product"),
        "fanout" => {
            let (l, r) = lower_two_children(ast, lang, node, env, params)?;
            Ok(Term::op_foreign("fn.fanout_fn", vec![l, r]))
        }
        "flow" => {
            // `x >>> f` is sequential composition `seq(x, f)`. Both sides
            // sit in arrow position, so quoted arrow values are unquoted
            // back into their body schemes.
            let (l, r) = lower_two_children(ast, lang, node, env, params)?;
            Ok(Term::seq(unquote_term(l), unquote_term(r)))
        }

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

        "field-access" => lower_field_access(ast, lang, node, env, params),

        "block" => lower_block(ast, lang, node, env, params),

        "assign" => {
            // Let-form `assign(target, value, body)` (3+ children): bind
            // the target name in a private compile-time env and lower the
            // body under it — the same substitution `bind_stmt` performs
            // for `block`/`rec` children.
            if children(node).len() >= 3 {
                let body_idx = children(node)[2];
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
        "dim_arrow" => lower_dim_arrow(ast, lang, node, env, params),
        "bind" => lower_bind(ast, lang, node, env, params),

        "assert" => {
            // Keyword assertion (ADR-0089/0092). In the arrow/compiled
            // model this is a side-channel op request: the AST eval engine
            // (tree-walk) keeps the assert failure side-channel, so the
            // lowerer only needs to emit the op and let the runtime fall
            // back to the AST engine.
            Ok(Term::op_native("assert", vec![]))
        }

        // ADR-0105: hardcoded `if` branch pending a data-driven `lower`
        // scheme fragment. Lowered to `cond >>> tag >>> Choice(then, else)`.
        "if" => lower_if(ast, lang, node, env, params),

        // ADR-0074/ADR-0105: `rec` region — scoped bindings ending in the
        // self-application produced by `desugar.data`'s recursor.
        "rec" => lower_rec(ast, lang, node, env, params),

        // `loop` node (ADR-0120 Phase 1): children `[input, step]` lower to
        // `seq(input, Loop(unquote step))`. `step` is the feedback body —
        // an arrow `(in, state) -> Either(result, next_state)` — so a
        // quoted step function is unquoted back into its body scheme.
        "loop" => lower_loop(ast, lang, node, env, params),

        // ADR-0105: generic binary operator dispatch for the canonical
        // `binary.<op>` family emitted by desugaring.
        op if op.starts_with("binary.") => {
            let bin_op = op.strip_prefix("binary.").unwrap_or(op);
            lower_binary(ast, lang, node, env, params, bin_op)
        }

        other => Err(format!(
            "lower: unsupported node kind `{}` at {}",
            other, idx
        )),
    }
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

    let mut last = None;
    let count = statements.len();
    for (i, &idx) in statements.iter().enumerate() {
        match bind_stmt(ast, lang, idx, &mut env, &mut local, params)? {
            // `eval_block`: the block's value is the last statement's
            // value — a trailing `assign`/`bind`/`op` yields its bound
            // value, not null.
            Some(term) if i + 1 == count => last = Some(term),
            Some(_) => {}
            None => last = Some(lower_node(ast, lang, idx, &env, params)?),
        }
    }

    last.ok_or_else(|| {
        format!(
            "block at {} produced no result expression",
            node_index(node)
        )
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
            lower_node(ast, lang, idx, &env, params)?;
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
    if child.kind == "assign" {
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
        let bound = if is_op_graded_arrow(ast, lang, value_idx)? {
            op_term_from_graded_arrow(ast, lang, &name, value_idx)?
        } else {
            lower_node(ast, lang, value_idx, env, params)?
        };
        let entry = env_entry(ast, lang, value_idx, bound);
        env.insert(name.clone(), entry.clone());
        local.insert(name);
        Ok(Some(entry.term))
    } else if child.kind == "bind" {
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
                Term::op_native_lit("proj_child", vec![], pos as i64)
            };
            return Ok(sequential_app(callee_term, arg_terms));
        }

        // Unknown callee: treat as a native op call.
        return Ok(Term::op_native(name, arg_terms));
    }

    let callee_term = lower_node(ast, lang, callee_idx, env, params)?;
    if callee.kind == "call" {
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

fn lower_binary(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
    op: &str,
) -> Result<Term, String> {
    let (l, r) = lower_two_children(ast, lang, node, env, params)?;
    Ok(Term::op_native(op, vec![l, r]))
}

fn lower_pair_combinator(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
    op: &str,
) -> Result<Term, String> {
    let (l, r) = lower_two_children(ast, lang, node, env, params)?;
    Ok(Term::op_native(op, vec![l, r]))
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
        let key = if key_node.kind == "var" || key_node.kind == "string" {
            key_node
                .payload
                .as_str()
                .ok_or_else(|| format!("entry key at {} is not a string", key_idx))?
                .to_string()
        } else {
            return Err(format!("object entry key at {} is not a literal", key_idx));
        };

        let value_idx = child_at_role_or_pos(entry, lang, "value", 1)
            .ok_or_else(|| format!("entry at {} missing value child", entry_idx))?;
        let value = lower_node(ast, lang, value_idx, env, params)?;

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

fn lower_op(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    _env: &HashMap<String, EnvEntry>,
    _params: &[String],
) -> Result<Term, String> {
    let name = node
        .payload
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op at {} missing payload.name", node_index(node)))?
        .to_string();

    let source = if let Some(signature_idx) = child_at_role_or_pos(node, lang, "signature", 1) {
        if let Some(signature) = ast.get(AstRef::new(signature_idx)) {
            if let Some(grade_idx) = child_at_role_or_pos(signature, lang, "grade", 1) {
                if let Some(grade) = ast.get(AstRef::new(grade_idx)) {
                    match grade.kind.as_str() {
                        "var" => grade
                            .payload
                            .as_str()
                            .map(|s| ArrSource::Foreign(s.to_string()))
                            .unwrap_or(ArrSource::Native),
                        _ => ArrSource::Native,
                    }
                } else {
                    ArrSource::Native
                }
            } else {
                ArrSource::Native
            }
        } else {
            ArrSource::Native
        }
    } else {
        ArrSource::Native
    };

    Ok(Term::Op {
        dim: Term::default_dim(),
        name,
        args: vec![],
        source,
        lit: None,
    })
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
                Term::op_native_lit("proj_child", vec![], 0),
                Term::op_native_lit("proj_child", vec![], 1),
            );
            let arg_in = Term::seq(
                Term::op_native_lit("proj_child", vec![], 0),
                Term::op_native_lit("proj_child", vec![], 0),
            );
            let arg_st = Term::op_native_lit("proj_child", vec![], 1);
            let body = Term::app(get_record, vec![arg_in, arg_st]);
            return Ok(Term::seq(
                Term::op_native("product", vec![input, step]),
                Term::loop_feedback(body),
            ));
        }
    }
    Ok(Term::seq(input, Term::loop_feedback(unquote_term(step))))
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
    let source = first_non_op_grade(ast, lang, grade_idx)
        .map(ArrSource::Foreign)
        .unwrap_or(ArrSource::Native);
    Ok(Term::Op {
        dim: Term::default_dim(),
        name: name.to_string(),
        args: vec![],
        source,
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

fn lower_dim_arrow(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, EnvEntry>,
    params: &[String],
) -> Result<Term, String> {
    let source_idx = child_at_role_or_pos(node, lang, "source", 0)
        .ok_or_else(|| format!("dim_arrow at {} missing source child", node_index(node)))?;
    let dim_idx = child_at_role_or_pos(node, lang, "dim", 1)
        .ok_or_else(|| format!("dim_arrow at {} missing dim child", node_index(node)))?;
    let target_idx = child_at_role_or_pos(node, lang, "target", 2)
        .ok_or_else(|| format!("dim_arrow at {} missing target child", node_index(node)))?;

    let source = lower_node(ast, lang, source_idx, env, params)?;
    let dim = lower_node(ast, lang, dim_idx, env, params)?;
    let target = lower_node(ast, lang, target_idx, env, params)?;

    let tag = Term::Arr {
        dim: Term::default_dim(),
        value: "Arrow".into(),
    };
    let record = Term::op_native(
        "record",
        vec![
            Term::Arr {
                dim: Term::default_dim(),
                value: "inputs".into(),
            },
            source,
            Term::Arr {
                dim: Term::default_dim(),
                value: "grade".into(),
            },
            dim,
            Term::Arr {
                dim: Term::default_dim(),
                value: "output".into(),
            },
            target,
        ],
    );
    Ok(Term::op_native("tag.construct", vec![tag, record]))
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
    let index = lang
        .child_role_index(&node.kind, role)
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
