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
//! `Send + Sync`, so the [`LangParseLibraryPlugin`] cannot contribute its
//! module through [`ModuleRuntime`](handl_plugin_module::ModuleRuntime). The
//! host wires the [`ParseModule`] directly via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter. See [`LangParseLibraryPlugin`].

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

// ---------------------------------------------------------------------------
// LangParseLibraryPlugin (wraps HandlLibraryPlugin, ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the `lang.parse` interface. Loaded at compile
/// time from the kit-stdlib data dir. The parse engine stays Rust
/// (ADR-0077 "Special"); this .hnl declares the interface only.
const LANG_PARSE_HNL: &str = include_str!("../../../kit-stdlib/data/lang_parse/lang_parse.hnl");

/// A `Library` plugin that registers the `lang.parse` implementation via
/// the generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The parse engine logic stays in Rust (ADR-0077 "Special"); only the plugin
/// scaffolding is wrapped with [`HandlLibraryPlugin`] for interface
/// declaration.
pub struct LangParseLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl LangParseLibraryPlugin {
    /// Create a new `LangParseLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "lang.parse",
                "lib.lang.parse",
                "lib-lang-parse",
                LANG_PARSE_HNL,
                &[],
            ),
        }
    }
}

impl Default for LangParseLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for LangParseLibraryPlugin {
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

/// Legacy `LibraryPlugin` — delegates to [`LangParseLibraryPlugin`].
pub use LangParseLibraryPlugin as LibraryPlugin;

#[cfg(test)]
mod tests;
