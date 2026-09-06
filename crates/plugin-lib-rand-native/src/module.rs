use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::HandlModule;

/// The handl source declaring the rand `op` interface — the single source
/// of truth for the rand API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const RAND_HNL: &str = include_str!("../../../handl-packages/rand/rand.hnl");

/// The op names declared in `rand.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
const RAND_OPS: &[&str] = &[
    "rand.int",
    "rand.float",
    "rand.bool",
    "rand.pick",
    "rand.choice",
    "rand.sample",
    "rand.shuffle",
    "rand.weighted_pick",
    "rand.string",
    "rand.bytes",
    "rand.uuid",
    "rand.gauss",
    "rand.exponential",
    "rand.poisson",
    "rand.binomial",
    "rand.perlin_1d",
    "rand.perlin_2d",
    "rand.perlin_3d",
    "rand.simplex_2d",
    "rand.simplex_3d",
    "rand.seeded_int",
    "rand.seeded_float",
    "rand.seeded_bool",
    "rand.seeded_pick",
    "rand.seeded_choice",
    "rand.seeded_sample",
    "rand.seeded_shuffle",
    "rand.seeded_string",
    "rand.seeded_gauss",
];

/// Build the [`HandlModule`] for rand.
///
/// All functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_rand_module() -> HandlModule {
    let registry = Arc::new(super::rand_op_registry());
    HandlModule::new("rand", RAND_HNL, &[]).with_ops(RAND_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for rand.
static RAND_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for rand, initializing it on first call.
fn rand_module() -> &'static HandlModule {
    RAND_MODULE.get_or_init(build_rand_module)
}

/// The `rand` module — random number generation and sampling.
///
/// Provides functions callable as `rand.int(max)`, `rand.int(min, max)`,
/// `rand.float()`, `rand.float(max)`, `rand.float(min, max)`,
/// `rand.bool()`, `rand.pick(array)`, `rand.choice(array, count)`, `rand.sample(array, count)`,
/// `rand.shuffle(array)`, `rand.weighted_pick(items, weights)`,
/// `rand.string(length, charset?)`, `rand.bytes(length)`,
/// `rand.uuid()`, `rand.gauss(mean, stddev)`, `rand.exponential(lambda)`,
/// `rand.poisson(lambda)`, `rand.binomial(n, p)`,
/// `rand.perlin_1d(x)`, `rand.perlin_2d(x, y)`, `rand.perlin_3d(x, y, z)`,
/// `rand.simplex_2d(x, y)`, `rand.simplex_3d(x, y, z)`,
/// `rand.seeded_int(seed, min, max)`, `rand.seeded_float(seed, min, max)`,
/// `rand.seeded_bool(seed)`, `rand.seeded_pick(seed, array)`,
/// `rand.seeded_choice(seed, array, count)`, `rand.seeded_sample(seed, array, count)`,
/// `rand.seeded_shuffle(seed, array)`, `rand.seeded_string(seed, length, charset?)`,
/// `rand.seeded_gauss(seed, mean, stddev)` in handl expressions.
///
/// All non-seeded functions use the system RNG via the `rand` crate.
/// Each call is independent — there is no shared state between calls.
/// Seeded functions are deterministic — same seed + same args always
/// produce the same result.
/// Perlin noise functions are deterministic (fixed permutation table).
///
/// The interface is declared in `rand.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct only delegates
/// [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct RandModule;

impl Module for RandModule {
    fn name(&self) -> &str {
        "rand"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        rand_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        rand_module().functions()
    }
}
