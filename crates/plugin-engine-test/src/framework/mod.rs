//! # Test framework library (ADR-0092 temporary Rust handls)
//!
//! The handl-callable test framework surface:
//!
//! - [`TestModule`] — `test.run`
//! - [`AssertModule`] — `assert.equal`, `assert.not_equal`, `assert.true`,
//!   `assert.false`, `assert.contains`, `assert.throws`
//! - [`PropertyModule`] — `property.for_all`
//! - [`BenchmarkModule`] — `benchmark.run`
//! - [`FuzzModule`] — `fuzz.run`
//! - [`SnapshotModule`] — `snapshot.match`, `snapshot.format`
//!
//! The framework contract is declared in handl:
//! `handl-packages/test/test.hnl` declares the `op` interface
//! (`assert.equal : (Value, Value) -> Bool`, …) and binds each op to a
//! Rust handl (`handl "assert.equal" = rust_assert_equal`). The Rust
//! modules below implement those handls and register them in an
//! [`OpRegistry`] (ADR-0077 `op`/`handl` Phase 1 form).
//!
//! Per ADR-0092 these Rust handls are **temporary bridges hosted in the
//! runner crate** — `plugin-lib-test` is deleted, and the pure-handl
//! implementations under `handl-packages/test/` replace them as the
//! required stdlib primitives land:
//!
//! - `assert.*` / `snapshot.match` / `test.run` need an **error-raising**
//!   primitive (`assert` must fail loudly) and, for `assert.throws` and
//!   `test.run`, **error capture** (call a function value and observe
//!   failure — today only the host-side `EvalContext::try_call_value`
//!   provides that).
//! - `property.for_all` / `fuzz.run` additionally need a seeded RNG
//!   reachable from handl (`rand.seeded_*` exists in `plugin-lib-rand` but
//!   is not yet wired into the canonical module chain).
//! - `benchmark.run` needs a clock primitive (`time.now`).
//!
//! All modules are stateless, `Copy`, and follow the `Function`
//! contract used by the rest of the standard library.

use std::cell::RefCell;
use std::sync::Arc;

use handl_plugin_lib_handl::{parse_cap_declarations, HandlModule, OpRegistry};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

use crate::runner::FailureDetail;

mod assert;
mod benchmark;
mod error;
mod function_value;
mod fuzz;
mod property;
mod shared;
mod snapshot;
mod test;

pub use assert::AssertModule;
pub use benchmark::BenchmarkModule;
pub use error::TestError;
pub use fuzz::FuzzModule;
pub use property::PropertyModule;
pub use snapshot::SnapshotModule;
pub use test::TestModule;

thread_local! {
    /// Last assert failure detail for the current worker thread.
    ///
    /// The assert functions set this as a side-channel so the runner can
    /// populate [`FailureDetail`] without parsing the human-readable error
    /// message. It is cleared before each test body and taken when the test
    /// fails.
    static LAST_ASSERT_FAILURE: RefCell<Option<FailureDetail>> = RefCell::new(None);
}

/// Set the last assert failure detail for the current worker thread.
pub(crate) fn set_last_assert_failure(detail: FailureDetail) {
    LAST_ASSERT_FAILURE.with(|f| *f.borrow_mut() = Some(detail));
}

/// Clear any stale assert failure detail before evaluating a test body.
pub(crate) fn clear_last_assert_failure() {
    LAST_ASSERT_FAILURE.with(|f| f.borrow_mut().take());
}

/// Take the last assert failure detail, if any, for the current worker thread.
pub(crate) fn take_last_assert_failure() -> Option<FailureDetail> {
    LAST_ASSERT_FAILURE.with(|f| f.borrow_mut().take())
}

#[cfg(test)]
pub mod test_helpers;
#[cfg(test)]
mod tests;

/// The handl source declaring the test `op` interface — the single source
/// of truth for the test API surface. Loaded at compile time from the
/// `handl-packages/test/` package dir.
const TEST_HNL: &str = include_str!("../../../../handl-packages/test/test.hnl");

/// The op names declared in `test.hnl`, grouped by module.
const TEST_OPS: &[&str] = &["test.run"];
const ASSERT_OPS: &[&str] = &[
    "assert.equal",
    "assert.not_equal",
    "assert.true",
    "assert.false",
    "assert.contains",
    "assert.throws",
];
const PROPERTY_OPS: &[&str] = &["property.for_all"];
const BENCHMARK_OPS: &[&str] = &["benchmark.run"];
const FUZZ_OPS: &[&str] = &["fuzz.run"];
const SNAPSHOT_OPS: &[&str] = &["snapshot.format", "snapshot.match"];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for test, registering the Rust handls under
/// their op names.
pub fn test_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("test.run", Arc::new(test::Run));
    registry.register("assert.equal", Arc::new(assert::Equal));
    registry.register("assert.not_equal", Arc::new(assert::NotEqual));
    registry.register("assert.true", Arc::new(assert::True));
    registry.register("assert.false", Arc::new(assert::False));
    registry.register("assert.contains", Arc::new(assert::Contains));
    registry.register("assert.throws", Arc::new(assert::Throws));
    registry.register("property.for_all", Arc::new(property::ForAll));
    registry.register("benchmark.run", Arc::new(benchmark::Run));
    registry.register("fuzz.run", Arc::new(fuzz::Run));
    registry.register("snapshot.format", Arc::new(snapshot::Format));
    registry.register("snapshot.match", Arc::new(snapshot::Match));
    registry
}

// ---------------------------------------------------------------------------
// TestLibraryPlugin (contributes 6 HandlModule instances with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the test implementation via 6
/// [`HandlModule`] instances (one per module namespace: `test`, `assert`,
/// `property`, `benchmark`, `fuzz`, `snapshot`), all sharing a single
/// [`OpRegistry`].
///
/// The test interface is declared in `test.hnl` via `op`/`handl`
/// declarations; the Rust test handls are registered in the op registry.
/// This is the ADR-0092 stopgap form; it is deleted once the pure-handl
/// implementations land.
pub struct TestLibraryPlugin {
    capabilities: Vec<handl_platform::CapabilityId>,
}

impl TestLibraryPlugin {
    /// Create a new `TestLibraryPlugin`.
    pub fn new() -> Self {
        let caps = parse_cap_declarations(TEST_HNL)
            .into_iter()
            .map(handl_platform::CapabilityId::from)
            .collect();
        Self { capabilities: caps }
    }

    /// Build the 6 [`HandlModule`] instances sharing a single op registry.
    fn build_modules(&self) -> Vec<HandlModule> {
        let registry = Arc::new(test_op_registry());
        let source = TEST_HNL;
        vec![
            HandlModule::new("test", source, &[]).with_ops(TEST_OPS, Arc::clone(&registry)),
            HandlModule::new("assert", source, &[]).with_ops(ASSERT_OPS, Arc::clone(&registry)),
            HandlModule::new("property", source, &[]).with_ops(PROPERTY_OPS, Arc::clone(&registry)),
            HandlModule::new("benchmark", source, &[])
                .with_ops(BENCHMARK_OPS, Arc::clone(&registry)),
            HandlModule::new("fuzz", source, &[]).with_ops(FUZZ_OPS, Arc::clone(&registry)),
            HandlModule::new("snapshot", source, &[]).with_ops(SNAPSHOT_OPS, Arc::clone(&registry)),
        ]
    }
}

impl Default for TestLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for TestLibraryPlugin {
    fn id(&self) -> &str {
        "lib.test"
    }

    fn name(&self) -> &str {
        "lib-test"
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        &self.capabilities
    }

    fn metadata(
        &self,
        _capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        None
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        for module in self.build_modules() {
            ctx.contribute::<ModuleRuntime>(ModuleRule::new(module))?;
        }
        Ok(())
    }

    fn run(
        &self,
        _request: &handl_platform::CapabilityRequest,
        _ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        Err(handl_platform::PlatformError::NotImplemented)
    }
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`TestLibraryPlugin`].
pub use TestLibraryPlugin as LibraryPlugin;
