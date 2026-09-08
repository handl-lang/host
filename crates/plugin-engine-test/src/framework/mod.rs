//! # Test framework assert side-channel (ADR-0092)
//!
//! The pure-handl test framework now lives in `handl-packages/test/` and
//! is registered by the canonical kit (`kit-canonical`).  This crate only
//! keeps the thread-local side-channel used by the tree-walk `assert`
//! keyword handler to store structured failure detail for the runner.

use std::cell::RefCell;

use crate::runner::FailureDetail;

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
