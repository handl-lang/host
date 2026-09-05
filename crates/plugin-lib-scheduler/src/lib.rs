//! # handl-plugin-lib-scheduler
//!
//! Generic bounded-job scheduler for the handl platform.
//!
//! Executes a collection of independent jobs under a [`SchedulePolicy`],
//! limiting parallelism and recording a per-job timeout for future
//! enforcement. The scheduler is intentionally generic: callers supply job
//! IDs and closures, and receive results keyed by those IDs. Ordering of
//! results reflects completion order; callers that need deterministic
//! ordering (such as the test runner) re-collect results into their own
//! canonical order.
//!
//! This crate is the generic home for the scheduling concern identified in
//! ADR-0089 *Generic infrastructure dependencies*. The test runner is the
//! first consumer.

#![warn(missing_docs)]

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Policy controlling how jobs are scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulePolicy {
    /// Maximum number of jobs that may run concurrently.
    pub max_concurrency: usize,
    /// Optional soft timeout for each individual job.
    ///
    /// **Note:** The initial implementation records this value but does not
    /// hard-kill jobs that exceed it. Future versions will enforce it.
    pub per_job_timeout: Option<Duration>,
}

impl SchedulePolicy {
    /// A policy that runs jobs sequentially.
    pub fn sequential() -> Self {
        Self {
            max_concurrency: 1,
            per_job_timeout: None,
        }
    }

    /// A policy that runs jobs with the given concurrency limit.
    pub fn parallel(max_concurrency: usize) -> Self {
        Self {
            max_concurrency: max_concurrency.max(1),
            per_job_timeout: None,
        }
    }
}

impl Default for SchedulePolicy {
    fn default() -> Self {
        Self {
            max_concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            per_job_timeout: None,
        }
    }
}

/// Error returned when a job fails to complete successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    /// The job panicked.
    Panic,
    /// The job exceeded its allotted time.
    ///
    /// **Note:** Not enforced by the initial implementation.
    Timeout,
}

/// Result of a single job, keyed by the caller-supplied ID.
#[derive(Debug)]
pub struct JobResult<T> {
    /// The outcome of the job.
    pub output: Result<T, JobError>,
}

impl<T> JobResult<T> {
    /// Create a successful result.
    pub fn ok(value: T) -> Self {
        Self { output: Ok(value) }
    }

    /// Create a failed result.
    pub fn err(error: JobError) -> Self {
        Self { output: Err(error) }
    }
}

/// A bounded, thread-pool based job scheduler.
#[derive(Debug, Default, Clone, Copy)]
pub struct BoundedScheduler;

impl BoundedScheduler {
    /// Run a collection of independent jobs under the given policy.
    ///
    /// Each job is a closure that returns a value of type `R`. Results are
    /// returned as `(id, result)` pairs in completion order.
    pub fn run<I, J, R>(jobs: Vec<(I, J)>, policy: &SchedulePolicy) -> Vec<(I, JobResult<R>)>
    where
        I: Clone + Eq + std::hash::Hash + Send + Debug + 'static,
        J: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let n_jobs = jobs.len();
        if n_jobs == 0 {
            return Vec::new();
        }

        let policy = *policy;
        let n_workers = policy.max_concurrency.max(1).min(n_jobs);
        let queue: Arc<Mutex<VecDeque<(I, Option<J>)>>> = Arc::new(Mutex::new(VecDeque::new()));
        for (id, job) in jobs {
            queue.lock().unwrap().push_back((id, Some(job)));
        }

        let (result_tx, result_rx) = mpsc::channel::<(I, JobResult<R>)>();

        let mut handles = Vec::with_capacity(n_workers);
        for _ in 0..n_workers {
            let queue = Arc::clone(&queue);
            let result_tx = result_tx.clone();
            let handle = thread::spawn(move || {
                loop {
                    let next = {
                        let mut q = queue.lock().unwrap();
                        q.pop_front()
                    };
                    match next {
                        Some((id, Some(job))) => {
                            let result = Self::execute(job, policy.per_job_timeout);
                            let _ = result_tx.send((id, result));
                        }
                        Some((_, None)) => {}
                        None => break,
                    }
                }
            });
            handles.push(handle);
        }

        // Drop the original sender so the channel closes once all workers finish.
        drop(result_tx);

        let mut results = Vec::with_capacity(n_jobs);
        while let Ok((id, result)) = result_rx.recv() {
            results.push((id, result));
        }

        for handle in handles {
            let _ = handle.join();
        }

        results
    }

    fn execute<J, R>(job: J, _timeout: Option<Duration>) -> JobResult<R>
    where
        J: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        // catch_unwind requires UnwindSafe; AssertUnwindSafe is appropriate
        // because we consume the closure immediately and do not share it.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
            Ok(value) => JobResult::ok(value),
            Err(_) => JobResult::err(JobError::Panic),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    #[test]
    fn empty_job_list_returns_empty_results() {
        let results: Vec<(String, JobResult<()>)> = BoundedScheduler::run(
            Vec::<(String, Box<dyn FnOnce() -> () + Send>)>::new(),
            &SchedulePolicy::sequential(),
        );
        assert!(results.is_empty());
    }

    #[test]
    fn sequential_policy_runs_one_at_a_time() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let jobs: Vec<(String, Box<dyn FnOnce() -> usize + Send>)> = (0..4usize)
            .map(|i| {
                let active = active.clone();
                let max_active = max_active.clone();
                (
                    format!("job-{}", i),
                    Box::new(move || {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(current, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(10));
                        active.fetch_sub(1, Ordering::SeqCst);
                        i
                    }) as Box<dyn FnOnce() -> usize + Send>,
                )
            })
            .collect();

        let _results = BoundedScheduler::run(jobs, &SchedulePolicy::sequential());
        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn parallel_policy_runs_multiple_concurrently() {
        let start = Instant::now();
        let jobs: Vec<(String, Box<dyn FnOnce() -> usize + Send>)> = (0..4usize)
            .map(|i| {
                (
                    format!("job-{}", i),
                    Box::new(move || {
                        thread::sleep(Duration::from_millis(50));
                        i
                    }) as Box<dyn FnOnce() -> usize + Send>,
                )
            })
            .collect();

        let results = BoundedScheduler::run(jobs, &SchedulePolicy::parallel(4));
        assert_eq!(results.len(), 4);
        let elapsed = start.elapsed();
        // If run sequentially, 4 * 50ms = 200ms. With 4 workers, ~50ms.
        assert!(elapsed < Duration::from_millis(150), "took {:?}", elapsed);
    }

    #[test]
    fn panics_are_caught() {
        let jobs: Vec<(String, Box<dyn FnOnce() -> i32 + Send>)> =
            vec![("panic".to_string(), Box::new(|| -> i32 { panic!("boom") }))];
        let results = BoundedScheduler::run(jobs, &SchedulePolicy::parallel(1));
        assert_eq!(results.len(), 1);
        let (_id, result) = &results[0];
        assert_eq!(result.output, Err(JobError::Panic));
    }

    #[test]
    fn results_return_in_completion_order() {
        let jobs: Vec<(String, Box<dyn FnOnce() -> i32 + Send>)> = vec![
            (
                "slow".to_string(),
                Box::new(|| {
                    thread::sleep(Duration::from_millis(30));
                    1
                }),
            ),
            ("fast".to_string(), Box::new(|| 2)),
        ];
        let results = BoundedScheduler::run(jobs, &SchedulePolicy::parallel(2));
        assert_eq!(results[0].0, "fast");
        assert_eq!(results[1].0, "slow");
    }
}
