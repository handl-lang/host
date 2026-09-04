//! `LintBehavior` contributor for expression diagnostics, backed by the
//! data-driven [`handl_plugin_engine_lint::LintEngine`].
//!
//! The `ExpressionDiagnostics` plugin contributes a `LintBehavior` rule to
//! `BasicLinter` and exposes a standalone [`lint_ast`] function plus a
//! `Plugin::run` entry point that accepts a serialized `Ast` + `root` index
//! and delegates to the lint engine.

use handl_language::{Ast, AstRef, Language};
use handl_platform::{
    BasicLinter, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, Lint, LintBehavior, LintRequest, PlatformError, Plugin, PluginContext,
};
use handl_plugin_engine_lint::{DiagnosticLevel, LintDiagnostic, LintEngine};

/// Expression `LintBehavior` contributor.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExpressionDiagnostics;

impl ExpressionDiagnostics {
    /// Creates a new expression diagnostics rule.
    pub fn new() -> Self {
        Self
    }
}

impl Behavior<Lint> for ExpressionDiagnostics {
    fn kind(&self) -> &str {
        "lint.expression"
    }

    fn apply(&self, op: &mut LintRequest) -> Result<(), PlatformError> {
        // The `LintRequest` carries a single `AstNode` with no arena, so full
        // tree-walking is not possible here. If the payload contains a
        // serialized `ast` arena + `root` index, delegate to the engine;
        // otherwise this is a no-op (backward compatible).
        if let Some(ast) = op.ast.payload.get("ast")
            && let Ok(ast) = serde_json::from_value::<Ast>(ast.clone())
            && let Some(root) = op
                .ast
                .payload
                .get("root")
                .and_then(|v| v.as_u64())
                .map(|n| AstRef::new(n as usize))
        {
            // A minimal empty language is used here — the real
            // language data is only available via the `Plugin::run`
            // path or the standalone `lint_ast` function.
            let lang = Language::new();
            let diags = LintEngine::new()
                .lint(&lang, &ast, root)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            for d in diags {
                op.diagnostics.push(format_diagnostic(&d));
            }
        }
        Ok(())
    }
}

impl LintBehavior for ExpressionDiagnostics {}

impl Plugin for ExpressionDiagnostics {
    fn id(&self) -> &str {
        "lint.expression"
    }

    fn name(&self) -> &str {
        "handl-plugin-diag-expression"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let rule: Box<dyn LintBehavior> = Box::new(ExpressionDiagnostics);
        ctx.contribute::<BasicLinter>(rule)
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        // Accept a payload with `lang` (serialized `Language`), `ast`
        // (serialized `Ast`), and `root` (arena index). Delegate to the lint
        // engine and return the diagnostics as JSON.
        let lang = request
            .payload
            .get("lang")
            .and_then(|v| serde_json::from_value::<Language>(v.clone()).ok())
            .unwrap_or_default();
        let ast = serde_json::from_value::<Ast>(
            request
                .payload
                .get("ast")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
        .map_err(|e| PlatformError::Plugin(e.to_string()))?;
        let root = AstRef::new(
            request
                .payload
                .get("root")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize,
        );
        let diags = LintEngine::new()
            .lint(&lang, &ast, root)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;
        let serialized: Vec<serde_json::Value> = diags
            .iter()
            .map(|d| {
                serde_json::json!({
                    "level": level_str(d.level),
                    "message": d.message,
                    "span": d.span.map(|s| serde_json::json!({"start": s.start, "end": s.end})),
                })
            })
            .collect();
        Ok(CapabilityResponse(serde_json::json!({
            "diagnostics": serialized
        })))
    }
}

/// Lint an AST subtree using the data-driven lint engine.
///
/// This is the primary API for tree-walking lint: it takes a `Language`, an
/// `Ast` arena, and a root `AstRef`, and returns [`LintDiagnostic`]s.
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if an arena index is out of bounds.
pub fn lint_ast(
    lang: &Language,
    ast: &Ast,
    root: AstRef,
) -> Result<Vec<LintDiagnostic>, handl_language::LangError> {
    LintEngine::new().lint(lang, ast, root)
}

/// Format a [`LintDiagnostic`] as a human-readable string for `LintRequest`.
fn format_diagnostic(d: &LintDiagnostic) -> String {
    let level = match d.level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warning => "warning",
        DiagnosticLevel::Info => "info",
    };
    format!("{level}: {}", d.message)
}

fn level_str(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warning => "warning",
        DiagnosticLevel::Info => "info",
    }
}
