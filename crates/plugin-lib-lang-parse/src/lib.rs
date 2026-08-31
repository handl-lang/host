//! # handl-plugin-lib-lang-parse
//!
//! The `parse` meta function as a composable `plugin-lib-*` leaf crate
//! (ADR-0047 §1).
//!
//! Exposes the `parse` module with one callable: `parse.parse(lang, tokens)`.
//! It extracts the [`handl_language::Language`] from the first argument,
//! deserializes the second argument as [`handl_language::Tokens`], builds a
//! sub-runtime via [`handl_runtime::RuntimeFactory`], parses the tokens into
//! an [`handl_language::Ast`], and returns it as a JSON-serialized value.
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
//! wires the [`ParseModule`] directly via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter. See [`plugin::LibraryPlugin::contribute`].

#![warn(missing_docs)]

use std::rc::Rc;

use handl_language::{Ast, Language, Tokens, as_language};
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

/// The `parse` module — exposes `parse.parse(lang, tokens)`.
///
/// Holds an [`Rc<dyn RuntimeFactory>`] (injected by the host) and an optional
/// delegate [`ModuleLookup`] for user host functions. Implements
/// [`ModuleLookup`] directly so it can be passed as the `delegate` to
/// [`RuntimeFactory::build`].
pub struct ParseModule {
    delegate: Option<Rc<dyn ModuleLookup>>,
    parse_fn: ParseFn,
}

impl ParseModule {
    /// Create a `parse` module backed by `factory`, with an optional
    /// `delegate` for user host functions.
    pub fn new(factory: Rc<dyn RuntimeFactory>, delegate: Option<Rc<dyn ModuleLookup>>) -> Self {
        let parse_fn = ParseFn {
            factory: Rc::clone(&factory),
            delegate: delegate.clone(),
        };
        Self { delegate, parse_fn }
    }
}

impl Module for ParseModule {
    fn name(&self) -> &str {
        "parse"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "parse" => Some(&self.parse_fn),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("parse", &self.parse_fn as &dyn Function)]
    }
}

impl ModuleLookup for ParseModule {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "parse" | "parse.parse" => Some(&self.parse_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// The `parse.parse(lang, tokens)` host function.
pub struct ParseFn {
    factory: Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Function for ParseFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang = extract_language(args.first(), "parse")?;
        let tokens_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("parse: expected (lang, tokens)".into()))?;
        let tokens: Tokens = serde_json::from_value(tokens_val.clone())
            .map_err(|e| EvalError::Other(format!("parse: {}", e)))?;
        let handle = self
            .factory
            .build(lang, self.delegate.clone())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let ast: Ast = handle
            .parse(&tokens)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        serde_json::to_value(&ast).map_err(|e| EvalError::Other(format!("parse: {}", e)))
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;

#[cfg(test)]
mod tests;
