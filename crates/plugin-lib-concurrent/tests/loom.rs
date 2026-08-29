//! Loom model-checking tests for `handl-plugin-lib-concurrent`.
//!
//! The crate's bundled `DefaultSyncRuntime`, `DefaultAtomicRuntime` and
//! `DefaultCancelRuntime` use `std::sync` primitives, which Loom cannot
//! intercept.  These tests therefore exercise the same concurrency contracts
//! with Loom-native types inside `loom::model` blocks.
//!
//! Run with:
//!   RUSTFLAGS="--cfg loom" cargo test -p handl-plugin-lib-concurrent --test loom

#![allow(unexpected_cfgs)]
#![cfg(loom)]

use loom::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use loom::sync::{Arc, Mutex};
use loom::thread;
use std::collections::VecDeque;

#[test]
fn atomic_counter_model() {
    loom::model(|| {
        let counter = Arc::new(AtomicU64::new(0));

        let c1 = counter.clone();
        let c2 = counter.clone();

        let t1 = thread::spawn(move || {
            c1.fetch_add(1, Ordering::SeqCst);
        });
        let t2 = thread::spawn(move || {
            c2.fetch_add(1, Ordering::SeqCst);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn channel_two_senders_one_receiver_model() {
    loom::model(|| {
        let queue: Arc<Mutex<VecDeque<u64>>> = Arc::new(Mutex::new(VecDeque::new()));

        let q1 = queue.clone();
        let q2 = queue.clone();

        let tx1 = thread::spawn(move || {
            q1.lock().unwrap().push_back(1);
        });
        let tx2 = thread::spawn(move || {
            q2.lock().unwrap().push_back(2);
        });

        tx1.join().unwrap();
        tx2.join().unwrap();

        let mut received: Vec<u64> = queue.lock().unwrap().drain(..).collect();
        assert_eq!(received.len(), 2);

        // Loom will explore both send orderings; the receiver should still see
        // exactly the two sent values.
        received.sort();
        assert_eq!(received, vec![1, 2]);
    });
}

#[test]
fn mutex_counter_model() {
    loom::model(|| {
        let counter = Arc::new(Mutex::new(0u64));

        let c1 = counter.clone();
        let c2 = counter.clone();

        let t1 = thread::spawn(move || {
            *c1.lock().unwrap() += 1;
        });
        let t2 = thread::spawn(move || {
            *c2.lock().unwrap() += 1;
        });

        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(*counter.lock().unwrap(), 2);
    });
}

#[test]
fn cancel_token_prevents_eval_when_cancelled() {
    loom::model(|| {
        // Model a token that is already cancelled before the cancelable work
        // has a chance to start. The thunk must never be executed.
        let cancelled = Arc::new(AtomicBool::new(true));
        let evaluated = Arc::new(AtomicU64::new(0));

        let c = cancelled.clone();
        let e = evaluated.clone();

        let worker = thread::spawn(move || {
            // This represents `concurrent.cancelable(token, work)`.
            if !c.load(Ordering::SeqCst) {
                e.fetch_add(1, Ordering::SeqCst);
            }
        });

        worker.join().unwrap();

        assert_eq!(evaluated.load(Ordering::SeqCst), 0);
    });
}
