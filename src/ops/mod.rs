//! Native op implementations (the Rust "handls") backing the `.hnl`
//! library packages in [`crate::packages`].
//!
//! Each submodule holds the `Function` impls plus an `*_op_registry()`
//! builder registering them under their declared op names (ADR-0077).
//! The impls were moved verbatim from the deleted `plugin-lib-*` shell
//! crates — only the `Module`/`Plugin` boilerplate was dropped.

pub mod collections;
/// `ip` lives in its own repo (`lib-ip`); re-export it under the former
/// `ops::ip` path so consumers (`kit-canonical`) keep resolving
/// `ops::ip::IpModule` / `ops::ip::LibraryPlugin`.
pub use handl_lib_ip as ip;
pub mod math;
pub mod strs;
pub mod toml;
pub mod yaml;

/// Back-compat alias so consumers can refer to
/// `handl_plugin_lib_stdlib::ops::r#type::{ToString, ToNumber, Coalesce}`.
/// The ops themselves moved to the `lib-type` crate.
pub use handl_lib_type::ops as r#type;
