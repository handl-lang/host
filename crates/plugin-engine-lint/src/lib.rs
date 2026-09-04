//! # handl-plugin-engine-lint
//!
//! The data-driven AST lint engine (ADR-0044 §3). Walks an [`Ast`] using
//! [`handl_language::Language`] data to produce expression-level diagnostics.
//!
//! The engine is **fully data-driven**: what constitutes a binding, a
//! reference, a terminal statement, or a block is resolved from
//! `lang.ast`/`lang.grammar` — never from hardcoded `kind ==` checks. The
//! lint rules themselves are part of the fixed tooling vocabulary (Rust), but
//! the *classification* of nodes is data-driven via `payload_schema`
//! conventions (`"binding": true`, `"reference": true`, `"terminates": true`)
//! and named child roles (`statements`).

use std::collections::HashMap;

use handl_language::{Ast, AstNode, AstRef, GrammarPosition, LangError, Language, Span};

/// Severity of a lint diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// Error-level diagnostic.
    Error,
    /// Warning-level diagnostic.
    Warning,
    /// Info-level diagnostic.
    Info,
}

/// A lint diagnostic with severity, message, and optional source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Diagnostic message.
    pub message: String,
    /// Optional source span (byte offsets).
    pub span: Option<Span>,
}

impl LintDiagnostic {
    fn warning(msg: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            message: msg.into(),
            span,
        }
    }
}

/// The data-driven AST lint engine.
///
/// Reads `Language` data to classify nodes (bindings, references, terminals,
/// blocks) and walks an [`Ast`] recursively to produce [`LintDiagnostic`]s.
/// Construct with [`LintEngine::new`] and run with [`LintEngine::lint`].
#[derive(Debug, Clone, Copy, Default)]
pub struct LintEngine;

impl LintEngine {
    /// Create a new lint engine with default options.
    pub fn new() -> Self {
        Self
    }

    /// Lint the subtree rooted at `root` in `ast` using `lang` data.
    ///
    /// Walks the AST recursively, resolving node classification and child
    /// roles from `lang.ast`/`lang.grammar`. Returns a vector of
    /// [`LintDiagnostic`]s in tree-walk order.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if an arena index is out of bounds.
    pub fn lint(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
    ) -> Result<Vec<LintDiagnostic>, LangError> {
        let classifier = NodeClassifier::build(lang);
        let mut diags = Vec::new();
        let mut scope = ScopeStack::new();
        let mut bindings: Vec<(String, Option<Span>)> = Vec::new();
        let mut references: Vec<String> = Vec::new();
        self.lint_node(
            lang,
            ast,
            root,
            &classifier,
            &mut scope,
            &mut diags,
            &mut bindings,
            &mut references,
        )?;
        // Rule: unused bindings — report bindings whose name is never
        // referenced anywhere in the tree. This is a conservative global
        // check (avoids false positives from scope-tracking complexity).
        let used: std::collections::HashSet<&str> =
            references.iter().map(|s| s.as_str()).collect();
        for (name, span) in &bindings {
            if !used.contains(name.as_str()) {
                diags.push(LintDiagnostic::warning(
                    format!("binding '{}' is never used", name),
                    *span,
                ));
            }
        }
        Ok(diags)
    }

    #[allow(clippy::too_many_arguments)]
    fn lint_node(
        &self,
        lang: &Language,
        ast: &Ast,
        node_ref: AstRef,
        classifier: &NodeClassifier,
        scope: &mut ScopeStack,
        diags: &mut Vec<LintDiagnostic>,
        bindings: &mut Vec<(String, Option<Span>)>,
        references: &mut Vec<String>,
    ) -> Result<(), LangError> {
        let node = ast.at(node_ref)?;

        // Rule: empty block warning.
        if classifier.is_block(lang, node) {
            let children = child_indices(node);
            if children.is_empty() {
                diags.push(LintDiagnostic::warning("empty block", node.span));
            }
        }

        // Rule: unreachable code after a terminal statement (block siblings).
        if classifier.is_block(lang, node) {
            self.check_unreachable(ast, node, classifier, diags)?;
        }

        // Rule: shadowing warning (binding shadows an existing binding in any
        // enclosing scope — checked before recording this binding).
        if let Some(name) = classifier.binding_name(node) {
            if scope.shadows(&name) {
                diags.push(LintDiagnostic::warning(
                    format!("binding '{}' shadows an outer binding", name),
                    node.span,
                ));
            }
            bindings.push((name.clone(), node.span));
        }

        // Track references for the unused-binding rule.
        if let Some(name) = classifier.reference_name(node) {
            references.push(name);
        }

        // Enter a new scope for block-shaped nodes.
        let is_block = classifier.is_block(lang, node);
        if is_block {
            scope.push();
        }

        // Record this node's binding in the current scope before walking
        // children (the body can reference it).
        if let Some(name) = classifier.binding_name(node) {
            scope.bind(&name);
        }

        // Recurse into children.
        for child_idx in child_indices(node) {
            self.lint_node(
                lang,
                ast,
                AstRef::new(child_idx),
                classifier,
                scope,
                diags,
                bindings,
                references,
            )?;
        }

        if is_block {
            scope.pop();
        }

        Ok(())
    }

    fn check_unreachable(
        &self,
        ast: &Ast,
        node: &AstNode,
        classifier: &NodeClassifier,
        diags: &mut Vec<LintDiagnostic>,
    ) -> Result<(), LangError> {
        let children = child_indices(node);
        let mut found_terminal = false;
        for &child_idx in &children {
            let child = ast.at(AstRef::new(child_idx))?;
            if found_terminal {
                diags.push(LintDiagnostic::warning(
                    "unreachable code after terminal statement",
                    child.span,
                ));
                // Only report the first unreachable sibling to avoid noise.
                break;
            }
            if classifier.is_terminal(child) {
                found_terminal = true;
            }
        }
        Ok(())
    }
}

/// Extract the `payload["children"]` arena indices from a node.
fn child_indices(node: &AstNode) -> Vec<usize> {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default()
}

/// A precomputed classification table mapping node kinds to their lint roles
/// (binding, reference, terminal, block), built from `Language` data.
struct NodeClassifier {
    /// node_kind → the payload field holding the binding name.
    binding_fields: HashMap<String, String>,
    /// node_kind → whether the shape declares `binding: true` in payload_schema.
    bindings: HashMap<String, bool>,
    /// node_kind → whether the shape declares `reference: true`.
    references: HashMap<String, bool>,
    /// node_kind → whether the shape declares `terminates: true`.
    terminals: HashMap<String, bool>,
    /// node_kind → whether the shape declares a variadic `statements` role.
    blocks: HashMap<String, bool>,
}

impl NodeClassifier {
    /// Build the classifier by scanning `lang.ast` and `lang.grammar`.
    fn build(lang: &Language) -> Self {
        let mut binding_fields: HashMap<String, String> = HashMap::new();
        let mut bindings: HashMap<String, bool> = HashMap::new();
        let mut references: HashMap<String, bool> = HashMap::new();
        let mut terminals: HashMap<String, bool> = HashMap::new();
        let mut blocks: HashMap<String, bool> = HashMap::new();

        // AST shapes: payload_schema conventions + variadic `statements` role.
        for shape in lang.ast.iter() {
            let kind = &shape.node_kind;
            if schema_flag(&shape.payload_schema, "binding") {
                bindings.insert(kind.clone(), true);
            }
            if schema_flag(&shape.payload_schema, "reference") {
                references.insert(kind.clone(), true);
            }
            if schema_flag(&shape.payload_schema, "terminates") {
                terminals.insert(kind.clone(), true);
            }
            if shape.role_is_variadic("statements") {
                blocks.insert(kind.clone(), true);
            }
        }

        // Grammar rows: prefix rows with payload_tokens carrying a `field`
        // store the binding name in that field. Map node_kind → field name.
        for row in lang.grammar.iter() {
            if row.position == GrammarPosition::Prefix {
                for spec in &row.child_shape.payload_tokens {
                    if let Some(field) = &spec.field {
                        binding_fields.insert(row.child_shape.node_kind.clone(), field.clone());
                    }
                }
            }
        }

        Self {
            binding_fields,
            bindings,
            references,
            terminals,
            blocks,
        }
    }

    /// Whether `node` is block-shaped (variadic `statements` role).
    fn is_block(&self, lang: &Language, node: &AstNode) -> bool {
        self.blocks.get(&node.kind).copied().unwrap_or(false)
            || lang
                .ast
                .get_by_identity(&node.kind)
                .map(|s| s.role_is_variadic("statements"))
                .unwrap_or(false)
    }

    /// Whether `node` is a terminal statement (`terminates: true`).
    fn is_terminal(&self, node: &AstNode) -> bool {
        self.terminals.get(&node.kind).copied().unwrap_or(false)
    }

    /// The binding name of `node`, if it is a binding.
    ///
    /// A node is a binding if its shape declares `binding: true` in
    /// `payload_schema`, or if a grammar row maps its kind to a payload field.
    /// The name is read from `node.payload[field]` (field resolved from
    /// grammar `payload_tokens`), falling back to `node.payload["name"]`.
    fn binding_name(&self, node: &AstNode) -> Option<String> {
        let is_binding = self.bindings.get(&node.kind).copied().unwrap_or(false)
            || self.binding_fields.contains_key(&node.kind);
        if !is_binding {
            return None;
        }
        let field = self.binding_fields.get(&node.kind);
        if let Some(f) = field
            && let Some(name) = node.payload.get(f).and_then(|v| v.as_str())
        {
            return Some(name.to_string());
        }
        node.payload
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// The referenced name of `node`, if it is a reference (`reference: true`).
    ///
    /// The name is read from `node.payload["value"]` or `node.payload["name"]`.
    fn reference_name(&self, node: &AstNode) -> Option<String> {
        if !self.references.get(&node.kind).copied().unwrap_or(false) {
            return None;
        }
        node.payload
            .get("value")
            .and_then(|v| v.as_str())
            .or_else(|| node.payload.get("name").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
    }
}

/// Read a boolean flag from a JSON `payload_schema` object property.
fn schema_flag(schema: &serde_json::Value, key: &str) -> bool {
    schema
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// A scope stack for tracking bindings during the tree walk (used for the
/// shadowing rule).
struct ScopeStack {
    scopes: Vec<Vec<String>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![Vec::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn bind(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name.to_string());
        }
    }

    /// Whether `name` is already bound in any scope on the stack (including
    /// the current scope — checked before recording this binding).
    fn shadows(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .any(|s| s.contains(&name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        Ast, AstNode, AstShapeDef, AstShapeRef, ChildRoleSpec, GrammarPosition, GrammarRow,
        Language, PayloadTokenSpec,
    };
    use serde_json::json;

    /// Build a minimal test language with `block`, `let`, `var`, `return`,
    /// and `lit` shapes, plus grammar rows for binding-name extraction.
    fn test_lang() -> Language {
        let mut lang = Language::new();

        // block: variadic `statements` role.
        lang.ast
            .merge(
                AstShapeDef::new("ast.block", "block").with_children(vec![ChildRoleSpec {
                    role: "statements".into(),
                    variadic: true,
                }]),
            )
            .unwrap();

        // let: binding shape (payload_schema binding: true), children [value, body].
        lang.ast
            .merge(
                AstShapeDef::new("ast.let", "let")
                    .with_payload_schema(json!({"binding": true}))
                    .with_children(vec![
                        ChildRoleSpec {
                            role: "value".into(),
                            variadic: false,
                        },
                        ChildRoleSpec {
                            role: "body".into(),
                            variadic: false,
                        },
                    ]),
            )
            .unwrap();

        // var: reference shape (payload_schema reference: true), arity 0.
        lang.ast
            .merge(
                AstShapeDef::new("ast.var", "var")
                    .with_payload_schema(json!({"reference": true})),
            )
            .unwrap();

        // return: terminal shape (payload_schema terminates: true), arity 1.
        lang.ast
            .merge(
                AstShapeDef::new("ast.return", "return")
                    .with_payload_schema(json!({"terminates": true})),
            )
            .unwrap();

        // lit: plain literal, arity 0.
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();

        // Grammar: let prefix with payload_tokens (field: "name").
        lang.grammar
            .merge(GrammarRow::new(
                "grammar.let",
                "token.keyword.let",
                GrammarPosition::Prefix,
                AstShapeRef::new("let", 2)
                    .with_payload_tokens(vec![PayloadTokenSpec {
                        token: "token.identifier".into(),
                        field: Some("name".into()),
                    }]),
            ))
            .unwrap();

        lang
    }

    /// A language with NO binding/reference/terminal data — rules must skip.
    fn bare_lang() -> Language {
        let mut lang = Language::new();
        lang.ast
            .merge(
                AstShapeDef::new("ast.block", "block").with_children(vec![ChildRoleSpec {
                    role: "statements".into(),
                    variadic: true,
                }]),
            )
            .unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang
    }

    #[test]
    fn empty_block_warning() {
        let lang = test_lang();
        let mut ast = Ast::new();
        ast.push(AstNode::new("block").with_payload(json!({"children": []})));
        let root = AstRef::new(0);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Warning);
        assert!(diags[0].message.contains("empty block"));
    }

    #[test]
    fn unreachable_code_after_return() {
        let lang = test_lang();
        let mut ast = Ast::new();
        // 0: lit(42)
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        // 1: return {children: [0]}
        ast.push(AstNode::new("return").with_payload(json!({"children": [0]})));
        // 2: lit(1)
        ast.push(AstNode::new("lit").with_payload(json!(1)));
        // 3: block {children: [1, 2]}
        ast.push(AstNode::new("block").with_payload(json!({"children": [1, 2]})));
        let root = AstRef::new(3);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        let unreachable: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unreachable"))
            .collect();
        assert_eq!(unreachable.len(), 1);
        assert_eq!(unreachable[0].level, DiagnosticLevel::Warning);
    }

    #[test]
    fn unused_binding_warning() {
        let lang = test_lang();
        let mut ast = Ast::new();
        // 0: lit(42) — let value
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        // 1: lit(1) — let body (never references x)
        ast.push(AstNode::new("lit").with_payload(json!(1)));
        // 2: let {name: "x", children: [0, 1]}
        ast.push(
            AstNode::new("let")
                .with_payload(json!({"name": "x", "children": [0, 1]})),
        );
        let root = AstRef::new(2);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("never used"))
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains("'x'"));
    }

    #[test]
    fn shadowing_warning() {
        let lang = test_lang();
        let mut ast = Ast::new();
        // 0: lit(2)  — outer let value
        ast.push(AstNode::new("lit").with_payload(json!(2)));
        // 1: lit(1)  — inner let value
        ast.push(AstNode::new("lit").with_payload(json!(1)));
        // 2: var(x)  — inner body references x
        ast.push(AstNode::new("var").with_payload(json!({"value": "x"})));
        // 3: inner let {name: "x", children: [1, 2]}
        ast.push(
            AstNode::new("let")
                .with_payload(json!({"name": "x", "children": [1, 2]})),
        );
        // 4: outer let {name: "x", children: [0, 3]}
        ast.push(
            AstNode::new("let")
                .with_payload(json!({"name": "x", "children": [0, 3]})),
        );
        let root = AstRef::new(4);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        let shadow: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("shadows"))
            .collect();
        assert_eq!(shadow.len(), 1);
        assert!(shadow[0].message.contains("'x'"));
    }

    #[test]
    fn no_false_positives() {
        let lang = test_lang();
        let mut ast = Ast::new();
        // 0: lit(42)
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        // 1: var(x) — references x
        ast.push(AstNode::new("var").with_payload(json!({"value": "x"})));
        // 2: let {name: "x", children: [0, 1]}
        ast.push(
            AstNode::new("let")
                .with_payload(json!({"name": "x", "children": [0, 1]})),
        );
        // 3: block {children: [2]}
        ast.push(AstNode::new("block").with_payload(json!({"children": [2]})));
        let root = AstRef::new(3);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
    }

    #[test]
    fn data_driven_skip_when_no_flags() {
        let lang = bare_lang();
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        ast.push(AstNode::new("lit").with_payload(json!(1)));
        // A "block" with children — no binding/reference/terminal data, so
        // only the empty-block rule could fire, and this block is non-empty.
        ast.push(AstNode::new("block").with_payload(json!({"children": [0, 1]})));
        let root = AstRef::new(2);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
    }

    #[test]
    fn data_driven_skip_empty_block_still_works() {
        // The empty-block rule is driven by the variadic `statements` role,
        // which IS present in the bare lang. So an empty block still warns.
        let lang = bare_lang();
        let mut ast = Ast::new();
        ast.push(AstNode::new("block").with_payload(json!({"children": []})));
        let root = AstRef::new(0);
        let diags = LintEngine::new().lint(&lang, &ast, root).unwrap();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("empty block"));
    }
}
