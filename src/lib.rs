//! # handl-plugin-lib-lang-compile
//!
//! The `compile` meta function as a composable `plugin-lib-*` leaf crate
//! (ADR-0047 §1, §3).
//!
//! Exposes the `compile` module with one callable:
//! `compile.compile(lang, ast, target)`. It extracts the
//! [`handl_language::Language`] from the first argument, deserializes the
//! second argument as an [`handl_language::Ast`] (or accepts a tagged
//! `__ast__` value), builds a sub-runtime via
//! [`handl_runtime::RuntimeFactory`], compiles the AST to `target`, and
//! returns a tagged compiled-program handle value
//! `{"__compiled__": <id>, "target": <target>}` referencing the
//! non-serializable compiled artifact held in the thread-local registry.
//!
//! This crate is a leaf: it depends on `handl-runtime` (the abstraction +
//! `RuntimeFactory`/`RuntimeHandle` traits + `compiled_value` helper),
//! `handl-language`, and `handl-platform`/`handl-plugin-module` only — no
//! engine impl crates, no `handl-kit-*`.
//!
//! ## Wiring note
//!
//! The meta modules hold an [`Rc<dyn RuntimeFactory>`] which is not
//! `Send + Sync`, so the [`LibraryPlugin`] cannot contribute its module
//! through [`ModuleRuntime`](handl_plugin_module::ModuleRuntime). The host
//! wires the [`CompileModule`] directly via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter. See [`plugin::LibraryPlugin::contribute`].

#![warn(missing_docs)]

use std::rc::Rc;

use handl_language::{Ast, Language, as_ast, as_language};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::{RuntimeFactory, compiled_value};
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

/// The `compile` module — exposes `compile.compile(lang, ast, target)`.
///
/// Holds an [`Rc<dyn RuntimeFactory>`] (injected by the host) and an optional
/// delegate [`ModuleLookup`] for user host functions. Implements
/// [`ModuleLookup`] directly so it can be passed as the `delegate` to
/// [`RuntimeFactory::build`].
pub struct CompileModule {
    delegate: Option<Rc<dyn ModuleLookup>>,
    compile_fn: CompileFn,
}

impl CompileModule {
    /// Create a `compile` module backed by `factory`, with an optional
    /// `delegate` for user host functions.
    pub fn new(factory: Rc<dyn RuntimeFactory>, delegate: Option<Rc<dyn ModuleLookup>>) -> Self {
        let compile_fn = CompileFn {
            factory: Rc::clone(&factory),
            delegate: delegate.clone(),
        };
        Self {
            delegate,
            compile_fn,
        }
    }
}

impl Module for CompileModule {
    fn name(&self) -> &str {
        "compile"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "compile" => Some(&self.compile_fn),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("compile", &self.compile_fn as &dyn Function)]
    }
}

impl ModuleLookup for CompileModule {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "compile" | "compile.compile" => Some(&self.compile_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// The `compile.compile(lang, ast, target)` host function.
pub struct CompileFn {
    factory: Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Function for CompileFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang = extract_language(args.first(), "compile")?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("compile: expected (lang, ast, target)".into()))?;
        // Accept either a raw serialized `Ast` value or a tagged `__ast__`
        // value (the form bound by `Scheme::Vau` — ADR-0041 §6, ADR-0045).
        let ast: Ast = if let Some(tagged) = as_ast(ast_val) {
            tagged
        } else {
            serde_json::from_value(ast_val.clone())
                .map_err(|e| EvalError::Other(format!("compile: {}", e)))?
        };
        let target = args.get(2).and_then(|v| v.as_str()).unwrap_or("native");
        let handle = self
            .factory
            .build(lang, self.delegate.clone())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let compiled = handle
            .compile(&ast, target)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(compiled_value(compiled.id, &compiled.target))
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;
