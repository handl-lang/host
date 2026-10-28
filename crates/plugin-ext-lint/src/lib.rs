//! `handl-plugin-ext-lint` — bootstrap linter for the v0.1 arrow surface
//! syntax (ADR-0087, rules-as-data per ADR-0097).
//!
//! Re-exports the platform `Lint` operation machinery ([`BasicLinter`], the
//! generic rule/capability registry) and runs the rule set declared in
//! `lang.lints` of the injected [`Language`] through the data-driven
//! [`LintEngine`] in `hnl-engine-lint`. The canonical rule set lives in
//! `handl-kit-data-prelude`'s `lints.yaml`; the host supplies the language
//! via [`LintContext`] (ADR-0128 §"The 0.8 seam") — this crate keeps no
//! normal dependency on the canonical kit.
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
//! | `record-as-block` | Warning | no | `!Match` + `!AnyChildKind` on `block` |
//!
//! The `scope.*` / `naming-convention` / `source.missing-module-doc` natives
//! ship in [`canonical_lint_natives`]; `source.fmt` is registered by
//! [`lint_engine`] because it needs the formatter (`plugin-ext-fmt`).
//! `.hnl`-authored `handl:<mod>.<fn>` rules resolve through the
//! [`NativeLintResolver`] carried on the [`LintContext`] — without one they
//! error as "unknown native", faithfully. Diagnostics use the shared types
//! from `plugin-ext-diag` (ADR-0091). See [`lint_source_with`],
//! [`check_source_with`], and [`fix_source`] for the public API.

pub use handl_language::Language;
pub use handl_platform::{BasicLinter, Lint, LintBehavior, LintRequest, RuleAcceptor};

pub mod handl_lint;
pub mod plugin;
pub use handl_lint::HandlNativeLint;
pub use plugin::{LINT_CAPABILITY, LintPlugin};

use std::collections::HashSet;
use std::sync::Arc;

use handl_language::LangError;
use hnl_engine_lint::{
    DiagnosticLevel, LintDiagnostic, LintEngine, LintFixOut, NativeHit, NativeLintResolver,
    canonical_lint_natives,
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

/// The injected lint bundle: the [`Language`] (carrying `lang.lints`) plus an
/// optional [`NativeLintResolver`] that dispatches `.hnl`-authored
/// `handl:<mod>.<fn>` rule names (ADR-0097).
///
/// `Plugin: Send + Sync` forbids holding a `Kit` (`!Sync`), so per ADR-0128
/// §"The 0.8 seam" the host assembles the kit-side pieces — `kit.language()`
/// and `kit.handl_lint_adapter().resolver()` — and hands them in. Both
/// halves are `Send + Sync`, so the context can ride inside a plugin.
#[derive(Clone)]
pub struct LintContext {
    lang: Arc<Language>,
    resolver: Option<NativeLintResolver>,
}

impl LintContext {
    /// A context over `lang` with no `.hnl`-rule resolver — `handl:` native
    /// names then resolve to the engine's "unknown native" error, same as an
    /// unregistered `!Native` name.
    pub fn new(lang: Language) -> Self {
        Self {
            lang: Arc::new(lang),
            resolver: None,
        }
    }

    /// A context over `lang` whose `.hnl`-authored lint rules dispatch
    /// through `resolver` — e.g. `kit.handl_lint_adapter().resolver()`.
    pub fn with_resolver(lang: Language, resolver: NativeLintResolver) -> Self {
        Self {
            lang: Arc::new(lang),
            resolver: Some(resolver),
        }
    }

    /// This context over a different language, keeping the resolver — the
    /// payload-`language` (ADR-0124) path: the caller's language with the
    /// host's `.hnl` dispatch.
    pub fn for_language(&self, lang: Language) -> Self {
        Self {
            lang: Arc::new(lang),
            resolver: self.resolver.clone(),
        }
    }

    /// The language carrying the `lang.lints` rule set.
    pub fn lang(&self) -> &Language {
        &self.lang
    }
}

/// Lint a handl source file using the injected [`LintContext`] and rule set.
pub fn lint_source_with(ctx: &LintContext, src: &str) -> Result<Vec<Diagnostic>, LintError> {
    lint_source_with_prelude(ctx, src, &HashSet::new())
}

/// Lint a handl source file using the injected [`LintContext`] and a
/// host-supplied wire prelude.
pub fn lint_source_with_prelude(
    ctx: &LintContext,
    src: &str,
    prelude: &HashSet<String>,
) -> Result<Vec<Diagnostic>, LintError> {
    let diags = lint_engine(ctx.resolver.clone()).lint_source(
        ctx.lang(),
        src,
        &hnl_engine_lex_pattern::DataLexerEngine,
        &hnl_engine_parse_pratt::PrattEngine::new(),
    )?;
    let mut out: Vec<Diagnostic> = diags
        .iter()
        .map(|d| convert(src, d))
        // `unbound-wire`/`unbound-export` come from the typechecker's
        // scope pass, not the engine's natives.
        .filter(|d| d.rule != "unbound-wire" && d.rule != "unbound-export")
        .collect();
    out.extend(scope_diagnostics_with_language(ctx.lang(), src, prelude));
    out.sort_by_key(|d| d.span.start);
    Ok(out)
}

/// `unbound-wire`/`unbound-export` diagnostics delegated to the bootstrap
/// typechecker's module scope pass using the supplied [`Language`].
fn scope_diagnostics_with_language(
    lang: &Language,
    src: &str,
    prelude: &HashSet<String>,
) -> Vec<Diagnostic> {
    match hnl_engine_typecheck::check_module_with_prelude(
        lang,
        src,
        prelude,
        &hnl_engine_lex_pattern::DataLexerEngine,
        &hnl_engine_parse_pratt::PrattEngine::new(),
    ) {
        Ok(report) => report
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "unbound-wire" || d.rule == "unbound-export")
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a source file is lint-clean (no Error diagnostics) using the
/// injected [`LintContext`].
pub fn check_source_with(ctx: &LintContext, src: &str) -> Result<bool, LintError> {
    let diags = lint_source_with(ctx, src)?;
    Ok(!diags.iter().any(|d| d.severity == Severity::Error))
}

/// Check if a source file is lint-clean (no Error diagnostics), with a
/// host-supplied wire prelude for `unbound-wire`.
pub fn check_source_with_prelude(
    ctx: &LintContext,
    src: &str,
    prelude: &HashSet<String>,
) -> Result<bool, LintError> {
    let diags = lint_source_with_prelude(ctx, src, prelude)?;
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
pub fn fix_source(ctx: &LintContext, src: &str) -> Result<String, LintError> {
    fix_source_with_prelude(ctx, src, &HashSet::new())
}

/// Apply fixable diagnostics to source text, with a host-supplied wire
/// prelude for `unbound-wire` (see [`lint_source_with_prelude`]).
///
/// # Errors
///
/// Returns [`LintError::Parse`] if the source cannot be parsed after fixing.
pub fn fix_source_with_prelude(
    ctx: &LintContext,
    src: &str,
    prelude: &HashSet<String>,
) -> Result<String, LintError> {
    let fixed = apply_fixes_to_fixpoint(src, |s| compute_fixes(ctx, s, prelude), 5)
        .map_err(|e| LintError::Parse(e.to_string()))?;
    // Verify the fixed source still parses.
    let _ = lint_source_with_prelude(ctx, &fixed, prelude)?;
    Ok(fixed)
}

/// Compute the next batch of fixes for the fixpoint loop.
///
/// The `fmt` fix spans the whole file and would overlap every other fix, so
/// when it is present it is applied alone; otherwise the remaining fixable
/// diagnostics (non-overlapping among themselves) are returned.
fn compute_fixes(ctx: &LintContext, src: &str, prelude: &HashSet<String>) -> Vec<Fix> {
    let diags = match lint_source_with_prelude(ctx, src, prelude) {
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
// Internal: engine wiring
// ---------------------------------------------------------------------------

/// A [`LintEngine`] for one lint pass: canonical `scope.*`/`naming-convention`/
/// `source.*` natives plus the host-registered `source.fmt` rule, which
/// needs `plugin-ext-fmt`. Built fresh per call — the injected
/// [`NativeLintResolver`] is what differs between hosts, so there is no
/// `OnceLock` cache.
///
/// The `simplify-paren` predicate lives solely in `lints.yaml` — LSP quick
/// fixes are built from the diagnostic's `fix` payload, so no
/// transform-engine pass is merged here.
fn lint_engine(resolver: Option<NativeLintResolver>) -> LintEngine {
    let mut natives = canonical_lint_natives();
    // The handl-native lint resolver (ADR-0097) routes `lints.yaml` rules
    // with `check: !Native "handl:<mod>.<fn>"` to `.hnl`-authored functions.
    // It is injected on the `LintContext`: without one, a `handl:` rule
    // errors as "unknown native" — same as any unregistered `!Native` name.
    if let Some(resolver) = resolver {
        natives.add_resolver(resolver);
    }
    LintEngine::new()
        .with_native_registry(natives)
        // `scope.unbound-wire` is delegated to the bootstrap
        // typechecker's scope pass in `lint_source_with_prelude` — the
        // canonical native's local scope walk diverged from it. Register
        // a no-op so the `lint.scope.unbound-wire` data rule still
        // resolves but emits nothing here.
        .with_native("scope.unbound-wire", |_ctx| Ok(Vec::new()))
        // `scope.unbound-export` is delegated to the bootstrap
        // typechecker's scope pass alongside `unbound-wire` — same
        // no-op registration so the data rule still resolves.
        .with_native("scope.unbound-export", |_ctx| Ok(Vec::new()))
        .with_native("source.fmt", |ctx| Ok(fmt_rule(ctx.lang, ctx.src)))
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
            let replacement =
                handl_plugin_ext_fmt::format_source_with_language(lang, src).unwrap_or_default();
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
    use std::sync::OnceLock;

    use handl_platform::{
        Behavior, CapabilityId, CapabilityRequest, KitRegistry, Lint, LintRequest, PlatformError,
        Plugin, PluginContext, PluginRegistry,
    };

    /// The canonical lint context — the kit-side pieces a host injects,
    /// memoized for the test binary (ADR-0128 §0.8; dev-dep only).
    fn canonical_ctx() -> LintContext {
        static CTX: OnceLock<LintContext> = OnceLock::new();
        CTX.get_or_init(|| {
            let kit = handl_kit_prelude::Kit::new().expect("canonical kit assembles");
            LintContext::with_resolver(
                kit.language().clone(),
                kit.handl_lint_adapter().resolver(),
            )
        })
        .clone()
    }

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
        let diags = lint_source_with(&canonical_ctx(), "").unwrap();
        // Empty input is lint-clean (no module doc required for empty modules).
        assert!(diags.is_empty());
    }

    #[test]
    fn lint_source_clean_input() {
        let src = "-- A simple module.\nmain = 42\n";
        let diags = lint_source_with(&canonical_ctx(), src).unwrap();
        let non_fmt: Vec<_> = diags.iter().filter(|d| d.rule != "fmt").collect();
        assert!(
            non_fmt.is_empty(),
            "expected no non-fmt diagnostics, got {:?}",
            non_fmt
        );
    }
    // --- private helpers: the "cannot check" degradation arms ---
    //
    // These paths are unreachable through the public API for well-formed
    // input, so they are covered by calling the helpers directly rather than
    // by leaving them untested.

    /// Source that cannot even be lexed, so every downstream stage fails.
    const UNLEXABLE: &str = "x = \"unterminated;";

    #[test]
    fn lint_source_with_uses_the_injected_language() {
        let ctx = canonical_ctx();
        let diags = lint_source_with(&ctx, "x = 1;").expect("lints");
        // The module has no doc comment, so at least one info diagnostic.
        assert!(!diags.is_empty(), "expected diagnostics, got none");
    }

    #[test]
    fn fix_source_applies_the_fmt_fix() {
        // A non-canonical file: the `fmt` rule supplies a whole-file fix, and
        // `compute_fixes` applies it alone (it overlaps every other fix).
        let ctx = canonical_ctx();
        let fixed = fix_source(&ctx, "x   =   1;").expect("fixes");
        assert!(
            check_source_with(&ctx, &fixed).expect("checks"),
            "the fixed source must be canonical, got {fixed:?}"
        );
    }

    #[test]
    fn compute_fixes_on_canonical_source_is_empty() {
        // No fmt fix present, so the non-fmt filter chain is exercised.
        let canonical =
            handl_plugin_ext_fmt::format_source_with_language(canonical_ctx().lang(), "x = 1;")
                .expect("formats");
        assert!(compute_fixes(&canonical_ctx(), &canonical, &HashSet::new()).is_empty());
    }

    #[test]
    fn unbound_wire_diagnostics_is_empty_when_the_source_cannot_be_checked() {
        let ctx = canonical_ctx();
        let prelude = HashSet::new();
        assert!(
            scope_diagnostics_with_language(ctx.lang(), UNLEXABLE, &prelude).is_empty(),
            "a check failure must degrade to no diagnostics, not propagate"
        );
    }

    #[test]
    fn compute_fixes_is_empty_when_the_source_cannot_be_linted() {
        assert!(compute_fixes(&canonical_ctx(), UNLEXABLE, &HashSet::new()).is_empty());
    }

    #[test]
    fn fmt_rule_declines_when_the_source_cannot_be_checked() {
        let ctx = canonical_ctx();
        assert!(fmt_rule(ctx.lang(), UNLEXABLE).is_empty());
    }

    #[test]
    fn fmt_rule_declines_on_whitespace_only_source() {
        // The early return: there is nothing to format.
        let ctx = canonical_ctx();
        assert!(fmt_rule(ctx.lang(), "   \n\t\n").is_empty());
    }

    #[test]
    fn fmt_rule_reports_a_fix_for_non_canonical_source() {
        let ctx = canonical_ctx();
        let hits = fmt_rule(ctx.lang(), "x   =   1;");
        assert_eq!(hits.len(), 1, "the fmt rule is fixable");
        assert!(hits[0].fix.is_some(), "the hit must carry a fix");
    }

    #[test]
    fn convert_maps_every_severity_level() {
        for (level, want) in [
            (DiagnosticLevel::Error, Severity::Error),
            (DiagnosticLevel::Warning, Severity::Warning),
            (DiagnosticLevel::Info, Severity::Info),
        ] {
            let engine_diag = LintDiagnostic {
                rule: "test-rule".into(),
                level,
                span: Some(handl_language::Span { start: 0, end: 1 }),
                message: "msg".into(),
                fix: None,
            };
            let converted = convert("ab", &engine_diag);
            assert_eq!(converted.severity, want);
            assert_eq!(converted.rule, "test-rule");
            assert_eq!(converted.span.end, 1);
        }
    }

    #[test]
    fn check_source_with_prelude_seeds_the_wire_scope() {
        // With `provided` in the prelude the module is clean; without it the
        // unbound wire is an Error.
        let ctx = canonical_ctx();
        let with_name: HashSet<String> = ["provided".to_string()].into_iter().collect();
        assert!(check_source_with_prelude(&ctx, "provided", &with_name).expect("checks"));
        assert!(!check_source_with_prelude(&ctx, "provided", &HashSet::new()).expect("checks"));
    }

    #[test]
    fn check_source_with_prelude_propagates_a_parse_error() {
        // Unlike `compute_fixes`/`fmt_rule`, the `check_source*` family
        // propagates the parse failure to the caller.
        assert!(check_source_with_prelude(&canonical_ctx(), UNLEXABLE, &HashSet::new()).is_err());
    }
}
