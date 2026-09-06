//! # handl-plugin-lib-rand-native
//!
//! Random number generation and sampling library for the handl runtime.
//!
//! Provides functions for generating random integers, floats, booleans,
//! picking and shuffling elements, generating random strings and byte
//! arrays, and producing version-4 UUIDs.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The rand interface is declared in
//! `handl-packages/rand/rand.hnl` via `op` declarations, and the Rust
//! random generation logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`RandLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry.
//!
//! The [`RandModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

mod common;
mod distribution;
mod error;
mod generation;
mod module;
mod noise;
mod numbers;
mod sampling;
mod seeded;

pub use error::RandError;
pub use module::RandModule;

#[cfg(test)]
mod distribution_tests;
#[cfg(test)]
mod generation_tests;
#[cfg(test)]
mod noise_tests;
#[cfg(test)]
mod numbers_tests;
#[cfg(test)]
mod sampling_tests;
#[cfg(test)]
mod seeded_tests;
#[cfg(test)]
mod test_helpers;

/// The handl source declaring the rand `op` interface — the single source
/// of truth for the rand API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const RAND_HNL: &str = include_str!("../../../handl-packages/rand/rand.hnl");

/// The op names declared in `rand.hnl`.
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

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for rand, registering the Rust handls under
/// their op names.
pub fn rand_op_registry() -> OpRegistry {
    use distribution::{Binomial, Exponential, Gauss, Poisson};
    use generation::{Bytes, String_, Uuid};
    use noise::{Perlin1D, Perlin2D, Perlin3D, Simplex2D, Simplex3D};
    use numbers::{Bool, Float, Int};
    use sampling::{Choice, Pick, Sample, Shuffle, WeightedPick};
    use seeded::{
        SeededBool, SeededChoice, SeededFloat, SeededGauss, SeededInt, SeededPick, SeededSample,
        SeededShuffle, SeededString,
    };

    let mut registry = OpRegistry::new();
    registry.register("rand.int", Arc::new(Int));
    registry.register("rand.float", Arc::new(Float));
    registry.register("rand.bool", Arc::new(Bool));
    registry.register("rand.pick", Arc::new(Pick));
    registry.register("rand.choice", Arc::new(Choice));
    registry.register("rand.sample", Arc::new(Sample));
    registry.register("rand.shuffle", Arc::new(Shuffle));
    registry.register("rand.weighted_pick", Arc::new(WeightedPick));
    registry.register("rand.string", Arc::new(String_));
    registry.register("rand.bytes", Arc::new(Bytes));
    registry.register("rand.uuid", Arc::new(Uuid));
    registry.register("rand.gauss", Arc::new(Gauss));
    registry.register("rand.exponential", Arc::new(Exponential));
    registry.register("rand.poisson", Arc::new(Poisson));
    registry.register("rand.binomial", Arc::new(Binomial));
    registry.register("rand.perlin_1d", Arc::new(Perlin1D));
    registry.register("rand.perlin_2d", Arc::new(Perlin2D));
    registry.register("rand.perlin_3d", Arc::new(Perlin3D));
    registry.register("rand.simplex_2d", Arc::new(Simplex2D));
    registry.register("rand.simplex_3d", Arc::new(Simplex3D));
    registry.register("rand.seeded_int", Arc::new(SeededInt));
    registry.register("rand.seeded_float", Arc::new(SeededFloat));
    registry.register("rand.seeded_bool", Arc::new(SeededBool));
    registry.register("rand.seeded_pick", Arc::new(SeededPick));
    registry.register("rand.seeded_choice", Arc::new(SeededChoice));
    registry.register("rand.seeded_sample", Arc::new(SeededSample));
    registry.register("rand.seeded_shuffle", Arc::new(SeededShuffle));
    registry.register("rand.seeded_string", Arc::new(SeededString));
    registry.register("rand.seeded_gauss", Arc::new(SeededGauss));
    registry
}

// ---------------------------------------------------------------------------
// RandLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `rand` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct RandLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl RandLibraryPlugin {
    /// Create a new `RandLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(rand_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("rand", "lib.rand", "lib-rand", RAND_HNL, &[])
                .with_ops(RAND_OPS, registry),
        }
    }
}

impl Default for RandLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for RandLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for rand with the op registry.
pub fn rand_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(rand_op_registry());
    HandlLibraryPlugin::new_with_caps("rand", "lib.rand", "lib-rand", RAND_HNL, &[])
        .with_ops(RAND_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`RandLibraryPlugin`].
pub use RandLibraryPlugin as LibraryPlugin;
