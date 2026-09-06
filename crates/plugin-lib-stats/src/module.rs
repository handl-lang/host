//! Module exposing `stats.*` to the runtime.
//!
//! This is a thin wrapper around the generic [`HandlModule`] from
//! [`handl_plugin_lib_handl`]. The actual stats logic is handl-authored in
//! `stats.hnl`; this module only provides the [`Module`] registration
//! boundary with the same public API as before.

use handl_platform::Module;

use crate::{Avg, Max, Median, Min, Product, Stddev, Sum};

/// The `stats` module — statistical aggregate functions.
///
/// Provides `stats.sum`, `stats.avg`, `stats.median`, `stats.stddev`,
/// `stats.product`, `stats.min`, `stats.max`.
///
/// The logic for every function is handl-authored in `stats.hnl` and loaded
/// via the generic [`HandlModule`](handl_plugin_lib_handl::HandlModule);
/// each `Function` impl only validates arguments and delegates to the loader.
#[derive(Clone, Copy)]
pub struct StatsModule;

impl Module for StatsModule {
    fn name(&self) -> &str {
        "stats"
    }

    fn function(&self, name: &str) -> Option<&dyn handl_platform::Function> {
        match name {
            "sum" => Some(&Sum),
            "avg" => Some(&Avg),
            "product" => Some(&Product),
            "min" => Some(&Min),
            "max" => Some(&Max),
            "median" => Some(&Median),
            "stddev" => Some(&Stddev),
            _ => None,
        }
    }
}
