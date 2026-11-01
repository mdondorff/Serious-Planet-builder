//! The core part of a synthetic load (BUDG-011): one share of the job system with the core count of the reserved subsystem as its ceiling, and that many idle-class spin jobs
//! running for as long as the load stands. The loads are real competitors for the hardware threads while the terrain share's own ceiling keeps terrain inside its
//! threads (BUDG-003, BUDG-008).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use planet_perf::reserve::{spin, SyntheticLoad};
use planet_streaming::jobs::{Class, JobSystem, Share};

/// How long a spin job works between two looks at the stop flag, in milliseconds.
const CHUNK_MS: f64 = 1.0;

/// A reserved share's core load: `cores` spin jobs on a share of ceiling `cores`.
pub struct CoreLoad {
    share: Share,
    stop: Arc<AtomicBool>,
    cores: usize,
}

impl CoreLoad {
    /// Start the load of `load` on `system`: the system needs `load.cores` workers beside the ones of its other shares. `iterations_per_ms` is the spin calibration.
    pub fn start(system: &JobSystem, load: &SyntheticLoad, iterations_per_ms: f64) -> CoreLoad {
        let cores = load.cores.max(1);
        let share = system.share(load.name, cores);
        let stop = Arc::new(AtomicBool::new(false));
        let chunk = (CHUNK_MS * iterations_per_ms).round().max(1.0) as u64;
        for _ in 0..cores {
            let stop = Arc::clone(&stop);
            share.spawn_class(
                Class::Idle,
                Box::new(move || {
                    while !stop.load(Ordering::Relaxed) {
                        spin(chunk);
                    }
                }),
            );
        }
        CoreLoad { share, stop, cores }
    }

    /// The cores the load was asked for.
    pub fn cores(&self) -> usize {
        self.cores
    }

    /// Spin jobs running now.
    pub fn running(&self) -> usize {
        self.share.running()
    }

    /// The most spin jobs that ran at once.
    pub fn peak(&self) -> usize {
        self.share.peak_running()
    }

    /// Ask the spin jobs to end; they stop within a chunk.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for CoreLoad {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_perf::reserve::LOADS;
    use planet_streaming::jobs::JobSystem;
    use planet_streaming::Priority;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    fn until(what: &str, mut ok: impl FnMut() -> bool) {
        let started = Instant::now();
        while !ok() {
            assert!(started.elapsed() < Duration::from_secs(20), "stalled waiting for {what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    // spec: BUDG-011
    #[test]
    fn a_core_load_keeps_exactly_its_cores_busy_and_never_more() {
        let system = JobSystem::new(10, Priority::Normal);
        let physics = LOADS.iter().find(|l| l.name == "physics").unwrap();
        let load = CoreLoad::start(&system, physics, 1000.0);
        until("both physics spin jobs", || load.running() == 2);
        // Queue more work on the same share than its ceiling: it still runs two at once.
        for _ in 0..5 {
            load.share.spawn_class(Class::Idle, Box::new(|| std::thread::sleep(Duration::from_millis(2))));
        }
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!((load.cores(), load.running()), (2, 2));
        assert_eq!(load.peak(), 2, "the share's ceiling holds");
        load.stop();
    }

    // spec: BUDG-011, BUDG-008
    #[test]
    fn terrains_ceiling_holds_beside_all_four_loads() {
        let cores: usize = LOADS.iter().map(|l| l.cores).sum();
        let system = JobSystem::new(cores + 4, Priority::Normal);
        let loads: Vec<CoreLoad> = LOADS.iter().map(|l| CoreLoad::start(&system, l, 1000.0)).collect();
        until("every load at its cores", || loads.iter().all(|l| l.running() == l.cores()));
        let terrain = system.share("terrain", 4);
        let latch = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicUsize::new(0));
        for _ in 0..10 {
            let (latch, started) = (Arc::clone(&latch), Arc::clone(&started));
            terrain.spawn_class(
                Class::Visible,
                Box::new(move || {
                    started.fetch_add(1, Ordering::SeqCst);
                    while !latch.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }),
            );
        }
        until("terrain at its ceiling", || terrain.running() == 4);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!((terrain.running(), terrain.peak_running(), started.load(Ordering::SeqCst)), (4, 4, 4), "terrain keeps its ceiling");
        assert!(loads.iter().all(|l| l.running() == l.cores() && l.peak() == l.cores()), "the loads keep theirs");
        latch.store(true, Ordering::SeqCst);
        loads.iter().for_each(CoreLoad::stop);
    }
}
