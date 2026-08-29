//! # handl-plugin-vm-values-json
//!
//! JSON-backed value representation for the handl VM.
//!
//! `NonFunctionValue` is a transparent newtype around `serde_json::Value`,
//! and `FunctionValue` is the closure-free first-class function encoding.
//! These are the concrete value types for JSON/YAML/TOML-style documents.
//!
//! The core VM abstractions are re-exported from `handl-plugin-vm` with the concrete
//! value type fixed to `NonFunctionValue`.

mod canonical_context;
mod formatter;
mod frontend;
mod value;

#[cfg(test)]
mod canonical_context_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod value_tests;

pub use canonical_context::CanonicalContext;
pub use formatter::Formatter;
pub use frontend::Frontend;
pub use value::{FunctionValue, HostValue, NonFunctionValue};

/// Concrete error type for the JSON-backed value representation.
pub type Error = handl_plugin_vm::Error<NonFunctionValue>;

// Re-export the generic VM traits and value bound so consumers can write
// `Eval<NonFunctionValue>`, `Context<NonFunctionValue>`, etc.
pub use handl_plugin_vm::{
    AsyncEval, AsyncFunction, Context, Eval, Function, GenericFunction, CallableLookup, CallableLookupRef,
    LazyFunction, Operation, OperationSig, Value, VirtualMachine,
};
