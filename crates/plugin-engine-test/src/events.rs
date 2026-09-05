//! Structured run events (ADR-0089 §"Observability: structured events from
//! day one").
//!
//! The runner emits a [`TestEvent`] for every stage transition a consumer
//! might care about: run lifecycle (`RunStarted`/`RunFinished`), suite
//! lifecycle (`SuiteStarted`/`SuiteFinished`), discovery (`TestDiscovered`), selection
//! (`TestSelected`), execution (`TestStarted`/`TestFinished`), and outcome
//! refinements (`TestCached`, `TestSkipped`, `TestFailed`).
//!
//! # Collection, not a bus
//!
//! A generic multi-subscriber event bus is explicitly **deferred** until a
//! second consumer appears (ADR-0089 §"Generic infrastructure dependencies"
//! lists "event bus" as a candidate for extraction). Today events are
//! collected into a plain `Vec<TestEvent>` supplied by the caller — see
//! [`crate::config::RunConfig::events`] and
//! [`crate::provider::TestProvider::discover`]. The vocabulary and the
//! collect-then-consume shape are written so that extraction is a move, not
//! a redesign.
//!
//! # Determinism
//!
//! Events are emitted in canonical plan order by the run coordinator, never
//! from scheduler worker threads, so parallel execution changes timing but
//! never event ordering (ADR-0089 §"Determinism").

use std::path::PathBuf;
use std::time::Duration;

use crate::runner::TestResult;

/// A structured event in the test-runner pipeline (ADR-0089).
///
/// The variants cover the full ADR event vocabulary. `SuiteStarted` and
/// `SuiteFinished` are emitted once per module, bracketing that module's
/// test events.
#[derive(Debug, Clone, PartialEq)]
pub enum TestEvent {
    /// A run began.
    RunStarted {
        /// Number of tests in the plan.
        total: usize,
    },
    /// A run completed.
    RunFinished {
        /// Number of passed tests.
        passed: usize,
        /// Number of failed tests.
        failed: usize,
        /// Number of skipped tests.
        skipped: usize,
        /// Wall-clock duration of the whole run.
        duration: Duration,
    },
    /// A suite of tests began.
    SuiteStarted {
        /// Suite name. Currently the module path that contains the suite.
        name: String,
    },
    /// A suite of tests completed.
    SuiteFinished {
        /// Suite name. Currently the module path that contains the suite.
        name: String,
    },
    /// A test was discovered by a provider.
    TestDiscovered {
        /// Module file path the test was discovered in.
        module: PathBuf,
        /// Test name.
        name: String,
    },
    /// A test passed the selection predicate and will run (or hit the
    /// result cache).
    TestSelected {
        /// Module file path.
        module: PathBuf,
        /// Test name.
        name: String,
    },
    /// A selected test was dispatched to the scheduler.
    TestStarted {
        /// Module file path.
        module: PathBuf,
        /// Test name.
        name: String,
    },
    /// A test finished executing.
    TestFinished {
        /// Module file path.
        module: PathBuf,
        /// Test name.
        name: String,
        /// The result of the test.
        result: TestResult,
        /// Measured execution duration inside the job (parallel-safe:
        /// measured by the worker around the body evaluation).
        duration: Duration,
    },
    /// A selected test's result was replayed from the result cache
    /// (`--cache`), without re-evaluation.
    TestCached {
        /// Module file path.
        module: PathBuf,
        /// Test name.
        name: String,
    },
    /// A test was skipped because the selection predicate rejected it.
    TestSkipped {
        /// Module file path.
        module: PathBuf,
        /// Test name.
        name: String,
    },
    /// A test failed. Carries the error message; the structured fields
    /// (expected/actual/diff/cause) live on the outcome's
    /// [`crate::runner::FailureDetail`].
    TestFailed {
        /// Module file path.
        module: PathBuf,
        /// Test name.
        name: String,
        /// The error message.
        message: String,
    },
}
