//! The core part of a synthetic load (BUDG-011): one share of the job system with the core count of the reserved subsystem as its ceiling, and that many idle-class spin jobs
//! running for as long as the load stands. The loads are real competitors for the hardware threads while the terrain share's own ceiling keeps terrain inside its
//! threads (BUDG-003, BUDG-008).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use planet_perf::reserve::{calibrate_spin, spin, total_cores, Calibration, SyntheticLoad, LOADS};
use planet_render::{BurnPass, GpuContext};
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

/// What the reserve scene runs beside the terrain: every load's cores on a job system of its own (the subsystems that will claim the reserves have pools of their
/// own), the main-thread spin of all loads once per frame, and the burn passes of the loads with GPU time submitted before the frame.
pub struct ReserveRun {
    // Dropped first: the spin jobs stop before their system joins its workers.
    cores: Vec<CoreLoad>,
    _system: JobSystem,
    spin_iterations: u64,
    burns: Vec<(BurnPass, u32)>,
    /// What the calibration found, one line each, for the log.
    pub notes: Vec<String>,
    /// The calibrated burn milliseconds per frame (0 when the GPU part could not be calibrated).
    pub burn_gpu_ms: f64,
}

impl ReserveRun {
    /// Calibrate the spin and the burns and start the cores of every load. The burn passes need timestamp queries: without them the GPU part is left out and the notes say so.
    pub fn start(ctx: &GpuContext) -> Result<ReserveRun, String> {
        let cal: Calibration = calibrate_spin().ok_or("the CPU spin cannot be calibrated: the clock does not advance")?;
        let mut notes = vec![format!("spin calibrated: {:.0} iterations per ms", cal.iterations_per_ms)];
        let system = JobSystem::new(total_cores(), planet_streaming::Priority::Normal);
        let cores: Vec<CoreLoad> = LOADS.iter().map(|l| CoreLoad::start(&system, l, cal.iterations_per_ms)).collect();
        // The main-thread spin is calibrated again with the cores running: the loads compete for the machine (shared cores, power, caches), and the main-thread work of a
        // reserved share must take its milliseconds under that competition, not on an idle machine.
        std::thread::sleep(std::time::Duration::from_millis(100));
        let cal = calibrate_spin().ok_or("the CPU spin cannot be calibrated beside the loads: the clock does not advance")?;
        notes.push(format!("spin calibrated beside the loads: {:.0} iterations per ms", cal.iterations_per_ms));
        let spin_iterations = cal.iterations_for(planet_perf::reserve::total_main_ms());
        let mut burns: Vec<(BurnPass, u32)> = Vec::new();
        let mut burn_gpu_ms = 0.0;
        for l in LOADS.iter().filter(|l| l.gpu_ms > 0.0) {
            let pass = BurnPass::new(ctx);
            match pass.calibrate(ctx, l.gpu_ms, 0.05) {
                Some(c) => {
                    notes.push(format!(
                        "{} burn calibrated: {} iterations take {:.3} ms (target {:.1} ms)",
                        l.name, c.iterations, c.measured_ms, l.gpu_ms
                    ));
                    burn_gpu_ms += c.measured_ms;
                    burns.push((pass, c.iterations));
                }
                None => notes.push(format!(
                    "{} burn NOT calibrated (no timestamp queries or no stable time): the GPU part of this load is left out",
                    l.name
                )),
            }
        }
        Ok(ReserveRun { cores, _system: system, spin_iterations, burns, notes, burn_gpu_ms })
    }

    /// One frame of the loads on the frame thread: submit the burns, then spin the main-thread milliseconds of every load. Returns the milliseconds the spin took.
    pub fn frame(&self, ctx: &GpuContext) -> f64 {
        self.burn(ctx);
        let t0 = std::time::Instant::now();
        spin(self.spin_iterations);
        t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Submit the burn passes alone (the GPU samples at the end of a flight).
    pub fn burn(&self, ctx: &GpuContext) {
        for (pass, iterations) in &self.burns {
            pass.submit(ctx, *iterations);
        }
    }

    /// The most busy spin jobs of every load, `(name, peak)`.
    pub fn peaks(&self) -> Vec<(&'static str, usize)> {
        LOADS.iter().zip(&self.cores).map(|(l, c)| (l.name, c.peak())).collect()
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
