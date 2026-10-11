//! # handl-plugin-engine-typecheck
//!
//! The **abstraction** crate for the typecheck operation (ADR-0054 §3).
//!
//! This crate defines the [`Typecheck`] operation marker,
//! [`TypecheckRequest`], [`TypecheckResult`], and the [`TypecheckEngine`]
//! trait, plus the convenience [`check_module`] entry point used by
//! `hnl typecheck`. `Foreign` scheme dispatch uses
//! [`handl_platform::ModuleLookup`] from the `handl-platform` crate (type
//! rules may call host functions).
//! **No typecheck engine code lives here except the bootstrap checker** — the
//! abstraction is the contract. The concrete engines are
//! `handl-plugin-engine-typecheck-handl` (an interpreted `.hnl` model, the
//! engine the canonical kit registers under the name `"handl"`) and this
//! crate's own Rust [`check_module`] bootstrap checker, which hosts fall back
//! to. Engines are selected via `runtime.typecheck` in the kit data.
//!
//! Engines consume [`handl_language::Language`] **data** — they look up
//! `lang.types[node.kind]` to find a [`handl_language::Term`] and
//! interpret it, not match on a static Rust enum of canonical kinds.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{Ast, LangError, Language, Span, Value};

/// The typecheck operation marker.
///
/// A zero-sized type identifying the typecheck stage. It carries no state;
/// its purpose is to namespace the typecheck abstraction and pair with
/// [`TypecheckRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Typecheck;

/// Request passed to a [`TypecheckEngine`].
///
/// Carries the [`Ast`] to typecheck plus a serializable options map. The
/// options are opaque to this crate — a concrete engine interprets them
/// against the `Language`'s `options` fragment set.
#[derive(Debug, Clone, Default)]
pub struct TypecheckRequest {
    /// The AST to typecheck.
    pub ast: Ast,
    /// Engine-specific options (opaque key/value pairs).
    pub options: serde_json::Value,
}

impl TypecheckRequest {
    /// Create a new request from an `Ast` with no options.
    pub fn new(ast: Ast) -> Self {
        Self {
            ast,
            options: serde_json::Value::Null,
        }
    }

    /// Attach an options value (builder style).
    pub fn with_options(mut self, options: serde_json::Value) -> Self {
        self.options = options;
        self
    }
}

/// A diagnostic emitted during typechecking (warning, note, etc.).
///
/// Diagnostics are non-fatal: a successful typecheck may still carry
/// warnings or notes. Fatal type errors are reported via the
/// [`LangError`] returned from [`TypecheckEngine::typecheck`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypecheckDiagnostic {
    /// The diagnostic severity (e.g. `"warning"`, `"note"`).
    pub severity: String,
    /// The human-readable diagnostic message.
    pub message: String,
    /// Optional source span the diagnostic refers to.
    pub span: Option<Span>,
}

/// The result of typechecking.
///
/// On success, carries the synthesized type (a [`Value`] — types are
/// values, ADR-0032) plus any non-fatal diagnostics. The diagnostics
/// vector is empty when the checker has nothing to report.
#[derive(Debug, Clone)]
pub struct TypecheckResult {
    /// The synthesized type (a value — types are values, ADR-0032).
    pub typ: Value,
    /// Diagnostics (warnings, notes — empty on a clean success).
    pub diagnostics: Vec<TypecheckDiagnostic>,
    /// Per-node synthesized/checked types (ADR-0032 proof erasure).
    ///
    /// Keys are `AstRef::index()` values; values are the type `Value`
    /// computed for that node. Empty if the engine does not record them.
    pub node_types: HashMap<usize, Value>,
}

/// The trait a typecheck engine implements.
///
/// An engine looks up `lang.types[node.kind]` →
/// [`handl_language::TypeRuleDef`] → [`handl_language::Term`] and
/// interprets the scheme, delegating value-level computation to a bound
/// eval engine and judgment ops to its own primitives. The engine is pure
/// data-driven: it does not match on a static `CanonicalKind` enum.
///
/// # Errors
///
/// Returns [`LangError`] on typecheck failure (unknown kind, missing type
/// rule, unification failure, unregistered foreign function, etc.).
pub trait TypecheckEngine {
    /// Typecheck `req.ast` under `lang`, returning the synthesized type.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure.
    fn typecheck(
        &self,
        lang: &Language,
        req: &TypecheckRequest,
    ) -> Result<TypecheckResult, LangError>;

    /// Produce an owned boxed clone of this engine (ADR-0047 §2).
    fn box_clone(&self) -> Box<dyn TypecheckEngine>;

    /// Produce a fresh owned engine with the given module registry wired in
    /// for `Foreign` dispatch (type rules may call host functions). Default:
    /// clone + ignore. ADR-0047 §2.
    fn with_modules(
        &self,
        _modules: Rc<dyn handl_platform::ModuleLookup>,
    ) -> Box<dyn TypecheckEngine> {
        self.box_clone()
    }

    /// Produce a fresh owned engine with an additional global binding.
    /// Default: clone + ignore. ADR-0047 §2.
    fn with_global(&self, _name: &str, _val: Value) -> Box<dyn TypecheckEngine> {
        self.box_clone()
    }
}

// ---------------------------------------------------------------------------
// Bootstrap check pass over the v0.1 surface AST (ADR-0088)
// ---------------------------------------------------------------------------

/// The bootstrap check pass implementation (ADR-0088).
pub mod bootstrap;

/// A typecheck report for a module (ADR-0088).
///
/// Produced by [`check_module`]. An empty `diagnostics` vector means the
/// module is clean. Diagnostics use the shared
/// [`handl_plugin_ext_diag::Diagnostic`] type (ADR-0091) — no local variant
/// types are defined in the typecheck crates.
#[derive(Debug, Clone, Default)]
pub struct TypecheckReport {
    /// Diagnostics found during checking (empty = clean).
    pub diagnostics: Vec<handl_plugin_ext_diag::Diagnostic>,
}

impl TypecheckReport {
    /// Whether the report is clean (no diagnostics).
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Error from the bootstrap typechecker (ADR-0088).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// Source failed to parse. The string carries the parse error message.
    Parse(String),
    /// The `Language` the checker assembled is invalid — e.g. transform-rule
    /// validation (ADR-0094) reported Error-severity diagnostics. The string
    /// carries the validation messages.
    Language(String),
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckError::Parse(msg) => write!(f, "parse error: {msg}"),
            CheckError::Language(msg) => write!(f, "language error: {msg}"),
        }
    }
}

impl std::error::Error for CheckError {}

impl From<LangError> for CheckError {
    fn from(err: LangError) -> Self {
        CheckError::Parse(err.to_string())
    }
}

/// Check a handl module source text (ADR-0088).
///
/// Parses the source and runs the bootstrap typecheck pass over the
/// v0.1 surface AST. Returns a report with **all** diagnostics found
/// (error recovery: reports all issues, not just the first).
///
/// The bootstrap checker implements the decidable core of ADR-0074's typing
/// rules: module-level binding/import/export checks, unbound-wire detection,
/// arrow kind checking, and structural type inference. Row-variable inference
/// and full `rec` fixpoint typing stay behind explicit `todo!` boundaries,
/// emitting `not-yet-checked` Info diagnostics rather than silently passing.
///
/// The caller supplies the assembled [`Language`] — canonical callers get it
/// from `handl-kit-data-canonical`/`handl-kit-canonical` (the single
/// YAML→`Language` assembly path, which also runs transform-rule validation,
/// ADR-0094). This crate no longer embeds its own copy of the canonical
/// data.
///
/// # Errors
///
/// Returns [`CheckError::Parse`] if the source cannot be parsed.
pub fn check_module(lang: &Language, src: &str) -> Result<TypecheckReport, CheckError> {
    bootstrap::check_module(lang, src)
}

/// Check a handl module source text with a host-supplied wire prelude.
///
/// Identical to [`check_module`], but `prelude` names are seeded into the
/// module scope before unbound-wire detection. A host passes the names its
/// runtime would auto-wire for `.hnl` sources — module-chain names, flat
/// host-function names, and ambient globals (ADR-0096). This crate carries
/// no host-name knowledge: the set comes from the host's module registry /
/// resolver (e.g. `handl_kit_canonical::canonical_wire_names`), keeping the
/// checker engine-generic.
///
/// # Errors
///
/// Returns [`CheckError::Parse`] if the source cannot be parsed.
pub fn check_module_with_prelude(
    lang: &Language,
    src: &str,
    prelude: &std::collections::HashSet<String>,
) -> Result<TypecheckReport, CheckError> {
    bootstrap::check_module_with_prelude(lang, src, prelude)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode};
    use serde_json::json;

    #[test]
    fn typecheck_marker_constructs() {
        let _ = Typecheck;
        let t1 = Typecheck;
        let t2 = Typecheck;
        assert_eq!(t1, t2);
        let t3 = Typecheck::default();
        assert_eq!(t1, t3);
    }

    #[test]
    fn typecheck_request_new_from_ast() {
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit"));
        let req = TypecheckRequest::new(ast);
        assert_eq!(req.ast.len(), 1);
        assert_eq!(req.options, serde_json::Value::Null);
    }

    #[test]
    fn typecheck_request_with_options() {
        let req = TypecheckRequest::new(Ast::new()).with_options(json!({"strict": true}));
        assert_eq!(req.options, json!({"strict": true}));
    }

    /// A stub engine that returns `NotImplemented`, used to verify trait
    /// dispatch shape.
    #[derive(Clone)]
    struct StubTypecheck;

    impl TypecheckEngine for StubTypecheck {
        fn typecheck(
            &self,
            _lang: &Language,
            _req: &TypecheckRequest,
        ) -> Result<TypecheckResult, LangError> {
            Err(LangError::NotImplemented)
        }

        fn box_clone(&self) -> Box<dyn TypecheckEngine> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn stub_engine_returns_not_implemented() {
        let engine = StubTypecheck;
        let lang = Language::new();
        let req = TypecheckRequest::new(Ast::new());
        let result = engine.typecheck(&lang, &req);
        assert_eq!(result.unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn stub_engine_via_dyn_reference() {
        let engine: Box<dyn TypecheckEngine> = Box::new(StubTypecheck);
        let lang = Language::new();
        let req = TypecheckRequest::new(Ast::new());
        let result = engine.typecheck(&lang, &req);
        assert!(matches!(result, Err(LangError::NotImplemented)));
    }

    #[test]
    fn typecheck_report_default_is_empty() {
        let report = TypecheckReport::default();
        assert!(report.diagnostics.is_empty());
        assert!(report.is_clean());
    }

    #[test]
    fn check_error_display() {
        let err = CheckError::Parse("unexpected token".to_string());
        assert_eq!(format!("{err}"), "parse error: unexpected token");
    }

    #[test]
    fn check_error_equality() {
        let a = CheckError::Parse("boom".to_string());
        let b = CheckError::Parse("boom".to_string());
        assert_eq!(a, b);
        assert_ne!(a, CheckError::Parse("other".to_string()));
    }

    /// The canonical language for `check_module` tests — the caller-supplied
    /// `Language` (dev-dep cycle is allowed in cargo).
    fn canonical_lang() -> &'static Language {
        static LANG: std::sync::OnceLock<Language> = std::sync::OnceLock::new();
        LANG.get_or_init(|| {
            handl_kit_data_canonical::language_from_yaml().expect("canonical language assembles")
        })
    }

    #[test]
    fn check_module_empty_is_clean() {
        let report = check_module(canonical_lang(), "").unwrap();
        assert!(report.is_clean());
    }

    #[test]
    fn check_module_simple_binding_is_clean() {
        let report = check_module(canonical_lang(), "double = x ~> x * 2").unwrap();
        assert!(
            report.is_clean(),
            "expected clean, got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn check_module_unbound_wire_errors() {
        let report = check_module(canonical_lang(), "main = foo").unwrap();
        assert!(
            report.diagnostics.iter().any(|d| d.rule == "unbound-wire"
                && d.severity == handl_plugin_ext_diag::Severity::Error),
            "expected unbound-wire error, got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn check_module_with_prelude_seeds_scope() {
        // A bare host wire (e.g. a module-chain name) fires `unbound-wire`
        // without a prelude and is in scope once the host supplies it.
        let bare = check_module(canonical_lang(), "main = somemod.trim(input) ~> input").unwrap();
        assert!(
            bare.diagnostics.iter().any(|d| d.rule == "unbound-wire"),
            "expected unbound-wire without prelude, got: {:?}",
            bare.diagnostics
        );
        let prelude: std::collections::HashSet<String> =
            ["somemod".to_string(), "input".to_string()]
                .into_iter()
                .collect();
        let seeded = check_module_with_prelude(
            canonical_lang(),
            "main = somemod.trim(input) ~> input",
            &prelude,
        )
        .unwrap();
        assert!(
            !seeded.diagnostics.iter().any(|d| d.rule == "unbound-wire"
                && d.severity == handl_plugin_ext_diag::Severity::Error),
            "prelude names should be in scope, got: {:?}",
            seeded.diagnostics
        );
        // Prelude names do not satisfy unbound-export: exports still need a
        // source-level binding.
        let exported =
            check_module_with_prelude(canonical_lang(), "export somemod", &prelude).unwrap();
        assert!(
            exported
                .diagnostics
                .iter()
                .any(|d| d.rule == "unbound-export"
                    && d.severity == handl_plugin_ext_diag::Severity::Error),
            "prelude must not satisfy unbound-export, got: {:?}",
            exported.diagnostics
        );
    }

    #[test]
    fn check_module_parse_error() {
        let result = check_module(canonical_lang(), "import from");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CheckError::Parse(_)));
    }
}
