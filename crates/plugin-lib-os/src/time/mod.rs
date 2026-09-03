mod time;

pub use time::{register_time_ops, Time, TimeModule, TIME_OPS};

pub mod time_stub;

#[cfg(test)]
mod time_tests;
