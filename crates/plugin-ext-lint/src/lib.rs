//! `handl-plugin-ext-lint` — bootstrap linter for the v0.1 arrow surface
//! syntax (ADR-0087, rules-as-data per ADR-0097).
//!
//! Re-exports the platform `Lint` operation machinery ([`BasicLinter`], the
//! generic rule/capability registry) and runs the rule set declared in
//! `lang.lints` of the injected [`Language`] through the data-driven
//! [`LintEngine`] in `hnl-tool-lint`. The canonical rule set lives in
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
use std::path::Path;
use std::sync::Arc;

use handl_language::LangError;
use handl_plugin_ext_diag::{Diagnostic, Fix, Severity, Span};
use hnl_tool_codegen::{ApplyOptions, ArtifactRef, ArtifactStore, Engine, FsStore, RunMeta};
use hnl_tool_lint::{
    DiagnosticLevel, FixpointOutcome, LintDiagnostic, LintEngine, LintFixError, LintFixOut,
    NativeHit, NativeLintResolver, canonical_lint_natives,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Error from the linter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintError {
    /// Source failed to parse (or a lint pass failed).
    Parse(String),
    /// A file-level fix ([`fix_file`]) failed — store read, splice
    /// planning, or the journaled `Engine::apply`.
    Fix(String),
}

impl std::fmt::Display for LintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintError::Parse(msg) => write!(f, "parse error: {msg}"),
            LintError::Fix(msg) => write!(f, "fix error: {msg}"),
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
        &hnl_tool_lex_pattern::DataLexerEngine,
        &hnl_tool_parse_pratt::PrattEngine::new(),
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
    match hnl_tool_typecheck::check_module_with_prelude(
        lang,
        src,
        prelude,
        &hnl_tool_lex_pattern::DataLexerEngine,
        &hnl_tool_parse_pratt::PrattEngine::new(),
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
/// The fixpoint itself lives in `hnl-tool-lint`
/// ([`LintEngine::fix_source`], PL-004 T-091): each round's fix batch is
/// selected by the whole-file-fix-subsumes policy and spliced in
/// descending byte order; lint failures propagate as honest errors
/// rather than silently ending the loop.
///
/// # Errors
///
/// Returns [`LintError::Parse`] if the source cannot be linted (before
/// or after fixing), [`LintError::Fix`] if the fixpoint does not
/// converge within the iteration budget.
pub fn fix_source_with_prelude(
    ctx: &LintContext,
    src: &str,
    prelude: &HashSet<String>,
) -> Result<String, LintError> {
    let outcome = fix_outcome(ctx, src)?;
    // Verify the fixed source still lints end-to-end (the typecheck
    // scope pass included) — the same guarantee the old
    // `apply_fixes_to_fixpoint` + re-lint path provided.
    let _ = lint_source_with_prelude(ctx, &outcome.source, prelude)?;
    Ok(outcome.source)
}

/// Fixpoint iteration budget — the same 5 the pre-T-091
/// `apply_fixes_to_fixpoint` loop used.
const MAX_FIX_ITERATIONS: usize = 5;

/// The engine-side fixpoint over `src` — shared by [`fix_source`] and
/// [`fix_file`]. `prelude` does not reach the fix computation (the
/// typecheck-scope diagnostics it feeds carry no fixes), so it is not
/// threaded here; callers that need the prelude-checked *diagnostics*
/// use [`lint_source_with_prelude`].
fn fix_outcome(ctx: &LintContext, src: &str) -> Result<FixpointOutcome, LintError> {
    lint_engine(ctx.resolver.clone())
        .fix_source(
            ctx.lang(),
            src,
            &hnl_tool_lex_pattern::DataLexerEngine,
            &hnl_tool_parse_pratt::PrattEngine::new(),
            MAX_FIX_ITERATIONS,
        )
        .map_err(|e| match e {
            // A lint failure (unparseable source, rule error) keeps the
            // historical `Parse` classification.
            LintFixError::Lint(le) => LintError::Parse(le.to_string()),
            other => LintError::Fix(other.to_string()),
        })
}

/// What [`fix_file`] did to one artifact.
#[derive(Debug, Clone)]
pub struct LintFileFix {
    /// The artifact ref that was fixed (relative to the store root).
    pub artifact: ArtifactRef,
    /// `true` when at least one fix round produced a journaled write.
    pub changed: bool,
    /// The shared journal run id (`lint-fix-…`) — pass to
    /// `hnl_tool_codegen::Engine::revert` to undo every round.
    /// `None` when the file was already clean.
    pub run_id: Option<String>,
    /// Fix rounds the fixpoint ran (0 = already clean).
    pub iterations: usize,
    /// Byte edits applied across all rounds.
    pub edits: usize,
}

/// Fix a `.hnl` file on disk through the codemod engine — the
/// `hnl lint --fix` write path (PL-004 T-091).
///
/// `root` is the [`FsStore`] root and `artifact` a validated
/// root-relative path — **not** a process-CWD-relative path, which is
/// what makes this safe to call for files anywhere under the project.
/// Each fixpoint round's `hnl_tool_lint::FixHit`s become
/// `hnl_tool_codemod::ByteEdit`s whose offsets
/// are valid against the store's *current* content (the previous
/// round's applied state), so every round plans through
/// [`hnl_tool_codemod::CodemodEngine::plan_byte_edits`] and applies
/// through [`Engine::apply`] under one shared `lint-fix` run id. The
/// write is journaled and atomic, drift-checked (`before_hash`), and
/// `Engine::revert(run_id)` restores the original — replacing the old
/// direct `write_atomic` path.
///
/// # Errors
///
/// [`LintError::Fix`] when the artifact cannot be read, is not UTF-8,
/// a splice batch fails validation, or an apply reports failures.
/// [`LintError::Parse`] when the source (or a fix intermediate) does
/// not lint.
pub fn fix_file(ctx: &LintContext, root: &Path, artifact: &str) -> Result<LintFileFix, LintError> {
    fix_file_with_prelude(ctx, root, artifact, &HashSet::new())
}

/// [`fix_file`] with a host-supplied wire prelude — see
/// [`lint_source_with_prelude`]. The prelude only affects the final
/// verification lint (scope diagnostics carry no fixes).
pub fn fix_file_with_prelude(
    ctx: &LintContext,
    root: &Path,
    artifact: &str,
    prelude: &HashSet<String>,
) -> Result<LintFileFix, LintError> {
    let store = FsStore::new(root);
    let artifact = ArtifactRef::new(artifact);
    let bytes = store
        .read(&artifact)
        .map_err(|e| LintError::Fix(format!("cannot read {artifact}: {e}")))?
        .ok_or_else(|| {
            LintError::Fix(format!(
                "lint fix: {artifact} not found under {}",
                root.display()
            ))
        })?;
    let src = String::from_utf8(bytes)
        .map_err(|_| LintError::Fix(format!("lint fix: {artifact} is not valid UTF-8")))?;

    let outcome = fix_outcome(ctx, &src)?;
    if outcome.rounds.is_empty() {
        return Ok(LintFileFix {
            artifact,
            changed: false,
            run_id: None,
            iterations: 0,
            edits: 0,
        });
    }
    // Same end-to-end verification `fix_source` performs.
    let _ = lint_source_with_prelude(ctx, &outcome.source, prelude)?;

    // One run id across all rounds: the journal accumulates every
    // round's before-images under `lint-fix-<id>` and `revert` restores
    // them in reverse order — a single undo returns the original file.
    let mut meta = RunMeta::new("lint-fix", env!("CARGO_PKG_VERSION"));
    meta.new_run_id();
    let run_id = meta.run_id.clone();

    let codemod = hnl_tool_codemod::CodemodEngine::new(&store);
    let lifecycle = Engine::new(&store);
    let opts = ApplyOptions::default();
    let mut edits = 0usize;

    for round in &outcome.rounds {
        let byte_edits: Vec<hnl_tool_codemod::ByteEdit> = round
            .iter()
            .map(|h| {
                hnl_tool_codemod::ByteEdit::new(
                    h.rule.clone(),
                    h.fix.start,
                    h.fix.end,
                    h.fix.replacement.clone(),
                )
            })
            .collect();
        edits += byte_edits.len();
        let plan = codemod.plan_byte_edits(&artifact, &byte_edits);
        if !plan.diagnostics.is_empty() {
            let detail = plan
                .diagnostics
                .iter()
                .map(|d| format!("{:?}: {}", d.kind, d.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(LintError::Fix(format!(
                "lint fix: {artifact}: splice plan declined: {detail}"
            )));
        }
        if plan.changes.is_empty() {
            continue;
        }
        let report = lifecycle
            .apply(&plan.changes, None, &meta, &opts)
            .map_err(|e| LintError::Fix(format!("lint fix apply failed for {artifact}: {e}")))?;
        if !report.clean() {
            let detail = report
                .failures
                .iter()
                .map(|(a, e)| format!("{a}: {e}"))
                .chain(report.fatal.iter().cloned())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(LintError::Fix(format!(
                "lint fix apply failed for {artifact}: {detail}"
            )));
        }
    }

    // The per-round splices must compose to exactly the fixpoint's
    // converged source — a divergence means the store and the in-memory
    // fixpoint disagree, which is a bug, not a partial success.
    let after = store
        .read(&artifact)
        .map_err(|e| LintError::Fix(format!("cannot re-read {artifact}: {e}")))?
        .unwrap_or_default();
    if after.as_slice() != outcome.source.as_bytes() {
        return Err(LintError::Fix(format!(
            "lint fix: {artifact}: applied rounds diverged from the fixpoint result"
        )));
    }

    Ok(LintFileFix {
        artifact,
        changed: true,
        run_id: Some(run_id),
        iterations: outcome.iterations,
        edits,
    })
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
            LintContext::with_resolver(kit.language().clone(), kit.handl_lint_adapter().resolver())
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
        let err = LintError::Fix("journal locked".to_string());
        assert_eq!(format!("{}", err), "fix error: journal locked");
    }

    #[test]
    fn lint_context_new_and_for_language() {
        // `new` carries no `.hnl` resolver — a `handl:` rule would resolve
        // to the engine's "unknown native" error. `for_language` reuses
        // the resolver across a payload language.
        let bare = LintContext::new(Language::new());
        assert!(bare.lang().lints.is_empty());
        // Resolver-less context: `lint_engine` skips `add_resolver`.
        // The empty language has no lexer rules, so the result is
        // whatever the engine makes of it — the point is the path runs.
        let _ = lint_source_with(&bare, "");
        let ctx = canonical_ctx();
        let swapped = ctx.for_language(Language::new());
        assert!(swapped.lang().lints.is_empty());
        // Same resolver is retained — observable only through dispatch,
        // so assert the context is usable.
        let _ = lint_source_with(&swapped, "").expect("empty source lints");
    }

    #[test]
    fn lint_source_empty_input() {
        let diags = lint_source_with(&canonical_ctx(), "").unwrap();
        // Empty input is lint-clean (no module doc required for empty modules).
        assert!(diags.is_empty());
    }

    #[test]
    fn lint_source_clean_input() {
        let src = "-- A simple module.\nmain := 42\n";
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
    const UNLEXABLE: &str = "x := \"unterminated;";

    #[test]
    fn lint_source_with_uses_the_injected_language() {
        let ctx = canonical_ctx();
        let diags = lint_source_with(&ctx, "x := 1;").expect("lints");
        // The module has no doc comment, so at least one info diagnostic.
        assert!(!diags.is_empty(), "expected diagnostics, got none");
    }

    #[test]
    fn fix_source_applies_the_fmt_fix() {
        // A non-canonical file: the `fmt` rule supplies a whole-file fix,
        // which the engine's `fix_batch` applies alone (it overlaps every
        // other fix).
        let ctx = canonical_ctx();
        let fixed = fix_source(&ctx, "x   :=   1;").expect("fixes");
        assert!(
            check_source_with(&ctx, &fixed).expect("checks"),
            "the fixed source must be canonical, got {fixed:?}"
        );
    }

    #[test]
    fn fix_outcome_on_canonical_source_has_no_rounds() {
        // No fmt fix present, so the fixpoint converges immediately.
        let canonical =
            handl_plugin_ext_fmt::format_source_with_language(canonical_ctx().lang(), "x := 1;")
                .expect("formats");
        let outcome = fix_outcome(&canonical_ctx(), &canonical).expect("fixpoint");
        assert!(outcome.rounds.is_empty());
        assert_eq!(outcome.iterations, 0);
    }

    #[test]
    fn fix_outcome_propagates_a_lint_failure() {
        // Unlike the old `compute_fixes` (which swallowed lint errors into
        // "no fixes"), the engine fixpoint reports them honestly.
        assert!(matches!(
            fix_outcome(&canonical_ctx(), UNLEXABLE),
            Err(LintError::Parse(_))
        ));
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
        let hits = fmt_rule(ctx.lang(), "x   :=   1;");
        assert_eq!(hits.len(), 1, "the fmt rule is fixable");
        assert!(hits[0].fix.is_some(), "the hit must carry a fix");
    }

    #[test]
    fn convert_maps_a_fix_and_a_missing_span() {
        // `span: None` falls back to a zero-width span at offset 0; a
        // `fix` payload attaches a `Fix` with its own byte range.
        let engine_diag = LintDiagnostic {
            rule: "test-rule".into(),
            level: DiagnosticLevel::Warning,
            span: None,
            message: "msg".into(),
            fix: Some(LintFixOut {
                start: 0,
                end: 2,
                replacement: "xy".into(),
            }),
        };
        let converted = convert("ab", &engine_diag);
        assert_eq!(converted.span.start, 0);
        let fix = converted.fix.expect("fix attached");
        assert_eq!((fix.span.start, fix.span.end), (0, 2));
        assert_eq!(fix.replacement, "xy");
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
        // Unlike `fix_outcome`/`fmt_rule`, the `check_source*` family
        // propagates the parse failure to the caller.
        assert!(check_source_with_prelude(&canonical_ctx(), UNLEXABLE, &HashSet::new()).is_err());
    }

    // --- fix_file: the journaled file-write path (PL-004 T-091) ---

    #[test]
    fn fix_file_writes_through_the_journaled_apply_and_reverts() {
        let ctx = canonical_ctx();
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let original = "x   :=   1;";
        std::fs::write(root.join("a.hnl"), original).expect("seed file");

        let report = fix_file(&ctx, root, "a.hnl").expect("fix_file");
        assert!(report.changed, "expected a write, got {report:?}");
        let run_id = report.run_id.clone().expect("a run id");
        assert!(report.edits > 0);

        // The file now holds exactly what the string fixpoint produces.
        let after = std::fs::read_to_string(root.join("a.hnl")).expect("read back");
        assert_eq!(after, fix_source(&ctx, original).expect("fix_source"));

        // Codegen bookkeeping landed under the reserved prefix.
        assert!(
            root.join(".handl/codegen").is_dir(),
            "expected .handl/codegen bookkeeping under {}",
            root.display()
        );

        // `Engine::revert` on the shared run id restores the original.
        let store = FsStore::new(root);
        let engine = Engine::new(&store);
        let rev = engine
            .revert(&run_id, &ApplyOptions::default())
            .expect("revert");
        assert!(rev.failures.is_empty() && !rev.aborted, "got {rev:?}");
        assert_eq!(
            std::fs::read_to_string(root.join("a.hnl")).expect("read back"),
            original
        );
    }

    #[test]
    fn fix_file_on_clean_source_is_a_noop() {
        let ctx = canonical_ctx();
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let canonical =
            handl_plugin_ext_fmt::format_source_with_language(ctx.lang(), "-- doc\nx := 1;\n")
                .expect("formats");
        std::fs::write(root.join("clean.hnl"), &canonical).expect("seed file");

        let report = fix_file(&ctx, root, "clean.hnl").expect("fix_file");
        assert!(!report.changed);
        assert!(report.run_id.is_none());
        assert_eq!(report.iterations, 0);
        assert_eq!(
            std::fs::read_to_string(root.join("clean.hnl")).expect("read back"),
            canonical
        );
    }

    #[test]
    fn fix_file_missing_artifact_is_a_fix_error() {
        let ctx = canonical_ctx();
        let dir = tempfile::tempdir().expect("tempdir");
        let err = fix_file(&ctx, dir.path(), "nope.hnl").expect_err("must fail");
        assert!(matches!(err, LintError::Fix(_)), "got {err:?}");
    }

    #[test]
    fn fix_file_rejects_non_utf8_source() {
        let ctx = canonical_ctx();
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("bad.hnl"), [0xff, 0xfe, 0x00]).expect("seed file");
        let err = fix_file(&ctx, root, "bad.hnl").expect_err("must fail");
        assert!(matches!(err, LintError::Fix(_)), "got {err:?}");
        assert!(format!("{err}").contains("not valid UTF-8"));
    }

    #[test]
    fn fix_file_rejects_a_traversal_ref() {
        let ctx = canonical_ctx();
        let dir = tempfile::tempdir().expect("tempdir");
        let err = fix_file(&ctx, dir.path(), "../escape.hnl").expect_err("must fail");
        assert!(matches!(err, LintError::Fix(_)), "got {err:?}");
    }
}
