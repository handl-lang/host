//! [`TransformCondition`] evaluation (ADR-0094 §3).
//!
//! Conditions are pure predicates over the *input* node (pre-remap) and the
//! input arena. `KindAt` resolves the child position by named role via
//! [`AstShapeDef::role_index`](handl_language::AstShapeDef::role_index)
//! (ADR-0081), falling back to the positional `index` for shapes that declare
//! no roles.

use handl_language::{Ast, AstNode, AstRef, Language, TransformCondition};

/// Evaluate `cond` against `node` in the input arena `ast`.
///
/// Returns `true` when the condition holds — i.e. the guarded rule may fire.
/// A condition that cannot be resolved (missing child, unknown role, absent
/// payload path) evaluates to `false`, never panics.
pub fn matches_condition(
    lang: &Language,
    ast: &Ast,
    node: &AstNode,
    cond: &TransformCondition,
) -> bool {
    match cond {
        TransformCondition::KindAt { index, role, kind } => {
            match resolve_child_index(lang, node, *index, role.as_deref()) {
                Some(old_idx) => ast
                    .get(AstRef::new(old_idx))
                    .map(|child| child.kind == *kind)
                    .unwrap_or(false),
                None => false,
            }
        }
        TransformCondition::AnyChildKind { kind } => node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter().any(|v| {
                    v.as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .map(|child| child.kind == *kind)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false),
        TransformCondition::PayloadEq { path, value } => payload_at(&node.payload, path)
            .map(|v| payload_eq(lang, node, v, value))
            .unwrap_or(false),
        TransformCondition::ChildPayloadEq {
            index,
            role,
            path,
            value,
        } => match resolve_child_index(lang, node, *index, role.as_deref()) {
            Some(old_idx) => ast
                .get(AstRef::new(old_idx))
                .and_then(|child| {
                    payload_at(&child.payload, path).map(|v| payload_eq(lang, child, v, value))
                })
                .unwrap_or(false),
            None => false,
        },
        TransformCondition::ChildCount(n) => node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| arr.len() == *n)
            .unwrap_or(*n == 0),
        TransformCondition::And(conds) => {
            conds.iter().all(|c| matches_condition(lang, ast, node, c))
        }
        TransformCondition::Or(conds) => {
            conds.iter().any(|c| matches_condition(lang, ast, node, c))
        }
        TransformCondition::Not(cond) => !matches_condition(lang, ast, node, cond),
    }
}

/// Resolve the child position to test for [`TransformCondition::KindAt`].
///
/// A declared `role` is resolved through the node's [`AstShapeDef`] first
/// (ADR-0081); the positional `index` is the role-less fallback.
fn resolve_child_index(
    lang: &Language,
    node: &AstNode,
    index: Option<usize>,
    role: Option<&str>,
) -> Option<usize> {
    let pos = role
        .and_then(|r| {
            lang.ast
                .get_by_identity(&node.kind)
                .and_then(|shape| shape.role_index(r))
        })
        .or(index)?;
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(pos))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
}

/// Payload equality for the `*PayloadEq` conditions.
///
/// Direct JSON equality, plus one data-driven extension: when the tested
/// node is a *name leaf* (its [`AstShapeDef`] declares `pattern_role:
/// "leaf"` — e.g. canonical `var`) and the payload under test is a string
/// naming an ambient global in [`Language::globals`], the name compares
/// equal to the global's bound value. Constants spelled as references —
/// the imported Bool globals `true`/`false`, which lex as `var` nodes
/// rather than `value` literals — therefore satisfy `value: true` /
/// `value: false` guards without the condition vocabulary hardcoding
/// global spellings.
fn payload_eq(
    lang: &Language,
    node: &AstNode,
    v: &serde_json::Value,
    value: &serde_json::Value,
) -> bool {
    if v == value {
        return true;
    }
    let is_name_leaf = lang
        .ast
        .get_by_identity(&node.kind)
        .and_then(|shape| shape.pattern_role.as_deref())
        == Some("leaf");
    if !is_name_leaf {
        return false;
    }
    v.as_str()
        .and_then(|name| lang.globals.get(name))
        .map(|global| global == value)
        .unwrap_or(false)
}

/// Navigate a dotted `path` (`.`-separated object keys; numeric segments
/// index into arrays) into `payload`.
fn payload_at<'a>(payload: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    if path.is_empty() {
        return Some(payload);
    }
    let mut cur = payload;
    for seg in path.split('.') {
        cur = if let Some(idx) = seg.parse::<usize>().ok().filter(|_| cur.is_array()) {
            cur.get(idx)?
        } else {
            cur.get(seg)?
        };
    }
    Some(cur)
}
