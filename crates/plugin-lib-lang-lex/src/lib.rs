//! # handl-plugin-lib-lang-lex
//!
//! The `lex` meta function as a composable `plugin-lib-*` leaf crate
//! (ADR-0047 §1).
//!
//! Exposes the `lex` module with one callable: `lex.lex(lang, text)`. It
//! extracts the [`handl_language::Language`] from the first argument, builds a
//! sub-runtime via [`handl_runtime::RuntimeFactory`], lexes `text` into
//! tokens, and returns them as a JSON-serialized [`handl_language::Tokens`]
//! value.
//!
//! This crate is a leaf: it depends on `handl-runtime` (the abstraction +
//! `RuntimeFactory`/`RuntimeHandle` traits), `handl-language`, and
//! `handl-platform`/`handl-plugin-module` only — no engine impl crates, no
//! `handl-kit-*`. The [`LibraryPlugin`] holds an [`Rc<dyn RuntimeFactory>`]
//! injected by the host; each invocation calls `factory.build(lang, delegate)`
//! then `handle.lex(...)`.
//!
//! ## Wiring note
//!
//! Unlike most `plugin-lib-*` crates, the meta modules hold an
//! [`Rc<dyn RuntimeFactory>`] which is not `Send + Sync`. The platform's
//! [`Plugin`](handl_platform::Plugin) trait requires `Send + Sync`, so the
//! meta [`LibraryPlugin`] cannot contribute its module through
//! [`ModuleRuntime`](handl_plugin_module::ModuleRuntime) (whose
//! [`ModuleRule`](handl_plugin_module::ModuleRule) requires `Send + Sync`).
//! Instead, the [`LexModule`] is wired directly into the runtime via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter (a [`Rc<dyn ModuleLookup>`]). The
//! [`LibraryPlugin::contribute`](plugin::LibraryPlugin::contribute) is a
//! no-op documenting this.

#![warn(missing_docs)]

use std::rc::Rc;

use handl_language::{Language, Source, Tokens, as_language};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::RuntimeFactory;
use serde_json::Value;

/// Extract a [`Language`] value from the first argument of a meta host
/// function call.
fn extract_language(arg: Option<&Value>, fn_name: &str) -> Result<Language, EvalError> {
    let v = arg.ok_or_else(|| {
        EvalError::Other(format!(
            "{}: expected a Language value as first argument",
            fn_name
        ))
    })?;
    as_language(v).ok_or_else(|| {
        EvalError::Other(format!(
            "{}: expected a Language value as first argument",
            fn_name
        ))
    })
}

/// The `lex` module — exposes `lex.lex(lang, text)`.
///
/// Holds an [`Rc<dyn RuntimeFactory>`] (injected by the host) and an optional
/// delegate [`ModuleLookup`] for user host functions. The module implements
/// [`ModuleLookup`] directly so it can be passed as the `delegate` to
/// [`RuntimeFactory::build`], matching the bare-name lookup the eval engines
/// use (e.g. `foreign.lex` resolves the flat name `"lex"`).
pub struct LexModule {
    delegate: Option<Rc<dyn ModuleLookup>>,
    lex_fn: LexFn,
}

impl LexModule {
    /// Create a `lex` module backed by `factory`, with an optional `delegate`
    /// for user host functions.
    pub fn new(factory: Rc<dyn RuntimeFactory>, delegate: Option<Rc<dyn ModuleLookup>>) -> Self {
        let lex_fn = LexFn {
            factory: Rc::clone(&factory),
            delegate: delegate.clone(),
        };
        Self { delegate, lex_fn }
    }
}

impl Module for LexModule {
    fn name(&self) -> &str {
        "lex"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "lex" => Some(&self.lex_fn),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("lex", &self.lex_fn as &dyn Function)]
    }
}

impl ModuleLookup for LexModule {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "lex" | "lex.lex" => Some(&self.lex_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// The `lex.lex(lang, text)` host function.
pub struct LexFn {
    factory: Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Function for LexFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang = extract_language(args.first(), "lex")?;
        let text = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("lex: expected (lang, text)".into()))?;
        let handle = self
            .factory
            .build(lang, self.delegate.clone())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let tokens: Tokens = handle
            .lex(&Source::new(text))
            .map_err(|e| EvalError::Other(e.to_string()))?;
        serde_json::to_value(&tokens).map_err(|e| EvalError::Other(format!("lex: {}", e)))
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;
