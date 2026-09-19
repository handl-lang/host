//! Thin safe wrappers over libclang.
//!
//! - [`strings`] — `CXString` → `String`, name/spelling/doc extraction.
//! - [`location`] — main-file / system-header predicates.
//! - [`visit`] — generic `clang_visitChildren` helpers (`children_of_kind`,
//!   `first_child`) that replace per-kind collector callbacks.

mod location;
mod strings;
mod visit;

pub(crate) use location::*;
pub(crate) use strings::*;
pub(crate) use visit::*;

#[cfg(test)]
mod location_test;
#[cfg(test)]
mod strings_test;
#[cfg(test)]
mod visit_test;
