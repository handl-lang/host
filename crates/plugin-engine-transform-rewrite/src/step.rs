//! [`RewriteStep`] interpreter (ADR-0094 §3).
//!
//! `Keep`/`KeepPath`/`KeepRest`/`Create` generalize `build_child` from the
//! legacy `handl-runtime::desugar_pass` with identical semantics;
//! `KeepRole` resolves its child position via
//! [`AstShapeDef::role_index`](handl_language::AstShapeDef::role_index)
//! (ADR-0081); `Splice` lifts a child's children into the rewritten node's
//! child list; `Apply` runs another named rule on a child first.
//!
//! Child indices in the *input* arena are translated through a [`Resolver`]:
//! the bottom-up walker resolves via the `remap` table, while the top-down
//! walker resolves by visiting the child lazily. The interpreter therefore
//! never touches the `remap` table itself.

use std::collections::HashMap;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, RewriteStep};
use serde_json::{Map, Value};

/// How a step interpreter obtains output-arena indices for input-arena
/// nodes.
///
/// Implemented by the walker's pass state: bottom-up walks answer from the
/// `remap` table; top-down walks answer by recursively visiting the child.
pub(crate) trait Resolver {
    /// Resolve the input-arena node `old_idx` to its output-arena index.
    ///
    /// `out` is the output arena being built — top-down resolvers may push
    /// into it while lazily visiting the child. Returns `Ok(None)` when the
    /// index is invalid or the node cannot be produced in the output arena.
    fn resolve(&mut self, out: &mut Ast, old_idx: usize) -> Result<Option<usize>, LangError>;

    /// Apply the rule `rule_id` to the input-arena node `old_idx` and
    /// resolve to the resulting output index.
    ///
    /// When the rule's `when` condition does not hold, the child is kept
    /// as-is (equivalent to [`resolve`](Self::resolve)).
    fn apply(
        &mut self,
        out: &mut Ast,
        rule_id: &str,
        old_idx: usize,
    ) -> Result<Option<usize>, LangError>;
}

/// Per-rule-application state for [`RewriteStep::Fresh`] (ADR-0094).
///
/// `counter` is a per-pass counter passed in from the walker; `names`
/// records which output-arena `var` node was minted for each hint in the
/// current rule application, so repeated `Fresh(hint)` steps reuse it.
struct FreshState {
    counter: usize,
    names: HashMap<String, usize>,
}

impl FreshState {
    /// Allocate or reuse a `var` leaf for `hint`.
    fn get_or_make(&mut self, hint: &str, out: &mut Ast) -> usize {
        if let Some(&idx) = self.names.get(hint) {
            return idx;
        }
        let id = self.counter;
        self.counter += 1;
        let name = format!("__fresh_{hint}_{id}");
        let idx = out
            .push(AstNode::new("var").with_payload(Value::from(name)))
            .index();
        self.names.insert(hint.to_string(), idx);
        idx
    }
}

/// The input-arena index of `original`'s child `i`, if present.
fn child_index(original: &AstNode, i: usize) -> Option<usize> {
    original
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(i))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
}

/// Build a rewritten node of `target_kind` from `steps` (ADR-0094 §3).
///
/// Span policy: the rewritten node inherits `original.span`. `patch` is a
/// **shallow patch** over the original node's payload: provided keys
/// overwrite, absent keys are preserved, and the `children` key is managed
/// by the rewrite itself (replaced by the built children, or removed when
/// the rule produces none).
pub(crate) fn build_rewrite(
    resolver: &mut dyn Resolver,
    lang: &Language,
    target_kind: &str,
    steps: &[RewriteStep],
    patch: Option<&Value>,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    fresh_counter: usize,
) -> Result<(AstRef, usize), LangError> {
    let mut fresh = FreshState {
        counter: fresh_counter,
        names: HashMap::new(),
    };
    let children: Vec<Value> = steps
        .iter()
        .map(|spec| build_step(spec, &mut fresh, resolver, lang, original, ast, out))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    // Payload policy (ADR-0094 §"Spans & payloads"): `patch` is a **shallow
    // patch** over the original node's payload — provided keys overwrite,
    // absent keys are preserved, and `children` is managed by the rewrite.
    // A non-object `patch` replaces the payload outright; when the rewrite
    // also builds children they take precedence (a scalar payload cannot
    // carry a `children` key).
    let mut payload: Value = match patch {
        Some(Value::Object(obj)) => {
            let mut merged = original
                .payload
                .as_object()
                .cloned()
                .unwrap_or_default();
            merged.remove("children");
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        Some(p) => p.clone(),
        None => {
            let mut kept = original
                .payload
                .as_object()
                .cloned()
                .unwrap_or_default();
            kept.remove("children");
            Value::Object(kept)
        }
    };
    if !children.is_empty() {
        match &mut payload {
            Value::Object(obj) => {
                obj.insert("children".to_string(), Value::Array(children));
            }
            _ => {
                payload = Value::Object(Map::from_iter([(
                    "children".to_string(),
                    Value::Array(children),
                )]));
            }
        }
    }
    let node = AstNode {
        kind: target_kind.to_string(),
        span: original.span,
        payload,
    };
    let ast_ref = out.push(node);
    Ok((ast_ref, fresh.counter))
}

/// Build the output children contributed by a single [`RewriteStep`].
///
/// Returns a `Vec` because `KeepRest` and `Splice` expand to multiple
/// children; all other variants produce exactly one.
fn build_step(
    spec: &RewriteStep,
    fresh: &mut FreshState,
    resolver: &mut dyn Resolver,
    lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
) -> Result<Vec<Value>, LangError> {
    match spec {
        RewriteStep::Fresh(hint) => Ok(vec![Value::from(fresh.get_or_make(hint, out))]),
        RewriteStep::Keep(i) => keep_at(resolver, out, original, *i),
        RewriteStep::KeepRole(role) => {
            // ADR-0081: resolve the named role through the original node's
            // shape; unresolvable roles produce a `Null` child (the same
            // shape a missed `Keep` yields).
            let idx = lang
                .ast
                .get_by_identity(&original.kind)
                .and_then(|shape| shape.role_index(role));
            match idx {
                Some(i) => keep_at(resolver, out, original, i),
                None => Ok(vec![Value::Null]),
            }
        }
        RewriteStep::KeepPath(path) => {
            // Navigate through the original AST's children by index path,
            // then resolve the final arena index.
            let mut current_node = original;
            let mut current_idx: Option<usize> = None;
            for &step in path {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array());
                let next_idx = children
                    .and_then(|arr| arr.get(step))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                match next_idx {
                    Some(idx) => {
                        current_idx = Some(idx);
                        match ast.get(AstRef::new(idx)) {
                            Some(n) => current_node = n,
                            None => return Ok(vec![Value::Null]),
                        }
                    }
                    None => return Ok(vec![Value::Null]),
                }
            }
            match current_idx {
                Some(idx) => Ok(vec![resolver
                    .resolve(out, idx)?
                    .map(Value::from)
                    .unwrap_or(Value::Null)]),
                None => Ok(vec![Value::Null]),
            }
        }
        RewriteStep::KeepRest(start) => {
            // Keep all children from `start` onward, each resolved into the
            // output arena (ADR-0060).
            let children = original.payload.get("children").and_then(|c| c.as_array());
            match children {
                Some(arr) => {
                    let mut out_children = Vec::new();
                    for v in arr.iter().skip(*start) {
                        if let Some(old_idx) = v.as_u64().map(|i| i as usize) {
                            if let Some(new_idx) = resolver.resolve(out, old_idx)? {
                                out_children.push(Value::from(new_idx));
                            }
                        }
                    }
                    Ok(out_children)
                }
                None => Ok(vec![]),
            }
        }
        RewriteStep::Splice(i) => {
            // Lift the indexed child's own children into this child list.
            match child_index(original, *i).and_then(|idx| ast.get(AstRef::new(idx))) {
                Some(child) => {
                    let grandchildren = child
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array());
                    match grandchildren {
                        Some(arr) => {
                            let indices: Vec<usize> = arr
                                .iter()
                                .filter_map(|v| v.as_u64().map(|x| x as usize))
                                .collect();
                            let mut out_children = Vec::new();
                            for old_idx in indices {
                                if let Some(new_idx) = resolver.resolve(out, old_idx)? {
                                    out_children.push(Value::from(new_idx));
                                }
                            }
                            Ok(out_children)
                        }
                        None => Ok(vec![]),
                    }
                }
                None => Ok(vec![]),
            }
        }
        RewriteStep::Create {
            kind,
            payload,
            children,
        } => {
            let child_indices: Vec<Value> = children
                .iter()
                .map(|c| build_step(c, fresh, resolver, lang, original, ast, out))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect();
            let node_payload = if child_indices.is_empty() {
                payload.clone().unwrap_or(Value::Null)
            } else {
                let mut p = Map::new();
                p.insert("children".to_string(), Value::Array(child_indices));
                if let Some(extra) = payload {
                    if let Some(obj) = extra.as_object() {
                        for (k, v) in obj {
                            p.insert(k.clone(), v.clone());
                        }
                    }
                }
                Value::Object(p)
            };
            // Span policy: `Create`d nodes get no span — provenance is via
            // the `RewriteTrace` of the rule that made them (ADR-0094 §
            // "Spans & payloads").
            let node = AstNode {
                kind: kind.clone(),
                span: None,
                payload: node_payload,
            };
            let new_ref = out.push(node);
            Ok(vec![Value::from(new_ref.index())])
        }
        RewriteStep::Apply { rule_id, index } => match child_index(original, *index) {
            Some(old_idx) => Ok(vec![resolver
                .apply(out, rule_id, old_idx)?
                .map(Value::from)
                .unwrap_or(Value::Null)]),
            None => Ok(vec![Value::Null]),
        },
    }
}

/// `Keep`-family helper: resolve `original`'s child `i` into the output
/// arena, producing a single `Value` (`Null` when the child is absent or
/// unresolvable).
fn keep_at(
    resolver: &mut dyn Resolver,
    out: &mut Ast,
    original: &AstNode,
    i: usize,
) -> Result<Vec<Value>, LangError> {
    match child_index(original, i) {
        Some(old_idx) => Ok(vec![resolver
            .resolve(out, old_idx)?
            .map(Value::from)
            .unwrap_or(Value::Null)]),
        None => Ok(vec![Value::Null]),
    }
}
