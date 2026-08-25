#![doc = "Pure, effect-free type-level virtual machine based on Normalization by Evaluation."]

pub mod checking;
pub mod core;
pub mod error;
pub mod reduction;
pub mod types;
pub mod unification;

pub use checking::*;
pub use core::*;
pub use error::*;
pub use reduction::*;
pub use types::*;
pub use unification::*;

#[cfg(test)]
mod tests;
