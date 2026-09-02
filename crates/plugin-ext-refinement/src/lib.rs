//! `handl-plugin-ext-refinement` -- annotation-free refinement inference for
//! handl's `Scheme` IR.
//!
//! This crate ports the research prototype's refinement inference engine
//! (from `research/arrow-calculus-kernel-poc/`) to handl's `Scheme` IR,
//! adapting the `RGraph`-based algorithms to operate over `Scheme`'s six
//! arrow primitives (`Arr`, `Seq`, `First`, `Choice`, `Loop`, `App`).
//!
//! # Architecture
//!
//! - [`types`] -- core refinement types: `Pred`, `RefTy`, `BaseTy`, `Contract`,
//!   `RefinementResult`, `LoopInvariant`, `Measure`.
//! - [`smt`] -- SMT-style implication checker (Z3 native + subprocess + toy fallback).
//! - [`contracts`] -- contract registry for base arrows (separate from `Scheme`,
//!   per ADR-0052).
//! - [`inference`] -- the main inference engine: `infer_refinement`, `infer_r`.
//! - [`loop_invariant`] -- fixpoint loop invariant discovery via weakest
//!   precondition computation.
//! - [`termination`] -- structural termination analysis with decreasing measure
//!   discovery.
//! - [`trait_inference`] -- annotation-free law recognition (Phase 9): detects
//!   which equational laws (functor identity/composition, coproduct functor
//!   identity/composition) a graph satisfies from its wiring structure alone,
//!   mirroring the Lean4 `TraitInference.lean` formalization.
//! - [`plugin`] -- plugin integration: `Refine` operation, `RefinementBehavior`,
//!   `BasicRefiner`, `RefinementCore`.
//!
//! # Usage
//!
//! ```
//! use handl_language::Scheme;
//! use handl_plugin_ext_refinement::contracts::ContractRegistry;
//! use handl_plugin_ext_refinement::inference::infer_refinement;
//!
//! let registry = ContractRegistry::standard();
//! let scheme = Scheme::arr_native("sort", vec![]);
//! let result = infer_refinement(&scheme, &registry).unwrap();
//! println!("Guarantee: {}", result.guarantee.pretty());
//! ```

pub mod arrow_lang;
pub mod arrow_loader;
pub mod arrow_parser;
pub mod contracts;
pub mod inference;
pub mod loader;
pub mod loop_invariant;
pub mod plugin;
pub mod smt;
pub mod termination;
pub mod trait_inference;
pub mod types;

// Re-export the most commonly used items at the crate root.
pub use arrow_loader::{
    infer_file, load_all_arrows, load_arrow_file, load_arrow_file_named, resolve_apps,
    // Module system (ADR-0069):
    CachingResolver, ChainResolver, FileResolver, LoadError, LoadedModule, MemoryResolver,
    ModuleLoader, ModuleResolver, ResolvedModule, ResolveError, ResolveErrorKind,
    contains_module_ref, contains_project, flatten_program, has_unresolved_apps,
    normalize_compile_time,
};
pub use arrow_parser::{
    parse_arrow_expr, parse_arrow_file, parse_arrow_module, ExportDecl, ImportDecl, ParsedFile,
    ParsedModule,
};
pub use contracts::ContractRegistry;
pub use inference::{format_scheme, infer_r, infer_refinement};
pub use loader::{load_dir, load_standard_axioms, parse_contracts};
pub use loop_invariant::{
    discover_invariant_fixpoint, discover_invariant_fixpoint_real, weakest_precondition,
};
pub use plugin::{BasicRefiner, Refine, RefineRequest, RefineResult, RefinementBehavior, RefinementCore};
pub use smt::{implies, implies_with_report, pred_to_smt, toy_implies, z3_implies, z3_verdict, Z3Verdict};
pub use termination::{analyze_termination, TerminationResult};
pub use trait_inference::{
    infer_law_annotations, inferred_trait, satisfies_law, uses_first, uses_left, Law, LawAnnotation,
};
pub use types::{BaseTy, Contract, LoopInvariant, Measure, Pred, RefinementResult, RefTy};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod e2e_tests;

#[cfg(test)]
mod loader_tests;

#[cfg(test)]
mod arrow_parser_tests;

#[cfg(test)]
mod parse_tests;

#[cfg(test)]
mod state_tests;

#[cfg(test)]
mod backtrack_tests;

#[cfg(test)]
mod trait_inference_tests;
