//! The stall probe (VIEW-056): times the phases of the window's frame and the locks the main thread waits on, so that a frame that takes a
//! second or more names its culprit in a report (`target/review/crashes`, "stall (the window went on)") and the title says where it is.
//!
//! The probe decides nothing and reads only the clock: it never changes what a frame does. Phases are marked in the order the frame runs them
//! (`mark` attributes the time since the previous mark to the phase just ended); the lock waits are recorded by the timed acquires of this
//! module (`read`, `write`, `lock`) in a thread-local of the thread that holds the probe, so the waits of worker threads are not counted.

use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

/// A frame at least this long is a stall.
pub const STALL: Duration = Duration::from_secs(1);

/// The stall limit of this run: [`STALL`], or `PLANET_STALL_MS` milliseconds when the variable is set (a probe of frames shorter than a second; it
/// decides nothing but which frames are reported).
pub fn limit() -> Duration {
    static LIMIT: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| std::env::var("PLANET_STALL_MS").ok().and_then(|v| v.parse::<u64>().ok()).map_or(STALL, Duration::from_millis))
}
/// A lock wait shorter than this is not recorded (an uncontended acquire takes microseconds).
pub const WAIT_NOTED: Duration = Duration::from_millis(2);
/// At most one stall report in this time; the others are one line in the log.
pub const REPORT_EVERY: Duration = Duration::from_secs(10);

/// The start of a run's phase clock and its marks (milliseconds since the first mark): where a run spends its startup (the process, the event loop, the world, the device,
/// the floor tiles, the frames). Recorded always (a few pushes), printed only under `PLANET_TIMING` by [`phase_report`].
static PHASE_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static PHASE_MARKS: Mutex<Vec<(&'static str, f64)>> = Mutex::new(Vec::new());

/// A phase of the run is reached now.
pub fn phase_mark(label: &'static str) {
    let ms = PHASE_START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3;
    if let Ok(mut m) = PHASE_MARKS.lock() {
        m.push((label, ms));
    }
}

/// The marks as one line, each with the time since the previous one: `world 340 ms, device 1500 ms, ...`.
pub fn phase_report() -> String {
    let marks = PHASE_MARKS.lock().map(|m| m.clone()).unwrap_or_default();
    let mut prev = 0.0;
    let mut parts: Vec<String> = Vec::new();
    for (label, at) in &marks {
        parts.push(format!("{label} +{:.0} ms", at - prev));
        prev = *at;
    }
    format!("{} (total {:.0} ms since the first mark)", parts.join(", "), prev)
}

thread_local! {
    static WAITS: RefCell<Vec<(&'static str, Duration)>> = const { RefCell::new(Vec::new()) };
}

/// Note that the current thread waited `waited` for the lock `name` (kept when it is long enough to matter).
pub fn waited(name: &'static str, waited: Duration) {
    if waited >= WAIT_NOTED {
        WAITS.with(|w| w.borrow_mut().push((name, waited)));
    }
}

/// `RwLock::read` timed under `name`.
pub fn read<'a, T>(lock: &'a RwLock<T>, name: &'static str) -> RwLockReadGuard<'a, T> {
    let t = Instant::now();
    let g = lock.read().expect(name);
    waited(name, t.elapsed());
    g
}

/// `RwLock::write` timed under `name`.
pub fn write<'a, T>(lock: &'a RwLock<T>, name: &'static str) -> RwLockWriteGuard<'a, T> {
    let t = Instant::now();
    let g = lock.write().expect(name);
    waited(name, t.elapsed());
    g
}

/// `Mutex::lock` timed under `name`.
pub fn lock<'a, T>(mutex: &'a Mutex<T>, name: &'static str) -> MutexGuard<'a, T> {
    let t = Instant::now();
    let g = mutex.lock().expect(name);
    waited(name, t.elapsed());
    g
}

/// The phases of the frame in progress.
#[derive(Debug)]
pub struct StallProbe {
    began: Instant,
    last: Instant,
    phases: Vec<(&'static str, Duration)>,
    /// The blocks the main thread decoded itself in each phase (parallel to `phases`; a phase beyond the list decoded none): a block decode on the
    /// main thread is a stall the warm-up thread should have taken (VIEW-059, CON-22).
    phase_decodes: Vec<u64>,
    last_decodes: u64,
    last_report: Option<Instant>,
    /// Frames at or over [`STALL`] so far.
    pub stalls: u64,
    /// Per phase over all frames: the milliseconds summed, the longest, and the frames; printed when the probe ends with `PLANET_STALL_MS` set.
    totals: Vec<(&'static str, f64, f64)>,
    frames: u64,
    last_totals: Instant,
}

impl StallProbe {
    /// With `PLANET_STALL_MS` set, the mean and the longest time of every phase over the run so far, every ten seconds (a probe of where frames below
    /// the limit go too).
    fn print_totals(&mut self, now: Instant) {
        if self.frames == 0
            || now.duration_since(self.last_totals) < Duration::from_secs(10)
            || std::env::var_os("PLANET_STALL_MS").is_none()
        {
            return;
        }
        self.last_totals = now;
        let mut rows = self.totals.clone();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        eprintln!("frame phases over {} frames (mean / longest ms):", self.frames);
        for (name, sum, max) in rows {
            eprintln!("  {name}: {:.2} / {max:.1}", sum / self.frames as f64);
        }
    }
}

impl Default for StallProbe {
    fn default() -> Self {
        let now = Instant::now();
        StallProbe {
            began: now,
            last: now,
            phases: Vec::new(),
            phase_decodes: Vec::new(),
            last_decodes: 0,
            last_report: None,
            stalls: 0,
            totals: Vec::new(),
            frames: 0,
            last_totals: now,
        }
    }
}

/// What a finished frame says when it stalled.
#[derive(Clone, Debug, PartialEq)]
pub struct Stall {
    pub frame: Duration,
    /// The phases, longest first.
    pub phases: Vec<(&'static str, Duration)>,
    /// The lock waits of the main thread, longest first.
    pub waits: Vec<(&'static str, Duration)>,
    /// The phases in which the main thread decoded a bake block itself, with the count (in the order the frame ran them).
    pub decodes: Vec<(&'static str, u64)>,
}

impl StallProbe {
    /// A frame begins: the phases and the waits of the last one are dropped.
    pub fn begin(&mut self) {
        let now = Instant::now();
        self.began = now;
        self.last = now;
        self.phases.clear();
        self.phase_decodes.clear();
        self.last_decodes = planet_cache::global_store::thread_block_decodes();
        WAITS.with(|w| w.borrow_mut().clear());
    }

    /// The phases of the frame that ended last, in the order they ran, in milliseconds (the gate's frame time, TEST-061).
    pub fn phases_ms(&self) -> Vec<(&'static str, f64)> {
        self.phases.iter().map(|(n, d)| (*n, d.as_secs_f64() * 1e3)).collect()
    }

    /// The blocks the main thread decoded in each phase of the frame that ended last, phases with none left out.
    pub fn main_decodes(&self) -> Vec<(&'static str, u64)> {
        self.phases.iter().zip(&self.phase_decodes).filter(|(_, d)| **d > 0).map(|((n, _), d)| (*n, *d)).collect()
    }

    /// The phase `name` ended now: the time since the last mark is its.
    pub fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.phases.push((name, now - self.last));
        let decodes = planet_cache::global_store::thread_block_decodes();
        self.phase_decodes.push(decodes - self.last_decodes);
        self.last_decodes = decodes;
        self.last = now;
    }

    /// The frame ended now. `Some` when it took at least [`STALL`]; whether a report is written is `report_due`'s.
    pub fn end(&mut self) -> Option<Stall> {
        self.end_at(Instant::now())
    }

    fn end_at(&mut self, now: Instant) -> Option<Stall> {
        let frame = now - self.began;
        self.frames += 1;
        self.print_totals(now);
        for (name, d) in &self.phases {
            let ms = d.as_secs_f64() * 1e3;
            match self.totals.iter_mut().find(|t| t.0 == *name) {
                Some(t) => {
                    t.1 += ms;
                    t.2 = t.2.max(ms);
                }
                None => self.totals.push((*name, ms, ms)),
            }
        }
        if frame < limit() {
            return None;
        }
        self.stalls += 1;
        let mut phases = self.phases.clone();
        phases.sort_by_key(|p| std::cmp::Reverse(p.1));
        let mut waits = WAITS.with(|w| w.borrow().clone());
        waits.sort_by_key(|p| std::cmp::Reverse(p.1));
        let decodes = self.main_decodes();
        Some(Stall { frame, phases, waits, decodes })
    }

    /// Whether a report is due for a stall that ended now (at most one in [`REPORT_EVERY`]); it counts the stall it says yes to.
    pub fn report_due(&mut self) -> bool {
        self.report_due_at(Instant::now())
    }

    fn report_due_at(&mut self, now: Instant) -> bool {
        match self.last_report {
            Some(t) if now.duration_since(t) < REPORT_EVERY => false,
            _ => {
                self.last_report = Some(now);
                true
            }
        }
    }
}

impl Stall {
    /// The text of the report: the frame, the phases and the lock waits, and what the caller adds (jobs in flight, the tile counts).
    pub fn text(&self, extra: &str) -> String {
        let ms = |d: Duration| format!("{:.0} ms", d.as_secs_f64() * 1e3);
        let mut s = format!("a frame took {} (the stall limit is {})\nphases of the frame, longest first:\n", ms(self.frame), ms(limit()));
        for (name, d) in &self.phases {
            s += &format!("  {name}: {}\n", ms(*d));
        }
        if self.waits.is_empty() {
            s += "locks the main thread waited on (2 ms or more): none\n";
        } else {
            s += "locks the main thread waited on (2 ms or more), longest first:\n";
            for (name, d) in &self.waits {
                s += &format!("  {name}: {}\n", ms(*d));
            }
        }
        for (name, n) in &self.decodes {
            s += &format!(
                "blocks the main thread decoded in {name}: {n}
"
            );
        }
        if !extra.is_empty() {
            s += extra;
            if !extra.ends_with('\n') {
                s.push('\n');
            }
        }
        s
    }

    /// The notice for the title: the frame time and the longest phase.
    pub fn notice(&self) -> String {
        let (name, d) = self.phases.first().copied().unwrap_or(("(no phase)", Duration::ZERO));
        format!("stall: a frame took {:.1} s, most of it in {name} ({:.1} s)", self.frame.as_secs_f64(), d.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_with(phases: &[(&'static str, u64)], began_ago_ms: u64) -> (StallProbe, Instant) {
        let now = Instant::now();
        let mut p = StallProbe::default();
        p.begin();
        p.began = now - Duration::from_millis(began_ago_ms);
        p.phases = phases.iter().map(|(n, ms)| (*n, Duration::from_millis(*ms))).collect();
        (p, now)
    }

    // spec: VIEW-056
    #[test]
    fn a_frame_under_a_second_is_no_stall_and_a_longer_one_lists_its_phases_longest_first() {
        let (mut p, now) = probe_with(&[("plan", 20), ("chunks", 900), ("render", 300)], 400);
        assert_eq!(p.end_at(now), None, "400 ms is no stall");
        let (mut p, now) = probe_with(&[("plan", 20), ("chunks", 900), ("render", 300)], 1_300);
        let s = p.end_at(now).expect("1.3 s is a stall");
        assert_eq!(s.phases.iter().map(|p| p.0).collect::<Vec<_>>(), ["chunks", "render", "plan"]);
        assert_eq!(p.stalls, 1);
        let text = s.text("jobs in flight: 3\n");
        assert!(text.contains("a frame took 1300 ms") && text.contains("chunks: 900 ms") && text.contains("jobs in flight: 3"), "{text}");
        assert!(text.contains("locks the main thread waited on (2 ms or more): none"), "{text}");
        assert!(s.notice().contains("most of it in chunks"), "{}", s.notice());
    }

    // spec: VIEW-056
    #[test]
    fn a_lock_the_main_thread_waits_on_is_named_and_a_short_wait_is_not_recorded() {
        let (mut p, _) = probe_with(&[("render", 1_500)], 1_500);
        p.begin();
        p.began -= Duration::from_millis(1_500);
        p.phases = vec![("render", Duration::from_millis(1_500))];
        let m = std::sync::Arc::new(Mutex::new(0));
        // A quick acquire records nothing.
        drop(lock(&m, "quick"));
        // A held lock makes the main thread wait for it: another thread holds it for 40 ms.
        let (held, is_held) = std::sync::mpsc::channel();
        let holder = {
            let m = std::sync::Arc::clone(&m);
            std::thread::Builder::new()
                .name("test-holder".into())
                .spawn(move || {
                    let g = m.lock().unwrap();
                    held.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(40));
                    drop(g);
                })
                .unwrap()
        };
        // The main thread asks for the lock only once the other holds it, whatever the load of the machine.
        is_held.recv().unwrap();
        drop(lock(&m, "the held lock"));
        holder.join().unwrap();
        let s = p.end().expect("a stall");
        assert_eq!(s.waits.len(), 1, "{:?}", s.waits);
        assert_eq!(s.waits[0].0, "the held lock");
        assert!(s.waits[0].1 >= Duration::from_millis(20), "{:?}", s.waits[0].1);
        assert!(s.text("").contains("the held lock"));
        // The next frame starts clean.
        p.begin();
        assert!(p.end().is_none());
        let rw = RwLock::new(1);
        drop(read(&rw, "r"));
        drop(write(&rw, "w"));
    }

    // spec: VIEW-056
    #[test]
    fn at_most_one_report_in_ten_seconds() {
        let mut p = StallProbe::default();
        let t = Instant::now();
        assert!(p.report_due_at(t));
        assert!(!p.report_due_at(t + Duration::from_secs(9)));
        assert!(p.report_due_at(t + Duration::from_secs(10)));
    }
}
