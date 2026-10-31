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
    /// Most idle-class jobs of this share running at once (`usize::MAX`: no limit of their own); they count in `running` too.
    idle_cap: usize,
    idle_running: usize,
}

struct Queued {
    share: usize,
    class: Class,
    job: Job,
}

struct State {
    queues: [VecDeque<Queued>; 4],
    shares: Vec<ShareState>,
    closed: bool,
    /// Threads exist that run only frame-critical jobs: the other workers leave that class to them.
    frame_dedicated: bool,
    /// Worker threads still running: a spawn after the last one is gone could never run.
    alive: usize,
}

impl State {
    /// The next job a free worker may take: the best class with a job whose share is under its ceiling (a closing system keeps the ceilings and drains).
    fn take(&mut self, frame_only: bool) -> Option<Queued> {
        for class in Class::ALL {
            if frame_only != (class == Class::FrameCritical) && (frame_only || self.frame_dedicated) {
                continue;
            }
            let shares = &self.shares;
            let at = self.queues[class as usize].iter().position(|q| {
                let s = &shares[q.share];
                s.running < s.ceiling && (class != Class::Idle || s.idle_running < s.idle_cap)
            });
            if let Some(at) = at {
                let q = self.queues[class as usize].remove(at).expect("a queued job");
                let s = &mut self.shares[q.share];
                s.running += 1;
                s.peak = s.peak.max(s.running);
                s.idle_running += usize::from(class == Class::Idle);
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
        Self::with_frame_workers(workers, priority, 0, Priority::Normal)
    }

    /// `workers` threads at `priority` for the three classes below frame-critical, and `frame_workers` more at `frame_priority` that run frame-critical jobs only
    /// (the plan of a frame runs at normal priority beside tile workers below normal, ADR 0046); with `frame_workers` at zero every worker takes every class.
    pub fn with_frame_workers(workers: usize, priority: Priority, frame_workers: usize, frame_priority: Priority) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queues: Default::default(),
                shares: Vec::new(),
                closed: false,
                frame_dedicated: frame_workers > 0,
                alive: workers.max(1) + frame_workers,
            }),
            wake: Condvar::new(),
        });
        let spec: Vec<(bool, Priority)> =
            (0..workers.max(1)).map(|_| (false, priority)).chain((0..frame_workers).map(|_| (true, frame_priority))).collect();
        let workers = spec
            .into_iter()
            .enumerate()
            .map(|(n, (frame_only, priority))| {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name(format!("{}-{n}", if frame_only { "job-frame-worker" } else { "job-worker" }))
                    .spawn(move || {
                        if priority != Priority::Normal {
                            priority::set_current(priority);
                        }
                        loop {
                            let queued = {
                                let mut st = shared.state.lock().expect("job state");
                                loop {
                                    if let Some(q) = st.take(frame_only) {
                                        break q;
                                    }
                                    if st.closed && st.queues.iter().all(VecDeque::is_empty) {
                                        st.alive -= 1;
                                        return;
                                    }
                                    st = shared.wake.wait(st).expect("job state");
                                }
                            };
                            // A panic of a job ends the job, not the worker.
                            let _ = catch_unwind(AssertUnwindSafe(queued.job));
                            {
                                let mut st = shared.state.lock().expect("job state");
                                let s = &mut st.shares[queued.share];
                                s.running -= 1;
                                s.idle_running -= usize::from(queued.class == Class::Idle);
                            }
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
        st.shares.push(ShareState {
            name: name.to_string(),
            ceiling: ceiling.max(1),
            running: 0,
            peak: 0,
            idle_cap: usize::MAX,
            idle_running: 0,
        });
        Share { shared: Arc::clone(&self.shared), index: st.shares.len() - 1 }
    }

    /// Like [`share`](JobSystem::share), and at most `idle_cap` of the share's idle-class jobs run at once, so a bake never holds a worker that a
    /// visible job of the share needs (STRM-060): the ceiling is sized for the visible work plus `idle_cap`.
    pub fn share_with_idle(&self, name: &str, ceiling: usize, idle_cap: usize) -> Share {
        let share = self.share(name, ceiling);
        self.shared.state.lock().expect("job state").shares[share.index].idle_cap = idle_cap.max(1);
        share
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

/// A subsystem's handle on the job system: its ceiling and its queue position by class. A share that outlives its `JobSystem` runs no spawned job (the
/// workers are gone); `fan_out` still completes on the calling thread.
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
        let mut st = self.shared.state.lock().expect("job state");
        if st.alive == 0 {
            let name = st.shares[self.index].name.clone();
            drop(st);
            panic!("job spawned on share {name}: its job system was dropped, so no worker is left to run it (keep the JobSystem alive as long as its shares are used)");
        }
        st.queues[class as usize].push_back(Queued { share: self.index, class, job });
        let dedicated = st.frame_dedicated;
        drop(st);
        // With workers that take one class only, the one woken may be a worker that cannot take this job: wake them all.
        if dedicated {
            self.shared.wake.notify_all();
        } else {
            self.shared.wake.notify_one();
        }
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

/// A [`Share`] whose every job runs in one class: the spawner of a subsystem whose work is all of one kind (the plan worker's jobs are frame-critical).
#[derive(Clone)]
pub struct ClassedShare {
    share: Share,
    class: Class,
}

impl Share {
    /// A spawner that puts every job in `class`.
    pub fn classed(&self, class: Class) -> ClassedShare {
        ClassedShare { share: self.clone(), class }
    }
}

impl Spawner for ClassedShare {
    fn spawn(&self, job: Job) {
        self.share.spawn_class(self.class, job);
    }

    fn spawn_in(&self, _class: JobClass, job: Job) {
        self.share.spawn_class(self.class, job);
    }
}

/// Slots of a share taken for helper threads, given back when dropped (BUDG-009).
pub struct Permits {
    share: Share,
    granted: usize,
}

impl Permits {
    pub fn granted(&self) -> usize {
        self.granted
    }
}

impl Drop for Permits {
    fn drop(&mut self) {
        if self.granted > 0 {
            self.share.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).shares[self.share.index].running -=
                self.granted;
            self.share.shared.wake.notify_all();
        }
    }
}

impl Share {
    /// Up to `k` of the share's free slots (ceiling minus jobs running), counted as running until the permits drop: the threads of a fan-out that borrows
    /// its data and so cannot be pool jobs. Never waits; `granted()` may be 0.
    pub fn try_acquire(&self, k: usize) -> Permits {
        let mut st = self.shared.state.lock().expect("job state");
        let s = &mut st.shares[self.index];
        let granted = k.min(s.ceiling.saturating_sub(s.running));
        s.running += granted;
        s.peak = s.peak.max(s.running);
        drop(st);
        Permits { share: self.clone(), granted }
    }

    /// The injected parallelism of the code below the job system (`planet_core::par::Par`): a fan-out takes the free slots of this share and runs on that many
    /// scoped threads, the calling thread (itself a job of this share) one more.
    pub fn par(&self) -> SharePar {
        SharePar { share: self.clone() }
    }
}

/// [`Share::par`].
#[derive(Clone)]
pub struct SharePar {
    share: Share,
}

impl planet_core::par::Par for SharePar {
    fn threads(&self) -> usize {
        let st = self.share.shared.state.lock().expect("job state");
        let s = &st.shares[self.share.index];
        // The calling thread is assumed to be a job of this share (it holds one slot): called from outside, this is one more than the ceiling allows.
        1 + s.ceiling.saturating_sub(s.running)
    }

    fn run(&self, n: usize, f: &(dyn Fn(usize) + Sync)) {
        if n <= 1 {
            for i in 0..n {
                f(i);
            }
            return;
        }
        let permits = self.share.try_acquire(n - 1);
        planet_core::par::run_scoped(1 + permits.granted(), n, f);
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

/// A share together with the job system it was made on: for a subsystem that has a system of its own (the tile pool of one streamer). Dropping it
/// drains the share's queued jobs and joins the workers.
pub struct OwnedShare {
    // The share first: it is dropped before the system whose workers it feeds.
    share: Share,
    _system: JobSystem,
}

impl OwnedShare {
    pub fn new(system: JobSystem, share: Share) -> Self {
        Self { share, _system: system }
    }

    pub fn share(&self) -> &Share {
        &self.share
    }
}

impl Spawner for OwnedShare {
    fn spawn(&self, job: Job) {
        self.share.spawn(job);
    }

    fn spawn_in(&self, class: JobClass, job: Job) {
        self.share.spawn_in(class, job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
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

    // spec: BUDG-008
    #[test]
    fn a_share_can_cap_its_idle_jobs_so_a_bake_never_holds_a_worker_a_visible_job_needs() {
        let system = JobSystem::new(4, Priority::Normal);
        let share = system.share_with_idle("terrain", 4, 1);
        let release = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        for id in 0..3 {
            share.spawn_class(Class::Idle, blocker(tx.clone(), id, Arc::clone(&release)));
        }
        rx.recv_timeout(Duration::from_secs(10)).expect("one idle job starts");
        assert!(rx.recv_timeout(Duration::from_millis(150)).is_err(), "a second idle job started over the idle cap");
        // Visible work still has the other workers.
        for id in 10..13 {
            share.spawn_class(Class::Visible, blocker(tx.clone(), id, Arc::clone(&release)));
        }
        let mut started = Vec::new();
        for _ in 0..3 {
            started.push(rx.recv_timeout(Duration::from_secs(10)).expect("visible jobs start beside the idle one"));
        }
        started.sort();
        assert_eq!(started, [10, 11, 12]);
        release.store(true, Ordering::SeqCst);
        drop(system);
        assert_eq!(share.peak_running(), 4);
    }

    // spec: BUDG-008
    #[test]
    fn frame_critical_jobs_run_on_their_own_workers_and_the_others_never_take_them() {
        let system = JobSystem::with_frame_workers(2, Priority::Normal, 1, Priority::Normal);
        let share = system.share("terrain", 3);
        let release = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        // Both ordinary workers are held.
        for id in 0..2 {
            share.spawn_class(Class::Visible, blocker(tx.clone(), id, Arc::clone(&release)));
        }
        for _ in 0..2 {
            rx.recv_timeout(Duration::from_secs(10)).expect("an ordinary worker starts");
        }
        // A frame-critical job still runs at once, on the frame worker.
        let (name_tx, name_rx) = mpsc::channel();
        share
            .classed(Class::FrameCritical)
            .spawn(Box::new(move || name_tx.send(std::thread::current().name().map(str::to_string)).unwrap()));
        let name = name_rx.recv_timeout(Duration::from_secs(10)).expect("the frame worker runs it").expect("a named thread");
        assert!(name.starts_with("job-frame-worker"), "ran on {name}");
        // A visible job never runs on the frame worker, though it is free.
        let (v_tx, v_rx) = mpsc::channel();
        share.spawn_class(Class::Visible, Box::new(move || v_tx.send(std::thread::current().name().map(str::to_string)).unwrap()));
        assert!(v_rx.recv_timeout(Duration::from_millis(150)).is_err(), "a visible job ran while both ordinary workers were held");
        release.store(true, Ordering::SeqCst);
        let name = v_rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        assert!(name.starts_with("job-worker"), "ran on {name}");
        drop(system);
    }

    // spec: BUDG-008
    #[test]
    fn a_frame_critical_job_is_never_left_waiting_with_ordinary_workers_idle() {
        // Ordinary workers that wake for a job they may not take must not swallow the wake-up of the frame worker.
        let system = JobSystem::with_frame_workers(6, Priority::Normal, 1, Priority::Normal);
        let share = system.share("terrain", 7);
        let plan = share.classed(Class::FrameCritical);
        for round in 0..300 {
            let (tx, rx) = mpsc::channel();
            plan.spawn(Box::new(move || tx.send(round).unwrap()));
            assert_eq!(rx.recv_timeout(Duration::from_secs(10)).expect("a frame-critical job runs"), round);
        }
    }

    // spec: BUDG-009
    #[test]
    fn a_par_of_a_share_takes_free_slots_as_permits_so_the_helpers_never_pass_the_ceiling() {
        use planet_core::par::Par;
        let system = JobSystem::new(8, Priority::Normal);
        let share = system.share("terrain", 4);
        let par = share.par();
        // Called by a job of the share (the plan job): the caller holds one slot, three are free.
        let (tx, rx) = mpsc::channel();
        let (p, s) = (par.clone(), share.clone());
        share.spawn_class(
            Class::FrameCritical,
            Box::new(move || {
                let now = Arc::new(AtomicUsize::new(0));
                let most = Arc::new(AtomicUsize::new(0));
                let threads = p.threads();
                let (n2, m2) = (Arc::clone(&now), Arc::clone(&most));
                p.run(32, &move |_| {
                    let k = n2.fetch_add(1, Ordering::SeqCst) + 1;
                    m2.fetch_max(k, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                    n2.fetch_sub(1, Ordering::SeqCst);
                });
                tx.send((threads, most.load(Ordering::SeqCst), s.peak_running(), s.running())).unwrap();
            }),
        );
        let (threads, most, peak, running_after) = rx.recv_timeout(Duration::from_secs(10)).expect("the job ends");
        assert_eq!(threads, 4, "the caller and three free slots");
        assert!(most <= 4, "{most} threads worked at once");
        assert_eq!(peak, 4, "the permits were counted in the share");
        assert_eq!(running_after, 1, "the permits are given back (only the calling job still runs)");
        // With the share full, a run still completes on the calling thread.
        let release = Arc::new(AtomicBool::new(false));
        let (btx, brx) = mpsc::channel();
        for id in 0..4 {
            share.spawn_class(Class::Visible, blocker(btx.clone(), id, Arc::clone(&release)));
        }
        for _ in 0..4 {
            brx.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        let sum = Mutex::new(0usize);
        par.run(10, &|i| *sum.lock().unwrap() += i);
        assert_eq!(*sum.lock().unwrap(), 45);
        assert_eq!(share.running(), 4, "a full share gave no permits");
        release.store(true, Ordering::SeqCst);
        drop(system);
    }

    // spec: BUDG-009
    #[test]
    fn a_job_spawned_on_a_share_whose_system_is_gone_is_refused_loudly_not_lost() {
        let system = JobSystem::new(2, Priority::Normal);
        let share = system.share("terrain", 2);
        drop(system);
        let refused = catch_unwind(AssertUnwindSafe(|| share.spawn_class(Class::Visible, Box::new(|| {}))));
        let message = refused.expect_err("the spawn is refused").downcast::<String>().map(|s| *s).unwrap_or_default();
        assert!(message.contains("terrain") && message.contains("job system was dropped"), "{message}");
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
            let ran = Arc::new(AtomicUsize::new(0));
            let r = Arc::clone(&ran);
            let work: Arc<dyn Fn(usize) -> usize + Send + Sync> = Arc::new(move |i| {
                r.fetch_add(1, Ordering::SeqCst);
                if i == 5 {
                    panic!("work failed")
                }
                i
            });
            let out = catch_unwind(AssertUnwindSafe(|| share.fan_out(Class::FrameCritical, 8, work)));
            assert_eq!(ran.load(Ordering::SeqCst), 8, "every index ran before the panic was raised");
            if let Err(p) = out {
                resume_unwind(p);
            }
        }));
        assert!(caught.is_err(), "the panic reaches the caller");
        release.store(true, Ordering::SeqCst);
        // A plain job that panics does not end the only worker either: the next job still runs.
        share.spawn_class(Class::Visible, Box::new(|| panic!("job failed")));
        let (tx, rx) = mpsc::channel();
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
