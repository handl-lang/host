//! Request and mode types for the transform operation (ADR-0094).

use handl_language::{Ast, AstRef};
use serde_json::Value;

/// Request passed to a [`TransformEngine`](crate::TransformEngine).
///
/// Carries the input [`Ast`], the root [`AstRef`], the named `pass` (ignored
/// for [`TransformMode::Pipeline`] and [`TransformMode::ApplyAt`]), the
/// requested [`TransformMode`], and an opaque `options` map.
#[derive(Debug, Clone)]
pub struct TransformRequest {
    /// The AST to transform.
    pub ast: Ast,
    /// The root node of `ast`.
    pub root: AstRef,
    /// The pass to run (ignored for [`TransformMode::Pipeline`] and
    /// [`TransformMode::ApplyAt`]).
    pub pass: String,
    /// The transform mode.
    pub mode: TransformMode,
    /// Engine-specific options (opaque to this crate).
    pub options: Value,
}

impl TransformRequest {
    /// Create a new request for a single pass.
    ///
    /// `mode` is set to [`TransformMode::Pass`]; `options` is `Null`.
    pub fn new(ast: Ast, root: AstRef, pass: impl Into<String>) -> Self {
        Self {
            ast,
            root,
            pass: pass.into(),
            mode: TransformMode::Pass,
            options: Value::Null,
        }
    }

    /// Attach an options value (builder style).
    pub fn with_options(mut self, options: Value) -> Self {
        self.options = options;
        self
    }

    /// The requested pass name.
    pub fn pass(&self) -> &str {
        &self.pass
    }
}

/// The mode of a transform request.
///
/// - [`Pass`](TransformMode::Pass): run a single named pass.
/// - [`Pipeline`](TransformMode::Pipeline): run an ordered list of passes,
///   feeding each pass's output AST into the next.
/// - [`ApplyAt`](TransformMode::ApplyAt): apply a single rule at one node
///   (surgical rewrite for LSP/codemod use cases).
/// - [`Plan`](TransformMode::Plan): match-only mode returning proposed
///   rewrites without allocating a new arena.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformMode {
    /// Run one named pass.
    Pass,
    /// Run an ordered sequence of passes, output-to-input.
    Pipeline(Vec<String>),
    /// Apply a specific rule at a specific node.
    ApplyAt {
        /// The node to rewrite.
        at: AstRef,
        /// The id of the [`TransformDef`](handl_language::TransformDef) to
        /// apply.
        rule_id: String,
    },
    /// Return matched sites + proposed rewrites, no new arena.
    Plan,
}
