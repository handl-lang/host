mod basic_operations;
mod helpers;
mod higher_order;
mod iteration;
mod module;

pub use module::ArrayModule;

#[cfg(test)]
mod array_tests;
#[cfg(test)]
mod basic_operations_tests;
#[cfg(test)]
mod higher_order_tests;
#[cfg(test)]
mod iteration_tests;
#[cfg(test)]
mod test_helpers;
