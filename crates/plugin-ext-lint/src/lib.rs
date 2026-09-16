//! `handl-plugin-ext-lint` — canonical bootstrap linter for the v0.1 arrow
//! surface syntax (ADR-0087, rules-as-data per ADR-0097).
//!
//! Re-exports the platform `Lint` operation machinery ([`BasicLinter`], the
//! generic rule/capability registry) and runs the **canonical rule set**
//! declared in `lang.lints` (`handl-kit-data-canonical`'s `lints.yaml`)
//! through the data-driven [`LintEngine`] in `plugin-engine-lint`.
//!
//! # Canonical rule set
//!
//! All rules are declared in `lints.yaml` — the crate carries no per-rule
//! logic beyond native registration:
//!
//! | Rule | Severity | Fixable | Check |
//! |------|----------|---------|-------|
//! | `unused-import` | Warning | yes | `!Native "scope.unused-import"` |
//! | `unbound-wire` | Error | no | `!Native "scope.unbound-wire"` |
//! | `shadowed-binding` | Warning | no | `!Native "scope.shadowed-binding"` |
//! | `dead-branch` | Warning | no | `!Match` on `if`/`ternary` constant guards |
//! | `naming-convention` | Warning | yes | `!Native "naming-convention"` |
//! | `missing-module-doc` | Info | no | `!Native "source.missing-module-doc"` |
//! | `fmt` | Warning | yes | `!Native "source.fmt"` (registered here) |
//! | `simplify-paren` | Warning | yes | `!Match` + `!Unwrap` fix (ADR-0097) |
//!
//! The `scope.*` / `naming-convention` / `source.missing-module-doc` natives
//! ship in [`canonical_lint_natives`]; `source.fmt` is registered by
//! [`lint_engine`] because it needs the formatter (`plugin-ext-fmt`).
//! Diagnostics use the shared types from `plugin-ext-diag` (ADR-0091). See
//! [`lint_source`], [`check_source`], and [`fix_source`] for the public API.

pub use handl_platform::{BasicLinter, Lint, LintBehavior, LintRequest, RuleAcceptor};
pub use handl_language::Language;

use std::collections::HashSet;
use std::sync::OnceLock;

use handl_language::{LangError};
use handl_plugin_engine_lint::{
    DiagnosticLevel, LintDiagnostic, LintEngine, LintFixOut, NativeHit, canonical_lint_natives,
};
use handl_plugin_ext_diag::{Diagnostic, Fix, Severity, Span, apply_fixes_to_fixpoint};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Error from the linter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintError {
    /// Source failed to parse.
    Parse(String),
}

impl std::fmt::Display for LintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for LintError {}

impl From<LangError> for LintError {
    fn from(err: LangError) -> Self {
        LintError::Parse(err.to_string())
    }
}

/// Lint a handl source file using the provided [`Language`] and rule set.
pub fn lint_source_with_language(lang: &Language, src: &str) -> Result<Vec<Diagnostic>, LintError> {
    lint_source_with_prelude_and_language(lang, src, &HashSet::new())
}

/// Lint a handl source file using the provided [`Language`] and a host-supplied
/// wire prelude.
pub fn lint_source_with_prelude_and_language(
    lang: &Language,
    src: &str,
    prelude: &HashSet<String>,
) -> Result<Vec<Diagnostic>, LintError> {
    let diags = lint_engine().lint_source(lang, src)?;
    let mut out: Vec<Diagnostic> = diags
        .iter()
        .map(|d| convert(src, d))
        // `unbound-wire` comes from the typechecker, not the engine's native.
        .filter(|d| d.rule != "unbound-wire")
        .collect();
    out.extend(unbound_wire_diagnostics_with_language(lang, src, prelude));
    out.sort_by_key(|d| d.span.start);
    Ok(out)
}

/// Lint a handl source file with the canonical rule set.
pub fn lint_source(src: &str) -> Result<Vec<Diagnostic>, LintError> {
    let lang = lint_language()?;
    lint_source_with_language(lang, src)
}

/// Lint a handl source file with a host-supplied wire prelude.
pub fn lint_source_with_prelude(
    src: &str,
    prelude: &HashSet<String>,
) -> Result<Vec<Diagnostic>, LintError> {
    let lang = lint_language()?;
    lint_source_with_prelude_and_language(lang, src, prelude)
}

/// `unbound-wire` diagnostics delegated to the bootstrap typechecker using the
/// supplied [`Language`].
fn unbound_wire_diagnostics_with_language(
    lang: &Language,
    src: &str,
    prelude: &HashSet<String>,
) -> Vec<Diagnostic> {
    match handl_plugin_engine_typecheck::check_module_with_prelude(lang, src, prelude) {
        Ok(report) => report
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "unbound-wire")
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a source file is lint-clean (no Error diagnostics) using the
/// provided [`Language`].
pub fn check_source_with_language(lang: &Language, src: &str) -> Result<bool, LintError> {
    let diags = lint_source_with_language(lang, src)?;
    Ok(!diags.iter().any(|d| d.severity == Severity::Error))
}

/// Check if a source file is lint-clean (no Error diagnostics).
pub fn check_source(src: &str) -> Result<bool, LintError> {
    let lang = lint_language()?;
    check_source_with_language(lang, src)
}

/// Check if a source file is lint-clean (no Error diagnostics), with a
/// host-supplied wire prelude for `unbound-wire`.
pub fn check_source_with_prelude(src: &str, prelude: &HashSet<String>) -> Result<bool, LintError> {
    let diags = lint_source_with_prelude(src, prelude)?;
    Ok(!diags.iter().any(|d| d.severity == Severity::Error))
}

/// Apply fixable diagnostics to source text.
///
/// Applies fixes in span order, re-parses, and re-runs lint to a fixpoint
/// (max 5 iterations, then reports unfixable).
///
/// # Errors
///
/// Returns [`LintError::Parse`] if the source cannot be parsed after fixing.
pub fn fix_source(src: &str) -> Result<String, LintError> {
    fix_source_with_prelude(src, &HashSet::new())
}

/// Apply fixable diagnostics to source text, with a host-supplied wire
/// prelude for `unbound-wire` (see [`lint_source_with_prelude`]).
///
/// # Errors
///
/// Returns [`LintError::Parse`] if the source cannot be parsed after fixing.
pub fn fix_source_with_prelude(src: &str, prelude: &HashSet<String>) -> Result<String, LintError> {
    let fixed = apply_fixes_to_fixpoint(src, |s| compute_fixes(s, prelude), 5)
        .map_err(|e| LintError::Parse(e.to_string()))?;
    // Verify the fixed source still parses.
    let _ = lint_source_with_prelude(&fixed, prelude)?;
    Ok(fixed)
}

/// Compute the next batch of fixes for the fixpoint loop.
///
/// The `fmt` fix spans the whole file and would overlap every other fix, so
/// when it is present it is applied alone; otherwise the remaining fixable
/// diagnostics (non-overlapping among themselves) are returned.
fn compute_fixes(src: &str, prelude: &HashSet<String>) -> Vec<Fix> {
    let diags = match lint_source_with_prelude(src, prelude) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    if let Some(fmt_fix) = diags
        .iter()
        .find(|d| d.rule == "fmt")
        .and_then(|d| d.fix.clone())
    {
        return vec![fmt_fix];
    }
    diags
        .iter()
        .filter(|d| d.rule != "fmt")
        .filter_map(|d| d.fix.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Internal: language assembly + engine wiring
// ---------------------------------------------------------------------------

/// Assemble the lint language once: canonical data, including the
/// `lints.yaml` rule set. The `simplify-paren` predicate lives solely in
/// `lints.yaml` — LSP quick fixes are built from the diagnostic's `fix`
/// payload, so no transform-engine pass is merged here.
pub fn lint_language() -> Result<&'static Language, LintError> {
    static LANG: OnceLock<Language> = OnceLock::new();
    let lang = LANG.get_or_init(|| {
        handl_kit_data_canonical::assemble()
            .expect("canonical language data must assemble")
            .lang()
            .clone()
    });
    Ok(lang)
}

/// The canonical [`LintEngine`]: canonical `scope.*`/`naming-convention`/
/// `source.*` natives plus the host-registered `source.fmt` rule, which
/// needs `plugin-ext-fmt`.
fn lint_engine() -> &'static LintEngine {
    static ENGINE: OnceLock<LintEngine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut natives = canonical_lint_natives();
        // Wire the handl-native lint resolver (ADR-0097) from the canonical
        // kit's registered `.hnl` modules. Without this, a `lints.yaml` rule
        // with `check: !Native "handl:<mod>.<fn>"` errors as "unknown
        // native" — the resolver is the production seam a host uses to let
        // `.hnl`-authored lint rules dispatch.
        if let Ok(kit) = handl_kit_canonical::Kit::new() {
            natives.add_resolver(kit.handl_lint_adapter().resolver());
        }
        LintEngine::new()
            .with_native_registry(natives)
            // `scope.unbound-wire` is delegated to the bootstrap
            // typechecker's scope pass in `lint_source_with_prelude` — the
            // canonical native's local scope walk diverged from it. Register
            // a no-op so the `lint.scope.unbound-wire` data rule still
            // resolves but emits nothing here.
            .with_native("scope.unbound-wire", |_ctx| Ok(Vec::new()))
            .with_native("source.fmt", |ctx| Ok(fmt_rule(ctx.lang, ctx.src)))
    })
}

/// `source.fmt` — the file is not canonical for the language. Fixable: the fix
/// is the formatted file.
fn fmt_rule(lang: &Language, src: &str) -> Vec<NativeHit> {
    if src.trim().is_empty() {
        return Vec::new();
    }
    match handl_plugin_ext_fmt::check_source_with_language(lang, src) {
        Ok(true) => Vec::new(),
        Ok(false) => {
            let replacement = handl_plugin_ext_fmt::format_source_with_language(lang, src)
                .unwrap_or_default();
            vec![NativeHit {
                args: Default::default(),
                span: Some(handl_language::Span {
                    start: 0,
                    end: src.len(),
                }),
                fix: Some(LintFixOut {
                    start: 0,
                    end: src.len(),
                    replacement,
                }),
            }]
        }
        // Parse errors are reported by the parse stage; don't double-report.
        Err(_) => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Internal: diagnostic conversion (engine → plugin-ext-diag)
// ---------------------------------------------------------------------------

/// Convert an engine [`LintDiagnostic`] (absolute byte spans) into a
/// `plugin-ext-diag` [`Diagnostic`] with line/column info.
fn convert(src: &str, d: &LintDiagnostic) -> Diagnostic {
    let severity = match d.level {
        DiagnosticLevel::Error => Severity::Error,
        DiagnosticLevel::Warning => Severity::Warning,
        DiagnosticLevel::Info => Severity::Info,
    };
    let span = d
        .span
        .map(|s| Span::from_byte_range(s.start, s.end, src))
        .unwrap_or_else(|| Span::from_byte_range(0, 0, src));
    let diag = Diagnostic::new(&d.rule, severity, span, &d.message);
    match &d.fix {
        Some(f) => diag.with_fix(Fix {
            span: Span::from_byte_range(f.start, f.end, src),
            replacement: f.replacement.clone(),
        }),
        None => diag,
    }
}

// ---------------------------------------------------------------------------
// Existing platform tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        Behavior, CapabilityId, CapabilityRequest, KitRegistry, Lint, LintRequest, PlatformError,
        Plugin, PluginContext, PluginRegistry,
    };

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicLinter::new();
        let meta = plugin.metadata(&CapabilityId::from("lint")).unwrap();
        assert_eq!(meta["command"], "lint");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_diagnostics_when_rule_matches() {
        struct Warn;

        impl Behavior<Lint> for Warn {
            fn kind(&self) -> &str {
                "unused"
            }

            fn apply(&self, op: &mut LintRequest) -> Result<(), PlatformError> {
                op.diagnostics.push("unused variable".to_string());
                Ok(())
            }
        }

        impl LintBehavior for Warn {}

        let mut linter = BasicLinter::new();
        linter.accept(Box::new(Warn)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = linter
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::json!({"kind": "unused"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            resp.0,
            serde_json::json!({"diagnostics": ["unused variable"]})
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicLinter::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }

    // --- canonical linter unit tests ---

    #[test]
    fn lint_error_display() {
        let err = LintError::Parse("bad token".to_string());
        assert_eq!(format!("{}", err), "parse error: bad token");
    }

    #[test]
    fn lint_source_empty_input() {
        let diags = lint_source("").unwrap();
        // Empty input is lint-clean (no module doc required for empty modules).
        assert!(diags.is_empty());
    }

    #[test]
    fn lint_source_clean_input() {
        let src = "-- A simple module.\nmain = 42\n";
        let diags = lint_source(src).unwrap();
        let non_fmt: Vec<_> = diags.iter().filter(|d| d.rule != "fmt").collect();
        assert!(
            non_fmt.is_empty(),
            "expected no non-fmt diagnostics, got {:?}",
            non_fmt
        );
    }
}
