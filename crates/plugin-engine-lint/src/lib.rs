//! # handl-plugin-engine-lint
//!
//! The data-driven AST lint engine (ADR-0044 §3). Walks an [`Ast`] using
//! [`handl_language::Language`] data to produce expression-level diagnostics.
//!
//! The engine is **fully data-driven**: what constitutes a binding, a
//! reference, a terminal statement, or a block is resolved from
//! `lang.ast`/`lang.grammar` — never from hardcoded `kind ==` checks.
//!
//! Lint rules come in two layers (ADR-0097 — Lint Rules as Language Data):
//!
//! - **Data rules** in `lang.lints` ([`LintRuleDef`](handl_language::LintRuleDef)s,
//!   merged from `lints.yaml`): `!Match` checks are node-local predicates in
//!   the ADR-0094 `TransformCondition` vocabulary; `!Native` checks resolve
//!   against a [`NativeLintRegistry`] (the `desugar.*`-native analogue).
//!   [`LintEngine::lint_module`] / [`LintEngine::lint_source`] run them.
//! - **Built-in checks** (empty block, unreachable code, shadowing, unused
//!   bindings) are themselves declared as `!Native "builtin.*"` rules in
//!   `lang.lints`; the canonical `lints.yaml` ships them, and the engine
//!   registers the implementations in [`canonical_lint_natives`].
//!   [`LintEngine::lint`] wraps the single AST as a one-statement
//!   [`LintModule`] and dispatches through [`LintEngine::lint_module`].

use std::collections::HashMap;

use handl_language::{Ast, AstNode, AstRef, GrammarPosition, LangError, Language, LintCheck, Span};

/// Module-level linting: statement splitting, parsing, and the shared
/// per-statement [`ModuleAnalysis`](module::ModuleAnalysis).
pub mod module;
/// Native lint rules and the [`NativeLintRegistry`](native::NativeLintRegistry).
pub mod native;

pub use module::{LintModule, LintStatement, ModuleAnalysis, parse_module};
pub use native::{
    CANONICAL_LINT_NATIVES, LintCtx, NativeHit, NativeLintFn, NativeLintRegistry,
    NativeLintResolver, canonical_lint_natives,
};

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

/// A source fix attached to a [`LintDiagnostic`]: replace the absolute byte
/// range `start..end` with `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFixOut {
    /// Absolute start byte offset.
    pub start: usize,
    /// Absolute end byte offset.
    pub end: usize,
    /// Replacement text.
    pub replacement: String,
}

/// A lint diagnostic with severity, message, optional source span, the name
/// of the rule that produced it, and an optional fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Diagnostic message.
    pub message: String,
    /// Optional source span (byte offsets).
    pub span: Option<Span>,
    /// The reported rule name (`LintRuleDef::name`; a builtin name for the
    /// fixed vocabulary checks).
    pub rule: String,
    /// Optional fix.
    pub fix: Option<LintFixOut>,
}

/// Parse a `LintRuleDef` severity label into a [`DiagnosticLevel`].
fn severity_of(severity: &str, rule_id: &str) -> Result<DiagnosticLevel, LangError> {
    match severity {
        "error" => Ok(DiagnosticLevel::Error),
        "warning" => Ok(DiagnosticLevel::Warning),
        "info" => Ok(DiagnosticLevel::Info),
        other => Err(LangError::engine(format!(
            "lint rule {}: unknown severity '{}'",
            rule_id, other
        ))),
    }
}

/// Render a native rule's message template: every `{key}` placeholder is
/// replaced by `args[key]`.
fn render_native_message(
    template: &str,
    args: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut out = template.to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{}}}", k), v);
    }
    out
}

/// The data-driven AST lint engine.
///
/// Reads `Language` data to classify nodes (bindings, references, terminals,
/// blocks) and dispatches `lang.lints` rules to produce [`LintDiagnostic`]s.
/// [`lint`](Self::lint) wraps a single AST as a one-statement module and
/// runs the same rule dispatch; [`lint_module`](Self::lint_module) /
/// [`lint_source`](Self::lint_source) run the `lang.lints` rule table over a
/// whole module.
///
/// `!Native` checks resolve against the engine's [`NativeLintRegistry`] —
/// wire it with [`with_native_registry`](Self::with_native_registry) or
/// [`with_canonical_lint_natives`](Self::with_canonical_lint_natives).
#[derive(Debug, Clone, Default)]
pub struct LintEngine {
    natives: NativeLintRegistry,
}

impl LintEngine {
    /// Create a new lint engine with no native rules registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wire an additional native-lint registry (its entries override
    /// existing names).
    pub fn with_native_registry(mut self, natives: NativeLintRegistry) -> Self {
        self.natives.extend(&natives);
        self
    }

    /// Wire a single native rule by name.
    pub fn with_native(
        mut self,
        name: impl Into<String>,
        f: impl Fn(&LintCtx<'_>) -> Result<Vec<NativeHit>, LangError> + Send + Sync + 'static,
    ) -> Self {
        self.natives.register(name, f);
        self
    }

    /// Wire the canonical surface's native lint rules (`scope.*`,
    /// `naming-convention`, `source.*`, and `builtin.*`) — the analogue of
    /// `RewriteEngine::with_desugar_natives` (ADR-0094).
    pub fn with_canonical_lint_natives(self) -> Self {
        self.with_native_registry(canonical_lint_natives())
    }

    /// Lint a whole module: run every
    /// [`LintRuleDef`](handl_language::LintRuleDef) in `lang.lints`.
    ///
    /// `!Match` rules are evaluated node-locally per statement; `!Native`
    /// rules dispatch to the registry with the shared [`ModuleAnalysis`].
    /// Diagnostics come back in rule order — hosts sort by span.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if a rule declares an unknown severity or a
    /// `!Native` check with no registered implementation.
    pub fn lint_module(
        &self,
        lang: &Language,
        module: &LintModule,
    ) -> Result<Vec<LintDiagnostic>, LangError> {
        let analysis = ModuleAnalysis::build(lang, module);
        let mut diags = Vec::new();
        for rule in lang.lints.iter() {
            let level = severity_of(&rule.severity, &rule.id)?;
            match &rule.check {
                LintCheck::Match { node_kind, when } => {
                    for stmt in &module.statements {
                        module::eval_match_rule(
                            lang,
                            stmt,
                            module.src,
                            rule,
                            node_kind,
                            when.as_ref(),
                            level,
                            &mut diags,
                        );
                    }
                }
                LintCheck::Native(name) => {
                    let f = self.natives.get(name).ok_or_else(|| {
                        LangError::engine(format!(
                            "lint rule {}: no native registered for '{}'",
                            rule.id, name
                        ))
                    })?;
                    let ctx = LintCtx {
                        lang,
                        src: module.src,
                        module: &module.statements,
                        analysis: &analysis,
                    };
                    for hit in f(&ctx)? {
                        diags.push(LintDiagnostic {
                            level,
                            message: render_native_message(&rule.message, &hit.args),
                            span: hit.span,
                            rule: rule.name.clone(),
                            fix: hit.fix,
                        });
                    }
                }
            }
        }
        Ok(diags)
    }

    /// Split `src` into statements, parse each, and lint the module (the
    /// bootstrap one-statement-per-parse convention — see [`parse_module`]).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if any statement fails to lex/parse or a rule
    /// fails (see [`lint_module`](Self::lint_module)).
    pub fn lint_source(
        &self,
        lang: &Language,
        src: &str,
    ) -> Result<Vec<LintDiagnostic>, LangError> {
        let module = parse_module(lang, src)?;
        self.lint_module(lang, &module)
    }

    /// Lint the subtree rooted at `root` in `ast` using `lang` data.
    ///
    /// Wraps the AST as a one-statement [`LintModule`] and dispatches
    /// `lang.lints` through [`lint_module`](Self::lint_module). A language
    /// with no lint rules declared (e.g. an embedded toy language) is
    /// lint-clean; a canonical language with the `builtin.*` rules declared
    /// runs the former built-in checks.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if an arena index is out of bounds or a rule
    /// fails (see [`lint_module`](Self::lint_module)).
    pub fn lint(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
    ) -> Result<Vec<LintDiagnostic>, LangError> {
        let src = "";
        let module = LintModule {
            src,
            statements: vec![LintStatement {
                text: String::new(),
                offset: 0,
                ast: ast.clone(),
                root,
            }],
        };
        self.lint_module(lang, &module)
    }
}

/// A precomputed classification table mapping node kinds to their lint roles
/// (binding, reference, terminal, block), built from `Language` data.
pub(crate) struct NodeClassifier {
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
    pub(crate) fn build(lang: &Language) -> Self {
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
    pub(crate) fn is_block(&self, lang: &Language, node: &AstNode) -> bool {
        self.blocks.get(&node.kind).copied().unwrap_or(false)
            || lang
                .ast
                .get_by_identity(&node.kind)
                .map(|s| s.role_is_variadic("statements"))
                .unwrap_or(false)
    }

    /// Whether `node` is a terminal statement (`terminates: true`).
    pub(crate) fn is_terminal(&self, node: &AstNode) -> bool {
        self.terminals.get(&node.kind).copied().unwrap_or(false)
    }

    /// The binding name of `node`, if it is a binding.
    ///
    /// A node is a binding if its shape declares `binding: true` in
    /// `payload_schema`, if a grammar row maps its kind to a payload field, or
    /// if it carries `target` + `value` child roles (ADR-0081). The name is
    /// resolved from `node.payload[field]`, `node.payload["name"]`, or the
    /// `target` child's `param_name`, never from a hardcoded `kind ==` check.
    pub(crate) fn binding_name(
        &self,
        lang: &Language,
        ast: &Ast,
        node: &AstNode,
    ) -> Option<String> {
        let is_payload_binding = self.bindings.get(&node.kind).copied().unwrap_or(false)
            || self.binding_fields.contains_key(&node.kind);
        if is_payload_binding {
            let field = self.binding_fields.get(&node.kind);
            if let Some(f) = field
                && let Some(name) = node.payload.get(f).and_then(|v| v.as_str())
            {
                return Some(name.to_string());
            }
            return node
                .payload
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        // Data-driven binding: a node with `target` and `value` child roles
        // (e.g. canonical `assign`).
        let target_idx = lang.child_role_index(&node.kind, "target").ok().flatten()?;
        let value_idx = lang.child_role_index(&node.kind, "value").ok().flatten()?;
        let children = crate::module::child_indices(node);
        let &target_ref = children.get(target_idx)?;
        let &value_ref = children.get(value_idx)?;
        // Ensure the target and value are actually present (defensive).
        let _ = value_ref;
        let target_node = ast.at(AstRef::new(target_ref)).ok()?;
        lang.param_name(ast, target_node).ok()
    }

    /// The referenced name of `node`, if it is a reference.
    ///
    /// A reference is a `reference: true` payload, a `pattern_role: "leaf"`,
    /// or a node with a `subject` child role (resolved via `Language::
    /// param_name`).
    pub(crate) fn reference_name(
        &self,
        lang: &Language,
        ast: &Ast,
        node: &AstNode,
    ) -> Option<String> {
        if self.references.get(&node.kind).copied().unwrap_or(false) {
            return node
                .payload
                .get("value")
                .and_then(|v| v.as_str())
                .or_else(|| node.payload.get("name").and_then(|v| v.as_str()))
                .map(|s| s.to_string());
        }

        let is_leaf = lang
            .ast
            .get_by_identity(&node.kind)
            .and_then(|s| s.pattern_role.as_deref())
            == Some("leaf");
        if is_leaf {
            return lang.param_name(ast, node).ok();
        }

        // `subject` role wrappers (e.g. type annotations) are references too.
        lang.child_role_index(&node.kind, "subject")
            .ok()
            .flatten()
            .and_then(|idx| {
                let children = crate::module::child_indices(node);
                children.get(idx).and_then(|&c| {
                    let child = ast.at(AstRef::new(c)).ok()?;
                    lang.param_name(ast, child).ok()
                })
            })
    }
}

/// Read a boolean flag from a JSON `payload_schema` object property.
fn schema_flag(schema: &serde_json::Value, key: &str) -> bool {
    schema.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// A scope stack for tracking bindings during the tree walk (used for the
/// shadowing rule).
pub(crate) struct ScopeStack {
    scopes: Vec<Vec<String>>,
}

impl ScopeStack {
    pub(crate) fn new() -> Self {
        Self {
            scopes: vec![Vec::new()],
        }
    }

    pub(crate) fn push(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub(crate) fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub(crate) fn bind(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name.to_string());
        }
    }

    /// Whether `name` is already bound in any scope on the stack (including
    /// the current scope — checked before recording this binding).
    pub(crate) fn shadows(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(&name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        Ast, AstNode, AstShapeDef, AstShapeRef, ChildRoleSpec, GrammarPosition, GrammarRow,
        Language, LintCheck, LintFix, LintRuleDef, PayloadTokenSpec, TransformCondition,
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
                AstShapeDef::new("ast.var", "var").with_payload_schema(json!({"reference": true})),
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
                AstShapeRef::new("let", 2).with_payload_tokens(vec![PayloadTokenSpec {
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

    /// Add the four `!Native "builtin.*"` rules to `lang`.
    fn add_builtin_lints(lang: &mut Language) {
        let builtins = [
            (
                "lint.builtin.empty-block",
                "empty-block",
                "empty block",
                "builtin.empty-block",
            ),
            (
                "lint.builtin.unreachable-code",
                "unreachable-code",
                "unreachable code after terminal statement",
                "builtin.unreachable-code",
            ),
            (
                "lint.builtin.shadowed-binding",
                "shadowed-binding",
                "binding '{name}' shadows an outer binding",
                "builtin.shadowed-binding",
            ),
            (
                "lint.builtin.unused-binding",
                "unused-binding",
                "binding '{name}' is never used",
                "builtin.unused-binding",
            ),
        ];
        for (id, name, message, native) in builtins {
            lang.lints
                .merge(LintRuleDef::new(
                    id,
                    name,
                    "warning",
                    message,
                    LintCheck::Native(native.into()),
                ))
                .unwrap();
        }
    }

    /// `test_lang` with the four built-in `!Native` rules declared.
    fn test_lang_with_builtins() -> Language {
        let mut lang = test_lang();
        add_builtin_lints(&mut lang);
        lang
    }

    #[test]
    fn empty_block_warning() {
        let lang = test_lang_with_builtins();
        let mut ast = Ast::new();
        ast.push(AstNode::new("block").with_payload(json!({"children": []})));
        let root = AstRef::new(0);
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Warning);
        assert!(diags[0].message.contains("empty block"));
    }

    #[test]
    fn unreachable_code_after_return() {
        let lang = test_lang_with_builtins();
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
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
        let unreachable: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unreachable"))
            .collect();
        assert_eq!(unreachable.len(), 1);
        assert_eq!(unreachable[0].level, DiagnosticLevel::Warning);
    }

    #[test]
    fn unused_binding_warning() {
        let lang = test_lang_with_builtins();
        let mut ast = Ast::new();
        // 0: lit(42) — let value
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        // 1: lit(1) — let body (never references x)
        ast.push(AstNode::new("lit").with_payload(json!(1)));
        // 2: let {name: "x", children: [0, 1]}
        ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})));
        // 3: block {children: [2]} — the let is nested, so its binding is checked.
        ast.push(AstNode::new("block").with_payload(json!({"children": [2]})));
        let root = AstRef::new(3);
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("never used"))
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains("'x'"));
    }

    #[test]
    fn shadowing_warning() {
        let lang = test_lang_with_builtins();
        let mut ast = Ast::new();
        // 0: lit(2)  — outer let value
        ast.push(AstNode::new("lit").with_payload(json!(2)));
        // 1: lit(1)  — inner let value
        ast.push(AstNode::new("lit").with_payload(json!(1)));
        // 2: var(x)  — inner body references x
        ast.push(AstNode::new("var").with_payload(json!({"value": "x"})));
        // 3: inner let {name: "x", children: [1, 2]}
        ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [1, 2]})));
        // 4: outer let {name: "x", children: [0, 3]}
        ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 3]})));
        let root = AstRef::new(4);
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
        let shadow: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("shadows"))
            .collect();
        assert_eq!(shadow.len(), 1);
        assert!(shadow[0].message.contains("'x'"));
    }

    #[test]
    fn no_false_positives() {
        let lang = test_lang_with_builtins();
        let mut ast = Ast::new();
        // 0: lit(42)
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        // 1: var(x) — references x
        ast.push(AstNode::new("var").with_payload(json!({"value": "x"})));
        // 2: let {name: "x", children: [0, 1]}
        ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})));
        // 3: block {children: [2]}
        ast.push(AstNode::new("block").with_payload(json!({"children": [2]})));
        let root = AstRef::new(3);
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
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
        // With no lint rules declared, the engine is lint-clean.
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
    }

    #[test]
    fn data_driven_skip_empty_block_still_works() {
        // The empty-block rule is driven by the variadic `statements` role,
        // which IS present in the bare lang. So an empty block still warns.
        let mut lang = bare_lang();
        add_builtin_lints(&mut lang);
        let mut ast = Ast::new();
        ast.push(AstNode::new("block").with_payload(json!({"children": []})));
        let root = AstRef::new(0);
        let diags = LintEngine::new()
            .with_canonical_lint_natives()
            .lint(&lang, &ast, root)
            .unwrap();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("empty block"));
    }

    // --- ADR-0097: `lang.lints` rule dispatch (lint_module) ----------------

    /// A language with a `paren` shape (role `body`) and a `var` leaf, for
    /// Match-rule tests. The statement `x = (y)` is assembled by hand — the
    /// toy language is not lexable, so we bypass `parse_module`.
    fn paren_lang() -> Language {
        let mut lang = test_lang();
        lang.ast
            .merge(
                AstShapeDef::new("ast.paren", "paren").with_children(vec![ChildRoleSpec {
                    role: "body".into(),
                    variadic: false,
                }]),
            )
            .unwrap();
        lang
    }

    /// The module `x = (y)`: a `paren` node spanning bytes 4..7 wrapping a
    /// `var` leaf `y` at 5..6.
    fn paren_module(src: &str) -> LintModule<'_> {
        let mut ast = Ast::new();
        let var = ast.push(
            AstNode::new("var")
                .with_span(Span { start: 5, end: 6 })
                .with_payload(json!("y")),
        );
        let paren = ast.push(
            AstNode::new("paren")
                .with_span(Span { start: 4, end: 7 })
                .with_payload(json!({"children": [var.index()]})),
        );
        LintModule {
            src,
            statements: vec![LintStatement {
                text: src.to_string(),
                offset: 0,
                ast,
                root: paren,
            }],
        }
    }

    #[test]
    fn lint_module_match_rule_fires_with_declarative_fix() {
        let mut lang = paren_lang();
        lang.lints
            .merge(
                LintRuleDef::new(
                    "lint.test.paren",
                    "simplify-paren",
                    "warning",
                    "redundant parentheses around '{source:body}'",
                    LintCheck::Match {
                        node_kind: "paren".into(),
                        when: Some(TransformCondition::ChildCount(1)),
                    },
                )
                .with_fix(LintFix::Unwrap {
                    index: None,
                    role: Some("body".into()),
                }),
            )
            .unwrap();
        let module = paren_module("x = (y)");
        let diags = LintEngine::new().lint_module(&lang, &module).unwrap();
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.rule, "simplify-paren");
        assert_eq!(d.level, DiagnosticLevel::Warning);
        assert_eq!(d.message, "redundant parentheses around 'y'");
        assert_eq!(d.span, Some(Span { start: 4, end: 7 }));
        assert_eq!(
            d.fix,
            Some(LintFixOut {
                start: 4,
                end: 7,
                replacement: "y".into()
            })
        );
    }

    #[test]
    fn lint_module_match_rule_when_filters() {
        let mut lang = paren_lang();
        lang.lints
            .merge(LintRuleDef::new(
                "lint.test.paren",
                "simplify-paren",
                "warning",
                "matched",
                LintCheck::Match {
                    node_kind: "paren".into(),
                    // The paren has exactly one child — this must not fire.
                    when: Some(TransformCondition::ChildCount(2)),
                },
            ))
            .unwrap();
        let module = paren_module("x = (y)");
        let diags = LintEngine::new().lint_module(&lang, &module).unwrap();
        assert!(diags.is_empty());
    }

    #[test]
    fn lint_module_native_rule_dispatches_and_templates() {
        let mut lang = paren_lang();
        lang.lints
            .merge(LintRuleDef::new(
                "lint.test.native",
                "test-native",
                "info",
                "saw '{name}'",
                LintCheck::Native("test.always".into()),
            ))
            .unwrap();
        let module = paren_module("x = (y)");
        let engine = LintEngine::new().with_native("test.always", |ctx| {
            assert_eq!(ctx.src, "x = (y)");
            assert_eq!(ctx.module.len(), 1);
            Ok(vec![NativeHit::named("thing", Span { start: 0, end: 1 })])
        });
        let diags = engine.lint_module(&lang, &module).unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "test-native");
        assert_eq!(diags[0].level, DiagnosticLevel::Info);
        assert_eq!(diags[0].message, "saw 'thing'");
    }

    #[test]
    fn lint_module_unknown_native_errors() {
        let mut lang = paren_lang();
        lang.lints
            .merge(LintRuleDef::new(
                "lint.test.missing",
                "missing",
                "warning",
                "m",
                LintCheck::Native("test.missing".into()),
            ))
            .unwrap();
        let module = paren_module("x = (y)");
        let err = LintEngine::new()
            .lint_module(&lang, &module)
            .unwrap_err();
        assert!(format!("{err}").contains("test.missing"), "{err}");
    }

    #[test]
    fn lint_module_unknown_severity_errors() {
        let mut lang = paren_lang();
        lang.lints
            .merge(LintRuleDef::new(
                "lint.test.sev",
                "sev",
                "bogus",
                "m",
                LintCheck::Match {
                    node_kind: "paren".into(),
                    when: None,
                },
            ))
            .unwrap();
        let module = paren_module("x = (y)");
        let err = LintEngine::new()
            .lint_module(&lang, &module)
            .unwrap_err();
        assert!(format!("{err}").contains("bogus"), "{err}");
    }
}
