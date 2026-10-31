//! Injected parallelism (CON-04; BUDG-009): code in the lower crates that splits a computation over threads asks a [`Par`] for them instead of starting threads
//! itself, so the job system can count them against a share (the plan job's origin heights are the plan share's work) and a test can run them in order.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Runs `n` pieces of work on up to [`Par::threads`] threads, the calling thread among them.
pub trait Par: Sync {
    /// Most threads that work at once, the caller counted (at least 1).
    fn threads(&self) -> usize;

    /// Run `f(0)` to `f(n - 1)`, each once, in any order and concurrently where threads are free, and return when all are done. A panic of `f` is raised in
    /// the caller. Results must not depend on which thread ran an index (CON-14): `f` writes to its own slot.
    fn run(&self, n: usize, f: &(dyn Fn(usize) + Sync));
}

/// `k` plain scoped threads: `k` threads work at once, the caller one of them, started for the call and gone at its end. What the lower crates used before the
/// job system and what a test without one uses; `0` and `1` run on the calling thread.
impl Par for usize {
    fn threads(&self) -> usize {
        (*self).max(1)
    }

    fn run(&self, n: usize, f: &(dyn Fn(usize) + Sync)) {
        run_scoped(self.threads().min(n), n, f);
    }
}

/// `workers` scoped threads (the caller is one) take the indices `0..n` from one counter.
pub fn run_scoped(workers: usize, n: usize, f: &(dyn Fn(usize) + Sync)) {
    if workers <= 1 || n <= 1 {
        for i in 0..n {
            f(i);
        }
        return;
    }
    let next = AtomicUsize::new(0);
    let work = || loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= n {
            return;
        }
        f(i);
    };
    std::thread::scope(|s| {
        for _ in 1..workers {
            crate::thread::spawn_named(s, "par-worker", work);
        }
        work();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // spec: BUDG-009
    #[test]
    fn every_index_runs_once_on_any_thread_count_and_zero_and_one_stay_on_the_caller() {
        for threads in [0usize, 1, 2, 5, 16] {
            let seen = Mutex::new(vec![0u32; 40]);
            threads.run(40, &|i| seen.lock().unwrap()[i] += 1);
            assert!(seen.lock().unwrap().iter().all(|&c| c == 1), "{threads} threads");
        }
        let caller = std::thread::current().id();
        let ids = Mutex::new(Vec::new());
        1usize.run(8, &|_| ids.lock().unwrap().push(std::thread::current().id()));
        assert!(ids.lock().unwrap().iter().all(|id| *id == caller));
        assert_eq!(0usize.threads(), 1);
    }
}
