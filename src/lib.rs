//! # handl-plugin-lib-lang-run-compiled
//!
//! The `run_compiled` meta function as a composable `plugin-lib-*` leaf crate
//! (ADR-0047 §1, §3).
//!
//! Exposes the `run_compiled` module with one callable:
//! `run_compiled.run_compiled(handle)`. It extracts the compiled-program
//! handle id and target from the tagged value
//! `{"__compiled__": <id>, "target": <target>}`, builds a sub-runtime via
//! [`handl_runtime::RuntimeFactory`] (with a dummy [`handl_language::Language`]
//! — the compiled artifact carries its own `lang`/`ast`), and executes the
//! artifact referenced by the handle, returning the result value.
//!
//! This crate is a leaf: it depends on `handl-runtime` (the abstraction +
//! `RuntimeFactory`/`RuntimeHandle` traits + `as_compiled_id`/
//! `as_compiled_target` helpers), `handl-language`, and
//! `handl-platform`/`handl-plugin-module` only — no engine impl crates, no
//! `handl-kit-*`.
//!
//! ## Wiring note
//!
//! The meta modules hold an [`Rc<dyn RuntimeFactory>`] which is not
//! `Send + Sync`, so the [`LibraryPlugin`] cannot contribute its module
//! through [`ModuleRuntime`](handl_plugin_module::ModuleRuntime). The host
//! wires the [`RunCompiledModule`] directly via
//! [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build)'s
//! `delegate` parameter. See [`plugin::LibraryPlugin::contribute`].

#![warn(missing_docs)]

use std::rc::Rc;

use handl_language::Language;
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::{CompiledHandle, RuntimeFactory, as_compiled_id, as_compiled_target};
use serde_json::Value;

/// The `run_compiled` module — exposes `run_compiled.run_compiled(handle)`.
///
/// Holds an [`Rc<dyn RuntimeFactory>`] (injected by the host) and an optional
/// delegate [`ModuleLookup`] for user host functions. Implements
/// [`ModuleLookup`] directly so it can be passed as the `delegate` to
/// [`RuntimeFactory::build`].
pub struct RunCompiledModule {
    delegate: Option<Rc<dyn ModuleLookup>>,
    run_compiled_fn: RunCompiledFn,
}

impl RunCompiledModule {
    /// Create a `run_compiled` module backed by `factory`, with an optional
    /// `delegate` for user host functions.
    pub fn new(factory: Rc<dyn RuntimeFactory>, delegate: Option<Rc<dyn ModuleLookup>>) -> Self {
        let run_compiled_fn = RunCompiledFn {
            factory: Rc::clone(&factory),
            delegate: delegate.clone(),
        };
        Self {
            delegate,
            run_compiled_fn,
        }
    }
}

impl Module for RunCompiledModule {
    fn name(&self) -> &str {
        "run_compiled"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "run_compiled" => Some(&self.run_compiled_fn),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("run_compiled", &self.run_compiled_fn as &dyn Function)]
    }
}

impl ModuleLookup for RunCompiledModule {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "run_compiled" | "run_compiled.run_compiled" => Some(&self.run_compiled_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// The `run_compiled.run_compiled(handle)` host function.
pub struct RunCompiledFn {
    factory: Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Function for RunCompiledFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let handle = args
            .first()
            .ok_or_else(|| EvalError::Other("run_compiled: expected (handle)".into()))?;
        let id = as_compiled_id(handle).ok_or_else(|| {
            EvalError::Other("run_compiled: expected a compiled-program handle".into())
        })?;
        let target = as_compiled_target(handle).unwrap_or("");
        let handle = CompiledHandle {
            id,
            target: target.to_string(),
        };

        // Build a runtime to execute the compiled artifact. The runtime
        // re-attaches the meta host functions (via the factory) so the
        // compiled program can recurse via `Foreign` dispatch.
        //
        // We don't have the original `lang` / `ast` here — the artifact
        // carries them (ADR-0047 §3). We build a runtime with a dummy
        // language (the artifact's `run` method uses its own `lang`/`ast`).
        let dummy_lang = Language::new();
        let runtime = self
            .factory
            .build(dummy_lang, self.delegate.clone())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        runtime
            .run_compiled(&handle)
            .map_err(|e| EvalError::Other(e.to_string()))
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;
