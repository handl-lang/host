//! The `parse` module — generic string parsing.
//!
//! Provides the function callable as `parse.parse(input, parser)` in handl
//! expressions.
//!
//! The parse *interface* is declared in `handl-packages/utils/parse.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). The function is
//! an FFI atom — it delegates to a registry of `DynParser` impls (from
//! `plugin-lib-parse`), specifically `Whatwg` and `Rfc` URL parsers, and
//! cannot be expressed as pure handl arrows without losing the exact error
//! messages the tests rely on. The Rust `Function` impl in [`ops`] is
//! registered as a handl (handler) in an [`OpRegistry`], and the generic
//! [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader dispatches
//! each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};
use handl_plugin_lib_parse::DynParser;
use handl_plugin_lib_url_rfc::Rfc;
use handl_plugin_lib_url_whatwg::Whatwg;

use ops::Parse;

/// The handl source declaring the parse `op` interface — the single
/// source of truth for the parse API surface. Loaded at compile time
/// from the kit-stdlib data dir.
///
/// `parse.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for the parse function. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); the function is
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const PARSE_HNL: &str = include_str!("../../../../handl-packages/utils/parse.hnl");

/// The op names declared in `parse.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const PARSE_OPS: &[&str] = &["parse.parse"];

/// Build the [`OpRegistry`] for parse, registering the Rust handl under
/// its op name.
///
/// The op name `"parse.parse"` maps to its Rust `Function` impl ([`Parse`]).
/// The impl is stateful — it holds a `HashMap` of named `DynParser`s — so it
/// is constructed with the default parsers and `Arc::new`ed into the
/// registry. No runtime guarantee checkers are attached — the parse atom is
/// trusted.
fn parse_op_registry() -> OpRegistry {
    let mut parsers: HashMap<String, Arc<dyn DynParser>> = HashMap::new();
    parsers.insert("url_whatwg".to_string(), Arc::new(Whatwg));
    parsers.insert("url_rfc".to_string(), Arc::new(Rfc));
    let mut registry = OpRegistry::new();
    registry.register("parse.parse", Arc::new(Parse { parsers }));
    registry
}

/// Build the [`HandlModule`] for parse.
///
/// The function is op-backed (an FFI atom). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_parse_module() -> HandlModule {
    let registry = Arc::new(parse_op_registry());
    HandlModule::new("parse", PARSE_HNL, &[]).with_ops(PARSE_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for parse.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static PARSE_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for parse, initializing it on first call.
fn parse_module() -> &'static HandlModule {
    PARSE_MODULE.get_or_init(build_parse_module)
}

/// The `parse` module — generic string parsing.
///
/// Provides `parse.parse(input, parser)`. The interface is declared in
/// `parse.hnl` via `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct ParseModule;

impl ParseModule {
    /// Creates a `ParseModule` with the default parsers.
    ///
    /// The module is backed by a shared, lazily-initialized [`HandlModule`]
    /// holding a registry of `DynParser`s (`url_whatwg`, `url_rfc`).
    pub fn new() -> Self {
        Self
    }
}

impl Default for ParseModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ParseModule {
    fn name(&self) -> &str {
        "parse"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        parse_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        parse_module().functions()
    }
}
