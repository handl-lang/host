//! Test providers (ADR-0089 §"Providers (extensible from day one)").
//!
//! A [`TestProvider`] turns a source of tests into a [`TestPlan`]. The
//! runner never cares where tests came from; providers are the seam that
//! lets non-filesystem sources (in-memory buffers, IDE documents, naming
//! conventions) plug in later without modifying the runner.
//!
//! Today there is a single built-in provider, [`FileSystemProvider`], which
//! discovers `.hnl`/`.handl` files under the given paths and parses them
//! with the canonical kit ⊕ test-language fragment. The future
//! `UnitProvider`/`PropertyProvider`/`DoctestProvider`/… split in the ADR
//! is deferred: discovery currently recognizes all declaration kinds in one
//! pass via data-driven `name`/`body` roles (ADR-0081), so splitting by
//! kind is a composition question, not a discovery-mechanism question.

use std::path::PathBuf;

use crate::events::TestEvent;
use crate::runner::{self, DiscoverError, TestPlan};

/// A source of tests (ADR-0089 provider API v1).
///
/// Implementations discover tests and return them as a [`TestPlan`];
/// per-module parse failures are recorded in
/// [`TestPlan::parse_errors`]. Providers may emit
/// [`TestEvent::TestDiscovered`] into `events` when a sink is supplied.
pub trait TestProvider {
    /// A short provider name, for diagnostics and `--list` attribution.
    fn name(&self) -> &'static str;

    /// Discover tests under `paths`.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoverError`] if a path does not exist or a file cannot
    /// be read. Per-module parse errors are recorded in
    /// [`TestPlan::parse_errors`] rather than propagated.
    fn discover(
        &self,
        paths: &[PathBuf],
        events: Option<&mut Vec<TestEvent>>,
    ) -> Result<TestPlan, DiscoverError>;
}

/// The default provider: discovers tests in `.hnl`/`.handl` files on the
/// filesystem.
///
/// Covers the ADR's `UnitProvider`/`PropertyProvider`/
/// `IntegrationProvider`/`BenchmarkProvider` inputs in a single pass —
/// every test-kind declares `name`/`body` roles in the test-language
/// fragment, and the `test_*` naming convention remains as a
/// backwards-compat fallback (ADR-0089).
#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemProvider;

impl TestProvider for FileSystemProvider {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn discover(
        &self,
        paths: &[PathBuf],
        events: Option<&mut Vec<TestEvent>>,
    ) -> Result<TestPlan, DiscoverError> {
        runner::discover_inner(paths, events)
    }
}

/// Discover tests via an explicit [`TestProvider`].
///
/// This is the provider-aware generalization of
/// [`crate::runner::discover`], which uses [`FileSystemProvider`] and
/// discards events. Pass `events` to collect
/// [`TestEvent::TestDiscovered`] events.
///
/// # Errors
///
/// Returns [`DiscoverError`] if a path does not exist or a file cannot be
/// read.
pub fn discover_with(
    provider: &dyn TestProvider,
    paths: &[PathBuf],
    events: Option<&mut Vec<TestEvent>>,
) -> Result<TestPlan, DiscoverError> {
    provider.discover(paths, events)
}
