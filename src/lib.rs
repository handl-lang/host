//! # handl-plugin-lib-lang-typecheck
//!
//! The `typecheck` meta function as a composable `plugin-lib-*` leaf crate
//! (ADR-0054 §6, ADR-0047 §1).
//!
//! Exposes the `typecheck` module with one callable:
//! `typecheck.typecheck(lang, ast)`. It extracts the
//! [`handl_language::Language`] from the first argument, accepts the second
//! argument as either a tagged `__ast__` value or a raw JSON-serialized
//! [`handl_language::Ast`], builds a sub-runtime via
//! [`handl_runtime::RuntimeFactory`], typechecks the AST, and returns the
//! synthesized type (a value — types are values, ADR-0032).
//!
//! This is the **pure-eval** path (ADR-0054 §"The typecheck host is pure"):
//! type expressions are evaluated via the bound typecheck engine without
//! running effects. The CLI's `typecheck` command calls this.
//!
//! This crate is a leaf: it depends on `handl-runtime` (the abstraction +
//! `RuntimeFactory`/`RuntimeHandle` traits), `handl-language`, and
//! `handl-platform`/`handl-plugin-module` only — no engine impl crates, no
//! `handl-kit-*`.
//!
//! ## Wiring note
//!
//! The meta modules hold an [`Rc<dyn RuntimeFactory>`] which is not
//! `Send + Sync`, so the [`LibraryPlugin`] cannot contribute its module
//! through [`ModuleRuntime`](handl_plugin_module::ModuleRuntime). The host
//! wires the [`TypecheckModule`] directly via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter. See [`plugin::LibraryPlugin::contribute`].

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

/// The `typecheck` module — exposes `typecheck.typecheck(lang, ast)`.
///
/// Holds an [`Rc<dyn RuntimeFactory>`] (injected by the host) and an optional
/// delegate [`ModuleLookup`] for user host functions. Implements
/// [`ModuleLookup`] directly so it can be passed as the `delegate` to
/// [`RuntimeFactory::build`].
pub struct TypecheckModule {
    delegate: Option<Rc<dyn ModuleLookup>>,
    typecheck_fn: TypecheckFn,
}

impl TypecheckModule {
    /// Create a `typecheck` module backed by `factory`, with an optional
    /// `delegate` for user host functions.
    pub fn new(
        factory: Rc<dyn RuntimeFactory>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        let typecheck_fn = TypecheckFn {
            factory: Rc::clone(&factory),
            delegate: delegate.clone(),
        };
        Self {
            delegate,
            typecheck_fn,
        }
    }
}

impl Module for TypecheckModule {
    fn name(&self) -> &str {
        "typecheck"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "typecheck" => Some(&self.typecheck_fn),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("typecheck", &self.typecheck_fn as &dyn Function)]
    }
}

impl ModuleLookup for TypecheckModule {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "typecheck" | "typecheck.typecheck" => Some(&self.typecheck_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// The `typecheck.typecheck(lang, ast)` host function (ADR-0054 §6).
///
/// Builds a sub-runtime via the [`RuntimeFactory`], typechecks the AST, and
/// returns the synthesized type as a value. This is the pure-eval path: type
/// expressions are evaluated without running effects.
pub struct TypecheckFn {
    factory: Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Function for TypecheckFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang = extract_language(args.first(), "typecheck")?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("typecheck: expected (lang, ast)".into()))?;
        // Accept either a raw serialized `Ast` value or a tagged `__ast__`
        // value (the form bound to an unevaluated expression (ADR-0041 §6)).
        let ast: Ast = if let Some(tagged) = as_ast(ast_val) {
            tagged
        } else {
            serde_json::from_value(ast_val.clone())
                .map_err(|e| EvalError::Other(format!("typecheck: {}", e)))?
        };
        let handle = self
            .factory
            .build(lang, self.delegate.clone())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let result = handle
            .typecheck(&ast)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(result.typ)
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;

#[cfg(test)]
mod tests;
