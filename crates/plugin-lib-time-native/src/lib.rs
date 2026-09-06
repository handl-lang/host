//! # handl-plugin-lib-time-native
//!
//! The native `time` module: the Rust `std::time` backend for the `time`
//! contract (`handl-packages/time/time.hnl`).
//!
//! Provides 3 clock-reading atoms:
//!
//! - `time.now_ms` — wall-clock milliseconds since the Unix epoch
//!   (`SystemTime::now()`).
//! - `time.instant_ns` — monotonic nanoseconds from a process-lifetime
//!   `Instant` base (see [`ops`] for the epoch choice). Only differences
//!   between readings are meaningful; this is the clock `benchmark` uses
//!   for elapsed-time measurement.
//! - `time.elapsed_ns` — nanoseconds elapsed since a prior `instant_ns`
//!   reading (saturating at 0).
//!
//! All three reads are effectful and capability-gated via the `cap`
//! declarations in `handl-packages/time-native/time-native.hnl`.
//!
//! The `time` *contract* (axioms + laws) lives in
//! `handl-packages/time/time.hnl`; this crate is one backend. `time.sleep_ms`
//! is deliberately absent — it is tracked in `project/BACKLOG.md`.

mod module;
mod ops;

pub use module::TimeModule;

#[cfg(test)]
mod tests;
