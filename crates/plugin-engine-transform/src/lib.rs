//! # handl-plugin-engine-transform
//!
//! The **abstraction** crate for the AST transform operation (ADR-0094).
//!
//! Splits the generic transform engine into an abstraction (operation marker +
//! request/output types + trait) and a future implementation crate
//! (`plugin-engine-transform-rewrite`). This crate defines the [`Transform`]
//! operation marker, [`TransformRequest`], the [`TransformEngine`] trait,
//! [`RuleSource`], and the [`NativeRewrite`] seam. **No traversal or rewrite
//! code lives here** — the abstraction is the contract; the implementation
//! crate will interpret [`TransformRule`] data against a [`Language`].
//!
//! Engines consume [`handl_language::Language`] data — they look up
//! `lang.transforms` and `lang.ast` to dispatch rules, never match on a static
//! Rust enum of canonical kinds.

#![warn(missing_docs)]

pub mod native;
pub mod output;
pub mod request;
pub mod source;
pub mod validate;

use std::collections::BTreeMap;
use std::sync::Arc;

use handl_language::{AstRef, LangError, Language};

use crate::native::NativeRewrite;
use crate::output::{TransformOutput};
use crate::request::TransformRequest;

/// The transform operation marker.
///
/// A zero-sized type identifying an AST-to-AST rewrite: it never evaluates,
/// typechecks, or emits. It carries no state; its purpose is to namespace the
/// transform abstraction and pair with [`TransformRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Transform;

/// A single site where a transform rule would fire.
///
/// Returned by [`TransformEngine::find_matches`] for lint, LSP, and
/// match-preview use cases. `bindings` maps pattern variables (role or index
/// names) to the input `AstRef`s they matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The input node the rule matches.
    pub at: AstRef,
    /// The id of the matching [`TransformDef`](handl_language::TransformDef).
    pub rule_id: String,
    /// Pattern bindings for the match (currently by role/index name).
    pub bindings: BTreeMap<String, AstRef>,
}

/// The trait a transform engine implements.
///
/// An engine looks up `lang.transforms` and rewrites `req.ast` according to the
/// requested [`TransformMode`]. It is pure data-driven: it does not match on a
/// static `CanonicalKind` enum.
pub trait TransformEngine {
    /// Apply the transform described by `req` to `req.ast`.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on transform failure (unknown rule, unsupported
    /// mode, native-rewrite error, etc.).
    fn transform(&self, lang: &Language, req: &TransformRequest) -> Result<TransformOutput, LangError>;

    /// Match-only: find every site where a rule would fire, without rewriting.
    ///
    /// Default: returns an empty vector. Implementations override this for
    /// lint, LSP "find references to pattern", and preview use cases.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure.
    fn find_matches(&self, _lang: &Language, _req: &TransformRequest) -> Result<Vec<Match>, LangError> {
        Ok(Vec::new())
    }

    /// Produce an owned boxed clone of this engine (ADR-0047 §2).
    fn box_clone(&self) -> Box<dyn TransformEngine>;

    /// Produce a fresh owned engine with the given native-rewrite registry.
    ///
    /// Default: clone + ignore (the engine does not wire in the
    /// [`NativeRewrite`] registry). Implementations override this to enable
    /// `TransformRule::Native` dispatch.
    fn with_natives(&self, _natives: Arc<dyn NativeRewrite>) -> Box<dyn TransformEngine> {
        self.box_clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::{NativeRewriteRegistry, RewriteCtx};
    use crate::output::{Rewrite, RewriteTrace};
    use crate::validate::validate_transforms;
    use handl_language::{Ast, AstNode, AstRef, AstShapeDef, ChildRoleSpec, Language, TransformDef, TransformRule};
    use serde_json::Value;

    #[test]
    fn transform_marker_constructs() {
        let _ = Transform;
        let t1 = Transform;
        let t2 = Transform;
        assert_eq!(t1, t2);
        assert_eq!(t1, Transform::default());
    }

    #[test]
    fn match_constructs() {
        let m = Match {
            at: AstRef::new(7),
            rule_id: "r".into(),
            bindings: BTreeMap::new(),
        };
        assert_eq!(m.at, AstRef::new(7));
        assert_eq!(m.rule_id, "r");
    }

    #[test]
    fn trace_new_ref_lookup() {
        let trace = RewriteTrace {
            rewrites: vec![
                Rewrite {
                    from: AstRef::new(1),
                    to: AstRef::new(2),
                    rule_id: "r".into(),
                },
                Rewrite {
                    from: AstRef::new(3),
                    to: AstRef::new(4),
                    rule_id: "s".into(),
                },
            ],
        };
        assert_eq!(trace.new_ref(AstRef::new(1)), Some(AstRef::new(2)));
        assert_eq!(trace.new_ref(AstRef::new(3)), Some(AstRef::new(4)));
        assert_eq!(trace.new_ref(AstRef::new(99)), None);
    }

    #[test]
    fn native_registry_dispatch_by_name() {
        let mut registry = NativeRewriteRegistry::new();
        registry.register(
            "one",
            Box::new(|ctx: &mut RewriteCtx| {
                let idx = ctx.push_leaf("lit", Value::from(1));
                Ok(AstRef::new(idx))
            }),
        );

        let lang = Language::new();
        let input = Ast::new();
        let mut output = Ast::new();
        let node = AstNode::new("lit");
        let remap: Vec<Option<usize>> = Vec::new();
        let mut ctx = RewriteCtx::new(&lang, &node, &input, &mut output, &remap);

        let got = registry.rewrite("one", &mut ctx).unwrap();
        assert!(registry.rewrite("missing", &mut ctx).is_err());
        drop(ctx);

        assert_eq!(output.at(got).unwrap().kind, "lit");
    }

    fn test_language() -> Language {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.old", "old").with_children(vec![ChildRoleSpec {
                role: "only".into(),
                variadic: false,
            }]))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.new", "new"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.cycle", "cycle"))
            .unwrap();
        lang
    }

    #[test]
    fn validate_warns_unknown_target_kind() {
        let mut lang = test_language();
        lang.transforms
            .merge(TransformDef::new(
                "t.unknown",
                "desugar",
                "old",
                TransformRule::Declarative {
                    target_kind: "missing".into(),
                    children: vec![handl_language::RewriteStep::Keep(0)],
                    payload: None,
                },
            ))
            .unwrap();
        let diags = validate_transforms(&lang);
        assert!(diags.iter().any(|d| d.message.contains("missing") && d.message.contains("unknown kind")));
    }

    #[test]
    fn validate_errors_out_of_bounds_keep() {
        let mut lang = test_language();
        lang.transforms
            .merge(TransformDef::new(
                "t.oob",
                "desugar",
                "old",
                TransformRule::Declarative {
                    target_kind: "new".into(),
                    children: vec![handl_language::RewriteStep::Keep(1)],
                    payload: None,
                },
            ))
            .unwrap();
        let diags = validate_transforms(&lang);
        assert!(diags.iter().any(|d| d.message.contains("Keep(1)") && d.message.contains("out of bounds")));
    }

    #[test]
    fn validate_errors_dangling_apply_rule_id() {
        let mut lang = test_language();
        lang.transforms
            .merge(TransformDef::new(
                "t.apply",
                "desugar",
                "old",
                TransformRule::Declarative {
                    target_kind: "new".into(),
                    children: vec![handl_language::RewriteStep::Apply {
                        rule_id: "no-such-rule".into(),
                        index: 0,
                    }],
                    payload: None,
                },
            ))
            .unwrap();
        let diags = validate_transforms(&lang);
        assert!(diags.iter().any(|d| d.message.contains("no-such-rule")));
    }

    #[test]
    fn validate_warns_on_potential_cycle() {
        let mut lang = test_language();
        lang.transforms
            .merge(TransformDef::new(
                "t.cycle",
                "desugar",
                "old",
                TransformRule::Declarative {
                    target_kind: "cycle".into(),
                    children: vec![handl_language::RewriteStep::Keep(0)],
                    payload: None,
                },
            ))
            .unwrap();
        lang.transforms
            .merge(TransformDef::new(
                "t.cycle-target",
                "desugar",
                "cycle",
                TransformRule::Declarative {
                    target_kind: "new".into(),
                    children: vec![],
                    payload: None,
                },
            ))
            .unwrap();
        let diags = validate_transforms(&lang);
        assert!(diags.iter().any(|d| d.message.contains("potential cycle")));
    }
}
