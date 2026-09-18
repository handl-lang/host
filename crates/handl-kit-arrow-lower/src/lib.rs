//! Shared canonical AST to arrow-calculus `Term` lowerer (ADR-0100).
//!
//! TECH DEBT (ADR-0105): `lower_node` still `match`es on canonical kind
//! strings (`value`, `var`, `assign`, `bind`, `op`, `flow`, …) to pick the
//! lowering combinator. The data-driven replacement is a `lower` scheme
//! fragment per `AstShapeDef` — the same shape `lang.eval`/`lang.types`
//! already use — so a renamed kind keeps lowering without an engine edit.
//! Until that fragment exists, this `match` is the Arrow dialect's lowering
//! table and new canonical constructs must add an arm here.

use std::collections::HashMap;

use handl_language::{ArrSource, Ast, AstNode, AstRef, Language, Term, Value};

/// Lower a canonical AST rooted at `root` to a `Term` arrow.
pub fn lower_ast(ast: &Ast, lang: &Language, root: AstRef) -> Result<Term, String> {
    lower_node(ast, lang, root.index(), &HashMap::new(), &[])
}

fn lower_node(
    ast: &Ast,
    lang: &Language,
    idx: usize,
    env: &HashMap<String, Term>,
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
            } else if let Some(term) = env.get(name) {
                Ok(term.clone())
            } else {
                // Unbound name: treat as a native op reference (e.g. a host fn).
                Ok(Term::op_native(name, vec![]))
            }
        }

        "function" => {
            let params = function_params(ast, lang, node)?;
            let body_idx = function_body_index(lang, node)?;
            lower_node(ast, lang, body_idx, env, &params)
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
            let (l, r) = lower_two_children(ast, lang, node, env, params)?;
            Ok(Term::seq(l, r))
        }

        "array" => {
            // Array literal: the native `array` op packs the evaluated
            // element schemes into a single array value (eval.array).
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
            // An `assign` as an expression lowers to the value being bound.
            let value_idx = child_at_role_or_pos(node, lang, "value", 1)
                .ok_or_else(|| format!("assign at {} missing value child", idx))?;
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
    env: &HashMap<String, Term>,
    params: &[String],
) -> Result<Term, String> {
    let mut env = env.clone();
    let statements = children(node);
    if statements.is_empty() {
        return Ok(Term::const_value(Value::Null));
    }

    let mut last = None;
    let count = statements.len();
    for (i, &idx) in statements.iter().enumerate() {
        match bind_stmt(ast, lang, idx, &mut env, params)? {
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
/// `desugar.data` for recursor self-application. All but the last child
/// bind into a private env; the node lowers to the last statement's value
/// (the self-application `Type.rec = __rec_inner(__rec_inner)`).
fn lower_rec(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
    params: &[String],
) -> Result<Term, String> {
    let stmts = children(node);
    let (last, init) = stmts
        .split_last()
        .ok_or_else(|| format!("rec at {} has no children", node_index(node)))?;
    let mut env = env.clone();
    for &idx in init {
        if bind_stmt(ast, lang, idx, &mut env, params)?.is_none() {
            lower_node(ast, lang, idx, &env, params)?;
        }
    }
    lower_node(ast, lang, *last, &env, params)
}

/// Bind a `block`/`rec` child statement into `env` when it is a binding
/// (`assign`, `bind`, `op`). Returns the bound value for bindings and
/// `None` for non-binding statements.
fn bind_stmt(
    ast: &Ast,
    lang: &Language,
    idx: usize,
    env: &mut HashMap<String, Term>,
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
        let value_idx = child_at_role_or_pos(child, lang, "value", 1)
            .ok_or_else(|| format!("assign at {} missing value child", idx))?;
        let bound = if is_op_graded_arrow(ast, lang, value_idx)? {
            op_term_from_graded_arrow(ast, lang, &name, value_idx)?
        } else {
            lower_node(ast, lang, value_idx, env, params)?
        };
        env.insert(name, bound.clone());
        Ok(Some(bound))
    } else if child.kind == "bind" {
        let name = child
            .payload
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("bind at {} missing payload.name", idx))?
            .to_string();
        let value_idx = child_at_role_or_pos(child, lang, "value", 0)
            .or_else(|| children(child).first().copied())
            .ok_or_else(|| format!("bind at {} missing value child", idx))?;
        let bound = if is_op_graded_arrow(ast, lang, value_idx)? {
            op_term_from_graded_arrow(ast, lang, &name, value_idx)?
        } else {
            lower_node(ast, lang, value_idx, env, params)?
        };
        env.insert(name, bound.clone());
        Ok(Some(bound))
    } else if child.kind == "op" {
        let name = child
            .payload
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} at {} missing payload.name", child.kind, idx))?
            .to_string();
        let bound = lower_node(ast, lang, idx, env, params)?;
        env.insert(name, bound.clone());
        Ok(Some(bound))
    } else {
        Ok(None)
    }
}

/// `json` template (ADR-0060): children `[template, arg0..argN]` — child 0
/// is a leaf `json` node whose string payload is the template; the op
/// substitutes evaluated args into `$i` holes and parses the result.
fn lower_json(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
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
    env: &HashMap<String, Term>,
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

        if let Some(callee_term) = env.get(name) {
            if arg_terms.is_empty() {
                return Ok(callee_term.clone());
            }
            if arg_terms.len() == 1 {
                return Ok(Term::seq(arg_terms.remove(0), callee_term.clone()));
            }
            // Pair the arguments and feed them to the known callee.
            let pair = if arg_terms.len() == 2 {
                Term::op_native("product", arg_terms)
            } else {
                Term::op_native("array", arg_terms)
            };
            return Ok(Term::seq(pair, callee_term.clone()));
        }

        if let Some(pos) = params.iter().position(|p| p == name) {
            // Parameter callee (`self(self)`, `f(x)` where `f` is a
            // parameter): dynamic application — the callee value comes
            // from the input tuple.
            let callee_term = if params.len() == 1 {
                Term::op_native("id", vec![])
            } else {
                Term::op_native_lit("proj_child", vec![], pos as i64)
            };
            return Ok(Term::app(callee_term, arg_terms));
        }

        // Unknown callee: treat as a native op call.
        return Ok(Term::op_native(name, arg_terms));
    }

    // Dynamic callee: `App` with the lowered callee and argument arrows.
    let callee_term = lower_node(ast, lang, callee_idx, env, params)?;
    Ok(Term::app(callee_term, arg_terms))
}

fn lower_binary(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
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
    env: &HashMap<String, Term>,
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
    env: &HashMap<String, Term>,
    params: &[String],
) -> Result<Term, String> {
    let entry_indices = children(node);
    let mut keys = Vec::new();
    let mut values = Vec::new();

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
    _env: &HashMap<String, Term>,
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
    env: &HashMap<String, Term>,
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
    let impl_ = lower_node(ast, lang, impl_idx, env, params)?;
    let body = lower_node(ast, lang, body_idx, env, params)?;
    Ok(Term::handle(body, target, impl_))
}

fn lower_arr(
    _ast: &Ast,
    _lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
    _params: &[String],
) -> Result<Term, String> {
    let name = node
        .payload
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("arr at {} missing payload.name", node_index(node)))?
        .to_string();
    if let Some(term) = env.get(&name) {
        Ok(term.clone())
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
    _env: &HashMap<String, Term>,
    _params: &[String],
) -> Result<Term, String> {
    // A bare graded-arrow expression is a signature, not a runnable arrow.
    Ok(Term::const_value(Value::Null))
}

fn lower_bind(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
    params: &[String],
) -> Result<Term, String> {
    let value_idx = child_at_role_or_pos(node, lang, "value", 0)
        .or_else(|| children(node).first().copied())
        .ok_or_else(|| format!("bind at {} missing value child", node_index(node)))?;
    lower_node(ast, lang, value_idx, env, params)
}

fn lower_if(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
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
    Ok(Term::seq(
        cond,
        Term::seq(Term::op_native("tag", vec![]), Term::choice(then_, else_)),
    ))
}

fn lower_field_access(
    ast: &Ast,
    lang: &Language,
    node: &AstNode,
    env: &HashMap<String, Term>,
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
    env: &HashMap<String, Term>,
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
    env: &HashMap<String, Term>,
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
