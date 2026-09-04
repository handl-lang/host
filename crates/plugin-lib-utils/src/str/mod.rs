//! The `str` module — string utility functions.
//!
//! Provides functions callable as `str.upper(s)`, `str.split(s, sep)`,
//! etc. in handl expressions.
//!
//! The str *interface* is declared in `handl-packages/utils/str.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 45
//! functions are FFI atoms — they call Rust `String`/`str` methods that
//! cannot be expressed as pure handl arrows without losing the exact
//! `TypeMismatch` error messages the tests rely on. The Rust
//! `Function` impls in [`ops`] are registered as handls (handlers) in an
//! [`OpRegistry`], and the generic
//! [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader dispatches
//! each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use ops::{
    CamelCase, Capitalize, CharAt, Concat, Contains, Count, EndsWith, Equals, Find, IsAlpha,
    IsAlphanumeric, IsEmpty, IsNumeric, Join, KebabCase, Length, Levenshtein, Lines, Lower,
    PadLeft, PadRight, PascalCase, RFind, Repeat, Replace, ReplaceFirst, Reverse, Similarity,
    Slice, Slugify, SnakeCase, Split, SplitAt, SplitWhitespace, StartsWith, StripPrefix,
    StripSuffix, TitleCase, Truncate, Trim, TrimEnd, TrimMatches, TrimStart, Upper, WordWrap,
};

/// The handl source declaring the str `op` interface — the single
/// source of truth for the str API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// `str.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 45 str functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const STR_HNL: &str = include_str!("../../../../handl-packages/utils/str.hnl");

/// The op names declared in `str.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const STR_OPS: &[&str] = &[
    "str.upper",
    "str.lower",
    "str.trim",
    "str.trim_start",
    "str.trim_end",
    "str.trim_matches",
    "str.reverse",
    "str.length",
    "str.concat",
    "str.is_empty",
    "str.equals",
    "str.is_alpha",
    "str.is_numeric",
    "str.is_alphanumeric",
    "str.capitalize",
    "str.title_case",
    "str.snake_case",
    "str.camel_case",
    "str.pascal_case",
    "str.kebab_case",
    "str.contains",
    "str.starts_with",
    "str.ends_with",
    "str.count",
    "str.find",
    "str.rfind",
    "str.levenshtein",
    "str.similarity",
    "str.split",
    "str.join",
    "str.replace",
    "str.replace_first",
    "str.repeat",
    "str.char_at",
    "str.slice",
    "str.pad_left",
    "str.pad_right",
    "str.strip_prefix",
    "str.strip_suffix",
    "str.lines",
    "str.split_at",
    "str.split_whitespace",
    "str.truncate",
    "str.slugify",
    "str.word_wrap",
];

/// Build the [`OpRegistry`] for str, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"str.upper"`) maps to its Rust `Function` impl
/// (e.g. [`Upper`]). No runtime guarantee checkers are attached — the str
/// atoms are trusted.
fn str_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("str.upper", Arc::new(Upper));
    registry.register("str.lower", Arc::new(Lower));
    registry.register("str.trim", Arc::new(Trim));
    registry.register("str.trim_start", Arc::new(TrimStart));
    registry.register("str.trim_end", Arc::new(TrimEnd));
    registry.register("str.trim_matches", Arc::new(TrimMatches));
    registry.register("str.reverse", Arc::new(Reverse));
    registry.register("str.length", Arc::new(Length));
    registry.register("str.concat", Arc::new(Concat));
    registry.register("str.is_empty", Arc::new(IsEmpty));
    registry.register("str.equals", Arc::new(Equals));
    registry.register("str.is_alpha", Arc::new(IsAlpha));
    registry.register("str.is_numeric", Arc::new(IsNumeric));
    registry.register("str.is_alphanumeric", Arc::new(IsAlphanumeric));
    registry.register("str.capitalize", Arc::new(Capitalize));
    registry.register("str.title_case", Arc::new(TitleCase));
    registry.register("str.snake_case", Arc::new(SnakeCase));
    registry.register("str.camel_case", Arc::new(CamelCase));
    registry.register("str.pascal_case", Arc::new(PascalCase));
    registry.register("str.kebab_case", Arc::new(KebabCase));
    registry.register("str.contains", Arc::new(Contains));
    registry.register("str.starts_with", Arc::new(StartsWith));
    registry.register("str.ends_with", Arc::new(EndsWith));
    registry.register("str.count", Arc::new(Count));
    registry.register("str.find", Arc::new(Find));
    registry.register("str.rfind", Arc::new(RFind));
    registry.register("str.levenshtein", Arc::new(Levenshtein));
    registry.register("str.similarity", Arc::new(Similarity));
    registry.register("str.split", Arc::new(Split));
    registry.register("str.join", Arc::new(Join));
    registry.register("str.replace", Arc::new(Replace));
    registry.register("str.replace_first", Arc::new(ReplaceFirst));
    registry.register("str.repeat", Arc::new(Repeat));
    registry.register("str.char_at", Arc::new(CharAt));
    registry.register("str.slice", Arc::new(Slice));
    registry.register("str.pad_left", Arc::new(PadLeft));
    registry.register("str.pad_right", Arc::new(PadRight));
    registry.register("str.strip_prefix", Arc::new(StripPrefix));
    registry.register("str.strip_suffix", Arc::new(StripSuffix));
    registry.register("str.lines", Arc::new(Lines));
    registry.register("str.split_at", Arc::new(SplitAt));
    registry.register("str.split_whitespace", Arc::new(SplitWhitespace));
    registry.register("str.truncate", Arc::new(Truncate));
    registry.register("str.slugify", Arc::new(Slugify));
    registry.register("str.word_wrap", Arc::new(WordWrap));
    registry
}

/// Build the [`HandlModule`] for str.
///
/// All 45 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_str_module() -> HandlModule {
    let registry = Arc::new(str_op_registry());
    HandlModule::new("str", STR_HNL, &[]).with_ops(STR_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for str.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static STR_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for str, initializing it on first call.
fn str_module() -> &'static HandlModule {
    STR_MODULE.get_or_init(build_str_module)
}

/// The `str` module — string utility functions.
///
/// Provides `str.upper`, `str.lower`, `str.trim`, `str.split`,
/// `str.join`, `str.replace`, `str.contains`, `str.starts_with`,
/// `str.ends_with`, `str.slice`, `str.reverse`, `str.repeat`,
/// `str.find`, `str.count`, `str.char_at`, `str.pad_left`,
/// `str.pad_right`, `str.length`, `str.concat`, `str.strip_prefix`,
/// `str.strip_suffix`, `str.lines`, `str.rfind`, `str.capitalize`,
/// `str.title_case`, `str.snake_case`, `str.camel_case`,
/// `str.pascal_case`, `str.kebab_case`, `str.split_at`,
/// `str.trim_matches`, `str.is_empty`, `str.equals`, `str.is_alpha`,
/// `str.is_numeric`, `str.is_alphanumeric`, `str.levenshtein`,
/// `str.similarity`, `str.split_whitespace`, `str.truncate`,
/// `str.slugify`, `str.word_wrap`, and more. The interface is declared
/// in `str.hnl` via `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct StrModule;

impl Module for StrModule {
    fn name(&self) -> &str {
        "str"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        str_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        str_module().functions()
    }
}

#[cfg(test)]
mod basic_tests;
#[cfg(test)]
mod case_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod transform_tests;
