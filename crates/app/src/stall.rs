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
/// A lock wait shorter than this is not recorded (an uncontended acquire takes microseconds).
pub const WAIT_NOTED: Duration = Duration::from_millis(2);
/// At most one stall report in this time; the others are one line in the log.
pub const REPORT_EVERY: Duration = Duration::from_secs(10);

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
    last_report: Option<Instant>,
    /// Frames at or over [`STALL`] so far.
    pub stalls: u64,
}

impl Default for StallProbe {
    fn default() -> Self {
        let now = Instant::now();
        StallProbe { began: now, last: now, phases: Vec::new(), last_report: None, stalls: 0 }
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
}

impl StallProbe {
    /// A frame begins: the phases and the waits of the last one are dropped.
    pub fn begin(&mut self) {
        let now = Instant::now();
        self.began = now;
        self.last = now;
        self.phases.clear();
        WAITS.with(|w| w.borrow_mut().clear());
    }

    /// The phase `name` ended now: the time since the last mark is its.
    pub fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.phases.push((name, now - self.last));
        self.last = now;
    }

    /// The frame ended now. `Some` when it took at least [`STALL`]; whether a report is written is `report_due`'s.
    pub fn end(&mut self) -> Option<Stall> {
        self.end_at(Instant::now())
    }

    fn end_at(&mut self, now: Instant) -> Option<Stall> {
        let frame = now - self.began;
        if frame < STALL {
            return None;
        }
        self.stalls += 1;
        let mut phases = self.phases.clone();
        phases.sort_by_key(|p| std::cmp::Reverse(p.1));
        let mut waits = WAITS.with(|w| w.borrow().clone());
        waits.sort_by_key(|p| std::cmp::Reverse(p.1));
        Some(Stall { frame, phases, waits })
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
        let mut s = format!("a frame took {} (the stall limit is {})\nphases of the frame, longest first:\n", ms(self.frame), ms(STALL));
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
        let holder = {
            let m = std::sync::Arc::clone(&m);
            std::thread::Builder::new()
                .name("test-holder".into())
                .spawn(move || {
                    let g = m.lock().unwrap();
                    std::thread::sleep(Duration::from_millis(40));
                    drop(g);
                })
                .unwrap()
        };
        std::thread::sleep(Duration::from_millis(10));
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
