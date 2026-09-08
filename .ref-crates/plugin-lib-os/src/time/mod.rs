mod time;

pub use time::{TIME_OPS, Time, TimeModule, register_time_ops};

pub mod time_stub;

#[cfg(test)]
mod time_tests;
