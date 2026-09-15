//! Shared canonical AST to arrow-calculus `Term` lowerer (ADR-0100).

use std::collections::HashMap;

use handl_language::{Ast, AstNode, AstRef, Language, Term, Value};

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
        "value" => Ok(Term::const_value(node.payload.clone())),

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

        "binary.add" => lower_binary(ast, lang, node, env, params, "math.add"),
        "binary.mul" => lower_binary(ast, lang, node, env, params, "math.mul"),
        "binary.sub" => lower_binary(ast, lang, node, env, params, "math.sub"),
        "binary.div" => lower_binary(ast, lang, node, env, params, "math.div"),

        "product" => lower_pair_combinator(ast, lang, node, env, params, "product"),
        "fanout" => lower_pair_combinator(ast, lang, node, env, params, "fanout"),
        "flow" => {
            let (l, r) = lower_two_children(ast, lang, node, env, params)?;
            Ok(Term::seq(l, r))
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

        other => Err(format!("lower: unsupported node kind `{}` at {}", other, idx)),
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
    for &idx in &statements {
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
            let value = lower_node(ast, lang, value_idx, &env, params)?;
            env.insert(name, value);
        } else {
            last = Some(lower_node(ast, lang, idx, &env, params)?);
        }
    }

    last.ok_or_else(|| format!("block at {} produced no result expression", node_index(node)))
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
    let callee_idx = child_at_role_or_pos(node, lang, "callee", 0)
        .unwrap_or(children[0]);
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
        name: "record".to_string(),
        args: values,
        source: handl_language::ArrSource::Native,
        lit: Some(Value::Array(keys)),
    })
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
    // Prefer the `params` child role; fall back to payload for hand-built ASTs.
    let params_idx = child_at_role_or_pos(node, lang, "params", 0)
        .or_else(|| {
            node.payload
                .get("params")
                .and_then(Value::as_u64)
                .map(|u| u as usize)
        })
        .ok_or_else(|| format!("function at {} missing params child/payload", node_index(node)))?;

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
                    return Err(format!("unsupported parameter kind `{}` at {}", child.kind, child_idx));
                }
            }
            Ok(names)
        }
        other => Err(format!("unsupported function params kind `{}` at {}", other, params_idx)),
    }
}

fn function_body_index(lang: &Language, node: &AstNode) -> Result<usize, String> {
    child_at_role_or_pos(node, lang, "body", 1)
        .or_else(|| node.payload.get("body").and_then(Value::as_u64).map(|u| u as usize))
        .ok_or_else(|| format!("function at {} missing body child/payload", node_index(node)))
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
