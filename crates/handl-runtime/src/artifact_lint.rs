//! Final-artifact lint (ADR-0154 §6) — the verification half of erasure.
//!
//! After the exec-transform pipeline (desugar → typecheck →
//! `runtime_assert_pass` → `erase_pass` →
//! [`exec_erasure_pass`](crate::exec_erasure_pass)) the runnable artifact
//! must contain **only 1-arrows and runtime values**: no former
//! applications, no metavariables, no undecided indices. This lint walks
//! the *reachable* AST — from the root through `payload["children"]` and
//! the audited [`ARENA_REF_PAYLOAD_KEYS`] — and reports every residual
//! type-level datum:
//!
//! - **Checker encodings embedded in payloads**: `{"__former__": …}`
//!   (a former application), `{"__meta__": …}` (an unsolved metavariable),
//!   `{"__neutral__": …}` (a stuck neutral — rigid/meta/unfold head), and
//!   `{"__type__": i}` (a universe index `Type(i)`). These are the
//!   `quote`/lift boundary encodings the checker serializes; one present
//!   in a *static* payload is elaboration output that leaked into exec.
//!   Runtime-minted cells via `eval`/`resolve` never appear here — the
//!   lint applies to the compiled artifact, not to dynamic values
//!   (laws-as-values stay legal, ADR-0154 §6).
//! - **Undecided type-level node kinds**: a kind the checker owns a
//!   typing rule for (`lang.types`) but no exec scheme exists
//!   (`lang.eval`) — e.g. `pi`/`sigma`/`forall`/`family`/`j` in the
//!   canonical language. Reaching exec, such a node hits the no-scheme
//!   fallback, which returns the raw payload (arena indices and all) as
//!   a "value" — the silent unsafety this lint turns into a diagnostic:
//!   a pipeline bug, or a site that needs a runtime backstop.
//!
//! Nodes that are *structural* sub-positions rather than evaluated
//! expressions — `entry`, `arm`, `alt` — carry no `lang.types` rule and
//! are not flagged; literal `value`/`lit` nodes carry no child
//! references and are likewise exempt.
//!
//! The lint is a *verification* pass, not a typing discipline: it checks
//! an invariant of the artifact, adding no binder annotations and no
//! quantity system (ADR-0154 §6, ADR-0153's withdrawal of coeffect
//! grades).

use std::collections::HashSet;

use handl_language::ast::ARENA_REF_PAYLOAD_KEYS;
use handl_language::{Ast, AstNode, AstRef, LangError, Language, Span};
use serde_json::Value;

/// Lint rule ids follow the `artifact.*` naming used by diagnostics
/// elsewhere (`source.*`, `scope.*` in `lang.lints`).
pub mod rules {
    /// A `{"__former__": …}` application embedded in the artifact.
    pub const FORMER_APPLICATION: &str = "artifact.former-application";
    /// An unsolved `{"__meta__": id}` metavariable in the artifact.
    pub const UNSOLVED_META: &str = "artifact.unsolved-meta";
    /// A stuck `{"__neutral__": …}` term (rigid/meta/unfold head).
    pub const STUCK_NEUTRAL: &str = "artifact.stuck-neutral";
    /// A `{"__type__": i}` universe index leaked into runnable position.
    pub const UNIVERSE_INDEX: &str = "artifact.universe-index";
    /// A checker-owned node kind with no exec scheme in runnable position.
    pub const TYPE_LEVEL_NODE: &str = "artifact.type-level-node";
}

/// `(payload key, rule id, human description)` — checker value encodings
/// that must not survive into the runnable artifact.
///
/// `__sym__`, `__arrow__`, `__ctor__`, `__tag__`, `__dim__`, … are
/// deliberately *absent*: nominal syms, defunctionalized 1-arrows,
/// constructor values, tagged records and n-cell descriptors are legal
/// runtime values (laws-as-values; ADR-0154 §§1, 6).
const TYPE_LEVEL_VALUE_KEYS: &[(&str, &str, &str)] = &[
    (
        "__former__",
        rules::FORMER_APPLICATION,
        "former application",
    ),
    ("__meta__", rules::UNSOLVED_META, "unsolved metavariable"),
    ("__neutral__", rules::STUCK_NEUTRAL, "stuck neutral term"),
    ("__type__", rules::UNIVERSE_INDEX, "universe index"),
];

/// One lint finding on the final runnable artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactLint {
    /// Arena index of the offending node.
    pub node: usize,
    /// The offending node's kind.
    pub node_kind: String,
    /// Source span, when the node carries one.
    pub span: Option<Span>,
    /// The lint rule that fired (see [`rules`]).
    pub rule: &'static str,
    /// Human-readable diagnostic message.
    pub message: String,
}

/// Lint the runnable artifact reachable from `root` (ADR-0154 §6).
///
/// Reachability follows `payload["children"]` plus the audited
/// [`ARENA_REF_PAYLOAD_KEYS`] (e.g. the closure form's `payload["body"]`)
/// — the same reference set [`Ast::subtree`] remaps — so erased-but-
/// orphaned arena entries are correctly ignored.
///
/// Returns all findings (not fail-fast): callers surface the full
/// diagnostic set rather than fixing one stuck node at a time.
pub fn lint_exec_artifact(lang: &Language, ast: &Ast, root: AstRef) -> Vec<ArtifactLint> {
    let mut findings = Vec::new();
    let mut visited: HashSet<usize> = HashSet::new();
    let mut stack = vec![root.index()];
    while let Some(i) = stack.pop() {
        if !visited.insert(i) {
            continue;
        }
        let Some(node) = ast.get(AstRef::new(i)) else {
            continue;
        };

        // (a) Checker encodings embedded anywhere in the node payload.
        if let Some((key, rule, what)) = find_type_level_key(&node.payload) {
            findings.push(ArtifactLint {
                node: i,
                node_kind: node.kind.clone(),
                span: node.span,
                rule,
                message: format!(
                    "{what} (`{key}`) embedded in `{}` payload at arena index {i}: \
                     elaboration output leaked into the runnable artifact \
                     (pipeline bug or missing runtime backstop, ADR-0154 §6)",
                    node.kind
                ),
            });
        }

        // (b) A checker-owned node kind with no exec scheme carrying
        // child references — an undecided type-level node (the no-scheme
        // fallback would return its raw payload as a "value").
        if is_undecided_type_level_node(lang, node) {
            findings.push(ArtifactLint {
                node: i,
                node_kind: node.kind.clone(),
                span: node.span,
                rule: rules::TYPE_LEVEL_NODE,
                message: format!(
                    "type-level node kind `{}` reached the runnable artifact at \
                     arena index {i}: `lang.types` owns a rule for it but no \
                     `lang.eval` scheme exists — stuck elaboration output \
                     (pipeline bug or missing runtime backstop, ADR-0154 §6)",
                    node.kind
                ),
            });
        }

        // Follow child references: `children` (ADR-0081 roles index into
        // it positionally) and the audited named arena-ref keys.
        if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
            for child in children {
                if let Some(idx) = child.as_u64() {
                    stack.push(idx as usize);
                }
            }
        }
        for key in ARENA_REF_PAYLOAD_KEYS {
            if let Some(idx) = node.payload.get(*key).and_then(|v| v.as_u64()) {
                stack.push(idx as usize);
            }
        }
    }
    findings
}

/// Lint the runnable artifact `ast` (root = last node, Pratt
/// convention), returning the first finding as a [`LangError`].
///
/// This is the pipeline hook: called after
/// [`crate::exec_erasure_pass::apply_exec_erasure`] so anything erasure
/// could not consume — a stuck-but-*referenced* term is live code DCE
/// cannot remove — becomes a diagnostic rather than silently evaluating
/// to junk (ADR-0154 §6).
///
/// # Errors
///
/// Returns [`LangError::Engine`] listing the findings (node index, kind,
/// and span of the first attached) when the artifact violates the
/// invariant.
pub fn check_exec_artifact(lang: &Language, ast: &Ast) -> Result<(), LangError> {
    if ast.is_empty() {
        return Ok(());
    }
    let root = AstRef::new(ast.len() - 1);
    let findings = lint_exec_artifact(lang, ast, root);
    if findings.is_empty() {
        return Ok(());
    }
    let first = &findings[0];
    let summary = findings
        .iter()
        .take(4)
        .map(|f| format!("[{}] {} node {}: {}", f.rule, f.node_kind, f.node, f.message))
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if findings.len() > 4 {
        format!("; … ({} more)", findings.len() - 4)
    } else {
        String::new()
    };
    Err(LangError::engine_at(
        format!("artifact lint: {summary}{suffix}"),
        first.span,
        Some(first.node_kind.clone()),
    ))
}

/// A node kind is an *undecided type-level node* when the checker owns a
/// typing rule for it (`lang.types`) but no exec scheme exists
/// (`lang.eval`), the node actually carries child references — i.e.
/// evaluating it can only mean the no-scheme payload fallback, which
/// leaks arena indices as a "value" (ADR-0154 §6) — and the kind is not
/// a *clause-pair kind* (see below).
///
/// The `lang.types`/`lang.eval` split is data-driven: `pi`, `sigma`,
/// `forall`, `family`, `j`, … qualify in the canonical language without
/// any kind-name dispatch here (engine-genericity, ADR-0044/0081).
/// `foreign.*` kinds dispatch host functions without a scheme and own no
/// type rule, so they exempt themselves.
///
/// ## Clause-pair exemption
///
/// `PairShape`-produced kinds — `arm` (match `pattern : body` pairs),
/// `entry` (record `key : value` pairs) — carry children but are
/// *structural data* interpreted by the parent's eval scheme
/// (`eval.match`, the object scheme), never dispatched to `eval_node`
/// standalone. The checker still owns a `type.*` rule for them (it
/// types clause bodies), so they satisfy the raw `types ∧ ¬eval`
/// condition — without this exemption every `match` in a real module
/// would false-positive.
fn is_undecided_type_level_node(lang: &Language, node: &AstNode) -> bool {
    if lang.types.get_by_identity(&node.kind).is_none()
        || lang.eval.get_by_identity(&node.kind).is_some()
    {
        return false;
    }
    if is_pair_shape_kind(lang, &node.kind) {
        return false;
    }
    node_carries_child_refs(node)
}

/// Whether `kind` is produced by a grammar `PairShape` — a delimited
/// block's `<lhs> <sep> <rhs>` clause node (match `arm`, record `entry`).
/// Such kinds are clause data consumed by the parent node's eval scheme,
/// not standalone-dispatched expressions.
fn is_pair_shape_kind(lang: &Language, kind: &str) -> bool {
    lang.grammar.iter().any(|row| {
        row.child_shape
            .delimited
            .as_ref()
            .and_then(|d| d.pair.as_ref())
            .map(|p| p.node_kind == kind)
            .unwrap_or(false)
    })
}

/// Whether the node's payload references other arena nodes — the
/// `children` array (ADR-0081) or a named arena-ref key
/// ([`ARENA_REF_PAYLOAD_KEYS`], e.g. the closure form's `body`).
///
/// Literal-payload nodes (`value`, `lit`, `var`) carry no references;
/// this is what exempts them from [`is_undecided_type_level_node`] even
/// when a `lang.types` rule exists for their kind (e.g. `type.value`).
fn node_carries_child_refs(node: &AstNode) -> bool {
    if node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    ARENA_REF_PAYLOAD_KEYS
        .iter()
        .any(|k| node.payload.get(*k).and_then(|v| v.as_u64()).is_some())
}

/// Deep-scan a payload [`Value`] for a checker-owned encoding key,
/// returning the matching `(key, rule, description)` entry.
///
/// Only object *keys* match: a `{"key": "__former__"}` entry payload
/// (the *spelling* of a runtime-minted former object, e.g. inside an
/// `.hnl` model building one as data) is a string value, not the
/// encoding — runtime-minted cells are allowed (ADR-0154 §6).
fn find_type_level_key(
    v: &Value,
) -> Option<&'static (&'static str, &'static str, &'static str)> {
    match v {
        Value::Object(map) => {
            for entry in TYPE_LEVEL_VALUE_KEYS {
                if map.contains_key(entry.0) {
                    return Some(entry);
                }
            }
            map.values().find_map(find_type_level_key)
        }
        Value::Array(arr) => arr.iter().find_map(find_type_level_key),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{EvalSchemeDef, Term, TypeRuleDef};
    use serde_json::json;

    fn node(kind: &str, payload: Value) -> AstNode {
        AstNode::new(kind).with_payload(payload)
    }

    /// A minimal language where `pi` has a type rule but no eval scheme,
    /// and `value` has both a type rule and a (payload-fallback-equivalent)
    /// eval scheme.
    fn typed_lang() -> Language {
        let mut lang = Language::new();
        lang.types
            .merge(TypeRuleDef::new("type.pi", "pi", Term::constant(json!(null))))
            .unwrap();
        lang.types
            .merge(TypeRuleDef::new("type.value", "value", Term::constant(json!(null))))
            .unwrap();
        lang
    }

    #[test]
    fn flags_embedded_former_application() {
        let mut ast = Ast::new();
        ast.push(node(
            "value",
            json!({"__former__": {"sym": {"__sym__": {"handle": 1, "name": "Vec"}}, "args": [2]}}),
        ));
        let findings = lint_exec_artifact(&Language::new(), &ast, AstRef::new(0));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, rules::FORMER_APPLICATION);
    }

    #[test]
    fn flags_unsolved_meta_and_stuck_neutral() {
        let mut ast = Ast::new();
        ast.push(node("value", json!({"__meta__": 7})));
        ast.push(node(
            "value",
            json!({"__neutral__": {"head": "meta", "id": 3, "spine": []}}),
        ));
        ast.push(node("block", json!({"children": [0, 1]})));
        let findings = lint_exec_artifact(&Language::new(), &ast, AstRef::new(2));
        assert_eq!(findings.len(), 2);
        // DFS pops children in reverse order (1 before 0).
        assert_eq!(findings[0].rule, rules::STUCK_NEUTRAL);
        assert_eq!(findings[1].rule, rules::UNSOLVED_META);
    }

    #[test]
    fn flags_universe_index() {
        let mut ast = Ast::new();
        ast.push(node("value", json!({"__type__": 1})));
        let findings = lint_exec_artifact(&Language::new(), &ast, AstRef::new(0));
        assert_eq!(findings[0].rule, rules::UNIVERSE_INDEX);
    }

    #[test]
    fn flags_type_level_node_kind() {
        let lang = typed_lang();
        let mut ast = Ast::new();
        ast.push(node("value", json!("A")));
        ast.push(node("value", json!("B")));
        ast.push(node("pi", json!({"children": [0, 1]})));
        let findings = lint_exec_artifact(&lang, &ast, AstRef::new(2));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, rules::TYPE_LEVEL_NODE);
        assert_eq!(findings[0].node, 2);
    }

    #[test]
    fn clean_artifact_passes() {
        let lang = typed_lang();
        let mut ast = Ast::new();
        ast.push(node("value", json!(42)));
        ast.push(node("value", json!({"__tag__": "type.number"})));
        ast.push(node("value", json!({"__sym__": {"handle": 1, "name": "Int"}})));
        ast.push(node(
            "value",
            json!({"__arrow__": {"params": ["x"], "binds": {}}}),
        ));
        ast.push(node("block", json!({"children": [0, 1, 2, 3]})));
        assert!(check_exec_artifact(&lang, &ast).is_ok());
    }

    #[test]
    fn literal_value_kind_not_flagged() {
        // `value` has a `lang.types` rule and no `lang.eval` scheme in
        // `typed_lang`, but its payload is literal (no child refs) — the
        // no-scheme fallback returning it is honest eval, not a leak.
        let lang = typed_lang();
        let mut ast = Ast::new();
        ast.push(node("value", json!(42)));
        assert!(lint_exec_artifact(&lang, &ast, AstRef::new(0)).is_empty());
    }

    #[test]
    fn unreachable_orphan_is_not_linted() {
        // Node 1 (`pi`) is never referenced from the root — an orphaned
        // (erased/dead) subtree must not produce findings.
        let lang = typed_lang();
        let mut ast = Ast::new();
        ast.push(node("value", json!(42)));
        ast.push(node("pi", json!({"children": [0, 0]})));
        ast.push(node("value", json!(7)));
        assert!(lint_exec_artifact(&lang, &ast, AstRef::new(2)).is_empty());
    }

    #[test]
    fn follows_body_arena_ref() {
        let lang = typed_lang();
        let mut ast = Ast::new();
        ast.push(node("value", json!({"__meta__": 0})));
        ast.push(node(
            "function",
            json!({"params": ["x"], "body": 0}),
        ));
        let findings = lint_exec_artifact(&lang, &ast, AstRef::new(1));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].node, 0);
    }

    #[test]
    fn marker_key_as_string_value_is_not_flagged() {
        // `{"key": "__former__"}` spells a runtime-minted object's field
        // name — data, not the encoding.
        let mut ast = Ast::new();
        ast.push(node("entry", json!({"key": "__former__", "children": []})));
        assert!(lint_exec_artifact(&Language::new(), &ast, AstRef::new(0)).is_empty());
    }

    #[test]
    fn check_reports_error_with_context() {
        let mut ast = Ast::new();
        ast.push(node("value", json!({"__meta__": 0})));
        let err = check_exec_artifact(&Language::new(), &ast).unwrap_err();
        assert!(err.to_string().contains("artifact.unsolved-meta"));
    }

    #[test]
    fn eval_schemed_kind_is_not_flagged() {
        // A kind with both a type rule *and* an eval scheme is runnable.
        let mut lang = typed_lang();
        lang.eval
            .merge(EvalSchemeDef::new(
                "eval.pi",
                "pi",
                Term::constant(json!(null)),
            ))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(node("value", json!("A")));
        ast.push(node("pi", json!({"children": [0]})));
        assert!(lint_exec_artifact(&lang, &ast, AstRef::new(1)).is_empty());
    }
}
