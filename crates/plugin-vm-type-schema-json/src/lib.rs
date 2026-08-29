//! JSON Schema to `handl-plugin-vm-type` bridge.

pub mod alias_table;
pub mod error;
pub mod interpreter;

pub use alias_table::*;
pub use error::*;
pub use interpreter::*;

#[cfg(test)]
mod tests;
