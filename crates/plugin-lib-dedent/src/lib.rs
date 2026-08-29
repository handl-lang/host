//! # handl-lib-dedent
//!
//! Provides a `dedent` module for the handl runtime.
//!
//! Similar to the TypeScript `dedent` package, [`DedentModule`] exposes
//! `dedent.dedent(text)` which strips the common leading whitespace from
//! every line in the input string. This is especially useful with template
//! string literals for producing clean multi-line output.
//!
//! ## Example (handl)
//!
//! ```text
//! dedent.dedent(`    Hello,
//!       World!`)
//! // => "Hello,\n  World!"
//! ```

pub mod dedent;
pub mod module;

pub use module::{Dedent, DedentModule};

#[cfg(test)]
mod dedent_tests;

#[cfg(test)]
mod module_tests;
