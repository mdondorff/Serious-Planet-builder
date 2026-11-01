//! The verdict rules of a time reading taken under load (ADR 0087 points 1 to 5, BUILD-031): one pure function over the reading, its budget and what kind of
//! time it is, so the console, the log and the A/B helper cannot disagree.
//!
//! | kind                                              | judged under load                                            |
//! |---------------------------------------------------|--------------------------------------------------------------|
//! | GPU time (timestamp queries)                      | 5 % margin, carries the +18 % note                           |
//! | CPU time, scene pinned to the P-cores             | 5 % margin                                                   |
//! | CPU time, not pinned                              | 30 % margin                                                  |
//! | frame time with streaming, regression comparison, milestone acceptance | never: needs a quiet window (ADR 0087 point 3) |
//!
//! A reading under the budget by at least the margin is a loaded pass; over the budget it is inconclusive (a loaded fail is never a verdict:
//! re-run quiet); in between it goes to the window queue.

/// The margin of a loaded pass (the measured quiet noise, ADR 0087 point 1), as a share of the budget.
pub const LOADED_MARGIN: f64 = 0.05;
/// The margin of an unpinned CPU-timed scene (ADR 0087 consequences: CPU verdicts with at least 30 % margin).
pub const CPU_UNPINNED_MARGIN: f64 = 0.30;
/// What a GPU timing under a full CPU load can add (SPIKE-58 phase 2: the shared laptop power budget), as a share of the quiet reading.
pub const GPU_LOAD_RISE: f64 = 0.18;
/// The note every GPU timing measured under load carries.
pub const GPU_LOAD_NOTE: &str = "GPU timings under CPU load can read up to 18 % high (shared power budget, ADR 0087)";

/// What kind of time a reading is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A GPU time from timestamp queries.
    Gpu,
    /// A CPU-timed scene; `pinned` when the competing builds ran on the E-cores at BelowNormal and the scene on the P-cores at High.
    Cpu { pinned: bool },
    /// The frame time median or p99 with streaming on.
    FrameStreaming,
    /// A comparison of CPU wall time between sessions.
    Regression,
    /// A reading of a milestone acceptance run.
    Acceptance,
}

/// The answer for one reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Under the budget by at least the margin: valid without a window.
    LoadedPass,
    /// Over the budget: not a verdict; re-run on a quiet machine.
    Inconclusive,
    /// Under the budget by less than the margin: the window queue decides.
    NeedsWindow,
    /// A kind that is only ever judged on a quiet machine.
    NeedsQuiet,
}

/// A verdict with the note it carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Judged {
    pub verdict: Verdict,
    pub note: Option<&'static str>,
}

impl Judged {
    /// The row text of a report: `loaded pass`, `inconclusive (re-run quiet)`, `window` or `needs a quiet window`, with the note.
    pub fn text(&self) -> String {
        let v = match self.verdict {
            Verdict::LoadedPass => "loaded pass",
            Verdict::Inconclusive => "inconclusive (re-run quiet)",
            Verdict::NeedsWindow => "within the margin: window queue",
            Verdict::NeedsQuiet => "needs a quiet window",
        };
        self.note.map_or_else(|| v.to_string(), |n| format!("{v} [{n}]"))
    }
}

/// The margin a kind needs to count as a loaded pass, `None` for a kind that is never judged under load.
pub fn margin(kind: Kind) -> Option<f64> {
    match kind {
        Kind::Gpu | Kind::Cpu { pinned: true } => Some(LOADED_MARGIN),
        Kind::Cpu { pinned: false } => Some(CPU_UNPINNED_MARGIN),
        Kind::FrameStreaming | Kind::Regression | Kind::Acceptance => None,
    }
}

/// Judge `reading` against `budget` (both in the same unit, lower is better) for a session measured under load.
pub fn loaded_verdict(kind: Kind, reading: f64, budget: f64) -> Judged {
    let note = (kind == Kind::Gpu).then_some(GPU_LOAD_NOTE);
    let verdict = match margin(kind) {
        None => Verdict::NeedsQuiet,
        Some(_) if !reading.is_finite() || !budget.is_finite() || budget <= 0.0 => Verdict::Inconclusive,
        Some(m) if reading <= budget * (1.0 - m) => Verdict::LoadedPass,
        Some(_) if reading > budget => Verdict::Inconclusive,
        Some(_) => Verdict::NeedsWindow,
    };
    Judged { verdict, note }
}

/// One time reading of a run row with the rule that judges it under load.
#[derive(Clone, Debug, PartialEq)]
pub struct RowVerdict {
    pub metric: &'static str,
    pub kind: Kind,
    pub reading: f64,
    pub budget: f64,
    pub judged: Judged,
}

/// The loaded verdicts of a run row (BUILD-032): which readings of the row are judged, and as what kind. Rows of the streaming scenes (`stream/`, `stream-lit/`)
/// have the frame time with streaming, which needs a quiet window. Rows of the terrain views (`terrain/`) have the GPU time (timestamp queries), the CPU plan time and
/// the waited frame total (CPU wall time, not pinned). Rows of the lit passes (`lit/`) have the GPU passes and the waited frame p99. Other rows carry no verdict.
pub fn row_verdicts(r: &crate::RunReport) -> Vec<RowVerdict> {
    let mut out = Vec::new();
    let mut add = |metric: &'static str, kind: Kind, reading: f64, budget: f64| {
        out.push(RowVerdict { metric, kind, reading, budget, judged: loaded_verdict(kind, reading, budget) });
    };
    let metric = |name: &str| r.metrics.iter().find(|(n, _)| n == name).map(|(_, s)| s.median);
    if r.path_name.starts_with("stream/") || r.path_name.starts_with("stream-lit/") {
        add("frame median ms", Kind::FrameStreaming, r.stats.median, crate::streaming::FRAME_MEDIAN_BUDGET_MS);
        add("frame p99 ms", Kind::FrameStreaming, r.stats.p99, crate::FRAME_P99_BUDGET_MS);
    } else if r.path_name.starts_with("terrain/") {
        if let Some(g) = metric("gpu_ms") {
            add("GPU median ms", Kind::Gpu, g, crate::TERRAIN_GPU_BUDGET_MS);
        }
        if let Some(c) = metric("cpu_ms") {
            add("CPU plan median ms", Kind::Cpu { pinned: false }, c, crate::PLAN_CPU_BUDGET_MS);
        }
        add("frame p99 ms", Kind::Cpu { pinned: false }, r.stats.p99, crate::FRAME_P99_BUDGET_MS);
    } else if r.path_name.starts_with("lit/") {
        // The lit passes (TEST-025): the GPU passes with their budgets and the waited frame p99, as the terrain views.
        if let Some(g) = metric("gpu_ms") {
            add("terrain pass GPU median ms", Kind::Gpu, g, crate::TERRAIN_GPU_BUDGET_MS);
        }
        if let Some(a) = metric("gpu_atmosphere_ms") {
            add("atmosphere GPU median ms", Kind::Gpu, a, crate::lit::ATMOSPHERE_GPU_BUDGET_MS);
        }
        if let Some(w) = metric("gpu_water_total_ms") {
            add("water GPU median ms", Kind::Gpu, w, crate::lit::water_budget_ms(&r.path_name));
        }
        add("frame p99 ms", Kind::Cpu { pinned: false }, r.stats.p99, crate::FRAME_P99_BUDGET_MS);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-032
    #[test]
    fn a_lit_row_carries_its_gpu_and_frame_verdicts() {
        let stat = |m: f64| crate::summarize(&[m, m], 0).unwrap();
        let r = crate::RunReport {
            path_name: "lit/coast-3m".into(),
            vram_cap_gb: 8,
            stats: stat(5.0),
            clocks_before: None,
            clocks_after: None,
            metrics: vec![("gpu_ms".into(), stat(4.0)), ("gpu_atmosphere_ms".into(), stat(0.9)), ("gpu_water_total_ms".into(), stat(2.1))],
            counts: vec![],
        };
        let v = row_verdicts(&r);
        let names: Vec<&str> = v.iter().map(|x| x.metric).collect();
        assert_eq!(names, ["terrain pass GPU median ms", "atmosphere GPU median ms", "water GPU median ms", "frame p99 ms"]);
        assert_eq!(v[2].budget, crate::lit::COAST_3M_WATER_GPU_BUDGET_MS, "the coast view keeps its own water budget");
        assert_eq!(v[0].judged.verdict, Verdict::LoadedPass);
        assert!(v[0].judged.note.is_some(), "a GPU reading carries the +18 % note");
    }

    // spec: BUILD-031
    #[test]
    fn a_gpu_reading_passes_at_the_margin_waits_inside_it_and_is_inconclusive_over_the_budget() {
        let judge = |r: f64| loaded_verdict(Kind::Gpu, r, 100.0);
        assert_eq!(judge(94.0).verdict, Verdict::LoadedPass);
        assert_eq!(judge(95.0).verdict, Verdict::LoadedPass, "exactly the margin passes");
        assert_eq!(judge(97.0).verdict, Verdict::NeedsWindow);
        assert_eq!(judge(100.0).verdict, Verdict::NeedsWindow, "at the budget is not over it");
        assert_eq!(judge(110.0).verdict, Verdict::Inconclusive);
        for r in [94.0, 97.0, 110.0] {
            assert_eq!(judge(r).note, Some(GPU_LOAD_NOTE), "every GPU timing carries the note");
        }
    }

    // spec: BUILD-031
    #[test]
    fn a_cpu_reading_needs_pinning_for_the_small_margin_and_thirty_percent_without() {
        let pinned = |r| loaded_verdict(Kind::Cpu { pinned: true }, r, 100.0).verdict;
        let unpinned = |r| loaded_verdict(Kind::Cpu { pinned: false }, r, 100.0).verdict;
        assert_eq!((pinned(94.0), unpinned(94.0)), (Verdict::LoadedPass, Verdict::NeedsWindow));
        assert_eq!((pinned(80.0), unpinned(80.0)), (Verdict::LoadedPass, Verdict::NeedsWindow));
        assert_eq!(unpinned(70.0), Verdict::LoadedPass);
        assert_eq!(unpinned(120.0), Verdict::Inconclusive);
        assert_eq!(loaded_verdict(Kind::Cpu { pinned: true }, 94.0, 100.0).note, None, "only GPU timings carry the note");
    }

    // spec: BUILD-031
    #[test]
    fn the_exclusive_kinds_are_never_judged_under_load() {
        for kind in [Kind::FrameStreaming, Kind::Regression, Kind::Acceptance] {
            for r in [1.0, 99.0, 150.0] {
                assert_eq!(loaded_verdict(kind, r, 100.0).verdict, Verdict::NeedsQuiet, "{kind:?} at {r}");
            }
            assert_eq!(margin(kind), None);
        }
    }

    // spec: BUILD-031
    #[test]
    fn an_unreadable_reading_or_budget_is_inconclusive_and_the_rows_read_plainly() {
        for (r, b) in [(f64::NAN, 100.0), (50.0, 0.0), (f64::INFINITY, 100.0), (50.0, f64::NAN)] {
            assert_eq!(loaded_verdict(Kind::Gpu, r, b).verdict, Verdict::Inconclusive, "{r} against {b}");
        }
        assert_eq!(loaded_verdict(Kind::Cpu { pinned: true }, 90.0, 100.0).text(), "loaded pass");
        assert_eq!(loaded_verdict(Kind::Regression, 90.0, 100.0).text(), "needs a quiet window");
        assert!(loaded_verdict(Kind::Gpu, 120.0, 100.0).text().starts_with("inconclusive (re-run quiet) [GPU timings"));
    }

    fn report(path: &str, metrics: &[(&str, f64)], median: f64, p99: f64) -> crate::RunReport {
        let st = |m: f64, p: f64| crate::Stats { samples: 10, median: m, p99: p, min: m, max: p, mean: m };
        crate::RunReport {
            path_name: path.into(),
            vram_cap_gb: 8,
            stats: st(median, p99),
            clocks_before: None,
            clocks_after: None,
            metrics: metrics.iter().map(|(n, v)| (n.to_string(), st(*v, *v))).collect(),
            counts: vec![],
        }
    }

    // spec: BUILD-032
    #[test]
    fn the_rows_of_the_streaming_scenes_need_a_quiet_window_and_the_terrain_views_are_judged_by_kind() {
        let v = row_verdicts(&report("stream-lit/dive", &[], 5.0, 8.0));
        assert_eq!(v.iter().map(|x| x.metric).collect::<Vec<_>>(), vec!["frame median ms", "frame p99 ms"]);
        assert!(v.iter().all(|x| x.judged.verdict == Verdict::NeedsQuiet));
        let v = row_verdicts(&report("terrain/ground-1m", &[("gpu_ms", 7.0), ("cpu_ms", 6.0)], 6.0, 15.0));
        assert_eq!(
            v.iter().map(|x| (x.metric, x.judged.verdict)).collect::<Vec<_>>(),
            vec![
                ("GPU median ms", Verdict::LoadedPass),
                ("CPU plan median ms", Verdict::NeedsWindow),
                ("frame p99 ms", Verdict::NeedsWindow),
            ]
        );
        assert_eq!(v[0].judged.note, Some(GPU_LOAD_NOTE));
        let v = row_verdicts(&report("terrain/orbit", &[("gpu_ms", 9.0)], 6.0, 25.0));
        assert_eq!((v[0].judged.verdict, v[1].judged.verdict), (Verdict::Inconclusive, Verdict::Inconclusive));
        assert!(row_verdicts(&report("lit/orbit", &[("gpu_ms", 1.0)], 1.0, 1.0)).is_empty(), "rows without a table carry no verdict");
    }
}
