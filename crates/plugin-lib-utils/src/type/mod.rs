//! The `typeof` module — type coercion, predicates, and utility functions.
//!
//! Provides functions callable as `typeof.to_string(v)`,
//! `typeof.to_number(v)`, `typeof.to_bool(v)`, `typeof.type_of(v)`,
//! `typeof.identity(v)`, `typeof.default(v)`, `typeof.coalesce(v)` in
//! handl expressions.
//!
//! Note: the module was originally named `type`, but `type` became a keyword
//! in v0.1 (ADR-0074), so the module was renamed to `typeof` to avoid the
//! conflict. The function `typeof.type_of(v)` is the primary type-checking
//! function used in handl source files.
//!
//! The typeof *interface* is declared in `handl-packages/utils/type.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 13
//! functions and 12 type-tag constants are FFI atoms — they call Rust
//! type-checking and coercion logic that cannot be expressed as pure
//! handl arrows without losing the exact error messages the tests rely
//! on. The Rust `Function` impls in [`ops`] are registered as handls
//! (handlers) in an [`OpRegistry`], and the generic
//! [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader dispatches
//! each call to the registered handler via [`OpFunction`].
//!
//! # NaN and Infinity
//!
//! `serde_json::Value` cannot represent NaN or Infinity — they serialize as
//! `null`. Values that would be NaN or Infinity are therefore handled by the
//! `null` branch in each function, yielding `"null"`, `0.0`, and `false`
//! respectively. This is a deliberate graceful-degradation choice, not a bug.

mod ops;

pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

/// The handl source declaring the typeof `op` interface — the single
/// source of truth for the typeof API surface. Loaded at compile time
/// from the kit-stdlib data dir.
///
/// `type.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 13 functions and 12
/// type-tag constants. The [`HandlModule`] loader strips comments and
/// declaration lines, leaving an empty expression (no pure-logic
/// arrows); all functions are resolved via [`OpFunction`] dispatching to
/// the [`OpRegistry`].
const TYPE_HNL: &str = include_str!("../../../../handl-packages/utils/type.hnl");

/// The op names declared in `type.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const TYPE_OPS: &[&str] = &[
    "typeof.string",
    "typeof.str",
    "typeof.number",
    "typeof.num",
    "typeof.bool",
    "typeof.null",
    "typeof.array",
    "typeof.arr",
    "typeof.object",
    "typeof.obj",
    "typeof.any",
    "typeof.unknown",
    "typeof.is",
    "typeof.must",
    "typeof.may",
    "typeof.not",
    "typeof.when",
    "typeof.to_string",
    "typeof.to_number",
    "typeof.to_bool",
    "typeof.type_of",
    "typeof.typeof",
    "typeof.identity",
    "typeof.default",
    "typeof.coalesce",
];

/// Build the [`OpRegistry`] for typeof, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"typeof.to_string"`) maps to its Rust `Function`
/// impl (e.g. [`ToString`]). The type-tag constants (e.g.
/// [`STRING_TAG`]) are registered under their tag op names. No runtime
/// guarantee checkers are attached — the typeof atoms are trusted.
fn type_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    // Type-tag constants
    registry.register("typeof.string", Arc::new(STRING_TAG));
    registry.register("typeof.str", Arc::new(STR_TAG));
    registry.register("typeof.number", Arc::new(NUMBER_TAG));
    registry.register("typeof.num", Arc::new(NUM_TAG));
    registry.register("typeof.bool", Arc::new(BOOL_TAG));
    registry.register("typeof.null", Arc::new(NULL_TAG));
    registry.register("typeof.array", Arc::new(ARRAY_TAG));
    registry.register("typeof.arr", Arc::new(ARR_TAG));
    registry.register("typeof.object", Arc::new(OBJECT_TAG));
    registry.register("typeof.obj", Arc::new(OBJ_TAG));
    registry.register("typeof.any", Arc::new(ANY_TAG));
    registry.register("typeof.unknown", Arc::new(UNKNOWN_TAG));
    // Predicates
    registry.register("typeof.is", Arc::new(TypeIs));
    registry.register("typeof.must", Arc::new(TypeMust));
    registry.register("typeof.may", Arc::new(TypeMay));
    registry.register("typeof.not", Arc::new(TypeNot));
    registry.register("typeof.when", Arc::new(TypeWhen));
    // Coercion
    registry.register("typeof.to_string", Arc::new(ToString));
    registry.register("typeof.to_number", Arc::new(ToNumber));
    registry.register("typeof.to_bool", Arc::new(ToBool));
    // Type-of
    registry.register("typeof.type_of", Arc::new(TypeOf));
    registry.register("typeof.typeof", Arc::new(TypeOf));
    // Utilities
    registry.register("typeof.identity", Arc::new(Identity));
    registry.register("typeof.default", Arc::new(Default));
    registry.register("typeof.coalesce", Arc::new(Coalesce));
    registry
}

/// Build the [`HandlModule`] for typeof.
///
/// All 13 functions and 12 type-tag constants are op-backed (FFI
/// atoms). No pure-logic arrows are used (`func_names` is empty); the
/// `HandlModule` dispatches every function lookup to the [`OpRegistry`]
/// via [`OpFunction`].
fn build_type_module() -> HandlModule {
    let registry = Arc::new(type_op_registry());
    HandlModule::new("typeof", TYPE_HNL, &[]).with_ops(TYPE_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for typeof.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static TYPE_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for typeof, initializing it on first call.
fn type_module() -> &'static HandlModule {
    TYPE_MODULE.get_or_init(build_type_module)
}

/// The `typeof` module — type coercion and utility functions.
///
/// Provides `typeof.to_string`, `typeof.to_number`, `typeof.to_bool`,
/// `typeof.type_of`, `typeof.typeof`, `typeof.identity`, `typeof.default`,
/// `typeof.coalesce`, `typeof.is`, `typeof.must`, `typeof.may`,
/// `typeof.not`, `typeof.when`, and type-tag constants (`typeof.string`,
/// `typeof.str`, `typeof.number`, `typeof.num`, `typeof.bool`,
/// `typeof.null`, `typeof.array`, `typeof.arr`, `typeof.object`,
/// `typeof.obj`, `typeof.any`, `typeof.unknown`). The interface is
/// declared in `type.hnl` via `op`/`handl` declarations and loaded via
/// the generic [`HandlModule`](handl_plugin_lib_handl::HandlModule); this
/// struct only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct TypeModule;

impl Module for TypeModule {
    fn name(&self) -> &str {
        "typeof"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        type_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        type_module().functions()
    }
}

#[cfg(test)]
mod utilities_tests;
