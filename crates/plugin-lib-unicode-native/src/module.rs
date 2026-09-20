//! The native `unicode` module.
//!
//! Composes ICU4X-backed native ops (registered as `op`s) with the pure-Handl
//! `unicode-handl` record as a fallback. The `op` functions take precedence,
//! so the table-dependent ops (`nfc`, `nfd`, `graphemes`, `grapheme_count`,
//! `width`, `casefold`, `decode`, `is_valid_identifier`, `is_xid_start`,
//! `is_xid_continue`) are evaluated natively, while `code_points`,
//! `from_code_points`, `is_ascii`, and `code_point_at` fall back to the
//! handl implementation unless native overrides are registered.

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::ops::{
    Casefold, CodePointAt, CodePoints, Decode, FromCodePoints, GraphemeCount, Graphemes, IsAscii,
    IsValidIdentifier, IsXidContinue, IsXidStart, Nfc, Nfd, Width,
};

/// The handl source providing the pure-Handl fallback (`unicode-handl`).
const UNICODE_HANDL_HNL: &str =
    include_str!("../../../handl-packages/unicode-handl/unicode-handl.hnl");

/// Record fields exposed by `unicode-handl.hnl`. These become `HandlFunction`
/// callables; any name also registered as a native `op` is shadowed by the op.
const UNICODE_HANDL_FUNCS: &[&str] = &[
    "code_points",
    "from_code_points",
    "is_ascii",
    "code_point_at",
    "graphemes",
    "grapheme_count",
    "width",
    "casefold",
];

/// Native `op` names implemented by this module.
const UNICODE_NATIVE_OPS: &[&str] = &[
    "unicode.code_points",
    "unicode.from_code_points",
    "unicode.is_ascii",
    "unicode.code_point_at",
    "unicode.decode",
    "unicode.is_valid_identifier",
    "unicode.is_xid_start",
    "unicode.is_xid_continue",
    "unicode.graphemes",
    "unicode.grapheme_count",
    "unicode.width",
    "unicode.casefold",
    "unicode.nfc",
    "unicode.nfd",
];

/// Build the [`OpRegistry`] for the native `unicode` ops.
fn unicode_native_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("unicode.code_points", Arc::new(CodePoints));
    registry.register("unicode.from_code_points", Arc::new(FromCodePoints));
    registry.register("unicode.is_ascii", Arc::new(IsAscii));
    registry.register("unicode.code_point_at", Arc::new(CodePointAt));
    registry.register("unicode.decode", Arc::new(Decode));
    registry.register("unicode.is_valid_identifier", Arc::new(IsValidIdentifier));
    registry.register("unicode.is_xid_start", Arc::new(IsXidStart));
    registry.register("unicode.is_xid_continue", Arc::new(IsXidContinue));
    registry.register("unicode.graphemes", Arc::new(Graphemes));
    registry.register("unicode.grapheme_count", Arc::new(GraphemeCount));
    registry.register("unicode.width", Arc::new(Width));
    registry.register("unicode.casefold", Arc::new(Casefold));
    registry.register("unicode.nfc", Arc::new(Nfc));
    registry.register("unicode.nfd", Arc::new(Nfd));
    registry
}

/// Build the composed [`HandlModule`] for `unicode`.
fn build_unicode_module() -> HandlModule {
    let registry = Arc::new(unicode_native_op_registry());
    HandlModule::new("unicode", UNICODE_HANDL_HNL, UNICODE_HANDL_FUNCS)
        .with_dependency(CharModule)
        .with_ops(UNICODE_NATIVE_OPS, registry)
}

static UNICODE_MODULE: OnceLock<HandlModule> = OnceLock::new();

fn unicode_module() -> &'static HandlModule {
    // Build under `with_loader_disabled`: `HandlModule::new`'s
    // `analyze_source` would otherwise run `loader.analyze` — a nested
    // eval whose `call_host` misses probe `modules.lookup`, which can
    // re-enter this very `OnceLock` and deadlock on the futex. The
    // bootstrap path is pure text transformation — no eval, no lookup.
    UNICODE_MODULE.get_or_init(|| {
        handl_plugin_lib_handl::with_loader_disabled(build_unicode_module)
    })
}

/// The `unicode` module: native ICU4X-backed ops over a pure-Handl fallback.
#[derive(Clone, Copy)]
pub struct UnicodeModule;

impl UnicodeModule {
    /// Eagerly initialize the shared [`HandlModule`].
    ///
    /// `function()`/`functions()` lazily init through `OnceLock`; a
    /// `modules.lookup` miss that probes `unicode` while its own
    /// `HandlModule::new` is mid-init (its `analyze_source` runs a nested
    /// eval that itself does `modules.lookup`) re-enters the same
    /// `OnceLock` and deadlocks. Hosts must call this at registration
    /// time, before any eval can reach `function()`.
    pub fn prewarm() {
        unicode_module();
    }
}

impl Module for UnicodeModule {
    fn name(&self) -> &str {
        "unicode"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        unicode_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        unicode_module().functions()
    }
}
