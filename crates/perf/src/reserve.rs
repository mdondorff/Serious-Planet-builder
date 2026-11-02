//! The synthetic loads of the reserve test (BUDG-011, ADR 0083 point 3): one per reserved share at its full size, as calibrated work and never a sleep. Pure: the clock is
//! injected, so the calibration has tier-A tests on fake clocks; the scene (`planet`) runs the loads, the GPU burn pass lives in the renderer.

/// One reserved share at its full size per frame (BUDG-001 to BUDG-003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyntheticLoad {
    pub name: &'static str,
    /// Milliseconds of main-thread work per frame.
    pub main_ms: f64,
    /// Milliseconds of GPU work per frame, 0 for a share without GPU time.
    pub gpu_ms: f64,
    /// Cores kept busy for the whole run (the share's ceiling in BUDG-003).
    pub cores: usize,
}

/// The four reserved shares: sky and weather, physics, game logic, networking.
pub const LOADS: [SyntheticLoad; 4] = [
    SyntheticLoad { name: "sky", main_ms: 1.5, gpu_ms: 3.0, cores: 1 },
    SyntheticLoad { name: "physics", main_ms: 2.0, gpu_ms: 0.5, cores: 2 },
    SyntheticLoad { name: "game-logic", main_ms: 2.0, gpu_ms: 0.0, cores: 2 },
    SyntheticLoad { name: "networking", main_ms: 0.5, gpu_ms: 0.0, cores: 1 },
];

/// The main-thread milliseconds of all loads per frame (7.0 with the table above).
pub fn total_main_ms() -> f64 {
    LOADS.iter().map(|l| l.main_ms).sum()
}

/// The GPU milliseconds of all loads per frame (3.5).
pub fn total_gpu_ms() -> f64 {
    LOADS.iter().map(|l| l.gpu_ms).sum()
}

/// The cores of all loads (6).
pub fn total_cores() -> usize {
    LOADS.iter().map(|l| l.cores).sum()
}

/// A monotonic clock in nanoseconds.
pub trait Timer {
    fn now_ns(&self) -> u64;
}

/// The real clock.
pub struct RealTimer(std::time::Instant);

impl Default for RealTimer {
    fn default() -> Self {
        RealTimer(std::time::Instant::now())
    }
}

impl Timer for RealTimer {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// Fixed integer work: `iterations` rounds of a xorshift mix. The result is returned so that the loop cannot be optimised away.
pub fn spin(iterations: u64) -> u64 {
    let mut x = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..iterations {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
    }
    std::hint::black_box(x)
}

/// What a calibration found: how many iterations of the work take a millisecond on this machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    pub iterations_per_ms: f64,
}

impl Calibration {
    /// The iterations that take `ms` milliseconds.
    pub fn iterations_for(&self, ms: f64) -> u64 {
        (ms * self.iterations_per_ms).round().max(0.0) as u64
    }
}

/// Calibrate `work` (called with an iteration count) against `timer`: the median of `repeats` measurements, each of a run grown by doubling until it lasts at least
/// `min_ms`. `None` when the timer does not advance (a fake or a broken clock).
pub fn calibrate(timer: &dyn Timer, work: &dyn Fn(u64), min_ms: f64, repeats: usize) -> Option<Calibration> {
    let mut rates: Vec<f64> = Vec::new();
    let mut n = 1u64 << 12;
    for _ in 0..repeats.max(1) {
        let ms = loop {
            let t0 = timer.now_ns();
            work(n);
            let ms = timer.now_ns().saturating_sub(t0) as f64 / 1e6;
            if ms >= min_ms {
                break ms;
            }
            if n >= 1 << 40 {
                return None;
            }
            n = if ms <= 0.0 { n * 16 } else { (n as f64 * (min_ms / ms * 1.2).clamp(2.0, 16.0)) as u64 };
        };
        rates.push(n as f64 / ms);
    }
    rates.sort_by(f64::total_cmp);
    Some(Calibration { iterations_per_ms: rates[rates.len() / 2] })
}

/// Calibrate [`spin`] on the real clock (about 5 ms per measurement, five measurements).
pub fn calibrate_spin() -> Option<Calibration> {
    calibrate(
        &RealTimer::default(),
        &|n| {
            spin(n);
        },
        5.0,
        5,
    )
}

/// Terrain's main-thread share in milliseconds per frame (BUDG-001), its GPU share (BUDG-002) and its cores (BUDG-003), and the frame median (BUDG-001).
pub const TERRAIN_MAIN_MS: f64 = 10.0;
pub const TERRAIN_GPU_MS: f64 = 5.0;
pub const TERRAIN_CORES: f64 = 12.0;
pub const FRAME_MEDIAN_MS: f64 = 16.6;

/// One reading of the reserve scene against the limit of its share.
#[derive(Clone, Debug, PartialEq)]
pub struct ShareReading {
    pub share: String,
    pub resource: &'static str,
    pub measured: f64,
    pub limit: f64,
    /// Whether an overrun of this row fails the reserve verdict (terrain's shares and the frame, BUDG-012); the loads' own rows are listed for the record.
    pub decides: bool,
}

impl ShareReading {
    /// How far the reading is over its limit as a share of the limit (negative when under).
    pub fn overrun(&self) -> f64 {
        (self.measured - self.limit) / self.limit
    }

    pub fn over(&self) -> bool {
        self.measured > self.limit
    }
}

/// The reserve report of one flight with the loads on (BUDG-007, BUDG-012).
#[derive(Clone, Debug, PartialEq)]
pub struct ReserveReport {
    pub rows: Vec<ShareReading>,
}

impl ReserveReport {
    /// The report of terrain's measured main-thread ms, GPU ms, busy threads and the frame median, and the reserved loads' own readings.
    pub fn new(
        terrain_main_ms: f64,
        terrain_gpu_ms: f64,
        terrain_cores: f64,
        frame_median_ms: f64,
        loads: Vec<ShareReading>,
    ) -> ReserveReport {
        let row =
            |share: &str, resource, measured, limit| ShareReading { share: share.to_string(), resource, measured, limit, decides: true };
        let mut rows = vec![
            row("terrain", "main-thread ms", terrain_main_ms, TERRAIN_MAIN_MS),
            row("terrain", "GPU ms", terrain_gpu_ms, TERRAIN_GPU_MS),
            row("terrain", "cores", terrain_cores, TERRAIN_CORES),
            row("frame", "median ms", frame_median_ms, FRAME_MEDIAN_MS),
        ];
        rows.extend(loads);
        ReserveReport { rows }
    }

    /// Whether terrain stays inside its shares and the frame inside its median (BUDG-012).
    pub fn pass(&self) -> bool {
        !self.rows.iter().any(|r| r.decides && r.over())
    }

    /// The deciding row with the largest overrun, if any row is over.
    pub fn largest_overrun(&self) -> Option<&ShareReading> {
        self.rows.iter().filter(|r| r.decides && r.over()).max_by(|a, b| a.overrun().total_cmp(&b.overrun()))
    }

    /// The report lines: each share with its measured value, limit and verdict, then the verdict and the largest overrun named (BUDG-007).
    pub fn lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .rows
            .iter()
            .map(|r| {
                format!(
                    "  {} {}: {:.2} (limit {:.2}) {}{}",
                    r.share,
                    r.resource,
                    r.measured,
                    r.limit,
                    if r.over() { "OVER" } else { "ok" },
                    if r.decides { "" } else { " [load, not decisive]" }
                )
            })
            .collect();
        match self.largest_overrun() {
            Some(r) => out.push(format!(
                "reserve verdict: FAIL; largest overrun: {} {} at {:.2} against {:.2} ({:+.0} %)",
                r.share,
                r.resource,
                r.measured,
                r.limit,
                r.overrun() * 100.0
            )),
            None => {
                out.push("reserve verdict: pass (terrain inside its shares and the frame inside its median with all loads on)".to_string())
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A clock that advances `ns_per_iteration` for every iteration of the work and is read through `work`.
    struct Fake {
        now: Cell<u64>,
    }

    impl Timer for Fake {
        fn now_ns(&self) -> u64 {
            self.now.get()
        }
    }

    fn fake_calibration(ns_per_iteration: f64) -> Calibration {
        let clock = Fake { now: Cell::new(0) };
        let work = |n: u64| clock.now.set(clock.now.get() + (n as f64 * ns_per_iteration) as u64);
        calibrate(&clock, &work, 5.0, 5).expect("a clock that advances")
    }

    // spec: BUDG-011
    #[test]
    fn the_loads_are_the_four_reserved_shares_at_their_full_size() {
        assert_eq!(
            LOADS.map(|l| (l.name, l.main_ms, l.gpu_ms, l.cores)),
            [("sky", 1.5, 3.0, 1), ("physics", 2.0, 0.5, 2), ("game-logic", 2.0, 0.0, 2), ("networking", 0.5, 0.0, 1),]
        );
        assert_eq!((total_main_ms(), total_gpu_ms(), total_cores()), (6.0, 3.5, 6));
    }

    // spec: BUDG-011
    #[test]
    fn a_slower_machine_spins_fewer_iterations_for_the_same_milliseconds() {
        let base = fake_calibration(1000.0);
        let slow = fake_calibration(2000.0);
        let fast = fake_calibration(500.0);
        assert!((base.iterations_per_ms - 1000.0).abs() < 1.0, "{base:?}");
        assert!((slow.iterations_per_ms / base.iterations_per_ms - 0.5).abs() < 1e-3, "half speed, half the iterations: {slow:?}");
        assert!((fast.iterations_per_ms / base.iterations_per_ms - 2.0).abs() < 1e-3, "double speed, double: {fast:?}");
        assert_eq!(base.iterations_for(2.0), 2000);
        assert_eq!(slow.iterations_for(2.0), 1000);
        assert_eq!(base.iterations_for(-1.0), 0);
    }

    // spec: BUDG-011
    #[test]
    fn a_clock_that_never_advances_gives_no_calibration() {
        let clock = Fake { now: Cell::new(0) };
        assert_eq!(calibrate(&clock, &|_| {}, 5.0, 3), None);
    }

    // spec: BUDG-011
    #[test]
    #[ignore = "wall clock: cargo xtask perf wallclock"]
    fn the_real_spin_takes_its_milliseconds_within_five_percent() {
        // The median of many short spins on the real clock; a loaded machine gets three calibrations to land inside. A wall-clock check: it
        // runs with `cargo xtask perf wallclock` on the reference machine, not in tier A (it failed on a shared runner, SPIKE-84); the
        // calibration itself is tier A on fake clocks.
        let timer = RealTimer::default();
        let mut worst = f64::MAX;
        for _ in 0..3 {
            let cal = calibrate_spin().expect("the real clock advances");
            let target_ms = 2.0;
            let n = cal.iterations_for(target_ms);
            let mut ms: Vec<f64> = (0..41)
                .map(|_| {
                    let t0 = timer.now_ns();
                    spin(n);
                    (timer.now_ns() - t0) as f64 / 1e6
                })
                .collect();
            ms.sort_by(f64::total_cmp);
            let rel = (ms[ms.len() / 2] - target_ms).abs() / target_ms;
            worst = worst.min(rel);
            if rel <= 0.05 {
                return;
            }
        }
        panic!("the 2 ms spin was off by {:.1} % in the best of three calibrations (limit 5 %)", worst * 100.0);
    }

    fn load_row(name: &str, measured: f64) -> ShareReading {
        ShareReading { share: name.into(), resource: "main-thread ms", measured, limit: 2.0, decides: false }
    }

    // spec: BUDG-012, BUDG-007
    #[test]
    fn terrain_inside_its_shares_and_a_frame_under_16_6_ms_is_a_pass_listing_every_share() {
        let r = ReserveReport::new(8.5, 4.2, 12.0, 12.0, vec![load_row("physics", 2.4)]);
        assert!(r.pass() && r.largest_overrun().is_none(), "a load over its own share does not decide");
        let lines = r.lines();
        assert!(lines[0].starts_with("  terrain main-thread ms: 8.50 (limit 10.00) ok"), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("terrain cores: 12.00 (limit 12.00) ok")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("physics main-thread ms: 2.40 (limit 2.00) OVER [load, not decisive]")), "{lines:?}");
        assert!(lines.last().unwrap().starts_with("reserve verdict: pass"), "{lines:?}");
    }

    // spec: BUDG-012, BUDG-007
    #[test]
    fn an_overrun_fails_the_verdict_and_names_the_largest_one() {
        let r = ReserveReport::new(10.5, 4.0, 14.0, 17.0, vec![]);
        assert!(!r.pass());
        let big = r.largest_overrun().unwrap();
        assert_eq!((big.share.as_str(), big.resource), ("terrain", "cores"));
        let last = r.lines().pop().unwrap();
        assert!(last.contains("FAIL") && last.contains("terrain cores at 14.00 against 12.00 (+17 %)"), "{last}");
        // The frame alone over its median fails and is named.
        let r = ReserveReport::new(9.0, 4.0, 12.0, 18.0, vec![]);
        assert!(!r.pass() && r.largest_overrun().unwrap().share == "frame");
    }
}
