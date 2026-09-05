//! # handl-plugin-lib-char-native
//!
//! The native `char` module: the Rust `std::char` backend for the `char`
//! contract (`handl-packages/char/char.hnl`).
//!
//! Provides 18 per-character atom functions:
//!
//! - `char.is_whitespace`, `is_alpha`, `is_numeric`, `is_alnum`,
//!   `is_uppercase`, `is_lowercase` — category atoms.
//! - `char.to_upper`, `to_lower`, `to_title` — case-mapping atoms (may
//!   expand: `to_upper("ß")` = "SS").
//! - `char.code_point`, `from_code_point` — scalar-value conversion.
//! - `char.at`, `slice`, `length`, `concat`, `code_point_at` —
//!   char-sequence access (indices are char indices, not bytes).
//! - `char.chars`, `from_chars` — O(n) bulk split/join between a string
//!   and a `List` of 1-char strings, so `str-handl` algorithms iterate in
//!   O(n) instead of O(n²) via `at`.
//!
//! The `char` *contract* (axioms + laws) lives in
//! `handl-packages/char/char.hnl`; this crate is one backend. `str-handl`
//! is written purely against `char.*`, so a different backend
//! (`char-ascii`, `char-js`, `char-icu`) can be swapped in without
//! touching a single string algorithm.

mod module;
mod ops;

pub use module::CharModule;

#[cfg(test)]
mod tests;
