//! Runtime axiom checker bridge (ADR-0077 Phase 3).
//!
//! Bridges the refinement system's inferred [`Contract`]s to the
//! [`RuntimePredicateChecker`] trait, enabling runtime enforcement of
//! `requires`/`guarantee` axioms via [`OpRegistry::register_with_checker`].
//!
//! ## Overview
//!
//! The refinement inference engine infers [`Contract`]s (precondition +
//! guarantee predicates) for base arrows and stores them in a
//! [`ContractRegistry`]. Until Phase 3, these contracts were used only at
//! typecheck/inference time — they were never enforced at runtime.
//!
//! This module closes that gap:
//! - [`ContractChecker`] wraps a [`Contract`] as a
//!   [`RuntimePredicateChecker`], evaluating the `precondition`/`guarantee`
//!   predicates against actual runtime values via [`Pred::eval`].
//! - [`register_contract_checkers`] takes a [`ContractRegistry`] and an
//!   [`OpRegistry`] and registers a [`ContractChecker`] for every contract
//!   whose op already has a registered `Function` impl (the handl).
//!
//! Once registered, [`OpFunction::call`] enforces the axioms at the dispatch
//! boundary: `requires` is checked before the call, `guarantee` is checked
//! against the return value after the call.

use std::sync::Arc;

use handl_language::ArrSource;
use handl_plugin_lib_handl::{OpRegistry, RuntimePredicateChecker};
use serde_json::Value;

use crate::contracts::ContractRegistry;
use crate::types::{Contract, Pred};

// ---------------------------------------------------------------------------
// ContractChecker
// ---------------------------------------------------------------------------

/// A [`RuntimePredicateChecker`] backed by a refinement [`Contract`].
///
/// Wraps a `Contract`'s `precondition` (checked as `requires`) and
/// `guarantee` (checked as `guarantee`) and evaluates them against
/// runtime values via [`Pred::eval`].
///
/// # Single-arg vs multi-arg
///
/// For single-argument ops, the precondition is evaluated against `args[0]`
/// directly. For multi-argument ops, the args are encoded as a JSON array
/// and the predicate's variable names are looked up as indices (0, 1, …)
/// via [`Pred::eval`]'s array-lookup convention.
#[derive(Debug, Clone)]
pub struct ContractChecker {
    /// The op name (for error messages).
    op_name: String,
    /// The contract to enforce.
    contract: Contract,
}

impl ContractChecker {
    /// Create a new `ContractChecker` for the given op name and contract.
    pub fn new(op_name: impl Into<String>, contract: Contract) -> Self {
        Self {
            op_name: op_name.into(),
            contract,
        }
    }
}

impl RuntimePredicateChecker for ContractChecker {
    fn check_requires(&self, args: &[Value]) -> Result<(), String> {
        // The precondition applies to the input. For single-arg ops, args[0]
        // is the input. For multi-arg ops, the args array is the input tuple
        // (encoded as a JSON array so Pred::eval can look up named indices).
        if self.contract.precondition == Pred::True {
            // Fast path: unconditional precondition always holds.
            return Ok(());
        }
        let input: Value;
        let input_ref: &Value = if args.len() == 1 {
            &args[0]
        } else {
            input = Value::Array(args.to_vec());
            &input
        };
        if self.contract.precondition.eval(input_ref) {
            Ok(())
        } else {
            Err(format!(
                "op '{}': requires predicate '{}' violated by input {:?}",
                self.op_name, self.contract.precondition, input_ref
            ))
        }
    }

    fn check_guarantee(&self, output: &Value) -> Result<(), String> {
        if self.contract.guarantee == Pred::True {
            // Fast path: unconditional guarantee always holds.
            return Ok(());
        }
        if self.contract.guarantee.eval(output) {
            Ok(())
        } else {
            Err(format!(
                "op '{}': guarantee predicate '{}' violated by output {:?}",
                self.op_name, self.contract.guarantee, output
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// register_contract_checkers
// ---------------------------------------------------------------------------

/// Reconstruct an [`ArrSource`] from its string key representation.
///
/// This mirrors `source_key` in [`crate::contracts`], reversing the
/// encoding so we can filter contracts by source kind.
fn parse_source_key(key: &str) -> ArrSource {
    match key {
        "Native" => ArrSource::Native,
        "Judgment" => ArrSource::Judgment,
        "DynamicForeign" => ArrSource::DynamicForeign,
        s if s.starts_with("Foreign(") && s.ends_with(')') => {
            let name = &s["Foreign(".len()..s.len() - 1];
            ArrSource::Foreign(name.to_string())
        }
        _ => ArrSource::Native,
    }
}

/// Register all contracts from a [`ContractRegistry`] as runtime axiom
/// checkers in an [`OpRegistry`] (ADR-0077 Phase 3).
///
/// For each `(op, source)` entry in the contract registry where `source` is
/// `ArrSource::Native` or `ArrSource::Foreign`, this registers a
/// [`ContractChecker`] via [`OpRegistry::register_with_checker`].
///
/// **Important:** This only attaches the *checker*. The actual `Function`
/// impl (the handl) must already be registered under the same op name (via
/// [`OpRegistry::register`]) — this function does NOT register the function
/// body, only the checker. If no function is registered for an op, that op
/// is skipped (a checker without a function body is useless, since
/// [`OpFunction::call`](handl_plugin_lib_handl::OpFunction) looks up the
/// function first).
///
/// To attach a checker to an already-registered op without replacing the
/// function, this function looks up the existing function, then
/// re-registers with `register_with_checker` (which replaces both the
/// function and the checker with the same function + new checker).
///
/// # Returns
///
/// The number of checkers successfully attached.
pub fn register_contract_checkers(
    registry: &mut OpRegistry,
    contracts: &ContractRegistry,
) -> usize {
    let mut count = 0;
    for (op_name, source_key, contract) in contracts.iter() {
        // Only register checkers for Native and Foreign sources (the sources
        // that have runtime Function impls). Judgment and DynamicForeign
        // sources are typecheck-time only.
        let source = parse_source_key(source_key);
        if !matches!(source, ArrSource::Native | ArrSource::Foreign(_)) {
            continue;
        }
        // Look up the existing function. If none is registered, skip —
        // a checker without a function body is useless.
        if let Some(func) = registry.get(op_name) {
            let checker: Arc<dyn RuntimePredicateChecker> =
                Arc::new(ContractChecker::new(op_name.as_str(), contract.clone()));
            registry.register_with_checker(op_name, func, Some(checker));
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_source_key_native() {
        assert!(matches!(parse_source_key("Native"), ArrSource::Native));
    }

    #[test]
    fn parse_source_key_judgment() {
        assert!(matches!(parse_source_key("Judgment"), ArrSource::Judgment));
    }

    #[test]
    fn parse_source_key_dynamic_foreign() {
        assert!(matches!(
            parse_source_key("DynamicForeign"),
            ArrSource::DynamicForeign
        ));
    }

    #[test]
    fn parse_source_key_foreign() {
        match parse_source_key("Foreign(fs.read)") {
            ArrSource::Foreign(name) => assert_eq!(name, "fs.read"),
            other => panic!("expected Foreign, got {other:?}"),
        }
    }

    #[test]
    fn parse_source_key_unknown_defaults_to_native() {
        assert!(matches!(parse_source_key("???"), ArrSource::Native));
    }
}
