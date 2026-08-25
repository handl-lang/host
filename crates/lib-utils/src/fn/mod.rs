pub(crate) mod combinators;
pub(crate) mod higher_order;
mod module;

pub use module::FnModule;

#[cfg(test)]
mod combinators_tests;
