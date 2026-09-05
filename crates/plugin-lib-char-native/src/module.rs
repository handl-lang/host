//! The `char` module — per-character atoms backed by Rust `std::char`.
//!
//! This is the *native* backend for the `char` contract
//! (`handl-packages/char/char.hnl`): category predicates, case maps,
//! scalar conversion, and char-sequence access with character indices.
//! `str-handl` calls only `char.*`, so this module is the floor it stands
//! on in the Rust runtime.
//!
//! The interface is declared in `handl-packages/char-native/char-native.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). The Rust
//! `Function` impls in [`ops`] are registered in an [`OpRegistry`], and the
//! generic [`HandlModule`] loader dispatches each call via [`OpFunction`].

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::ops::{
    At, Chars, CodePoint, CodePointAt, Concat, FromChars, FromCodePoint, IsAlnum, IsAlpha,
    IsLowercase, IsNumeric, IsUppercase, IsWhitespace, Length, Slice, ToLower, ToTitle, ToUpper,
};

/// The handl source declaring the `char` op interface — the single
/// source of truth for the char API surface. Loaded at compile time.
const CHAR_NATIVE_HNL: &str =
    include_str!("../../../handl-packages/char-native/char-native.hnl");

/// The op names declared in `char-native.hnl` — each maps to a Rust
/// `Function` impl registered in the [`OpRegistry`].
const CHAR_OPS: &[&str] = &[
    "char.is_whitespace",
    "char.is_alpha",
    "char.is_numeric",
    "char.is_alnum",
    "char.is_uppercase",
    "char.is_lowercase",
    "char.to_upper",
    "char.to_lower",
    "char.to_title",
    "char.code_point",
    "char.from_code_point",
    "char.at",
    "char.slice",
    "char.length",
    "char.concat",
    "char.code_point_at",
    "char.chars",
    "char.from_chars",
];

/// Build the [`OpRegistry`] for char, registering the Rust handls under
/// their op names.
fn char_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("char.is_whitespace", Arc::new(IsWhitespace));
    registry.register("char.is_alpha", Arc::new(IsAlpha));
    registry.register("char.is_numeric", Arc::new(IsNumeric));
    registry.register("char.is_alnum", Arc::new(IsAlnum));
    registry.register("char.is_uppercase", Arc::new(IsUppercase));
    registry.register("char.is_lowercase", Arc::new(IsLowercase));
    registry.register("char.to_upper", Arc::new(ToUpper));
    registry.register("char.to_lower", Arc::new(ToLower));
    registry.register("char.to_title", Arc::new(ToTitle));
    registry.register("char.code_point", Arc::new(CodePoint));
    registry.register("char.from_code_point", Arc::new(FromCodePoint));
    registry.register("char.at", Arc::new(At));
    registry.register("char.slice", Arc::new(Slice));
    registry.register("char.length", Arc::new(Length));
    registry.register("char.concat", Arc::new(Concat));
    registry.register("char.code_point_at", Arc::new(CodePointAt));
    registry.register("char.chars", Arc::new(Chars));
    registry.register("char.from_chars", Arc::new(FromChars));
    registry
}

/// Build the [`HandlModule`] for char.
///
/// All 18 functions are op-backed (FFI atoms); `func_names` is empty and
/// every lookup dispatches to the [`OpRegistry`] via [`OpFunction`].
fn build_char_module() -> HandlModule {
    let registry = Arc::new(char_op_registry());
    HandlModule::new("char", CHAR_NATIVE_HNL, &[]).with_ops(CHAR_OPS, registry)
}

static CHAR_MODULE: OnceLock<HandlModule> = OnceLock::new();

fn char_module() -> &'static HandlModule {
    CHAR_MODULE.get_or_init(build_char_module)
}

/// The `char` module — per-character atoms for the `str` floor.
///
/// Provides `char.is_whitespace`, `is_alpha`, `is_numeric`, `is_alnum`,
/// `is_uppercase`, `is_lowercase`, `to_upper`, `to_lower`, `to_title`,
/// `code_point`, `from_code_point`, `at`, `slice`, `length`, `concat`,
/// `code_point_at`, `chars`, `from_chars`. All indices are character
/// indices; semantics follow Rust `std::char` (Unicode-aware).
#[derive(Clone, Copy)]
pub struct CharModule;

impl Module for CharModule {
    fn name(&self) -> &str {
        "char"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        char_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        char_module().functions()
    }
}
