//! Generation workers run below normal priority (STRM-020, ADR 0046), tier A: the priority a worker thread reads back is the one
//! the pool was built with, on the platforms that have one; the calling thread keeps its own. No test here measures time: a
//! priority changes when a thread runs, not what it computes.

use planet_streaming::{priority, GenWorkers, Priority, Spawner, ThreadPool};
use std::sync::mpsc::channel;
use std::sync::Arc;

/// What each of `jobs` jobs on `pool` reads back as its thread's priority.
fn worker_priorities(pool: &dyn Spawner, jobs: usize) -> Vec<Option<Priority>> {
    let (tx, rx) = channel();
    for _ in 0..jobs {
        let tx = tx.clone();
        pool.spawn(Box::new(move || {
            let _ = tx.send(priority::current());
        }));
    }
    (0..jobs).map(|_| rx.recv().expect("a job reports")).collect()
}

const HAS_PRIORITY: bool = cfg!(any(windows, target_os = "linux"));

// spec: STRM-020
#[test]
fn pool_workers_read_back_the_priority_the_pool_was_built_with_and_the_caller_keeps_its_own() {
    let caller = priority::current();
    let pool = ThreadPool::with_priority(3, Priority::BelowNormal);
    let got = worker_priorities(&pool, 12);
    if HAS_PRIORITY {
        assert!(got.iter().all(|p| *p == Some(Priority::BelowNormal)), "workers read back {got:?}");
    } else {
        assert!(got.iter().all(Option::is_none), "no priority on this platform, read back {got:?}");
    }
    assert_eq!(priority::current(), caller, "building the pool changed the calling thread");
    drop(pool);

    let normal = ThreadPool::new(2);
    let got = worker_priorities(&normal, 6);
    if HAS_PRIORITY {
        assert!(got.iter().all(|p| *p == Some(Priority::Normal)), "default workers read back {got:?}");
    }
}

// spec: STRM-020
#[test]
fn generation_workers_carry_their_priority_into_the_pool_and_it_changes_no_size_or_limit() {
    let plain = GenWorkers::new(4);
    let low = plain.with_priority(Priority::BelowNormal);
    assert_eq!((low.workers, low.ahead), (plain.workers, plain.ahead));
    assert_eq!(plain.apply(Default::default()).job_limit(), low.apply(Default::default()).job_limit());
    let pool: Arc<ThreadPool> = low.pool();
    if HAS_PRIORITY {
        assert!(worker_priorities(pool.as_ref(), 8).iter().all(|p| *p == Some(Priority::BelowNormal)));
    }
}

// spec: STRM-020
#[test]
fn a_thread_can_set_and_read_back_its_own_priority_and_the_names_parse() {
    let handle = std::thread::spawn(|| {
        let low = priority::set_current(Priority::BelowNormal);
        let read_low = priority::current();
        let back = priority::set_current(Priority::Normal);
        (low, read_low, back, priority::current())
    });
    let (low, read_low, back, read_back) = handle.join().expect("thread");
    if HAS_PRIORITY {
        assert!(low && back, "lowering and restoring a thread's own priority is always allowed");
        assert_eq!((read_low, read_back), (Some(Priority::BelowNormal), Some(Priority::Normal)));
    } else {
        assert!(!low && !back && read_low.is_none(), "a platform without the call reports that it did not apply");
    }
    assert_eq!(Priority::parse("below-normal"), Some(Priority::BelowNormal));
    assert_eq!(Priority::parse("normal").map(|p| p.name()), Some("normal"));
    assert_eq!(Priority::parse("high"), None);
}
