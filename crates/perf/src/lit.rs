//! The M5 lit-path views and their verdicts (TEST-025; the owner's M5 budgets of 2026-10-15, ADR 0034, ADR 0036): per-pass GPU
//! timestamps of waited frames of the lit path at 1440p, the metric names the logs carry, and the budget rows of a static view and of a
//! flight. Pure over measured numbers; the scene (`planet::litperf`) measures, `cargo xtask accept M5` reads the logs back through the
//! same functions.
//!
//! Budgets (owner, 2026-10-15): atmosphere at most 1 ms and water at most 2 ms of GPU time at 1440p (medians of a static view that is
//! valid for GPU figures, ADR 0036); the terrain pass keeps M2's 8 ms; the p99 of the waited frame is judged against 20 ms at any clock
//! state as the terrain scene judges it. The frame median of 16.6 ms is the pipelined frame time of the streaming scenes on the lit
//! path (TEST-019, `--lit`); the GPU span of a waited frame is a reported figure beside it (timestamps serialise the passes, so it is an
//! upper bound), not a verdict. Flights are report only.

use crate::clock::GpuValidity;
use crate::{summarize, Stats};

/// Atmosphere budget: the three passes it adds (sky view, aerial perspective, composite), median of their per-frame sum.
pub const ATMOSPHERE_GPU_BUDGET_MS: f64 = 1.0;
/// Water budget: the water pass and the copy of the scene target it reads, median of their per-frame sum.
pub const WATER_GPU_BUDGET_MS: f64 = 2.0;
/// Water budget of the held view coast 3 m (ADR 0060): the worst-case view's own number; the other views keep [`WATER_GPU_BUDGET_MS`].
pub const COAST_3M_WATER_GPU_BUDGET_MS: f64 = 2.2;

/// The water budget of the view at `path` (`lit/coast-3m` or `coast-3m`): [`COAST_3M_WATER_GPU_BUDGET_MS`] for coast 3 m, else
/// [`WATER_GPU_BUDGET_MS`].
pub fn water_budget_ms(path: &str) -> f64 {
    if path == "coast-3m" || path.ends_with("/coast-3m") {
        COAST_3M_WATER_GPU_BUDGET_MS
    } else {
        WATER_GPU_BUDGET_MS
    }
}

/// The passes with timestamps, in the renderer's order (`planet_render::GpuPass::ALL`; a test in the app crate pins the equality).
pub const PASS_NAMES: [&str; 8] =
    ["terrain", "water", "sky-view", "aerial-perspective", "sky-composite", "exposure-histogram", "exposure-adapt", "tonemap"];

/// The log metric of a pass: the terrain pass is `gpu_ms` (as in the M4 logs), the others `gpu_<name>_ms` with underscores.
pub fn pass_metric(pass: &str) -> String {
    if pass == "terrain" {
        "gpu_ms".to_string()
    } else {
        format!("gpu_{}_ms", pass.replace('-', "_"))
    }
}

/// One waited, timed frame of the lit path.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LitFrame {
    /// The frame's wall time from the call until the GPU finished it.
    pub total_ms: f64,
    pub cpu_ms: f64,
    pub pass_ms: [Option<f64>; 8],
    /// The copy of the scene target before the water pass.
    pub scene_copy_ms: Option<f64>,
    /// First timestamp to last: the frame's GPU span.
    pub span_ms: Option<f64>,
}

impl LitFrame {
    fn pass(&self, name: &str) -> Option<f64> {
        PASS_NAMES.iter().position(|p| *p == name).and_then(|i| self.pass_ms[i])
    }

    /// Sky view + aerial perspective + composite of this frame; `None` unless all three ran.
    pub fn atmosphere_ms(&self) -> Option<f64> {
        Some(self.pass("sky-view")? + self.pass("aerial-perspective")? + self.pass("sky-composite")?)
    }

    /// Water pass + scene copy of this frame; `None` unless the water pass ran and the copy was timed.
    pub fn water_total_ms(&self) -> Option<f64> {
        Some(self.pass("water")? + self.scene_copy_ms?)
    }
}

/// The metrics of a set of frames: `cpu_ms`, one per pass (`gpu_ms` is the terrain pass), the copy, the atmosphere and water sums and the
/// span. A metric no frame has is absent.
pub fn metrics_of(frames: &[LitFrame]) -> Vec<(String, Stats)> {
    let mut out = Vec::new();
    let mut add = |name: String, v: Vec<f64>| {
        if let Some(s) = summarize(&v, 0) {
            out.push((name, s));
        }
    };
    add("cpu_ms".into(), frames.iter().map(|f| f.cpu_ms).collect());
    for (i, p) in PASS_NAMES.iter().enumerate() {
        add(pass_metric(p), frames.iter().filter_map(|f| f.pass_ms[i]).collect());
    }
    add("gpu_scene_copy_ms".into(), frames.iter().filter_map(|f| f.scene_copy_ms).collect());
    add("gpu_atmosphere_ms".into(), frames.iter().filter_map(LitFrame::atmosphere_ms).collect());
    add("gpu_water_total_ms".into(), frames.iter().filter_map(LitFrame::water_total_ms).collect());
    add("gpu_span_ms".into(), frames.iter().filter_map(|f| f.span_ms).collect());
    out
}

/// What a row of the verdict says.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Ok,
    /// Within the budget at a clock below the floor (ADR 0057): a slower clock only overstates GPU time, so this is a pass. Carries
    /// the state the figure was measured at.
    OkBelowFloor(String),
    Over,
    /// The view's clock state does not allow GPU verdicts (ADR 0036); the figure is shown without one.
    NotValid(String),
    /// The view has no such pass in it (no water, no atmosphere).
    NotInView(&'static str),
    /// A flight or an informational row: shown, never gated.
    Reported,
    /// The figure was not measured (no timestamps).
    NotMeasured,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LitVerdict {
    pub measure: &'static str,
    pub stat: &'static str,
    pub value: Option<f64>,
    pub budget: Option<f64>,
    pub gated: bool,
    pub status: Status,
}

impl LitVerdict {
    /// A gated row that fails the view.
    pub fn fails(&self) -> bool {
        self.gated && self.status == Status::Over
    }

    pub fn line(&self, view: &str) -> String {
        let v = self.value.map_or("n/a".to_string(), |v| format!("{v:.3} ms"));
        let b = self.budget.map_or(String::new(), |b| format!(" (budget {b} ms)"));
        let word = match (&self.status, self.gated) {
            (Status::Ok, _) => "ok".to_string(),
            (Status::OkBelowFloor(r), _) => format!("ok at a low clock ({r}: a slower clock only overstates the time)"),
            (Status::Over, true) => "OVER".to_string(),
            (Status::Over, false) => "over (not gated)".to_string(),
            (Status::NotValid(r), _) => format!("NOT VALID for GPU budgets ({r})"),
            (Status::NotInView(w), _) => format!("not in view ({w})"),
            (Status::Reported, _) => "reported".to_string(),
            (Status::NotMeasured, _) => "not measured".to_string(),
        };
        format!("  {view}: {} {} {v}{b} {word}", self.measure, self.stat)
    }
}

fn stat_of<'a>(m: &'a [(String, Stats)], name: &str) -> Option<&'a Stats> {
    m.iter().find(|(n, _)| n == name).map(|(_, s)| s)
}

/// The rows of a view. `gated` is true for a static view and false for a flight. `frame` is the waited frame time, whose verdict
/// holds at any clock state; every GPU row needs `gpu` to be valid (ADR 0036). `water_draws` and `atmosphere` say what the view holds.
pub fn view_verdicts(
    metrics: &[(String, Stats)],
    frame: &Stats,
    gpu: &GpuValidity,
    gated: bool,
    water_draws: u64,
    atmosphere: bool,
) -> Vec<LitVerdict> {
    view_verdicts_with(metrics, frame, gpu, gated, water_draws, atmosphere, WATER_GPU_BUDGET_MS)
}

/// [`view_verdicts`] with the water budget of the view (`water_budget_ms(path)`, ADR 0060).
pub fn view_verdicts_with(
    metrics: &[(String, Stats)],
    frame: &Stats,
    gpu: &GpuValidity,
    gated: bool,
    water_draws: u64,
    atmosphere: bool,
    water_budget: f64,
) -> Vec<LitVerdict> {
    let mut rows = Vec::new();
    let judged = |measure: &'static str, stat: &'static str, value: Option<f64>, budget: f64| {
        let status = match (&value, gpu) {
            (None, _) => Status::NotMeasured,
            (Some(_), GpuValidity::Invalid(r)) => Status::NotValid(r.clone()),
            (Some(v), GpuValidity::Valid(_)) if *v <= budget => Status::Ok,
            (Some(_), GpuValidity::Valid(_)) => Status::Over,
            // Below the floor (ADR 0057): within the budget even at the slow clock is a pass; over it proves nothing.
            (Some(v), GpuValidity::Below(_, r)) if *v <= budget => Status::OkBelowFloor(r.clone()),
            (Some(_), GpuValidity::Below(_, r)) => Status::NotValid(format!("over budget at a clock below the floor, inconclusive: {r}")),
            // (an exhaustive match keeps a new validity state from being judged by accident)
        };
        LitVerdict { measure, stat, value, budget: Some(budget), gated, status }
    };
    let median = |name: &str| stat_of(metrics, name).map(|s| s.median);
    rows.push(judged("terrain pass", "median", median("gpu_ms"), crate::TERRAIN_GPU_BUDGET_MS));
    if atmosphere {
        rows.push(judged(
            "atmosphere (sky view + aerial perspective + composite)",
            "median",
            median("gpu_atmosphere_ms"),
            ATMOSPHERE_GPU_BUDGET_MS,
        ));
    } else {
        rows.push(LitVerdict {
            measure: "atmosphere (sky view + aerial perspective + composite)",
            stat: "median",
            value: None,
            budget: Some(ATMOSPHERE_GPU_BUDGET_MS),
            gated,
            status: Status::NotInView("the view draws no atmosphere"),
        });
    }
    if water_draws > 0 {
        rows.push(judged("water (pass + scene copy)", "median", median("gpu_water_total_ms"), water_budget));
    } else {
        rows.push(LitVerdict {
            measure: "water (pass + scene copy)",
            stat: "median",
            value: None,
            budget: Some(water_budget),
            gated,
            status: Status::NotInView("no water node in the view"),
        });
    }
    // Reported, never judged: the frame median of the owner's budget is the streaming scenes' (TEST-019 with --lit).
    let span = median("gpu_span_ms");
    rows.push(LitVerdict {
        measure: "GPU span of the lit frame (reported; the frame median is the streaming --lit scenes')",
        stat: "median",
        value: span,
        budget: None,
        gated: false,
        status: if span.is_some() { Status::Reported } else { Status::NotMeasured },
    });
    // The frame verdict holds at any clock state (ADR 0034 as amended): the p99 of the waited frame against CON-21's 20 ms.
    let over = frame.p99 > crate::FRAME_P99_BUDGET_MS;
    rows.push(LitVerdict {
        measure: "frame (waited)",
        stat: "p99",
        value: Some(frame.p99),
        budget: Some(crate::FRAME_P99_BUDGET_MS),
        gated,
        status: if over { Status::Over } else { Status::Ok },
    });
    rows
}

/// The informational rows of a view: every pass and the copy as median / p99 / max, one line, in the pass order.
pub fn pass_rows_line(metrics: &[(String, Stats)]) -> String {
    let mut cells: Vec<String> = PASS_NAMES
        .iter()
        .map(|p| (*p, pass_metric(p)))
        .chain(std::iter::once(("scene-copy", "gpu_scene_copy_ms".to_string())))
        .map(|(label, metric)| match stat_of(metrics, &metric) {
            Some(s) => format!("{label} {:.3}/{:.3}/{:.3}", s.median, s.p99, s.max),
            None => format!("{label} -"),
        })
        .collect();
    cells.insert(0, "GPU ms per pass (median / p99 / max):".to_string());
    cells.join(", ").replacen(", ", " ", 1)
}

/// ADR 0036 point 3 (amended by ADR 0059): a view's run-to-run noise `(max - min) / median` of the medians of one per-pass metric
/// and the absolute spread. It counts as noisy only above both 10 % and 0.1 ms (`noisy_with_floor`; `noisy_now` is the 10 % alone).
pub fn noise_floor_report(run_medians: &[f64]) -> Option<(f64, f64, bool, bool)> {
    let rel = crate::run_to_run_noise(run_medians)?;
    let (lo, hi) = run_medians.iter().fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let abs = hi - lo;
    let noisy_now = rel > crate::gate::MAX_NOISE;
    let noisy_with_floor = crate::gate::noise_exceeds(run_medians, crate::gate::NOISE_FLOOR_PASS_MS);
    Some((rel, abs, noisy_now, noisy_with_floor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{ClockState, GpuValidity};

    fn valid() -> GpuValidity {
        GpuValidity::Valid(ClockState { pstate: "P0".into(), mhz: 1700.0 })
    }

    fn frame(t: f64, passes: [Option<f64>; 8], copy: Option<f64>, span: Option<f64>) -> LitFrame {
        LitFrame { total_ms: t, cpu_ms: 0.5, pass_ms: passes, scene_copy_ms: copy, span_ms: span }
    }

    // terrain, water, sky-view, aerial, composite, histogram, adapt, tonemap
    fn lit(atmo: f64, water: f64) -> LitFrame {
        frame(
            10.0,
            [Some(3.0), Some(water), Some(atmo * 0.2), Some(atmo * 0.3), Some(atmo * 0.5), Some(0.1), Some(0.01), Some(0.2)],
            Some(0.3),
            Some(7.0),
        )
    }

    // spec: TEST-025
    #[test]
    fn the_metric_names_are_the_terrain_scenes_for_the_terrain_pass_and_one_per_pass_otherwise() {
        assert_eq!(pass_metric("terrain"), "gpu_ms");
        assert_eq!(pass_metric("sky-view"), "gpu_sky_view_ms");
        assert_eq!(pass_metric("aerial-perspective"), "gpu_aerial_perspective_ms");
        assert_eq!(pass_metric("exposure-adapt"), "gpu_exposure_adapt_ms");
        assert_eq!(pass_metric("tonemap"), "gpu_tonemap_ms");
        let m = metrics_of(&[lit(0.8, 1.2), lit(0.9, 1.4), lit(1.0, 1.0)]);
        let names: Vec<&str> = m.iter().map(|(n, _)| n.as_str()).collect();
        for want in [
            "cpu_ms",
            "gpu_ms",
            "gpu_water_ms",
            "gpu_sky_view_ms",
            "gpu_sky_composite_ms",
            "gpu_exposure_histogram_ms",
            "gpu_tonemap_ms",
            "gpu_scene_copy_ms",
            "gpu_atmosphere_ms",
            "gpu_water_total_ms",
            "gpu_span_ms",
        ] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        let atmo = stat_of(&m, "gpu_atmosphere_ms").unwrap();
        assert!((atmo.median - 0.9).abs() < 1e-9, "the per-frame sum of the three passes: {atmo:?}");
        let water = stat_of(&m, "gpu_water_total_ms").unwrap();
        assert!((water.median - 1.5).abs() < 1e-9, "water pass 1.2 + copy 0.3 is the median of 1.5, 1.7, 1.3: {water:?}");
        // A debug-like frame has neither sum.
        let m = metrics_of(&[frame(3.0, [Some(2.0), None, None, None, None, None, None, None], None, Some(2.0))]);
        assert!(
            stat_of(&m, "gpu_atmosphere_ms").is_none() && stat_of(&m, "gpu_water_total_ms").is_none() && stat_of(&m, "gpu_ms").is_some()
        );
    }

    // spec: TEST-025
    #[test]
    fn coast_3m_has_a_water_budget_of_2_2_ms_and_every_other_view_keeps_2_ms() {
        assert_eq!(COAST_3M_WATER_GPU_BUDGET_MS, 2.2);
        assert_eq!((water_budget_ms("lit/coast-3m"), water_budget_ms("coast-3m")), (2.2, 2.2));
        for other in ["lit/coast-2km", "lit/orbit", "lit/ground-1m", "lit/aerial-20km", "lit/coast-glide", "lit-formats/orbit/compact"] {
            assert_eq!(water_budget_ms(other), WATER_GPU_BUDGET_MS, "{other}");
        }
        let f = summarize(&[10.0, 11.0, 12.0], 0).unwrap();
        // The water sum is the pass plus the 0.3 ms copy: 2.15 ms is within 2.2 and over 2; 2.25 ms is over both.
        let water = |m: &[(String, Stats)], path: &str| {
            let rows = view_verdicts_with(m, &f, &valid(), true, 40, true, water_budget_ms(path));
            rows.into_iter().find(|r| r.measure.starts_with("water")).unwrap()
        };
        let within = metrics_of(&(0..5).map(|_| lit(0.5, 1.85)).collect::<Vec<_>>());
        let over = metrics_of(&(0..5).map(|_| lit(0.5, 1.95)).collect::<Vec<_>>());
        assert_eq!(water(&within, "lit/coast-3m").status, Status::Ok);
        assert_eq!(water(&over, "lit/coast-3m").status, Status::Over);
        assert_eq!(water(&within, "lit/coast-2km").status, Status::Over, "another view at 2.15 ms fails its 2 ms");
        assert!(water(&over, "lit/coast-3m").line("coast-3m").contains("budget 2.2 ms"));
        // The boundary itself, on the figures of the ADR: exactly 2.2 ms is within, 2.21 ms is over; another view at 2.1 ms is over.
        let sum = |v: f64| vec![("gpu_water_total_ms".to_string(), summarize(&[v; 5], 0).unwrap())];
        assert_eq!(water(&sum(2.2), "lit/coast-3m").status, Status::Ok);
        assert_eq!(water(&sum(2.21), "lit/coast-3m").status, Status::Over);
        assert_eq!(water(&sum(2.1), "lit/coast-2km").status, Status::Over);
        assert_eq!(water(&sum(2.0), "lit/coast-2km").status, Status::Ok);
    }

    // spec: TEST-025
    #[test]
    fn a_static_view_is_judged_against_the_owners_budgets_and_the_boundaries_are_inclusive() {
        let frames: Vec<LitFrame> = (0..5).map(|_| lit(1.0, 1.7)).collect();
        let m = metrics_of(&frames);
        let f = summarize(&[10.0, 11.0, 12.0], 0).unwrap();
        let rows = view_verdicts(&m, &f, &valid(), true, 40, true);
        let judged: Vec<&LitVerdict> = rows.iter().filter(|r| r.gated).collect();
        assert!(judged.iter().all(|r| r.status == Status::Ok), "atmosphere exactly 1 ms and water 1.7 + 0.3 = 2 ms are within: {rows:?}");
        assert!(judged.len() == 4 && rows.len() == 5, "the GPU span is reported, not judged: {rows:?}");
        let span = rows.iter().find(|r| r.measure.starts_with("GPU span")).unwrap();
        assert!(!span.gated && span.status == Status::Reported && span.line("v").ends_with("reported"), "{span:?}");
        // Over: atmosphere 1.5 ms, water 2.5 ms.
        let m = metrics_of(&(0..5).map(|_| lit(1.5, 2.2)).collect::<Vec<_>>());
        let rows = view_verdicts(&m, &f, &valid(), true, 40, true);
        let over: Vec<&str> = rows.iter().filter(|r| r.fails()).map(|r| r.measure).collect();
        assert_eq!(over, ["atmosphere (sky view + aerial perspective + composite)", "water (pass + scene copy)"], "{rows:?}");
        let line = rows.iter().find(|r| r.fails()).unwrap().line("orbit");
        assert!(line.contains("orbit") && line.contains("1.500 ms") && line.contains("budget 1 ms") && line.ends_with("OVER"), "{line}");
    }

    // spec: TEST-025
    #[test]
    fn a_view_below_the_clock_floor_shows_its_figures_without_a_gpu_verdict_but_the_frame_verdict_stands() {
        let m = metrics_of(&(0..5).map(|_| lit(1.5, 2.2)).collect::<Vec<_>>());
        let slow = summarize(&[10.0, 30.0, 31.0], 0).unwrap();
        let rows = view_verdicts(&m, &slow, &GpuValidity::Invalid("P2 1150 MHz, below the 1400 MHz floor".into()), true, 10, true);
        for r in rows.iter().filter(|r| r.measure != "frame (waited)" && r.gated) {
            assert!(matches!(r.status, Status::NotValid(_)) && !r.fails(), "{r:?}");
            assert!(r.line("v").contains("NOT VALID for GPU budgets (P2 1150 MHz"), "{}", r.line("v"));
        }
        let frame_row = rows.iter().find(|r| r.measure == "frame (waited)").unwrap();
        assert!(frame_row.fails(), "p99 31 ms is over 20 ms at any clock state: {frame_row:?}");
    }

    // spec: TEST-034
    #[test]
    fn below_the_floor_a_figure_within_its_budget_passes_and_one_over_it_is_inconclusive_and_a_valid_clock_changes_nothing() {
        use crate::clock::ClockState;
        let below = GpuValidity::Below(ClockState { pstate: "P0".into(), mhz: 1300.0 }, "P0 1282 MHz, below the 1400 MHz floor".into());
        let f = summarize(&[10.0, 11.0, 12.0], 0).unwrap();
        let gpu_rows = |rows: &[LitVerdict]| rows.iter().filter(|r| r.gated && r.measure != "frame (waited)").cloned().collect::<Vec<_>>();
        // Within every budget at the slow clock: a pass, named as such, and never a failure.
        let within = metrics_of(&(0..5).map(|_| lit(0.8, 1.5)).collect::<Vec<_>>());
        let rows = gpu_rows(&view_verdicts(&within, &f, &below, true, 40, true));
        assert!(rows.len() == 3 && rows.iter().all(|r| matches!(r.status, Status::OkBelowFloor(_)) && !r.fails()), "{rows:?}");
        let line = rows[0].line("coast-2km");
        assert!(line.contains("ok at a low clock (P0 1282 MHz, below the 1400 MHz floor") && line.contains("overstates"), "{line}");
        // Over a budget at the slow clock: inconclusive (the slow clock may be the cause), not a failure.
        let over = metrics_of(&(0..5).map(|_| lit(1.5, 2.2)).collect::<Vec<_>>());
        let rows = gpu_rows(&view_verdicts(&over, &f, &below, true, 40, true));
        assert!(rows.iter().any(|r| matches!(&r.status, Status::NotValid(w) if w.contains("inconclusive"))), "{rows:?}");
        assert!(rows.iter().all(|r| !r.fails()), "{rows:?}");
        // At a valid clock nothing changed: within is ok, over is OVER.
        let rows = gpu_rows(&view_verdicts(&within, &f, &valid(), true, 40, true));
        assert!(rows.iter().all(|r| r.status == Status::Ok), "{rows:?}");
        let rows = gpu_rows(&view_verdicts(&over, &f, &valid(), true, 40, true));
        assert!(rows.iter().any(LitVerdict::fails), "{rows:?}");
        // The frame verdict stays at any clock.
        let slow = summarize(&[10.0, 30.0, 31.0], 0).unwrap();
        let rows = view_verdicts(&within, &slow, &below, true, 40, true);
        assert!(rows.iter().find(|r| r.measure == "frame (waited)").unwrap().fails());
    }

    // spec: TEST-025
    #[test]
    fn a_view_without_water_or_atmosphere_says_so_and_a_flight_is_never_gated() {
        let m = metrics_of(
            &(0..5)
                .map(|_| frame(5.0, [Some(2.0), None, Some(0.2), Some(0.3), Some(0.5), Some(0.1), Some(0.01), Some(0.2)], None, Some(3.0)))
                .collect::<Vec<_>>(),
        );
        let f = summarize(&[5.0, 5.0], 0).unwrap();
        let rows = view_verdicts(&m, &f, &valid(), true, 0, true);
        let water = rows.iter().find(|r| r.measure.starts_with("water")).unwrap();
        assert!(
            matches!(water.status, Status::NotInView(_))
                && !water.fails()
                && water.line("ground-1m").contains("not in view (no water node")
        );
        let rows = view_verdicts(&m, &f, &valid(), true, 3, false);
        assert!(rows.iter().any(|r| r.measure.starts_with("atmosphere") && matches!(r.status, Status::NotInView(_))));
        // A flight over budget: shown as over, not gated.
        let bad = metrics_of(&(0..5).map(|_| lit(3.0, 4.0)).collect::<Vec<_>>());
        let rows = view_verdicts(&bad, &f, &valid(), false, 40, true);
        assert!(rows.iter().all(|r| !r.fails() && !r.gated), "{rows:?}");
        assert!(rows.iter().any(|r| r.line("coast-glide").contains("over (not gated)")));
        // Without timestamps nothing is invented.
        let none = metrics_of(&[LitFrame { total_ms: 5.0, cpu_ms: 1.0, ..Default::default() }]);
        let rows = view_verdicts(&none, &f, &valid(), true, 3, true);
        assert!(
            rows.iter().filter(|r| r.measure != "frame (waited)").all(|r| matches!(r.status, Status::NotMeasured | Status::NotInView(_))),
            "{rows:?}"
        );
    }

    // spec: TEST-025
    #[test]
    fn the_pass_row_lists_every_pass_in_order_and_the_proposed_noise_floor_is_reported_not_applied() {
        let m = metrics_of(&(0..5).map(|_| lit(0.9, 1.2)).collect::<Vec<_>>());
        let line = pass_rows_line(&m);
        let at = |s: &str| line.find(s).unwrap_or_else(|| panic!("{s} in {line}"));
        assert!(
            at("terrain") < at("water")
                && at("water") < at("sky-view")
                && at("sky-composite") < at("tonemap")
                && at("tonemap") < at("scene-copy"),
            "{line}"
        );
        // ADR 0036 point 3 (ADR 0059): 0.2 ms vs 0.23 ms is 15 % noise but 0.03 ms: noisy now, not with the floor.
        let (rel, abs, now, floor) = noise_floor_report(&[0.20, 0.23, 0.21]).unwrap();
        assert!(rel > 0.1 && abs < 0.1 && now && !floor, "{rel} {abs}");
        let (_, _, now, floor) = noise_floor_report(&[2.0, 2.4, 2.1]).unwrap();
        assert!(now && floor);
        let (_, _, now, floor) = noise_floor_report(&[2.0, 2.05, 2.02]).unwrap();
        assert!(!now && !floor);
    }
}
