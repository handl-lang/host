//! # handl-plugin-lib-lang-eval
//!
//! The `eval` meta function as a composable `plugin-lib-*` leaf crate
//! (ADR-0047 §1).
//!
//! Exposes the `eval` module with one callable: `eval.eval(lang, ast)`. It
//! extracts the [`handl_language::Language`] from the first argument, accepts
//! the second argument as either a tagged `__ast__` value (the form bound
//! to an unevaluated expression) or a raw JSON-serialized
//! [`handl_language::Ast`], builds a sub-runtime via
//! [`handl_runtime::RuntimeFactory`], evaluates the AST, and returns the
//! result value directly (no serialization).
//!
//! This crate is a leaf: it depends on `handl-runtime` (the abstraction +
//! `RuntimeFactory`/`RuntimeHandle` traits), `handl-language`, and
//! `handl-platform`/`handl-plugin-module` only — no engine impl crates, no
//! `handl-kit-*`.
//!
//! ## Wiring note
//!
//! The meta modules hold an [`Rc<dyn RuntimeFactory>`] which is not
//! `Send + Sync`, so the [`LangEvalLibraryPlugin`] cannot contribute its
//! module through [`ModuleRuntime`](handl_plugin_module::ModuleRuntime). The
//! host wires the [`EvalModule`] directly via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter. See [`LangEvalLibraryPlugin`].

#![warn(missing_docs)]

use std::rc::Rc;

use handl_language::{Ast, Language, as_ast, as_language};
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

/// The `eval` module — exposes `eval.eval(lang, ast)`.
///
/// Holds an [`Rc<dyn RuntimeFactory>`] (injected by the host) and an optional
/// delegate [`ModuleLookup`] for user host functions. Implements
/// [`ModuleLookup`] directly so it can be passed as the `delegate` to
/// [`RuntimeFactory::build`].
pub struct EvalModule {
    delegate: Option<Rc<dyn ModuleLookup>>,
    eval_fn: EvalFn,
}

impl EvalModule {
    /// Create an `eval` module backed by `factory`, with an optional
    /// `delegate` for user host functions.
    pub fn new(factory: Rc<dyn RuntimeFactory>, delegate: Option<Rc<dyn ModuleLookup>>) -> Self {
        let eval_fn = EvalFn {
            factory: Rc::clone(&factory),
            delegate: delegate.clone(),
        };
        Self { delegate, eval_fn }
    }
}

impl Module for EvalModule {
    fn name(&self) -> &str {
        "eval"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "eval" => Some(&self.eval_fn),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("eval", &self.eval_fn as &dyn Function)]
    }
}

impl ModuleLookup for EvalModule {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "eval" | "eval.eval" => Some(&self.eval_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// The `eval.eval(lang, ast)` host function.
pub struct EvalFn {
    factory: Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Function for EvalFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang = extract_language(args.first(), "eval")?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("eval: expected (lang, ast)".into()))?;
        // Accept either a raw serialized `Ast` value or a tagged `__ast__`
        // value (the form bound to an unevaluated expression (ADR-0041 §6)).
        let ast: Ast = if let Some(tagged) = as_ast(ast_val) {
            tagged
        } else {
            serde_json::from_value(ast_val.clone())
                .map_err(|e| EvalError::Other(format!("eval: {}", e)))?
        };
        let handle = self
            .factory
            .build(lang, self.delegate.clone())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        handle
            .eval(&ast)
            .map_err(|e| EvalError::Other(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// LangEvalLibraryPlugin (wraps HandlLibraryPlugin, ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the `lang.eval` interface. Loaded at compile
/// time from the kit-stdlib data dir. The eval engine stays Rust
/// (ADR-0077 "Special"); this .hnl declares the interface only.
const LANG_EVAL_HNL: &str = include_str!("../../../handl-packages/lang_eval/lang_eval.hnl");

/// A `Library` plugin that registers the `lang.eval` implementation via
/// the generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The eval engine logic stays in Rust (ADR-0077 "Special"); only the plugin
/// scaffolding is wrapped with [`HandlLibraryPlugin`] for interface
/// declaration.
pub struct LangEvalLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl LangEvalLibraryPlugin {
    /// Create a new `LangEvalLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "lang.eval",
                "lib.lang.eval",
                "lib-lang-eval",
                LANG_EVAL_HNL,
                &[],
            ),
        }
    }
}

impl Default for LangEvalLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for LangEvalLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Legacy `LibraryPlugin` — delegates to [`LangEvalLibraryPlugin`].
pub use LangEvalLibraryPlugin as LibraryPlugin;

#[cfg(test)]
mod tests;
