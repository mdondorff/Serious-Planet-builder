//! The job system (ADR 0083 point 2; BUDG-004, BUDG-008, BUDG-009): one set of worker threads for all background work, a share per subsystem with a
//! ceiling on its jobs running at once, and the classes frame-critical, visible, prefetch and idle in that order. A subsystem owns a [`Share`]; it
//! spawns jobs on it and fans a computation out over it ([`Share::fan_out`]) and never starts a thread of its own.

use crate::priority::{self, Priority};
use crate::{Job, JobClass, Spawner};
use std::collections::VecDeque;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// The priority class of a job, best first. A free worker takes a job of the best class that has an eligible one; within a class jobs run in
/// submission order; a running job is never preempted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// Work a frame waits for (the plan of the frame).
    FrameCritical,
    /// Visible streaming and edits: the tiles and chunks the view draws.
    Visible,
    /// Prefetch: work for where the camera will be.
    Prefetch,
    /// Idle: bakes nobody waits for.
    Idle,
}

impl Class {
    const ALL: [Class; 4] = [Class::FrameCritical, Class::Visible, Class::Prefetch, Class::Idle];
}

impl From<JobClass> for Class {
    fn from(c: JobClass) -> Class {
        match c {
            JobClass::Visible => Class::Visible,
            JobClass::Prefetch => Class::Prefetch,
            JobClass::Idle => Class::Idle,
        }
    }
}

struct ShareState {
    name: String,
    /// Most jobs of this share running at once.
    ceiling: usize,
    running: usize,
    /// The most jobs of this share that ran at once so far.
    peak: usize,
}

struct Queued {
    share: usize,
    job: Job,
}

struct State {
    queues: [VecDeque<Queued>; 4],
    shares: Vec<ShareState>,
    closed: bool,
}

impl State {
    /// The next job a free worker may take: the best class with a job whose share is under its ceiling (a closing system keeps the ceilings and drains).
    fn take(&mut self) -> Option<Queued> {
        for class in Class::ALL {
            let shares = &self.shares;
            let at = self.queues[class as usize].iter().position(|q| shares[q.share].running < shares[q.share].ceiling);
            if let Some(at) = at {
                let q = self.queues[class as usize].remove(at).expect("a queued job");
                let s = &mut self.shares[q.share];
                s.running += 1;
                s.peak = s.peak.max(s.running);
                return Some(q);
            }
        }
        None
    }
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

/// The workers: `workers` threads for every share together.
pub struct JobSystem {
    shared: Arc<Shared>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl JobSystem {
    /// `workers` threads (at least one), each setting its own thread's `priority` before its first job.
    pub fn new(workers: usize, priority: Priority) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State { queues: Default::default(), shares: Vec::new(), closed: false }),
            wake: Condvar::new(),
        });
        let workers = (0..workers.max(1))
            .map(|n| {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name(format!("job-worker-{n}"))
                    .spawn(move || {
                        if priority != Priority::Normal {
                            priority::set_current(priority);
                        }
                        loop {
                            let queued = {
                                let mut st = shared.state.lock().expect("job state");
                                loop {
                                    if let Some(q) = st.take() {
                                        break q;
                                    }
                                    if st.closed && st.queues.iter().all(VecDeque::is_empty) {
                                        return;
                                    }
                                    st = shared.wake.wait(st).expect("job state");
                                }
                            };
                            // A panic of a job ends the job, not the worker.
                            let _ = catch_unwind(AssertUnwindSafe(queued.job));
                            shared.state.lock().expect("job state").shares[queued.share].running -= 1;
                            // A job of this share that waited for the ceiling may start now.
                            shared.wake.notify_all();
                        }
                    })
                    .expect("start a job worker")
            })
            .collect();
        Self { shared, workers: Mutex::new(workers) }
    }

    /// A share named `name` with at most `ceiling` jobs running at once (at least one).
    pub fn share(&self, name: &str, ceiling: usize) -> Share {
        let mut st = self.shared.state.lock().expect("job state");
        st.shares.push(ShareState { name: name.to_string(), ceiling: ceiling.max(1), running: 0, peak: 0 });
        Share { shared: Arc::clone(&self.shared), index: st.shares.len() - 1 }
    }
}

impl Drop for JobSystem {
    fn drop(&mut self) {
        self.shared.state.lock().expect("job state").closed = true;
        self.shared.wake.notify_all();
        for w in self.workers.lock().expect("job workers").drain(..) {
            let _ = w.join();
        }
    }
}

/// A subsystem's handle on the job system: its ceiling and its queue position by class.
#[derive(Clone)]
pub struct Share {
    shared: Arc<Shared>,
    index: usize,
}

impl Share {
    pub fn name(&self) -> String {
        self.shared.state.lock().expect("job state").shares[self.index].name.clone()
    }

    pub fn ceiling(&self) -> usize {
        self.shared.state.lock().expect("job state").shares[self.index].ceiling
    }

    /// Jobs of this share running now.
    pub fn running(&self) -> usize {
        self.shared.state.lock().expect("job state").shares[self.index].running
    }

    /// The most jobs of this share that ran at once since the share was made.
    pub fn peak_running(&self) -> usize {
        self.shared.state.lock().expect("job state").shares[self.index].peak
    }

    /// A job of `class`: it runs when a worker is free and fewer than the ceiling of this share are running.
    pub fn spawn_class(&self, class: Class, job: Job) {
        self.shared.state.lock().expect("job state").queues[class as usize].push_back(Queued { share: self.index, job });
        self.shared.wake.notify_one();
    }

    /// `f(0)` to `f(n - 1)` on this share, the results in index order whatever the number of workers (CON-14). The calling thread works on the indices too,
    /// so the call makes progress when every worker is busy; up to `ceiling - 1` helper jobs of `class` run the rest. A panic of `f` is raised again
    /// here once every index has finished. The caller's own thread is not counted in the share's ceiling.
    pub fn fan_out<R: Send + 'static>(&self, class: Class, n: usize, f: Arc<dyn Fn(usize) -> R + Send + Sync>) -> Vec<R> {
        struct Run<R> {
            next: AtomicUsize,
            results: Mutex<Vec<Option<std::thread::Result<R>>>>,
            finished: Mutex<usize>,
            all: Condvar,
        }
        fn work<R>(run: &Run<R>, n: usize, f: &(dyn Fn(usize) -> R + Send + Sync)) {
            loop {
                let i = run.next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    return;
                }
                let r = catch_unwind(AssertUnwindSafe(|| f(i)));
                run.results.lock().expect("fan-out results")[i] = Some(r);
                let mut done = run.finished.lock().expect("fan-out count");
                *done += 1;
                if *done == n {
                    run.all.notify_all();
                }
            }
        }
        if n == 0 {
            return Vec::new();
        }
        let run = Arc::new(Run {
            next: AtomicUsize::new(0),
            results: Mutex::new((0..n).map(|_| None).collect()),
            finished: Mutex::new(0),
            all: Condvar::new(),
        });
        for _ in 0..(self.ceiling().saturating_sub(1)).min(n - 1) {
            let (run, f) = (Arc::clone(&run), Arc::clone(&f));
            // A helper that starts after the caller has taken the last index finds none and ends.
            self.spawn_class(class, Box::new(move || work(&run, n, f.as_ref())));
        }
        work(&run, n, f.as_ref());
        let mut done = run.finished.lock().expect("fan-out count");
        while *done < n {
            done = run.all.wait(done).expect("fan-out count");
        }
        drop(done);
        let results = std::mem::take(&mut *run.results.lock().expect("fan-out results"));
        results.into_iter().map(|r| r.expect("every index ran")).map(|r| r.unwrap_or_else(|p| resume_unwind(p))).collect()
    }
}

impl Spawner for Share {
    fn spawn(&self, job: Job) {
        self.spawn_class(Class::Visible, job);
    }

    fn spawn_in(&self, class: JobClass, job: Job) {
        self.spawn_class(class.into(), job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A job that holds its worker until `release` is set, and says when it started.
    fn blocker(started: mpsc::Sender<usize>, id: usize, release: Arc<AtomicBool>) -> Job {
        Box::new(move || {
            started.send(id).unwrap();
            while !release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    }

    // spec: BUDG-008
    #[test]
    fn a_share_never_runs_more_jobs_at_once_than_its_ceiling_while_workers_are_free() {
        let system = JobSystem::new(6, Priority::Normal);
        let share = system.share("terrain", 2);
        let release = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        for id in 0..5 {
            share.spawn_class(Class::Visible, blocker(tx.clone(), id, Arc::clone(&release)));
        }
        // Two start; the other three wait although four workers are free.
        for _ in 0..2 {
            rx.recv_timeout(Duration::from_secs(10)).expect("a job starts");
        }
        assert!(rx.recv_timeout(Duration::from_millis(150)).is_err(), "a third job started over the ceiling");
        assert_eq!((share.running(), share.peak_running()), (2, 2));
        release.store(true, Ordering::SeqCst);
        for _ in 0..3 {
            rx.recv_timeout(Duration::from_secs(10)).expect("a waiting job starts when one ends");
        }
        drop(system);
        assert_eq!(share.peak_running(), 2, "the ceiling held for the whole run");
    }

    // spec: BUDG-008
    #[test]
    fn a_better_class_passes_queued_jobs_of_a_worse_class_and_a_class_keeps_submission_order() {
        let system = JobSystem::new(1, Priority::Normal);
        let share = system.share("terrain", 1);
        let release = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        // The only worker is held; the rest queue up in a mixed order.
        share.spawn_class(Class::Visible, blocker(tx.clone(), 100, Arc::clone(&release)));
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 100);
        let order = Arc::new(Mutex::new(Vec::new()));
        for (class, id) in
            [(Class::Idle, 1), (Class::Prefetch, 2), (Class::Visible, 3), (Class::Idle, 4), (Class::FrameCritical, 5), (Class::Visible, 6)]
        {
            let order = Arc::clone(&order);
            share.spawn_class(class, Box::new(move || order.lock().unwrap().push(id)));
        }
        release.store(true, Ordering::SeqCst);
        drop(system);
        assert_eq!(*order.lock().unwrap(), [5, 3, 6, 2, 1, 4]);
    }

    // spec: BUDG-008
    #[test]
    fn two_shares_keep_their_own_ceilings_and_a_full_share_does_not_block_another() {
        let system = JobSystem::new(4, Priority::Normal);
        let (a, b) = (system.share("terrain", 1), system.share("sky", 2));
        let release = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        for id in 0..3 {
            a.spawn_class(Class::Visible, blocker(tx.clone(), id, Arc::clone(&release)));
        }
        for id in 10..12 {
            b.spawn_class(Class::Idle, blocker(tx.clone(), id, Arc::clone(&release)));
        }
        let mut started = Vec::new();
        for _ in 0..3 {
            started.push(rx.recv_timeout(Duration::from_secs(10)).expect("start"));
        }
        started.sort();
        assert_eq!(started, [0, 10, 11], "one of a, both of b");
        release.store(true, Ordering::SeqCst);
        drop(system);
        assert_eq!((a.peak_running(), b.peak_running()), (1, 2));
    }

    // spec: BUDG-009
    #[test]
    fn a_fan_out_returns_the_results_in_index_order_for_any_worker_count_and_ceiling() {
        let f: Arc<dyn Fn(usize) -> u64 + Send + Sync> = Arc::new(|i| (i as u64).wrapping_mul(2_654_435_761) ^ 7);
        let want: Vec<u64> = (0..200).map(|i| f(i)).collect();
        for (workers, ceiling) in [(1, 1), (1, 8), (3, 2), (8, 8), (8, 1)] {
            let system = JobSystem::new(workers, Priority::Normal);
            let share = system.share("terrain", ceiling);
            assert_eq!(share.fan_out(Class::FrameCritical, 200, Arc::clone(&f)), want, "{workers} workers, ceiling {ceiling}");
            assert!(share.peak_running() <= ceiling, "helpers stay inside the ceiling");
        }
        let system = JobSystem::new(2, Priority::Normal);
        assert!(system.share("terrain", 2).fan_out(Class::Visible, 0, f).is_empty());
    }

    // spec: BUDG-009
    #[test]
    fn a_fan_out_makes_progress_when_every_worker_is_busy_and_raises_a_panic_of_the_work() {
        let system = JobSystem::new(1, Priority::Normal);
        let share = system.share("terrain", 4);
        let release = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        share.spawn_class(Class::Visible, blocker(tx, 0, Arc::clone(&release)));
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        // The only worker is held: the caller does all of it.
        let sum: usize = share.fan_out(Class::FrameCritical, 50, Arc::new(|i| i)).into_iter().sum();
        assert_eq!(sum, (0..50).sum::<usize>());
        let caught = catch_unwind(AssertUnwindSafe(|| {
            share.fan_out(Class::FrameCritical, 8, Arc::new(|i| if i == 5 { panic!("work failed") } else { i }));
        }));
        assert!(caught.is_err(), "the panic reaches the caller");
        release.store(true, Ordering::SeqCst);
        // The worker is still alive after the panic.
        let (tx, rx) = mpsc::channel();
        drop(system.share("again", 1));
        share.spawn_class(Class::Visible, Box::new(move || tx.send(1).unwrap()));
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 1);
    }

    // spec: BUDG-009
    #[test]
    fn a_share_is_a_spawner_and_a_job_class_maps_to_its_class() {
        let system = JobSystem::new(2, Priority::Normal);
        let share = system.share("terrain", 2);
        let (tx, rx) = mpsc::channel();
        let spawner: &dyn Spawner = &share;
        let t = tx.clone();
        spawner.spawn(Box::new(move || t.send("visible").unwrap()));
        spawner.spawn_in(JobClass::Idle, Box::new(move || tx.send("idle").unwrap()));
        let mut got = vec![rx.recv_timeout(Duration::from_secs(10)).unwrap(), rx.recv_timeout(Duration::from_secs(10)).unwrap()];
        got.sort();
        assert_eq!(got, ["idle", "visible"]);
        assert_eq!(
            [Class::from(JobClass::Visible), Class::from(JobClass::Prefetch), Class::from(JobClass::Idle)],
            [Class::Visible, Class::Prefetch, Class::Idle]
        );
        assert_eq!(share.name(), "terrain");
    }
}
