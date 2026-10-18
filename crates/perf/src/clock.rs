//! GPU clock state of a measured view (ADR 0034 as amended by ADR 0036): which views are valid for GPU figures (every clock
//! sample at or above [`GPU_CLOCK_FLOOR_MHZ`], whatever the P-state), what "equal clock state" means, and the warm-up loop
//! that brings the GPU to the floor. Pure functions over recorded samples; the render, the
//! sampler and the time source of the warm-up are closures, so tier A drives them with fakes.

use crate::ClockSample;
use std::fmt;

/// The graphics clock (MHz) every sample of a view must read for its GPU figures to count, whatever the P-state (ADR 0036
/// point 2, owner 2026-10-15). Measured on the reference GPU (loaded clocks 1,470-1,710 MHz, the coolest loaded start
/// 1,365 MHz); another reference machine needs its own measured floor (CON-21).
pub const GPU_CLOCK_FLOOR_MHZ: f64 = 1400.0;
/// Two graphics clocks are equal when they differ by at most this fraction of the larger one.
pub const CLOCK_TOLERANCE: f64 = 0.05;
/// How long the warm-up tries to read [`GPU_CLOCK_FLOOR_MHZ`] per view, in seconds (inclusive).
pub const WARMUP_CAP_S: f64 = 30.0;
/// Frames rendered between two readings of the state during the warm-up.
pub const WARMUP_CHUNK_FRAMES: usize = 60;
/// Consecutive readings at or above the floor that end the warm-up of the lit views (a clock that reads the floor once and falls
/// below it in the first run is not lifted: window 6, coast-2km, coast-3m and the flights).
pub const WARMUP_HOLD_READINGS: usize = 3;
/// The GPU temperature (Celsius) a view's warm-up starts at or below (ADR 0057). Window 7: the views that started at 54 to 59 C held
/// 1,455 to 1,567 MHz under the software power cap, those that started at 66 to 77 C read 1,282 to 1,380 MHz.
pub const COOL_START_C: f64 = 60.0;
/// How long the cool-down waits for [`COOL_START_C`], in seconds, and the pause between two temperature readings.
pub const COOL_CAP_S: f64 = 120.0;
pub const COOL_POLL_S: f64 = 5.0;
/// A warm-up chunk of the lit views renders at least this long (seconds), so the readings see a sustained load, not a burst.
pub const WARMUP_MIN_CHUNK_S: f64 = 1.0;

/// The part of a [`ClockSample`] the clock rules look at.
#[derive(Clone, Debug, PartialEq)]
pub struct ClockPoint {
    pub pstate: String,
    pub mhz: f64,
    /// The sample's `clocks_throttle_reasons.active` mask as `nvidia-smi` prints it; empty when the log does not carry it (then the
    /// power-cap rule of [`gpu_validity`] cannot apply).
    pub throttle: String,
}

impl From<&ClockSample> for ClockPoint {
    fn from(c: &ClockSample) -> Self {
        Self { pstate: c.pstate.clone(), mhz: c.graphics_mhz, throttle: c.throttle_reasons.clone() }
    }
}

/// The clock state of a view: its P-state and the median graphics clock of its samples.
#[derive(Clone, Debug, PartialEq)]
pub struct ClockState {
    pub pstate: String,
    pub mhz: f64,
}

impl fmt::Display for ClockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:.0} MHz", self.pstate, self.mhz)
    }
}

/// Whether a view's GPU figures (`gpu_ms`, the GPU budget) may be judged or compared; `frame_ms` is judged at any state.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuValidity {
    Valid(ClockState),
    /// Every sample was read and the GPU ran below the floor (ADR 0057): the figures are over-stated by the slow clock, so a figure
    /// within its budget is a pass and a figure over it is inconclusive. Carries the median state and the reason
    /// (`P2 1150 MHz, below the 1400 MHz floor`, the lowest sample). Comparisons need [`GpuValidity::Valid`].
    Below(ClockState, String),
    /// The reason names what is missing (`no clock sample`).
    Invalid(String),
}

impl GpuValidity {
    /// Valid for judging a figure over its budget and for comparing views (the clock reached the floor).
    pub fn is_valid(&self) -> bool {
        matches!(self, GpuValidity::Valid(_))
    }
}

/// The state in the words of a below-floor reason (`P0 1282 MHz, below the 1400 MHz floor`), as the logs carry them: the lowest
/// sample's state, or `None` for any other text.
pub fn parse_below_floor(text: &str) -> Option<ClockState> {
    if !text.ends_with(" MHz floor") {
        return None;
    }
    let head = text.split(", below the ").next().filter(|h| h.len() < text.len())?;
    let mut it = head.rsplitn(3, ' ');
    let (unit, mhz, pstate) = (it.next()?, it.next()?.parse::<f64>().ok()?, it.next()?);
    (unit == "MHz").then(|| ClockState { pstate: pstate.to_string(), mhz })
}

/// The clock state of a view from every sample taken before and after its runs (`None` is a missing sample). The P-state is
/// printed, not judged (`P0`, or `P0/P2` when the samples disagree); the clock is the median of the samples.
pub fn view_clock_state(points: &[Option<ClockPoint>]) -> Result<ClockState, String> {
    if points.is_empty() || points.iter().any(Option::is_none) {
        return Err("no clock sample".into());
    }
    let points: Vec<&ClockPoint> = points.iter().flatten().collect();
    let mut states: Vec<&str> = points.iter().map(|p| p.pstate.as_str()).collect();
    states.sort_unstable();
    states.dedup();
    let mut mhz: Vec<f64> = points.iter().map(|p| p.mhz).collect();
    mhz.sort_by(f64::total_cmp);
    let n = mhz.len();
    let median = if n % 2 == 1 { mhz[n / 2] } else { (mhz[n / 2 - 1] + mhz[n / 2]) / 2.0 };
    Ok(ClockState { pstate: states.join("/"), mhz: median })
}

/// Whether the clock below the floor is the laptop's sustained clock (ADR 0060): every sample reads P0, none reports a throttle
/// reason that changes the measurement (hardware slowdown, thermal, power brake, or one that cannot be read) and at least one reports
/// the software power cap (`0x4`), the only reason for the slow clock.
pub fn power_capped_p0(points: &[ClockPoint]) -> bool {
    !points.is_empty()
        && points.iter().all(|p| p.pstate == "P0" && !crate::gate::is_throttling(&p.throttle))
        && points.iter().any(|p| crate::gate::has_power_cap(&p.throttle))
}

/// ADR 0036 point 2: a view is valid for GPU figures only if every sample exists and reads at least
/// [`GPU_CLOCK_FLOOR_MHZ`]; the P-state does not matter (P2 at 1,710 MHz is valid, P0 at 1,200 MHz is not). One sample
/// below the floor, or an unreadable one, invalidates the view. ADR 0060: except a view whose samples all read P0 with the software
/// power cap as their only throttle reason ([`power_capped_p0`]), which is valid whatever its clock; below the floor in any other
/// state ADR 0057 point 2 applies (the figure within budget passes, over it is inconclusive).
pub fn gpu_validity(points: &[Option<ClockPoint>]) -> GpuValidity {
    let state = match view_clock_state(points) {
        Ok(s) => s,
        Err(e) => return GpuValidity::Invalid(e),
    };
    let all: Vec<ClockPoint> = points.iter().flatten().cloned().collect();
    if power_capped_p0(&all) && all.iter().all(|p| !p.mhz.is_nan()) {
        return GpuValidity::Valid(state);
    }
    // Any sample not at or above the floor (a NaN reads as below) invalidates; the reason names the lowest such sample.
    let low = points.iter().flatten().filter(|p| p.mhz.is_nan() || p.mhz < GPU_CLOCK_FLOOR_MHZ).min_by(|a, b| a.mhz.total_cmp(&b.mhz));
    let reason = |l: &ClockPoint| format!("{} {:.0} MHz, below the {GPU_CLOCK_FLOOR_MHZ:.0} MHz floor", l.pstate, l.mhz);
    match low {
        None => GpuValidity::Valid(state),
        // (a NaN clock is unreadable, not slow: it stays invalid)
        Some(l) if l.mhz.is_nan() => GpuValidity::Invalid(reason(l)),
        Some(l) => GpuValidity::Below(state, reason(l)),
    }
}

/// ADR 0036 point 2: graphics clocks within [`CLOCK_TOLERANCE`] of the larger; the P-state is printed, not compared. The
/// caller requires both views to be GPU-valid ([`gpu_validity`]) first.
pub fn equal_clock_state(a: &ClockState, b: &ClockState) -> bool {
    (a.mhz - b.mhz).abs() <= CLOCK_TOLERANCE * a.mhz.max(b.mhz)
}

/// What the warm-up of one view did.
#[derive(Clone, Debug, PartialEq)]
pub struct WarmUpOutcome {
    pub reached: bool,
    /// Consecutive readings at the floor that were required (1 for the terrain scene's warm-up).
    pub hold: usize,
    pub chunks: usize,
    pub seconds: f64,
    /// The last reading (`None`: the sampler gave nothing).
    pub last: Option<ClockPoint>,
}

impl WarmUpOutcome {
    /// The architect's line (ADR 0034 point 8, floor per ADR 0036).
    pub fn line(&self, view: &str) -> String {
        let last = self.last.as_ref().map_or("no reading".to_string(), |p| format!("{} {:.0} MHz", p.pstate, p.mhz));
        if self.reached {
            let held = if self.hold > 1 { format!(", held {} readings", self.hold) } else { String::new() };
            format!(
                "warm-up {view}: {last} (floor {GPU_CLOCK_FLOOR_MHZ:.0} MHz{held}) after {:.1} s ({} chunks)",
                self.seconds, self.chunks
            )
        } else {
            format!("warm-up {view}: still {last}, below the {GPU_CLOCK_FLOOR_MHZ:.0} MHz floor, after {:.1} s ({} chunks, cap {WARMUP_CAP_S} s)", self.seconds, self.chunks)
        }
    }
}

/// Render chunks and read the clock after each until it reads [`GPU_CLOCK_FLOOR_MHZ`] or `cap_s` seconds have passed (checked
/// after the reading, so a reading at the floor at exactly the cap counts). `now` returns seconds. At least one chunk is rendered.
pub fn warm_up(
    cap_s: f64,
    render_chunk: impl FnMut() -> Result<(), String>,
    sample: impl FnMut() -> Option<ClockPoint>,
    now: impl FnMut() -> f64,
) -> Result<WarmUpOutcome, String> {
    warm_up_held(cap_s, 1, render_chunk, sample, now)
}

/// [`warm_up`] that ends only after `hold` consecutive readings at the floor (a reading below the floor restarts the count), or at
/// the cap.
pub fn warm_up_held(
    cap_s: f64,
    hold: usize,
    mut render_chunk: impl FnMut() -> Result<(), String>,
    mut sample: impl FnMut() -> Option<ClockPoint>,
    mut now: impl FnMut() -> f64,
) -> Result<WarmUpOutcome, String> {
    let hold = hold.max(1);
    let start = now();
    let mut chunks = 0;
    let mut run = 0;
    loop {
        render_chunk()?;
        chunks += 1;
        let last = sample();
        let seconds = now() - start;
        if last.as_ref().is_some_and(|p| p.mhz >= GPU_CLOCK_FLOOR_MHZ) {
            run += 1;
        } else {
            run = 0;
        }
        let reached = run >= hold;
        if reached || seconds >= cap_s {
            return Ok(WarmUpOutcome { reached, hold, chunks, seconds, last });
        }
    }
}

/// What the cool-down before a view's warm-up did.
#[derive(Clone, Debug, PartialEq)]
pub struct CoolOutcome {
    /// The temperature was at or below the target at the last reading.
    pub cooled: bool,
    pub start_c: Option<f64>,
    pub end_c: Option<f64>,
    pub seconds: f64,
}

impl CoolOutcome {
    pub fn line(&self, view: &str) -> String {
        let t = |v: Option<f64>| v.map_or("unreadable".to_string(), |v| format!("{v:.0} C"));
        if self.cooled {
            format!("cool-down {view}: {} -> {} after {:.0} s (target {COOL_START_C:.0} C)", t(self.start_c), t(self.end_c), self.seconds)
        } else {
            format!(
                "cool-down {view}: NOT cooled, {} -> {} after {:.0} s (target {COOL_START_C:.0} C, cap {COOL_CAP_S:.0} s)",
                t(self.start_c),
                t(self.end_c),
                self.seconds
            )
        }
    }
}

/// Wait, idle, until the GPU temperature reads `target_c` or less, or `cap_s` seconds have passed (checked after each reading, a
/// reading at the target counts). `sample` returns the temperature (`None`: unreadable, which ends the wait at once: there is
/// nothing to wait on), `pause` sleeps [`COOL_POLL_S`] seconds, `now` returns seconds.
pub fn cool_down(
    target_c: f64,
    cap_s: f64,
    mut sample: impl FnMut() -> Option<f64>,
    mut pause: impl FnMut(),
    mut now: impl FnMut() -> f64,
) -> CoolOutcome {
    let start = now();
    let mut start_c = None;
    loop {
        let reading = sample();
        let seconds = now() - start;
        if start_c.is_none() {
            start_c = reading;
        }
        match reading {
            None => return CoolOutcome { cooled: false, start_c, end_c: None, seconds },
            Some(c) if c <= target_c => return CoolOutcome { cooled: true, start_c, end_c: Some(c), seconds },
            Some(c) if seconds >= cap_s => return CoolOutcome { cooled: false, start_c, end_c: Some(c), seconds },
            Some(_) => pause(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pstate: &str, mhz: f64) -> Option<ClockPoint> {
        Some(ClockPoint { pstate: pstate.into(), mhz, throttle: String::new() })
    }

    fn pm(pstate: &str, mhz: f64, mask: &str) -> Option<ClockPoint> {
        Some(ClockPoint { pstate: pstate.into(), mhz, throttle: mask.into() })
    }

    // spec: TEST-034
    #[test]
    fn a_power_capped_p0_view_is_valid_whatever_its_clock_and_nothing_else_below_the_floor_is() {
        const CAP: &str = "0x0000000000000004";
        // ADR 0060: P0 below the floor with the software power cap as the only throttle reason (window 9's coast views) is valid.
        let capped = gpu_validity(&[pm("P0", 1245.0, CAP), pm("P0", 1290.0, CAP), pm("P0", 1365.0, CAP)]);
        assert_eq!(capped, GpuValidity::Valid(ClockState { pstate: "P0".into(), mhz: 1290.0 }));
        // The cap is still the only reason when idle and applications-clock bits ride along.
        assert!(gpu_validity(&[pm("P0", 1300.0, "0x0000000000000005"), pm("P0", 1300.0, CAP)]).is_valid());
        // A thermal, hardware-slowdown or power-brake reason beside the cap: not valid (ADR 0057 point 2 applies).
        for bad in ["0x000000000000000c", "0x0000000000000024", "0x0000000000000044", "0x0000000000000084"] {
            let v = gpu_validity(&[pm("P0", 1290.0, CAP), pm("P0", 1290.0, bad)]);
            assert!(matches!(v, GpuValidity::Below(..)), "{bad}: {v:?}");
        }
        // An unreadable mask is an unknown reason, not the cap.
        assert!(matches!(gpu_validity(&[pm("P0", 1290.0, CAP), pm("P0", 1290.0, "[Not Supported]")]), GpuValidity::Below(..)));
        // Any other state: P2 below the floor with the cap, or one P2 sample among P0 ones, is not valid.
        assert!(matches!(gpu_validity(&[pm("P2", 1290.0, CAP), pm("P2", 1290.0, CAP)]), GpuValidity::Below(..)));
        assert!(matches!(gpu_validity(&[pm("P0", 1290.0, CAP), pm("P2", 1290.0, CAP)]), GpuValidity::Below(..)));
        // P0 below the floor with no throttle reason at all (nothing explains the slow clock) is not valid either.
        assert!(matches!(gpu_validity(&[pm("P0", 1290.0, "0x0000000000000000")]), GpuValidity::Below(..)));
        // A missing sample is still a missing sample, and a log without masks keeps the ADR 0057 verdict.
        assert!(matches!(gpu_validity(&[pm("P0", 1290.0, CAP), None]), GpuValidity::Invalid(_)));
        assert!(matches!(gpu_validity(&[p("P0", 1290.0)]), GpuValidity::Below(..)));
        // At or above the floor nothing changes: the cap does not matter.
        assert!(gpu_validity(&[pm("P0", 1470.0, CAP)]).is_valid());
    }

    // spec: TEST-012
    #[test]
    fn a_view_is_valid_for_gpu_figures_when_every_sample_reads_the_floor_whatever_the_pstate() {
        // P0 at a loaded clock, and P2 at 1,710 MHz (the light views of the 2026-10-15 window): both valid.
        let ok = gpu_validity(&[p("P0", 1470.0), p("P0", 1500.0), p("P0", 1450.0), p("P0", 1470.0)]);
        assert_eq!(ok, GpuValidity::Valid(ClockState { pstate: "P0".into(), mhz: 1470.0 }));
        let p2 = gpu_validity(&[p("P2", 1710.0), p("P2", 1710.0)]);
        assert_eq!(p2, GpuValidity::Valid(ClockState { pstate: "P2".into(), mhz: 1710.0 }));
        // Exactly the floor is valid (inclusive); the floor is the owner's 1,400 MHz.
        assert_eq!(GPU_CLOCK_FLOOR_MHZ, 1400.0);
        assert!(gpu_validity(&[p("P0", 1400.0)]).is_valid());
        // Below the floor is not, in P0 as in P2.
        assert_eq!(
            gpu_validity(&[p("P2", 1200.0), p("P2", 1200.0)]),
            GpuValidity::Below(ClockState { pstate: "P2".into(), mhz: 1200.0 }, "P2 1200 MHz, below the 1400 MHz floor".into())
        );
        assert!(!gpu_validity(&[p("P0", 1399.0)]).is_valid());
        // Mixed samples: one below the floor invalidates, and the reason names the lowest one.
        assert_eq!(
            gpu_validity(&[p("P0", 1470.0), p("P2", 1150.0), p("P0", 1470.0)]),
            GpuValidity::Below(ClockState { pstate: "P0/P2".into(), mhz: 1470.0 }, "P2 1150 MHz, below the 1400 MHz floor".into())
        );
        // Mixed P-states, all at or above the floor: valid, the states are printed together.
        let mixed = gpu_validity(&[p("P0", 1470.0), p("P2", 1710.0)]);
        assert_eq!(mixed, GpuValidity::Valid(ClockState { pstate: "P0/P2".into(), mhz: 1590.0 }));
        // A missing sample is unknown, never valid; no samples at all neither; an unreadable (NaN) clock neither.
        assert_eq!(gpu_validity(&[p("P0", 1470.0), None]), GpuValidity::Invalid("no clock sample".into()));
        assert_eq!(gpu_validity(&[]), GpuValidity::Invalid("no clock sample".into()));
        assert!(!gpu_validity(&[p("P0", f64::NAN)]).is_valid());
        assert!(!gpu_validity(&[p("P0", 1470.0), p("P0", f64::NAN)]).is_valid(), "one unreadable clock among good ones invalidates");
    }

    // spec: TEST-012
    #[test]
    fn equal_clock_state_is_clocks_within_five_percent_and_the_pstate_is_not_compared() {
        let s = |pstate: &str, mhz: f64| ClockState { pstate: pstate.into(), mhz };
        assert!(equal_clock_state(&s("P0", 1500.0), &s("P0", 1425.0)), "exactly 5 % of the larger is equal");
        assert!(!equal_clock_state(&s("P0", 1500.0), &s("P0", 1424.0)));
        assert!(equal_clock_state(&s("P0", 1470.0), &s("P2", 1500.0)), "P0 against P2 at close clocks is equal");
        assert!(!equal_clock_state(&s("P2", 1710.0), &s("P0", 1470.0)), "14 % apart is not equal in any P-state");
        assert_eq!(s("P0", 1395.4).to_string(), "P0 1395 MHz");
    }

    // spec: TEST-012
    #[test]
    fn the_warm_up_stops_at_the_first_reading_at_the_floor_in_any_pstate() {
        let mut readings = vec![p("P2", 1150.0), p("P0", 1300.0), p("P2", 1710.0), p("P0", 1470.0)].into_iter();
        let mut t = 0.0;
        let mut rendered = 0;
        let out = warm_up(
            30.0,
            || {
                rendered += 1;
                Ok(())
            },
            || readings.next().unwrap(),
            || {
                t += 1.5;
                t
            },
        )
        .unwrap();
        assert!(out.reached && out.chunks == 3 && rendered == 3, "{out:?}");
        assert_eq!(out.last, p("P2", 1710.0));
        assert!(out.line("ground-1m").starts_with("warm-up ground-1m: P2 1710 MHz (floor 1400 MHz) after"), "{}", out.line("ground-1m"));
    }

    // spec: TEST-029
    #[test]
    fn a_held_warm_up_needs_consecutive_readings_at_the_floor_and_a_dip_restarts_the_count() {
        // The window-6 pattern: the floor is read once, the clock falls below it under the sustained load, then holds.
        let mut readings =
            vec![p("P0", 1702.0), p("P0", 1327.0), p("P0", 1450.0), p("P0", 1420.0), p("P0", 1410.0), p("P0", 1500.0)].into_iter();
        let mut t = 0.0;
        let out = warm_up_held(
            30.0,
            3,
            || Ok(()),
            || readings.next().unwrap(),
            || {
                t += 1.0;
                t
            },
        )
        .unwrap();
        assert!(out.reached && out.hold == 3 && out.chunks == 5, "the dip at reading 2 restarts the count: {out:?}");
        assert!(out.line("coast-3m").contains("held 3 readings"), "{}", out.line("coast-3m"));
        // At the cap with the clock never holding, the outcome says not reached.
        let mut t = 0.0;
        let mut flip = 0;
        let out = warm_up_held(
            10.0,
            3,
            || Ok(()),
            || {
                flip += 1;
                if flip % 2 == 0 {
                    p("P0", 1300.0)
                } else {
                    p("P0", 1500.0)
                }
            },
            || {
                t += 1.0;
                t
            },
        )
        .unwrap();
        assert!(!out.reached && out.seconds >= 10.0, "{out:?}");
        // The one-reading warm-up is the held warm-up with hold 1.
        assert_eq!(WARMUP_HOLD_READINGS, 3);
    }

    // spec: TEST-012
    #[test]
    fn the_warm_up_gives_up_at_the_cap_and_reports_the_state_it_measured() {
        let mut t = 0.0;
        let out = warm_up(
            30.0,
            || Ok(()),
            || p("P2", 1150.0),
            || {
                t += 10.0;
                t
            },
        )
        .unwrap();
        // The clock reads 10 (start), 20, 30, 40: elapsed 10, 20, 30 after three chunks, and 30 is the cap (inclusive).
        assert!(!out.reached && out.chunks == 3 && out.seconds >= 30.0, "{out:?}");
        let line = out.line("ground-1m");
        assert!(line.contains("still P2 1150 MHz, below the 1400 MHz floor") && line.contains("cap 30"), "{line}");
        // A sampler that never answers is a failure to reach P0, not a pass.
        let mut t = 0.0;
        let none = warm_up(
            5.0,
            || Ok(()),
            || None,
            || {
                t += 5.0;
                t
            },
        )
        .unwrap();
        assert!(!none.reached && none.line("v").contains("no reading"));
    }

    // spec: TEST-012
    #[test]
    fn a_reading_at_the_floor_at_exactly_the_cap_counts_and_a_render_error_stops_the_warm_up() {
        let mut t = -10.0;
        let out = warm_up(
            10.0,
            || Ok(()),
            || p("P0", 1400.0),
            || {
                t += 10.0;
                t
            },
        )
        .unwrap();
        assert!(out.reached && out.seconds == 10.0, "{out:?}");
        let err = warm_up(30.0, || Err("lost device".into()), || None, || 0.0).unwrap_err();
        assert_eq!(err, "lost device");
    }
    // spec: TEST-033
    #[test]
    fn the_cool_down_waits_for_the_target_in_polls_and_stops_at_the_cap_or_an_unreadable_sensor() {
        // 74 C falls 5 C a poll: 74, 69, 64, 59 -> cooled at the third pause.
        let mut temps = vec![Some(74.0), Some(69.0), Some(64.0), Some(59.0)].into_iter();
        let (t, pauses) = (std::cell::Cell::new(0.0), std::cell::Cell::new(0));
        let out = cool_down(
            60.0,
            120.0,
            || temps.next().unwrap(),
            || {
                pauses.set(pauses.get() + 1);
                t.set(t.get() + COOL_POLL_S);
            },
            || t.get(),
        );
        assert!(out.cooled && pauses.get() == 3 && out.start_c == Some(74.0) && out.end_c == Some(59.0), "{out:?}");
        assert_eq!(out.seconds, 15.0);
        assert!(out.line("coast-2km").starts_with("cool-down coast-2km: 74 C -> 59 C after "), "{}", out.line("coast-2km"));
        // Already cool: no pause, no wait.
        let mut pauses = 0;
        let out = cool_down(60.0, 120.0, || Some(55.0), || pauses += 1, || 0.0);
        assert!(out.cooled && pauses == 0 && out.seconds == 0.0, "{out:?}");
        // A GPU that stays hot gives up at the cap, and the line says NOT cooled with the cap.
        let mut t = 0.0;
        let out = cool_down(
            60.0,
            120.0,
            || Some(72.0),
            || {},
            || {
                t += 40.0;
                t
            },
        );
        assert!(!out.cooled && out.seconds >= 120.0 && out.end_c == Some(72.0), "{out:?}");
        let line = out.line("orbit");
        assert!(line.contains("NOT cooled") && line.contains("cap 120 s"), "{line}");
        // An unreadable sensor ends the wait at once.
        let mut pauses = 0;
        let out = cool_down(60.0, 120.0, || None, || pauses += 1, || 0.0);
        assert!(!out.cooled && pauses == 0 && out.end_c.is_none() && out.line("v").contains("unreadable"), "{out:?}");
        // The target is the temperature the held views of window 7 started at.
        assert_eq!((COOL_START_C, COOL_CAP_S, COOL_POLL_S), (60.0, 120.0, 5.0));
    }

    // spec: TEST-034
    #[test]
    fn a_below_floor_state_is_read_back_from_the_words_the_logs_carry_and_a_nan_clock_stays_invalid() {
        let st = parse_below_floor("P0 1282 MHz, below the 1400 MHz floor").unwrap();
        assert_eq!((st.pstate.as_str(), st.mhz), ("P0", 1282.0));
        assert!(parse_below_floor("P2 1710 MHz").is_none() && parse_below_floor("no clock sample").is_none());
        assert!(parse_below_floor("P5 840 MHz, below the 1400 MHz floor").is_some());
        assert!(matches!(gpu_validity(&[p("P0", f64::NAN)]), GpuValidity::Invalid(_)));
        assert!(!gpu_validity(&[p("P0", 1399.0)]).is_valid(), "Below is not valid for comparisons");
    }
}
