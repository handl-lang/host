//! Named native rewrites wired behind `TransformRule::Native` (ADR-0094 §3).

pub mod desugar;

pub use desugar::{DESUGAR_NATIVES, desugar_native_registry};
