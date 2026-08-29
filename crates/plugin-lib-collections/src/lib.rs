//! # handl-lib-collections
//!
//! Collection types for the handl runtime.
//!
//! Provides the `collections` module with `vec`, `list`, `queue`,
//! `stack`, `deque`, `hash_map`, `hash_set`, `btree_map`, `btree_set`,
//! `priority_queue`, and `bit_set`.
//!
//! Currently implemented:
//!
//! - `vec.*` — back-ended dynamic array
//! - `list.*` — front-ended list
//! - `queue.*` — FIFO queue
//! - `stack.*` — LIFO stack
//! - `deque.*` — double-ended queue
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_vm_common::ModuleRuntime;
//! use handl_plugin_lib_collections::CollectionsModule;
//!
//! let host = ModuleRuntime::new()
//!     .with(CollectionsModule);
//! ```

mod array;
mod bit_set;
mod btree_map;
mod btree_set;
mod common;
mod deque;
mod error;
mod hash_map;
mod hash_set;
mod list;
mod module;
mod ops;
pub mod priority_queue;
pub mod queue;
pub mod stack;
pub mod vec;

pub use error::CollectionsError;
pub use module::CollectionsModule;

pub mod plugin;
pub use plugin::LibraryPlugin;
