//! Named native rewrites wired behind `TransformRule::Native` (ADR-0094 §3).

pub mod desugar;

pub use desugar::{desugar_native_registry, DESUGAR_NATIVES};
