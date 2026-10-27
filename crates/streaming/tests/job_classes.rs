//! Job classes of the pool (STRM-060, ADR 0083 point 2), tier A: a waiting visible job starts before any waiting idle job, and idle jobs
//! take at most the workers the pool grants them. No test here measures time.

use planet_streaming::{JobClass, ManualSpawner, Priority, Spawner, ThreadPool};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Occupy the one worker of `pool` until the returned sender is used.
fn hold_worker(pool: &ThreadPool) -> Sender<()> {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel::<()>();
    pool.spawn(Box::new(move || {
        started_tx.send(()).expect("report");
        let _ = release_rx.recv();
    }));
    started_rx.recv().expect("the worker holds the job");
    release_tx
}

// spec: STRM-060
#[test]
fn a_waiting_visible_job_starts_before_any_waiting_idle_job() {
    let pool = ThreadPool::new(1);
    let release = hold_worker(&pool);
    let order = Arc::new(Mutex::new(Vec::new()));
    let (done_tx, done_rx) = channel();
    for (class, name) in [
        (JobClass::Idle, "idle 1"),
        (JobClass::Idle, "idle 2"),
        (JobClass::Prefetch, "prefetch"),
        (JobClass::Visible, "visible 1"),
        (JobClass::Visible, "visible 2"),
    ] {
        let (order, done) = (order.clone(), done_tx.clone());
        pool.spawn_in(
            class,
            Box::new(move || {
                order.lock().expect("order").push(name);
                done.send(()).expect("report");
            }),
        );
    }
    release.send(()).expect("release the worker");
    for _ in 0..5 {
        done_rx.recv_timeout(Duration::from_secs(30)).expect("a job ran");
    }
    assert_eq!(*order.lock().expect("order"), ["visible 1", "visible 2", "prefetch", "idle 1", "idle 2"]);
}

// spec: STRM-060
#[test]
fn idle_jobs_take_at_most_the_workers_granted_and_a_visible_job_never_waits_for_them() {
    let pool = ThreadPool::with_classes(3, Priority::Normal, 1);
    let (running, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (release_tx, release_rx) = channel::<()>();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let (started_tx, started_rx) = channel();
    for _ in 0..3 {
        let (running, peak, release_rx, started) = (running.clone(), peak.clone(), release_rx.clone(), started_tx.clone());
        pool.spawn_in(
            JobClass::Idle,
            Box::new(move || {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                started.send(()).expect("report");
                let _ = release_rx.lock().expect("release").recv();
                running.fetch_sub(1, Ordering::SeqCst);
            }),
        );
    }
    started_rx.recv_timeout(Duration::from_secs(30)).expect("one idle job runs");
    // Two workers are free for visible work although two idle jobs wait.
    let (v_tx, v_rx) = channel();
    for n in 0..2 {
        let v = v_tx.clone();
        pool.spawn(Box::new(move || v.send(n).expect("report")));
    }
    for _ in 0..2 {
        v_rx.recv_timeout(Duration::from_secs(30)).expect("a visible job ran beside the running idle job");
    }
    assert!(started_rx.try_recv().is_err(), "a second idle job started while the first held the only idle worker");
    for _ in 0..3 {
        release_tx.send(()).expect("release");
    }
    started_rx.recv_timeout(Duration::from_secs(30)).expect("the next idle job ran when the first ended");
    started_rx.recv_timeout(Duration::from_secs(30)).expect("the last idle job ran");
    drop(pool);
    assert_eq!(peak.load(Ordering::SeqCst), 1, "idle jobs ran side by side");
}

// spec: STRM-060
#[test]
fn the_manual_spawner_runs_the_best_class_first_and_the_oldest_within_it() {
    let s = ManualSpawner::default();
    let order = Arc::new(Mutex::new(Vec::new()));
    for (class, name) in [(JobClass::Idle, "i"), (JobClass::Visible, "v1"), (JobClass::Prefetch, "p"), (JobClass::Visible, "v2")] {
        let order = order.clone();
        s.spawn_in(class, Box::new(move || order.lock().expect("order").push(name)));
    }
    assert_eq!(s.run(10), 4);
    assert_eq!(*order.lock().expect("order"), ["v1", "v2", "p", "i"]);
}
