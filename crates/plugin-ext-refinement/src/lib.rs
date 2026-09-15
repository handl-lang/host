//! `handl-plugin-ext-refinement` -- annotation-free refinement inference for
//! handl's `Term` IR.
//!
//! This crate ports the research prototype's refinement inference engine
//! (from `research/arrow-calculus-kernel-poc/`) to handl's `Term` IR,
//! adapting the `RGraph`-based algorithms to operate over `Term`'s six
//! arrow primitives (`Arr`, `Seq`, `First`, `Choice`, `Loop`, `App`).
//!
//! # Architecture
//!
//! - [`types`] -- core refinement types: `Pred`, `RefTy`, `BaseTy`, `Contract`,
//!   `RefinementResult`, `LoopInvariant`, `Measure`.
//! - [`smt`] -- SMT-style implication checker (Z3 native + subprocess + toy fallback).
//! - [`contracts`] -- contract registry for base arrows (separate from `Term`,
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
//! use handl_language::Term;
//! use handl_plugin_ext_refinement::contracts::ContractRegistry;
//! use handl_plugin_ext_refinement::inference::infer_refinement;
//!
//! let registry = ContractRegistry::standard();
//! let scheme = Term::arr_native("sort", vec![]);
//! let result = infer_refinement(&scheme, &registry).unwrap();
//! println!("Guarantee: {}", result.guarantee.pretty());
//! ```

pub mod arrow_lang;
pub mod arrow_loader;
pub mod arrow_parser;
pub mod checker;
pub mod contracts;
pub mod inference;
pub mod loader;
pub mod loop_invariant;
pub mod partial_eval;
pub mod plugin;
pub mod smt;
pub mod termination;
pub mod trait_inference;
pub mod types;

// Re-export the most commonly used items at the crate root.
pub use arrow_loader::{
    // Module system (ADR-0069):
    CachingResolver,
    ChainResolver,
    FileResolver,
    LoadError,
    LoadedModule,
    MemoryResolver,
    ModuleLoader,
    ModuleResolver,
    ResolveError,
    ResolveErrorKind,
    ResolvedModule,
    contains_module_ref,
    contains_project,
    flatten_program,
    has_unresolved_apps,
    infer_file,
    load_all_arrows,
    load_arrow_file,
    load_arrow_file_named,
    normalize_compile_time,
    resolve_apps,
};
pub use arrow_parser::{
    ExportDecl, ImportDecl, ParsedFile, ParsedModule, parse_arrow_expr, parse_arrow_file,
    parse_arrow_module, parse_arrow_module_no_rewrite,
};
pub use checker::{ContractChecker, register_contract_checkers};
pub use contracts::ContractRegistry;
pub use inference::{format_scheme, infer_r, infer_refinement};
pub use loader::{load_dir, load_standard_axioms, parse_contracts};
pub use loop_invariant::{
    discover_invariant_fixpoint, discover_invariant_fixpoint_real, weakest_precondition,
};
pub use partial_eval::{
    Arr, ArrayRecSpec, Directive, Pev, Proof, Verdict, check_proposition, prove_array_rec,
    prove_length_append, prove_length_map, recognize_array_rec,
};
pub use plugin::{
    BasicRefiner, Refine, RefineRequest, RefineResult, RefinementBehavior, RefinementCore,
};
pub use smt::{
    Z3Verdict, implies, implies_with_report, pred_to_smt, toy_implies, z3_implies, z3_verdict,
};
pub use termination::{TerminationResult, analyze_termination};
pub use trait_inference::{
    Law, LawAnnotation, infer_law_annotations, inferred_trait, satisfies_law, uses_first, uses_left,
};
pub use types::{BaseTy, Contract, LoopInvariant, Measure, Pred, RefTy, RefinementResult};

/// Source dialect detected from surface syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Canonical,
    Contract,
    Arrow,
}

/// A top-level (column-0) source line, with `--` line comments removed.
/// Returns `None` for blank lines, comments, and indented lines (contract
/// clause lines are indented and never carry a directive keyword).
fn top_level_line(line: &str) -> Option<&str> {
    if line.chars().next().is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let code = line.split_once("--").map(|(c, _)| c).unwrap_or(line);
    let t = code.trim_end();
    (!t.is_empty()).then_some(t)
}

/// True when the source is a contract-dialect file: the first top-level
/// statement line starts with `axiom `.
fn is_contract_source(src: &str) -> bool {
    src.lines()
        .filter_map(top_level_line)
        .next()
        .is_some_and(|l| l.starts_with("axiom "))
}

/// True when the source shows arrow-dialect markers: top-level
/// `cap`/`op`/`handl`/`tag` declaration lines or arrow-combinator
/// expressions (`arr(`, `first(`, `choice(`, `loop(`, `comp(`, `app(`,
/// `|||`, `rec {`).
fn has_arrow_markers(src: &str) -> bool {
    const DIRECTIVES: [&str; 4] = ["cap ", "op ", "handl ", "tag "];
    src.lines().any(|line| {
        let code = line.split_once("--").map(|(c, _)| c).unwrap_or(line);
        if let Some(top) = top_level_line(line) {
            if DIRECTIVES.iter().any(|d| top.starts_with(d)) {
                return true;
            }
        }
        contains_arrow_combinator(code)
    })
}

/// True when `code` mentions an arrow-calculus combinator as a *standalone*
/// expression, rather than as the tail of a longer identifier or a field
/// access. Handl surface code that calls a function like `array.first(…)` or
/// `fn.comp(…)` must not be mistaken for the arrow kernel's `first`/`comp`
/// combinators, so each combinator name is matched word-bounded (not preceded
/// by `.` or an identifier character).
fn contains_arrow_combinator(code: &str) -> bool {
    if code.contains("|||") {
        return true;
    }

    // `name(` — call-form combinators.
    for name in ["arr", "first", "choice", "loop", "comp", "app"] {
        if standalone_call(code, name) {
            return true;
        }
    }

    // `rec {` — `rec` standalone followed by optional whitespace and `{`.
    standalone_rec(code)
}

/// Whether `name` occurs in `code` as a standalone `name(` call: not preceded
/// by an identifier character or `.`, and immediately followed by `(`.
fn standalone_call(code: &str, name: &str) -> bool {
    let mut search = 0;
    while let Some(rel) = code[search..].find(name) {
        let start = search + rel;
        let end = start + name.len();
        let before_ok = code[..start]
            .chars()
            .next_back()
            .map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        let after_ok = code[end..].chars().next() == Some('(');
        if before_ok && after_ok {
            return true;
        }
        search = end;
    }
    false
}

/// Whether `rec` occurs in `code` as a standalone identifier followed by
/// (optional whitespace and) `{`.
fn standalone_rec(code: &str) -> bool {
    let mut search = 0;
    while let Some(rel) = code[search..].find("rec") {
        let start = search + rel;
        let end = start + 3;
        let before_ok = code[..start]
            .chars()
            .next_back()
            .map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        if before_ok {
            let after = code[end..].trim_start().chars().next();
            if after == Some('{') {
                return true;
            }
        }
        search = end;
    }
    false
}

/// Detect the surface dialect of `src`.
///
/// Contract files (`axiom`/`requires:`/`guarantee:`) and arrow-module files
/// (`cap`/`op`/`handl`/`tag` decls, arrow combinators) are routed to the
/// refinement loaders. Arrow detection requires the source to successfully
/// parse as an arrow module, not just contain marker tokens.
pub fn detect_dialect(src: &str) -> Dialect {
    if is_contract_source(src) {
        return Dialect::Contract;
    }
    if has_arrow_markers(src) && parse_arrow_module(src).is_ok() {
        return Dialect::Arrow;
    }
    Dialect::Canonical
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod checker_tests;

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
