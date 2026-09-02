//! Module exposing `stats.*` to the runtime.

use handl_platform::Module;

use crate::{Avg, Max, Median, Min, Product, Stddev, Sum};

/// The `stats` module — statistical aggregate functions.
///
/// Provides `stats.sum`, `stats.avg`, `stats.median`, `stats.stddev`,
/// `stats.product`, `stats.min`, `stats.max`.
///
/// The logic for every function is handl-authored in `stats.hnl`; each
/// `Function` impl only validates arguments and delegates to the shared
/// bridge ([`crate::stats_via_handl`]).
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
