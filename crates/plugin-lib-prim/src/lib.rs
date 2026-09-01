//! # handl-lib-prim
//!
//! Minimal FFI layer for handl's arrow-based stdlib.
//!
//! Provides the `prim` module — the three irreducible atom functions that
//! cannot be expressed as handl arrow graphs:
//!
//! - `prim.add(a: Int, b: Int) -> Int` — integer addition
//! - `prim.compare(a: Int, b: Int) -> Int` — returns -1, 0, or 1
//! - `prim.alloc(n: Int) -> List` — creates an empty array of size n
//!
//! Everything else in the stdlib is built atop these via handl arrow graphs.

pub mod module;
pub mod ops;
pub mod plugin;

pub use module::PrimModule;
pub use plugin::LibraryPlugin;

#[cfg(test)]
mod tests;
