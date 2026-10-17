//! Native op implementations (the Rust "handls") backing the `.hnl`
//! library packages in [`crate::packages`].
//!
//! Each submodule holds the `Function` impls plus an `*_op_registry()`
//! builder registering them under their declared op names (ADR-0077).
//! The impls were moved verbatim from the deleted `plugin-lib-*` shell
//! crates — only the `Module`/`Plugin` boilerplate was dropped.

pub mod base64;
pub mod collections;
pub mod hex;
pub mod ip;
pub mod json;
pub mod math;
pub mod parse;
pub mod serialize;
pub mod strs;
pub mod toml;
pub mod url_rfc;
pub mod url_whatwg;
pub mod yaml;

/// Back-compat alias so consumers can refer to
/// `handl_plugin_lib_stdlib::ops::r#type::{ToString, ToNumber, Coalesce}`.
/// The ops themselves moved to the `lib-type` crate.
pub use handl_lib_type::ops as r#type;
