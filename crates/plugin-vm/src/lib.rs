//! # handl-plugin-vm
//!
//! Core virtual-machine abstractions for the handl language.
//!
//! This crate defines the evaluation contract: `VirtualMachine`,
//! `Context`, `Host`, `Function`, `Operation`, `Error`, etc.
//! It does not contain a concrete implementation; it is generic over
//! the value type `V` via the [`Value`] trait.
//!
//! The canonical tree-walking VM lives in `handl-plugin-vm-backend-rust-interpreter`.
//! Reusable helpers, mocks, and host combinators live in `handl-plugin-vm-common`.

pub mod error;
pub mod eval;
pub mod expr;
pub mod function_repr;
pub mod host;
pub mod mock;
pub mod operation;
pub mod runtime;
pub mod value;

pub use error::Error;
pub use eval::{AsyncEval, Eval};
pub use expr::{Expr, ExprFactory, HandlerClause, Pattern};
pub use function_repr::FunctionRepr;
pub use host::{AsyncFunction, Function, GenericFunction, Host, HostRef, LazyFunction};
pub use mock::{MockHost, NoopHost};
pub use operation::{Operation, OperationSig};
pub use runtime::{Context, VirtualMachine};
pub use value::Value;
