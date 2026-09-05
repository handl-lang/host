//! # handl-plugin-lib-unicode-native
//!
//! The native `unicode` module: a Rust/ICU4X backend for the `unicode`
//! contract (`handl-packages/unicode/unicode.hnl`).
//!
//! It is composed with the pure-Handl `unicode-handl` fallback: the native
//! ops take precedence, and any names not implemented natively fall through
//! to the handl-authored record (`unicode-handl/unicode-handl.hnl`) over
//! the `char` floor.

pub mod module;
pub mod ops;

#[cfg(test)]
mod tests;

pub use module::UnicodeModule;
