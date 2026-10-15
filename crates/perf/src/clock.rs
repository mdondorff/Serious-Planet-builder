//! GPU clock state of a measured view (ADR 0034): which views are valid for GPU figures, what "equal clock state" means,
//! and the warm-up loop that brings the GPU to its loaded state. Pure functions over recorded samples; the render, the
//! sampler and the time source of the warm-up are closures, so tier A drives them with fakes.

use crate::ClockSample;
use std::fmt;

/// The performance state a GPU figure must be measured in (`nvidia-smi` `pstate`).
pub const LOADED_PSTATE: &str = "P0";
/// Two graphics clocks are equal when they differ by at most this fraction of the larger one.
pub const CLOCK_TOLERANCE: f64 = 0.05;
/// How long the warm-up tries to reach [`LOADED_PSTATE`] per view, in seconds (inclusive).
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

/// Whether a view's GPU figures (`gpu_ms`, `frame_ms`) may be judged or compared.
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

/// The clock state of a view from every sample taken before and after its runs (`None` is a missing sample). The
/// P-state must be the same in all samples; the clock is their median.
pub fn view_clock_state(points: &[Option<ClockPoint>]) -> Result<ClockState, String> {
    if points.is_empty() || points.iter().any(Option::is_none) {
        return Err("no clock sample".into());
    }
    let points: Vec<&ClockPoint> = points.iter().flatten().collect();
    let first = &points[0].pstate;
    if points.iter().any(|p| &p.pstate != first) {
        let mut states: Vec<&str> = points.iter().map(|p| p.pstate.as_str()).collect();
        states.sort_unstable();
        states.dedup();
        return Err(format!("mixed {}", states.join("/")));
    }
    let mut mhz: Vec<f64> = points.iter().map(|p| p.mhz).collect();
    mhz.sort_by(f64::total_cmp);
    let n = mhz.len();
    let median = if n % 2 == 1 { mhz[n / 2] } else { (mhz[n / 2 - 1] + mhz[n / 2]) / 2.0 };
    Ok(ClockState { pstate: first.clone(), mhz: median })
}

/// ADR 0034 point 4: a view is valid for GPU figures only if it has a clock state and that state is P0.
pub fn gpu_validity(points: &[Option<ClockPoint>]) -> GpuValidity {
    match view_clock_state(points) {
        Ok(s) if s.pstate == LOADED_PSTATE => GpuValidity::Valid(s),
        Ok(s) => GpuValidity::Invalid(s.to_string()),
        Err(e) => GpuValidity::Invalid(e),
    }
}

/// ADR 0034 point 5: the same P-state and graphics clocks within [`CLOCK_TOLERANCE`] of the larger.
pub fn equal_clock_state(a: &ClockState, b: &ClockState) -> bool {
    a.pstate == b.pstate && (a.mhz - b.mhz).abs() <= CLOCK_TOLERANCE * a.mhz.max(b.mhz)
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
    /// The architect's line (ADR 0034 point 8).
    pub fn line(&self, view: &str) -> String {
        let last = self.last.as_ref().map_or("no reading".to_string(), |p| format!("{} {:.0} MHz", p.pstate, p.mhz));
        if self.reached {
            format!("warm-up {view}: {LOADED_PSTATE} after {:.1} s ({} chunks)", self.seconds, self.chunks)
        } else {
            format!("warm-up {view}: still {last} after {:.1} s ({} chunks, cap {WARMUP_CAP_S} s)", self.seconds, self.chunks)
        }
    }
}

/// Render chunks and read the state after each until it reads [`LOADED_PSTATE`] or `cap_s` seconds have passed (checked
/// after the reading, so a P0 reading at exactly the cap counts). `now` returns seconds. At least one chunk is rendered.
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
        let reached = last.as_ref().is_some_and(|p| p.pstate == LOADED_PSTATE);
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
    fn a_view_is_valid_for_gpu_figures_only_in_p0_with_every_sample_present() {
        let ok = gpu_validity(&[p("P0", 1395.0), p("P0", 1410.0), p("P0", 1380.0), p("P0", 1395.0)]);
        assert_eq!(ok, GpuValidity::Valid(ClockState { pstate: "P0".into(), mhz: 1395.0 }));
        assert_eq!(gpu_validity(&[p("P2", 1150.0), p("P2", 1155.0)]), GpuValidity::Invalid("P2 1152 MHz".into()));
        assert_eq!(gpu_validity(&[p("P3", 1150.0)]), GpuValidity::Invalid("P3 1150 MHz".into()));
        // One P2 sample among P0 ones: mixed, not valid.
        assert_eq!(gpu_validity(&[p("P0", 1395.0), p("P2", 1150.0), p("P0", 1395.0)]), GpuValidity::Invalid("mixed P0/P2".into()));
        // A missing sample cannot show P0; no samples at all neither.
        assert_eq!(gpu_validity(&[p("P0", 1395.0), None]), GpuValidity::Invalid("no clock sample".into()));
        assert_eq!(gpu_validity(&[]), GpuValidity::Invalid("no clock sample".into()));
    }

    // spec: TEST-012
    #[test]
    fn equal_clock_state_is_the_same_pstate_and_clocks_within_five_percent() {
        let s = |pstate: &str, mhz: f64| ClockState { pstate: pstate.into(), mhz };
        assert!(equal_clock_state(&s("P0", 1400.0), &s("P0", 1350.0)));
        assert!(equal_clock_state(&s("P0", 1400.0), &s("P0", 1330.0)), "exactly 5 % of the larger is equal");
        assert!(!equal_clock_state(&s("P0", 1400.0), &s("P0", 1329.0)));
        assert!(!equal_clock_state(&s("P0", 1400.0), &s("P2", 1400.0)));
        assert!(!equal_clock_state(&s("P0", 1395.0), &s("P2", 1150.0)));
        assert_eq!(s("P0", 1395.4).to_string(), "P0 1395 MHz");
    }

    // spec: TEST-012
    #[test]
    fn the_warm_up_stops_at_the_first_p0_reading() {
        let mut readings = vec![p("P2", 1150.0), p("P1", 1300.0), p("P0", 1395.0), p("P0", 1395.0)].into_iter();
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
        assert_eq!(out.last, p("P0", 1395.0));
        assert!(out.line("ground-1m").starts_with("warm-up ground-1m: P0 after"), "{}", out.line("ground-1m"));
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
        assert!(line.contains("still P2 1150 MHz") && line.contains("cap 30"), "{line}");
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
    fn a_p0_reading_at_exactly_the_cap_counts_and_a_render_error_stops_the_warm_up() {
        let mut t = -10.0;
        let out = warm_up(
            10.0,
            || Ok(()),
            || p("P0", 1395.0),
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
