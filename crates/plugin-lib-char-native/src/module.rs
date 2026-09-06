//! The `char` module — per-character atoms backed by Rust `std::char`.
//!
//! This is the *native* backend for the `char` contract
//! (`handl-packages/char/char.hnl`): category predicates, case maps,
//! scalar conversion, and char-sequence access with character indices.
//! `str-handl` calls only `char.*`, so this module is the floor it stands
//! on in the Rust runtime.
//!
//! The interface is declared in `handl-packages/char-native/char-native.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 18 functions
//! are op-backed (FFI atoms), so the module dispatches each call directly to
//! the Rust `Function` impls in [`ops`] — no `.hnl` runtime is needed.

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};

use crate::ops::{
    At, Chars, CodePoint, CodePointAt, Concat, FromChars, FromCodePoint, IsAlnum, IsAlpha,
    IsLowercase, IsNumeric, IsUppercase, IsWhitespace, Length, Slice, ToLower, ToTitle, ToUpper,
};

/// Build the op table for char: each op name maps to its Rust `Function`
/// impl.
/// Op names are keyed by their *local* name (the part after the last `.`),
/// matching `HandlModule::with_ops` semantics: `ModuleLookup` resolves the
/// qualified `char.at` by splitting on the first `.` and asking the `char`
/// module for `at`.
fn char_ops() -> Vec<(&'static str, Arc<dyn Function + Send + Sync>)> {
    vec![
        ("is_whitespace", Arc::new(IsWhitespace) as Arc<dyn Function + Send + Sync>),
        ("is_alpha", Arc::new(IsAlpha)),
        ("is_numeric", Arc::new(IsNumeric)),
        ("is_alnum", Arc::new(IsAlnum)),
        ("is_uppercase", Arc::new(IsUppercase)),
        ("is_lowercase", Arc::new(IsLowercase)),
        ("to_upper", Arc::new(ToUpper)),
        ("to_lower", Arc::new(ToLower)),
        ("to_title", Arc::new(ToTitle)),
        ("code_point", Arc::new(CodePoint)),
        ("from_code_point", Arc::new(FromCodePoint)),
        ("at", Arc::new(At)),
        ("slice", Arc::new(Slice)),
        ("length", Arc::new(Length)),
        ("concat", Arc::new(Concat)),
        ("code_point_at", Arc::new(CodePointAt)),
        ("chars", Arc::new(Chars)),
        ("from_chars", Arc::new(FromChars)),
    ]
}

static CHAR_OPS: OnceLock<Vec<(&'static str, Arc<dyn Function + Send + Sync>)>> = OnceLock::new();

fn ops() -> &'static [(&'static str, Arc<dyn Function + Send + Sync>)] {
    CHAR_OPS.get_or_init(char_ops)
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
        ops()
            .iter()
            .find(|(op, _)| *op == name)
            .map(|(_, f)| &**f as &dyn Function)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        ops()
            .iter()
            .map(|(op, f)| (*op, &**f as &dyn Function))
            .collect()
    }
}
