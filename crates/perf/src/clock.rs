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

/// The part of a [`ClockSample`] the clock rules look at.
#[derive(Clone, Debug, PartialEq)]
pub struct ClockPoint {
    pub pstate: String,
    pub mhz: f64,
}

impl From<&ClockSample> for ClockPoint {
    fn from(c: &ClockSample) -> Self {
        Self { pstate: c.pstate.clone(), mhz: c.graphics_mhz }
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
    /// The reason names the measured state (`P2 1150 MHz`, `mixed P0/P2`, `no clock sample`).
    Invalid(String),
}

impl GpuValidity {
    pub fn is_valid(&self) -> bool {
        matches!(self, GpuValidity::Valid(_))
    }
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

/// ADR 0036 point 2: a view is valid for GPU figures only if every sample exists and reads at least
/// [`GPU_CLOCK_FLOOR_MHZ`]; the P-state does not matter (P2 at 1,710 MHz is valid, P0 at 1,200 MHz is not). One sample
/// below the floor, or an unreadable one, invalidates the view.
pub fn gpu_validity(points: &[Option<ClockPoint>]) -> GpuValidity {
    let state = match view_clock_state(points) {
        Ok(s) => s,
        Err(e) => return GpuValidity::Invalid(e),
    };
    // Any sample not at or above the floor (a NaN reads as below) invalidates; the reason names the lowest such sample.
    let low = points.iter().flatten().filter(|p| p.mhz.is_nan() || p.mhz < GPU_CLOCK_FLOOR_MHZ).min_by(|a, b| a.mhz.total_cmp(&b.mhz));
    match low {
        None => GpuValidity::Valid(state),
        Some(l) => GpuValidity::Invalid(format!("{} {:.0} MHz, below the {GPU_CLOCK_FLOOR_MHZ:.0} MHz floor", l.pstate, l.mhz)),
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
            format!("warm-up {view}: {last} (floor {GPU_CLOCK_FLOOR_MHZ:.0} MHz) after {:.1} s ({} chunks)", self.seconds, self.chunks)
        } else {
            format!("warm-up {view}: still {last}, below the {GPU_CLOCK_FLOOR_MHZ:.0} MHz floor, after {:.1} s ({} chunks, cap {WARMUP_CAP_S} s)", self.seconds, self.chunks)
        }
    }
}

/// Render chunks and read the clock after each until it reads [`GPU_CLOCK_FLOOR_MHZ`] or `cap_s` seconds have passed (checked
/// after the reading, so a reading at the floor at exactly the cap counts). `now` returns seconds. At least one chunk is rendered.
pub fn warm_up(
    cap_s: f64,
    mut render_chunk: impl FnMut() -> Result<(), String>,
    mut sample: impl FnMut() -> Option<ClockPoint>,
    mut now: impl FnMut() -> f64,
) -> Result<WarmUpOutcome, String> {
    let start = now();
    let mut chunks = 0;
    loop {
        render_chunk()?;
        chunks += 1;
        let last = sample();
        let seconds = now() - start;
        let reached = last.as_ref().is_some_and(|p| p.mhz >= GPU_CLOCK_FLOOR_MHZ);
        if reached || seconds >= cap_s {
            return Ok(WarmUpOutcome { reached, chunks, seconds, last });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pstate: &str, mhz: f64) -> Option<ClockPoint> {
        Some(ClockPoint { pstate: pstate.into(), mhz })
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
        assert_eq!(gpu_validity(&[p("P2", 1200.0), p("P2", 1200.0)]), GpuValidity::Invalid("P2 1200 MHz, below the 1400 MHz floor".into()));
        assert!(!gpu_validity(&[p("P0", 1399.0)]).is_valid());
        // Mixed samples: one below the floor invalidates, and the reason names the lowest one.
        assert_eq!(
            gpu_validity(&[p("P0", 1470.0), p("P2", 1150.0), p("P0", 1470.0)]),
            GpuValidity::Invalid("P2 1150 MHz, below the 1400 MHz floor".into())
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
}
