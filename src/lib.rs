//! # handl-lib-stdlib (stdlib package table + native op atoms)
//!
//! The handl-backed standard library, consolidated per the
//! "library-as-data" architecture: every `plugin-lib-*` shell crate is
//! replaced by a row in [`packages::PACKAGES`] / [`packages::MODULES`]
//! (name → `.hnl` source + func names + op names + dep names), and all
//! native op atoms (the Rust `Function` impls) live in [`ops`].
//!
//! - [`packages`] — the table, [`packages::PackageResolver`],
//!   [`packages::module`]/[`packages::module_ref`]/[`packages::library`]
//!   builders, and the generic [`packages::PackagePlugin`].
//! - [`ops`] — the moved `*_op_registry()` builders and `Function` impls.
//! - [`arrow_iter`] — host-side `while_loop`/`for_loop`/`fix_loop`
//!   [`handl_language::Scheme`] builders (moved from
//!   `plugin-lib-arrow-iter`); the composite `*_step` ops they emit are
//!   interpreted by the v2 eval engine.
//! - [`shared`] / [`function_value`] — shared arg/callable helpers used by
//!   the op impls (moved from `plugin-lib-utils`).

pub mod arrow_iter;
pub mod function_value;
pub mod ops;
pub mod packages;
pub mod shared;

pub use packages::{
    DEFAULT_RESOLVER, MODULES, ModuleSpec, PACKAGES, PackagePlugin, PackageResolver, PackageSpec,
    build_module, library, module, module_ref, module_spec, module_with,
};
