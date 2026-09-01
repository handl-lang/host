//! # handl-plugin-engine-typecheck
//!
//! The **abstraction** crate for the typecheck operation (ADR-0054 §3).
//!
//! Splits `plugin-ext-typecheck` into an abstraction (operation marker +
//! trait, no engine) and an engine crate. This crate defines the
//! [`Typecheck`] operation marker, [`TypecheckRequest`],
//! [`TypecheckResult`], and the [`TypecheckEngine`] trait. `Foreign`
//! scheme dispatch uses [`handl_platform::ModuleLookup`] from the
//! `handl-platform` crate (type rules may call host functions).
//! **No typecheck engine code lives here** — the abstraction is the
//! contract; a concrete engine (e.g. a normalization-based checker)
//! implements it.
//!
//! Engines consume [`handl_language::Language`] **data** — they look up
//! `lang.types[node.kind]` to find a [`handl_language::Scheme`] and
//! interpret it, not match on a static Rust enum of canonical kinds.

#![warn(missing_docs)]

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
}

/// The trait a typecheck engine implements.
///
/// An engine looks up `lang.types[node.kind]` →
/// [`handl_language::TypeRuleDef`] → [`handl_language::Scheme`] and
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
}
