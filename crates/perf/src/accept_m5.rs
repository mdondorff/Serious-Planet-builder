//! The hardware half of `cargo xtask accept M5` (TEST-026): reads the logs of the first M5 perf window and judges M5's hardware
//! criteria from them, or refuses. Pure over the file contents, like `accept` for M4; the xtask reads the files, runs the other steps
//! and writes `docs/milestones/M5-report.md`.
//!
//! What gates (the owner's M5 budgets of 2026-10-15, ADR 0034, ADR 0036): in the lit scene `lit`, for each of the five held views,
//! the atmosphere (sky view + aerial perspective + composite) at most 1 ms, the water (pass + scene copy) at most 2 ms, 2.2 ms for coast 3 m (ADR 0060; the coast views
//! must hold water in every run), the terrain pass at most 8 ms (M2's budget carried to the lit path by ADR 0034 point 4; all medians,
//! needing a view valid for GPU figures), and the p99 of the waited frame at most 20 ms (any clock state, ADR 0034 as amended); in the five gated streaming scenes on the lit path (`<scene>-lit.json`) every verdict of TEST-018 and
//! TEST-019 (frame median 16.6 ms, p99 20 ms, no blank tile, VRAM, RAM, lag), exactly as M4 judged the debug path. What is reported and
//! never gated: the coastal flights (both shore-filter settings), the compact against vertex ratios of `lit-formats`, the GPU span,
//! `dive-fast` and `dive-cold` on the lit path, the host load of every case. A window whose GPU rows cannot be judged (a view below the
//! clock floor, an idle GPU start, a missing water node in a coast view) is INCOMPLETE, not a pass and not a failure; a window that
//! cannot be trusted at all (invalid session, smoke, another source, another size, partial logs, an old format) is refused.

use crate::accept::{
    count, read_scene_as, str_at, SceneFiles, SceneRead, COLD_SCENE, GATED_SCENES, PRODUCTION_CELLS, PRODUCTION_FIELD, REPORT_ONLY_SCENE,
};
use crate::clock::{ClockState, GpuValidity};
use crate::compare::{parse, Json};
use crate::lit::{pass_rows_line, LitVerdict, Status};
use crate::prefill::PrefillState;
use crate::Stats;

/// The held views of `lit` that gate, in the scene's order.
pub const STATIC_VIEWS: [&str; 5] = ["ground-1m", "aerial-20km", "coast-2km", "coast-3m", "orbit"];
/// The views whose water must be in the picture.
pub const COAST_VIEWS: [&str; 2] = ["coast-2km", "coast-3m"];
/// The flights as logged (the second setting of the shore filter has the suffix).
pub const FLIGHT_PATHS: [&str; 4] = ["coast-glide", "coast-glide@filter0", "coast-descent", "coast-descent@filter0"];
/// The views `lit-formats` measures in both formats.
pub const FORMAT_VIEWS: [&str; 4] = ["ground-1m", "aerial-20km", "coast-2km", "orbit"];
/// What `protocol.field` of the lit scenes must say: the production world's field drawn lit with compact tiles (the shipped format).
pub fn lit_field() -> String {
    format!("{PRODUCTION_FIELD}; lit path (atmosphere, water), compact tiles")
}
/// The frame size of the window.
pub const WINDOW_SIZE: (f64, f64) = (2560.0, 1440.0);

/// The files of a window.
#[derive(Clone, Debug, Default)]
pub struct WindowFiles {
    /// `lit.json`.
    pub lit: Option<Vec<u8>>,
    /// `lit-formats.json`.
    pub formats: Option<Vec<u8>>,
    /// The streaming scenes' lit logs: `scene` is the plain scene name, `json` and `frames` are `<scene>-lit.json` and
    /// `<scene>-lit.frames.txt`.
    pub streams: Vec<SceneFiles>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    /// No gated row failed, but some could not be judged (listed): not a pass.
    Incomplete,
}

impl Outcome {
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Pass => "PASS",
            Outcome::Fail => "FAIL",
            Outcome::Incomplete => "INCOMPLETE",
        }
    }
}

/// One view or flight read from `lit.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewRead {
    pub path: String,
    pub rows: Vec<LitVerdict>,
    pub pass_line: String,
    pub facts: Vec<String>,
}

/// A held view or flight that was not in the log (flights are optional).
pub type FlightRead = Result<ViewRead, String>;

#[derive(Clone, Debug, PartialEq)]
pub struct Judgement5 {
    pub views: Vec<ViewRead>,
    pub flights: Vec<FlightRead>,
    /// The `lit-formats` lines, or why there are none.
    pub formats: Result<Vec<String>, String>,
    /// The gated streaming scenes on the lit path.
    pub streams: Vec<SceneRead>,
    pub fast: Result<SceneRead, Vec<String>>,
    pub cold: Result<SceneRead, Vec<String>>,
    pub outcome: Outcome,
    /// What could not be judged (empty unless the outcome is `Incomplete`), and the failing rows (`Fail`).
    pub incomplete: Vec<String>,
    pub failing: Vec<String>,
    pub source: String,
    pub field: String,
    /// Load notes of the lit cases (reported, never judged, TEST-023; the streaming cases' are in their logs).
    pub load_notes: Vec<String>,
    pub facts: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Evaluation5 {
    Refused(Vec<String>),
    Judged(Box<Judgement5>),
}

fn num(j: &Json, key: &str) -> Option<f64> {
    j.get(key).and_then(Json::num)
}

fn median_of(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// The runs of a log with this path.
fn runs_of<'a>(root: &'a Json, path: &str) -> Vec<&'a Json> {
    match root.get("runs") {
        Some(Json::Arr(a)) => a.iter().filter(|r| str_at(r, "path") == Some(path)).collect(),
        _ => Vec::new(),
    }
}

/// Combine the runs of a view into one set of metrics: the median of the runs' medians, the largest p99 and maximum, the mean of the
/// means, the samples added.
fn combined_metrics(runs: &[&Json]) -> Vec<(String, Stats)> {
    let mut names: Vec<String> = Vec::new();
    for r in runs {
        if let Some(Json::Obj(m)) = r.get("metrics") {
            for (k, _) in m {
                if !names.contains(k) {
                    names.push(k.clone());
                }
            }
        }
    }
    names
        .into_iter()
        .filter_map(|name| {
            let each: Vec<&Json> = runs.iter().filter_map(|r| r.get("metrics").and_then(|m| m.get(&name))).collect();
            let col = |k: &str| each.iter().filter_map(|m| num(m, k)).collect::<Vec<f64>>();
            let (med, p99, max, mean, n) = (col("median"), col("p99"), col("max"), col("mean"), col("samples"));
            if med.is_empty() || p99.is_empty() || max.is_empty() {
                return None;
            }
            let median = median_of(med);
            Some((
                name,
                Stats {
                    samples: n.iter().sum::<f64>() as usize,
                    median,
                    p99: p99.iter().cloned().fold(f64::MIN, f64::max),
                    min: median,
                    max: max.iter().cloned().fold(f64::MIN, f64::max),
                    mean: if mean.is_empty() { median } else { mean.iter().sum::<f64>() / mean.len() as f64 },
                },
            ))
        })
        .collect()
}

/// The waited frame time of a view from its runs' headline statistics.
fn combined_frame(runs: &[&Json]) -> Option<Stats> {
    let col = |k: &str| runs.iter().filter_map(|r| num(r, k)).collect::<Vec<f64>>();
    let (med, p99, max, mean, n) = (col("median_ms"), col("p99_ms"), col("max_ms"), col("mean_ms"), col("samples"));
    if med.len() != runs.len() || p99.len() != runs.len() || max.len() != runs.len() {
        return None;
    }
    let median = median_of(med);
    Some(Stats {
        samples: n.iter().sum::<f64>() as usize,
        median,
        p99: p99.iter().cloned().fold(f64::MIN, f64::max),
        min: median,
        max: max.iter().cloned().fold(f64::MIN, f64::max),
        mean: mean.iter().sum::<f64>() / mean.len().max(1) as f64,
    })
}

/// The GPU validity of a view's runs, derived from the logged data: the session fit for GPU verdicts (its warnings judged again) and
/// the clock samples of the view's runs judged again against today's floor (ADR 0057). A view whose samples are not all in the log
/// keeps the recorded per-run verdict. The second value says where the derived verdict differs from the recorded one.
fn gpu_validity_of(root: &Json, runs: &[&Json]) -> (GpuValidity, Option<String>) {
    let recorded = recorded_gpu_validity(root, runs);
    if !crate::validity::log_gpu_session_valid(root) {
        return (recorded, None);
    }
    match crate::validity::view_gpu_validity_from_log(runs) {
        None => (recorded, Some("clock samples missing in the log: the recorded GPU verdict is used".to_string())),
        Some(now) => {
            let same = std::mem::discriminant(&now) == std::mem::discriminant(&recorded);
            let note = (!same).then(|| format!("GPU clock verdict judged again from the logged samples: {now:?}, recorded: {recorded:?}"));
            (now, note)
        }
    }
}

/// The GPU validity as the run recorded it: every run valid (the log's per-view `gpu_valid`) and the session fit for GPU verdicts.
fn recorded_gpu_validity(root: &Json, runs: &[&Json]) -> GpuValidity {
    if !crate::validity::log_gpu_session_valid(root) {
        let why = crate::validity::log_gpu_only_reasons(root);
        return GpuValidity::Invalid(why.first().cloned().unwrap_or_else(|| "the session is not fit for GPU verdicts".to_string()));
    }
    // A run below the floor (ADR 0057) keeps its words in the log (`P0 1282 MHz, below the 1400 MHz floor`); any other text of a
    // run that is not valid is a missing sample. The view is Below only when every run that is not valid is below the floor, and
    // it names the lowest.
    let mut lowest: Option<(ClockState, String)> = None;
    for r in runs {
        if matches!(r.get("gpu_valid"), Some(Json::Bool(true))) {
            continue;
        }
        let text = str_at(r, "gpu_clock_state").unwrap_or("no clock state");
        match crate::clock::parse_below_floor(text) {
            None => return GpuValidity::Invalid(text.to_string()),
            Some(state) => {
                if lowest.as_ref().is_none_or(|(l, _)| state.mhz < l.mhz) {
                    lowest = Some((state, text.to_string()));
                }
            }
        }
    }
    if let Some((state, text)) = lowest {
        return GpuValidity::Below(state, text);
    }
    let state = runs.first().and_then(|r| str_at(r, "gpu_clock_state")).unwrap_or("");
    // `P2 1710 MHz` (the log's words) back into a state; any other text is kept as the P-state with an unknown clock.
    let mut it = state.rsplitn(3, ' ');
    let (unit, mhz, p) = (it.next(), it.next().and_then(|m| m.parse::<f64>().ok()), it.next());
    match (unit, mhz, p) {
        (Some("MHz"), Some(m), Some(p)) => GpuValidity::Valid(ClockState { pstate: p.to_string(), mhz: m }),
        _ => GpuValidity::Valid(ClockState { pstate: state.to_string(), mhz: f64::NAN }),
    }
}

fn read_view(root: &Json, path: &str, gated: bool) -> Result<ViewRead, String> {
    let runs = runs_of(root, path);
    if runs.is_empty() {
        return Err(format!("no run `{path}`"));
    }
    let frame = combined_frame(&runs).ok_or_else(|| format!("`{path}` has no frame statistics"))?;
    let metrics = combined_metrics(&runs);
    let (gpu, clock_note) = gpu_validity_of(root, &runs);
    let counts = |k: &str| runs.iter().filter_map(|r| r.get("counts").and_then(|c| count(c, k))).max().unwrap_or(0);
    // Water must be in every run of the view, not in one of them: the smallest count decides.
    let water_min = runs.iter().map(|r| r.get("counts").and_then(|c| count(c, "water_nodes")).unwrap_or(0)).min().unwrap_or(0);
    let rows = crate::lit::view_verdicts_with(&metrics, &frame, &gpu, gated, water_min, true, crate::lit::water_budget_ms(path));
    let mut facts = vec![format!(
        "{} run(s); waited frame median {:.3} / p99 {:.3} / max {:.3} ms; GPU clock state {}",
        runs.len(),
        frame.median,
        frame.p99,
        frame.max,
        match &gpu {
            GpuValidity::Valid(s) => format!("{s}"),
            GpuValidity::Below(_, r) => format!("below the floor ({r}): within-budget figures pass, over-budget ones are inconclusive"),
            GpuValidity::Invalid(r) => format!("NOT VALID for GPU budgets ({r})"),
        }
    )];
    facts.extend(clock_note);
    // The heat and the power beside the GPU rows (ADR 0057): the clock below the floor is the power cap trading clock for heat.
    let samples: Vec<&Json> = runs.iter().flat_map(|r| ["clocks_before", "clocks_after"].into_iter().filter_map(|k| r.get(k))).collect();
    let temps: Vec<f64> = samples.iter().filter_map(|c| num(c, "temperature_c")).collect();
    if let (Some(lo), Some(hi)) = (temps.iter().copied().min_by(f64::total_cmp), temps.iter().copied().max_by(f64::total_cmp)) {
        let power = samples.iter().filter_map(|c| num(c, "power_w")).fold(f64::MIN, f64::max);
        let cap = samples.iter().any(|c| str_at(c, "throttle_reasons").is_some_and(crate::gate::has_power_cap));
        facts.push(format!(
            "GPU thermal: {lo:.0}-{hi:.0} C, up to {power:.0} W{}",
            if cap { ", software power cap (0x4) read" } else { "" }
        ));
    }
    facts.push(format!(
        "{} nodes, {} with water, {} water draws; tile format {}",
        counts("nodes").max(counts("nodes_max")),
        counts("water_nodes"),
        counts("water_draws"),
        if counts("tile_format_compact") == 1 { "compact" } else { "vertex" }
    ));
    if let Some(Json::Obj(l)) = runs[0].get("load") {
        if let Some((_, Json::Num(c))) = l.iter().find(|(k, _)| k == "competing_mean_pct") {
            facts.push(format!("host load: competing CPU {c:.1} % (mean, recorded not judged)"));
        }
    }
    Ok(ViewRead { path: path.to_string(), rows, pass_line: pass_rows_line(&metrics), facts })
}

/// The refusals that apply to the header of a lit log (the session, the source, the field, the size).
fn check_header(root: &Json, what: &str, field_wanted: &str, why: &mut Vec<String>) {
    if !crate::validity::log_frame_valid(root) {
        let reasons = crate::validity::log_frame_void_reasons(root);
        why.push(format!(
            "{what}: the session is INVALID for frame verdicts (ADR 0020, ADR 0036): {}",
            if reasons.is_empty() {
                "no frame_valid or valid_for_budgets in the log (old format?)".to_string()
            } else {
                reasons.join("; ")
            }
        ));
    }
    let protocol = root.get("protocol");
    match protocol.and_then(|p| str_at(p, "source")) {
        None => why.push(format!("{what}: old-format log: no protocol.source")),
        Some(s)
            if !s.starts_with("store ")
                || s.contains("NOT the production source")
                || !s.contains(&format!(", {PRODUCTION_CELLS} cells")) =>
        {
            why.push(format!(
                "{what}: the bake was read from `{s}`, not from the production store of {PRODUCTION_CELLS} cells per face edge"
            ))
        }
        Some(_) => {}
    }
    match protocol.and_then(|p| str_at(p, "field")) {
        None => why.push(format!("{what}: old-format log: no protocol.field")),
        Some(f) if f != field_wanted => why.push(format!("{what}: protocol.field is `{f}`, not `{field_wanted}`")),
        Some(_) => {}
    }
    let size = protocol.and_then(|p| p.get("internal_resolution"));
    let ok = matches!(size, Some(Json::Arr(a)) if a.len() == 2 && a[0].num() == Some(WINDOW_SIZE.0) && a[1].num() == Some(WINDOW_SIZE.1));
    if !ok {
        why.push(format!("{what}: the frame size is not 2560x1440 (a smoke run or another size is never a window)"));
    }
}

/// Judge a window.
pub fn evaluate(files: &WindowFiles) -> Evaluation5 {
    let mut refusals: Vec<String> = Vec::new();
    // ---- lit.json ----
    let mut views: Vec<ViewRead> = Vec::new();
    let mut flights: Vec<FlightRead> = Vec::new();
    let mut load_notes: Vec<String> = Vec::new();
    let mut lit_source = String::new();
    let mut lit_adapter = String::new();
    let mut facts: Vec<String> = Vec::new();
    match &files.lit {
        None => refusals.push("lit: no log `lit.json` (the lit views and flights of `cargo xtask perf m5`)".into()),
        Some(bytes) => match parse(bytes) {
            Err(e) => refusals.push(format!("lit: the log is not readable JSON: {e}")),
            Ok(root) => {
                let mut why = Vec::new();
                check_header(&root, "lit", &lit_field(), &mut why);
                for v in STATIC_VIEWS {
                    let path = format!("lit/{v}");
                    let n = runs_of(&root, &path).len();
                    if n < 3 {
                        why.push(format!(
                            "lit: `{path}` has {n} run(s) where the protocol takes 3 (a partial log, `--views`, is not a window)"
                        ));
                    }
                }
                if why.is_empty() {
                    for v in STATIC_VIEWS {
                        match read_view(&root, &format!("lit/{v}"), true) {
                            Ok(r) => views.push(r),
                            Err(e) => why.push(format!("lit: {e}")),
                        }
                    }
                    for f in FLIGHT_PATHS {
                        // The other setting of the shore filter is named by its value: `@filter0` in logs written while the look's default
                        // was 1, `@filter1` since the owner made it 0 (M5 gate, 2026-10-18).
                        let f =
                            if runs_of(&root, &format!("lit/{f}")).is_empty() { f.replace("@filter0", "@filter1") } else { f.to_string() };
                        flights.push(read_view(&root, &format!("lit/{f}"), false).map_err(|e| format!("{e} (flights are report only)")));
                    }
                    lit_source = root.get("protocol").and_then(|p| str_at(p, "source")).unwrap_or("").to_string();
                    lit_adapter = format!(
                        "adapter {}; power scheme {}",
                        root.get("adapter").and_then(|a| str_at(a, "name")).unwrap_or("unknown"),
                        str_at(&root, "power_scheme").unwrap_or("not read")
                    );
                    facts.push(format!(
                        "lit.json: session valid for frame verdicts: yes; GPU verdicts: {}",
                        if crate::validity::log_gpu_session_valid(&root) {
                            "eligible".to_string()
                        } else {
                            format!("NOT VALID ({})", crate::validity::log_gpu_only_reasons(&root).join("; "))
                        }
                    ));
                    facts.extend(
                        crate::validity::session_verdict_notes(&root)
                            .into_iter()
                            .map(|n| format!("lit.json verdict derived from the logged data: {n}")),
                    );
                    if let Some(Json::Arr(n)) = root.get("load_notes") {
                        load_notes.extend(n.iter().filter_map(|x| if let Json::Str(s) = x { Some(format!("lit: {s}")) } else { None }));
                    }
                    if let Some(Json::Arr(w)) = root.get("warmups") {
                        for x in w {
                            if let (Some(view), Some(Json::Bool(reached))) = (str_at(x, "view"), x.get("reached")) {
                                if !reached {
                                    facts.push(format!("warm-up of {view}: the clock floor was not reached within the cap"));
                                }
                            }
                        }
                    }
                }
                refusals.extend(why);
            }
        },
    }
    // ---- the streaming scenes on the lit path ----
    let mut streams: Vec<SceneRead> = Vec::new();
    for scene in GATED_SCENES {
        match files.streams.iter().find(|f| f.scene == scene) {
            None => refusals.push(format!("{scene}-lit: no log was given")),
            Some(f) => match read_scene_as(f, true) {
                Ok(r) if scene == "dive" && !r.prefill.counts_for_the_gated_dive() => refusals.push(format!(
                    "{scene}-lit: the log's source `{}` says {}: the gated dive is judged only from a run that started with levels F-{} pre-filled in the tile pack (F the render floor, ADR 0045 point 3)",
                    r.source_text,
                    r.prefill.describe(),
                    crate::prefill::GATED_PREFILL_LEVELS
                )),
                Ok(r) => streams.push(r),
                Err(why) => refusals.extend(why.into_iter().map(|w| format!("{scene}-lit: {w}"))),
            },
        }
    }
    if refusals.is_empty() {
        for s in &streams {
            if s.source != lit_source.split("; ").next().unwrap_or("") {
                refusals.push(format!("{}-lit read `{}` while lit read `{lit_source}`: one window, one source", s.scene, s.source));
            }
            if !s.facts[0].eq(lit_adapter.as_str()) {
                refusals
                    .push(format!("{}-lit: {} against lit's {lit_adapter}: one window, one adapter and power scheme", s.scene, s.facts[0]));
            }
        }
    }
    if !refusals.is_empty() {
        return Evaluation5::Refused(refusals);
    }
    // ---- report-only parts ----
    let fast = match files.streams.iter().find(|f| f.scene == REPORT_ONLY_SCENE) {
        Some(f) => read_scene_as(f, true).and_then(|r| {
            if r.prefill.counts_for_the_gated_dive() {
                Ok(r)
            } else {
                Err(vec![format!("{REPORT_ONLY_SCENE}-lit: not shown, the log's source says {} (like the gated dive it is shown only from a pre-filled run)", r.prefill.describe())])
            }
        }),
        None => Err(vec![format!("{REPORT_ONLY_SCENE}-lit was not run (report-only)")]),
    };
    let cold = match files.streams.iter().find(|f| f.scene == COLD_SCENE) {
        Some(f) => read_scene_as(f, true).and_then(|r| {
            if r.prefill == PrefillState::Cold {
                Ok(r)
            } else {
                Err(vec![format!(
                    "{COLD_SCENE}-lit: not shown, the log's source says {} where the cold start must say cold",
                    r.prefill.describe()
                )])
            }
        }),
        None => Err(vec![format!("{COLD_SCENE}-lit was not run (tracked and report-only)")]),
    };
    // The report-only streaming logs must come from the lit session too, or they are not shown.
    let same_session = |r: SceneRead| -> Result<SceneRead, Vec<String>> {
        if r.source == lit_source.split("; ").next().unwrap_or("") && r.facts.first().map(String::as_str) == Some(lit_adapter.as_str()) {
            Ok(r)
        } else {
            Err(vec![format!("{}-lit: not shown, it was read from another source, adapter or power scheme than lit.json", r.scene)])
        }
    };
    let fast = fast.and_then(same_session);
    let cold = cold.and_then(same_session);
    let formats = match &files.formats {
        None => Err("lit-formats was not run (report-only; the compact against vertex ratio is TO FILL)".to_string()),
        Some(bytes) => parse(bytes).map_err(|e| format!("lit-formats: not readable JSON: {e}")).and_then(|root| {
            let mut why = Vec::new();
            check_header(&root, "lit-formats", &lit_field(), &mut why);
            if !why.is_empty() {
                return Err(format!("not shown: {}", why.join("; ")));
            }
            let mut lines: Vec<String> = crate::validity::session_verdict_notes(&root)
                .into_iter()
                .map(|n| format!("lit-formats verdict derived from the logged data: {n}"))
                .collect();
            for v in FORMAT_VIEWS {
                let c = runs_of(&root, &format!("lit-formats/{v}/compact"));
                let ratios: Vec<String> = ["terrain", "atmosphere", "water", "span"]
                    .iter()
                    .filter_map(|k| {
                        c.first()
                            .and_then(|r| r.get("counts"))
                            .and_then(|cs| count(cs, &format!("ratio_{k}_permille")))
                            .map(|p| format!("{k} {:.3}", p as f64 / 1000.0))
                    })
                    .collect();
                lines.push(if ratios.is_empty() {
                    format!("{v}: compact / vertex ratio not stored (unequal or invalid clock state, or not measured)")
                } else {
                    format!("{v}: compact / vertex GPU time ratio (median): {}", ratios.join(", "))
                });
            }
            if let Some(Json::Arr(n)) = root.get("load_notes") {
                load_notes.extend(n.iter().filter_map(|x| if let Json::Str(s) = x { Some(format!("lit-formats: {s}")) } else { None }));
            }
            Ok(lines)
        }),
    };
    // ---- the outcome ----
    let mut failing: Vec<String> = Vec::new();
    let mut incomplete: Vec<String> = Vec::new();
    for v in &views {
        let coast = COAST_VIEWS.iter().any(|c| v.path == format!("lit/{c}"));
        for r in &v.rows {
            let line = r.line(&v.path);
            match (&r.status, r.gated) {
                (Status::Over, true) => failing.push(line.trim().to_string()),
                (Status::NotValid(_), true) | (Status::NotMeasured, true) => incomplete.push(line.trim().to_string()),
                (Status::NotInView(_), true) if coast && r.measure.starts_with("water") => {
                    incomplete.push(format!("{} (a coast view must hold water)", line.trim()))
                }
                _ => {}
            }
        }
    }
    for s in &streams {
        for v in s.verdicts.iter().filter(|v| v.fails()) {
            failing.push(v.line(&format!("{}-lit", s.scene)).trim().to_string());
        }
    }
    let outcome = if !failing.is_empty() {
        Outcome::Fail
    } else if !incomplete.is_empty() {
        Outcome::Incomplete
    } else {
        Outcome::Pass
    };
    Evaluation5::Judged(Box::new(Judgement5 {
        source: streams.first().map(|s| s.source.clone()).unwrap_or_default(),
        field: streams.first().map(|s| s.field.clone()).unwrap_or_default(),
        views,
        flights,
        formats,
        streams,
        fast,
        cold,
        outcome,
        incomplete,
        failing,
        load_notes,
        facts,
    }))
}

/// The refusal printed when the logs cannot be judged: what is missing and the exact commands.
pub fn refusal_text(reasons: &[String], logs_dir: &str) -> String {
    let mut s =
        String::from("accept M5 REFUSES to judge the hardware criteria: no valid perf window in the logs, and no verdict is written.\n\n");
    for r in reasons {
        s += &format!("  - {r}\n");
    }
    s += &format!("\n{}", window_commands(logs_dir));
    s
}

/// The commands for the architect's window and for the acceptance after it.
pub fn window_commands(logs_dir: &str) -> String {
    format!(
        "Architect, in a perf window on Rasierklinge (coders never run these; the readiness gate of ADR 0020 decides; the store is the regenerated 16384-cell store with its pre-fill):\n  \
         cargo xtask perf stream dive --smoke --store <store>   (once, before the window: a read-only store check, not a measurement)\n  \
         cargo xtask perf m5 --store <store> --out {logs_dir}\n  \
         (runs `perf lit`, `perf lit-formats` and the seven streaming scenes with --lit, one child per streaming scene: lit.json, lit-formats.json and <scene>-lit.json with <scene>-lit.frames.txt in {logs_dir}/;\n  \
         do not pass --views, --allow-memory-bake or --smoke: those logs are refused)\n\
         Then:\n  cargo xtask accept M5\n  (reads {logs_dir}/, writes docs/milestones/M5-report.md from the logs when they are valid)\n"
    )
}

/// Findings the report must list that no log carries; the architect fills them after the window (first two columns fixed text).
pub const FINDING_SLOTS: [(&str, &str); 14] = [
    ("Look review (ADR 0037)", "the owner's single look review of the contact sheet, the six-frame strips and the 1440p flight videos (`cargo xtask capture flights`), shore_filter 0 and 1; sign-off or the changes asked for"),
    ("M4 budgets on the lit path", "the gated streaming rows above against the M4 report's debug-path figures (frame median and p99 per scene); the difference is the lit path's cost in the pipelined loop"),
    ("Atmosphere from orbit", "the orbit row's atmosphere sum against 1 ms (ad hoc 1.3 ms in item 5); if it misses: the orbit early-out for pixels outside the disc or a cheaper orbit march, not the Bruneton fallback"),
    ("Water pass", "the water sum per view against 2 ms (2.2 ms for coast 3 m, ADR 0060); the coast views at 3 m and 2 km"),
    ("Compact against vertex GPU ratio", "the `lit-formats` lines above; the M4 report left this open (ADR 0033)"),
    ("Per-pass costs", "the pass rows of every view: which pass is the largest where; exposure histogram and tone map at 1440p"),
    ("ADR 0059 (noise floors 0.1 ms per pass, 0.5 ms per frame total)", "the first window run with the floors: which views they rescued (the `run-to-run noise` lines of the console output) and whether their verdicts were stable; the owner confirms or amends"),
    ("Shore filter in motion", "glide and descent with shore_filter 0 and 1: the owner's judgement from the videos; the terrain beach shading flash (status, tracked) in the same frames"),
    ("Store regeneration (GLOBAL_VERSION 4)", "the 16384 regeneration time, merger bytes and the ocean channel's cost as `generate --global-dir` printed them (no log carries them)"),
    ("Host load", "the load lines of every case: any LOAD NOTE; whether a note should ever void a session (TEST-023)"),
    ("Proposed ADRs awaiting the owner", "0045, 0051, 0053 and the question of the vertex-format retirement (SPIKE-12)"),
    ("ADR 0037 window", "keep blessing first goldens of lit, sky and water under policy, or empty `ADR_0037_M5_CLASSES`"),
    ("Viewer manual checks", "mouse look locally (SPIKE-11) and the keyboard feel (VIEW-004), repeated at the gate"),
    ("Recommendation", "go / no-go for M6 (milestone gate after M5)"),
];

fn word(s: &Status, gated: bool) -> String {
    match (s, gated) {
        (Status::Ok, _) => "pass".to_string(),
        (Status::OkBelowFloor(r), _) => format!("ok at a low clock ({r}; a slower clock only overstates the time)"),
        (Status::Over, true) => "FAIL".to_string(),
        (Status::Over, false) => "over (not gated)".to_string(),
        (Status::NotValid(r), _) => format!("NOT VALID for GPU budgets ({r})"),
        (Status::NotInView(w), _) => format!("not in view ({w})"),
        (Status::Reported, _) => "reported".to_string(),
        (Status::NotMeasured, _) => "not measured".to_string(),
    }
}

fn view_rows(v: &ViewRead, only_gated: Option<bool>) -> String {
    v.rows
        .iter()
        .filter(|r| only_gated.is_none_or(|g| r.gated == g))
        .map(|r| {
            format!(
                "| {} | {} {} | {} | {} |\n",
                v.path,
                r.measure,
                r.stat,
                word(&r.status, r.gated),
                match (r.value, r.budget) {
                    (Some(x), Some(b)) => format!("{x:.3} ms against {b} ms"),
                    (Some(x), None) => format!("{x:.3} ms"),
                    _ => "n/a".to_string(),
                }
            )
        })
        .collect()
}

fn stream_rows(r: &SceneRead, scene_label: &str, only: impl Fn(&crate::streaming::Verdict) -> bool, as_reported: bool) -> String {
    r.verdicts
        .iter()
        .filter(|v| only(v))
        .map(|v| {
            let w = match v.status {
                crate::streaming::Status::Ok => "pass",
                crate::streaming::Status::Over if as_reported => "over (not gated)",
                crate::streaming::Status::Over => "FAIL",
                crate::streaming::Status::TrackedWithin => "tracked, within",
                crate::streaming::Status::TrackedOver => "tracked, over",
                crate::streaming::Status::Reported => "reported",
                crate::streaming::Status::NotMeasured => "not measured",
            };
            format!("| {scene_label} | {} | {w} | {} |\n", v.name, v.detail.replace('|', "/"))
        })
        .collect()
}

/// The report written from valid logs (`docs/milestones/M5-report.md`). `steps_md` is the automated steps' table.
pub fn render_report(j: &Judgement5, steps_md: &str, date: &str) -> String {
    let mut s = format!("# M5 Atmosphere & ocean: report ({date})\n\n");
    s += "Generated by `cargo xtask accept M5` from the perf logs of one valid window; every row is recomputed from the logs, nothing is estimated. The architect completes the sections marked TO FILL; the owner's look review (ADR 0037) and the go / no-go for M6 are the milestone gate.\n\n";
    s += &format!("Source: `{}`; field: `{}`.\n\n", j.source, j.field);
    s += &format!(
        "## Verdict on the hardware criteria: {}\n\nGated (the owner's M5 budgets of 2026-10-15, ADR 0034, ADR 0036): in each of the five held views the atmosphere (sky view + aerial perspective + composite) median at most 1 ms, the water (pass + scene copy) median at most 2 ms (2.2 ms for coast 3 m, ADR 0060), the terrain pass median at most 8 ms (M2's budget on the lit path, ADR 0034 point 4; GPU rows need a view valid for GPU figures, clock floor 1,400 MHz) and the waited frame p99 at most 20 ms (any clock state); on the five streaming scenes on the lit path every verdict of TEST-018 and TEST-019 (frame median 16.6 ms, p99 20 ms, no blank tile, VRAM, RAM, detail lag). Reported and never gated: the coastal flights, the compact against vertex ratio, the GPU span, `dive-fast` and `dive-cold` on the lit path, host load. INCOMPLETE means no gated row failed but some could not be judged; it is not a pass.\n\n",
        j.outcome.word()
    );
    if !j.failing.is_empty() {
        s += "Failing rows:\n\n";
        for f in &j.failing {
            s += &format!("- {f}\n");
        }
        s += "\n";
    }
    if !j.incomplete.is_empty() {
        s += "Rows that could not be judged (re-run the view in a window with the GPU at its loaded clock):\n\n";
        for f in &j.incomplete {
            s += &format!("- {f}\n");
        }
        s += "\n";
    }
    s += "## Criteria (gated): the lit views\n\n| View | Measure | Result | Evidence |\n|---|---|---|---|\n";
    for v in &j.views {
        s += &view_rows(v, Some(true));
    }
    s += "\n## Criteria (gated): the streaming scenes on the lit path\n\n| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
    for r in &j.streams {
        s += &stream_rows(
            r,
            &format!("{}-lit", r.scene),
            |v| {
                !matches!(
                    v.status,
                    crate::streaming::Status::TrackedWithin | crate::streaming::Status::TrackedOver | crate::streaming::Status::Reported
                )
            },
            false,
        );
    }
    s += "\n## Tracked and reported, not gated\n\n| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
    for v in &j.views {
        s += &view_rows(v, Some(false));
    }
    for r in &j.streams {
        s += &stream_rows(
            r,
            &format!("{}-lit", r.scene),
            |v| {
                matches!(
                    v.status,
                    crate::streaming::Status::TrackedWithin | crate::streaming::Status::TrackedOver | crate::streaming::Status::Reported
                )
            },
            false,
        );
    }
    s += "\n## GPU time per pass (median / p99 / max, ms)\n\n";
    for v in &j.views {
        s += &format!(
            "- **{}**: {}\n",
            v.path,
            v.pass_line
                .trim_start_matches("GPU ms per pass (median / p99 / max): ")
                .trim_start_matches("GPU ms per pass (median / p99 / max):")
        );
    }
    s += "\nThe atmosphere row of the table above is the sum of its three passes per frame; the water row is the water pass plus the scene copy (the gap before it); the static tables' upload is a queue write on a body's load and no pass of a steady frame.\n";
    s += "\n## Coastal flights (report only; shore_filter at the look's default and the other setting)\n\n";
    for f in &j.flights {
        match f {
            Ok(v) => {
                s += &format!(
                    "### {}\n\n| View | Measure | Result | Evidence |\n|---|---|---|---|\n{}\n- {}\n\n",
                    v.path,
                    view_rows(v, None),
                    v.pass_line
                );
            }
            Err(e) => s += &format!("- {e}\n"),
        }
    }
    s += "## Compact against vertex (report only)\n\n";
    match &j.formats {
        Ok(lines) => {
            for l in lines {
                s += &format!("- {l}\n");
            }
        }
        Err(e) => s += &format!("- {e}\n"),
    }
    s += "\n## Facts of the window (reported, never judged)\n\n";
    for f in &j.facts {
        s += &format!("- {f}\n");
    }
    for v in j.views.iter() {
        s += &format!("- **{}**\n", v.path);
        for f in &v.facts {
            s += &format!("  - {f}\n");
        }
    }
    for r in &j.streams {
        s += &format!("- **{}-lit**\n", r.scene);
        for f in &r.facts {
            s += &format!("  - {f}\n");
        }
    }
    s += "\n## Host load notes (recorded, never judged)\n\n";
    if j.load_notes.is_empty() {
        s += "- none\n";
    }
    for n in &j.load_notes {
        s += &format!("- {n}\n");
    }
    s += "\n## dive-fast on the lit path (report-only, not part of the verdict)\n\n";
    match &j.fast {
        Ok(r) => {
            s += "| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
            s += &stream_rows(r, "dive-fast-lit", |_| true, true);
        }
        Err(why) => {
            for w in why {
                s += &format!("- {w}\n");
            }
        }
    }
    s += "\n## dive-cold on the lit path (the cold start: tracked, report-only)\n\n";
    match &j.cold {
        Ok(r) => {
            s += "| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
            s += &stream_rows(r, "dive-cold-lit", |_| true, true);
        }
        Err(why) => {
            for w in why {
                s += &format!("- {w}\n");
            }
        }
    }
    s += "\n## Findings to list (TO FILL by the architect)\n\n| Finding | What to record | Value / decision |\n|---|---|---|\n";
    for (name, what) in FINDING_SLOTS {
        s += &format!("| {name} | {what} | TO FILL |\n");
    }
    s += "\n## Automated steps\n\n";
    s += steps_md;
    s += "\n## Open risks, learnings, proposed ADRs awaiting the owner, recommendation\n\nTO FILL by the architect (the `accept` skill, steps 3-6): the verifier's table over the milestone, open risks, learnings, the proposed ADRs awaiting the owner and the recommendation for M6. M5 is a go / no-go gate: stop for the owner.\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accept::{DIVE_DESCENT_FRAMES, DIVE_FRAMES, HOLD_FRAMES};
    use crate::lit::{metrics_of, LitFrame};
    use crate::streaming::{frames_text, FrameRecord};
    use crate::{gate, prefill, report_json_full, summarize, AdapterFacts, DeviceKind, Protocol, RunReport};

    const STORE: &str = "store C:/bake, 16384 cells";
    const SCHEME: &str = "Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)";

    #[derive(Clone)]
    struct Fx {
        source: String,
        field: String,
        mhz: u32,
        p8_start: bool,
        throttled: bool,
        size: (u32, u32),
        runs: usize,
        atmosphere_orbit: f64,
        water_nodes_coast: u64,
        skip_view: Option<&'static str>,
        stream_source: Option<String>,
        stream_tiles: &'static str,
    }

    fn fx() -> Fx {
        Fx {
            source: STORE.into(),
            field: lit_field(),
            mhz: 1700,
            p8_start: false,
            throttled: false,
            size: (2560, 1440),
            runs: 3,
            atmosphere_orbit: 0.8,
            water_nodes_coast: 400,
            skip_view: None,
            stream_source: None,
            stream_tiles: "compact",
        }
    }

    fn sample(mhz: u32) -> crate::ClockSample {
        crate::parse_nvidia_smi(&format!("NVIDIA GeForce RTX 3080 Ti Laptop GPU, {mhz}, 6000, 87.5, 61, P2, 0x0000000000000000")).unwrap()
    }

    // terrain, water, sky-view, aerial, composite, histogram, adapt, tonemap
    fn frame(atmo: f64, water: f64) -> LitFrame {
        LitFrame {
            total_ms: 9.0,
            cpu_ms: 0.4,
            pass_ms: [Some(3.0), Some(water), Some(atmo * 0.2), Some(atmo * 0.3), Some(atmo * 0.5), Some(0.1), Some(0.01), Some(0.2)],
            scene_copy_ms: Some(0.3),
            span_ms: Some(6.0),
        }
    }

    fn run(path: &str, f: &Fx, atmo: f64, water: f64, water_nodes: u64) -> RunReport {
        let frames: Vec<LitFrame> = (0..20).map(|_| frame(atmo, water)).collect();
        let totals: Vec<f64> = frames.iter().map(|x| x.total_ms).collect();
        RunReport {
            path_name: path.into(),
            vram_cap_gb: 8,
            stats: summarize(&totals, 0).unwrap(),
            clocks_before: Some(if f.p8_start {
                crate::parse_nvidia_smi("NVIDIA GeForce RTX 3080 Ti Laptop GPU, 210, 405, 15, 50, P8, 0x0000000000000001").unwrap()
            } else {
                sample(f.mhz)
            }),
            // The throttled fixture is backed by the logged data (accept judges throttling again from the masks): the orbit run reads
            // the hardware-slowdown bit.
            clocks_after: Some(if f.throttled && path == "lit/orbit" {
                crate::parse_nvidia_smi(&format!(
                    "NVIDIA GeForce RTX 3080 Ti Laptop GPU, {}, 6000, 87.5, 61, P2, 0x0000000000000008",
                    f.mhz
                ))
                .unwrap()
            } else {
                sample(f.mhz)
            }),
            metrics: metrics_of(&frames),
            counts: vec![
                ("nodes".into(), 3000),
                ("water_nodes".into(), water_nodes),
                ("water_draws".into(), water_nodes),
                ("tile_format_compact".into(), 1),
            ],
        }
    }

    fn adapter() -> AdapterFacts {
        AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete }
    }

    fn protocol(f: &Fx) -> Protocol {
        Protocol {
            warmup_frames: 60,
            measured_frames: 300,
            runs: 3,
            internal_resolution: f.size,
            field: Some(f.field.clone()),
            source: Some(f.source.clone()),
            ..Protocol::default()
        }
    }

    fn validity(f: &Fx) -> gate::Validity {
        gate::validity(&if f.throttled { vec!["lit/orbit".to_string()] } else { vec![] }, &[])
    }

    fn lit_json(f: &Fx) -> Vec<u8> {
        let mut runs = Vec::new();
        for v in STATIC_VIEWS {
            if f.skip_view == Some(v) {
                continue;
            }
            let atmo = if v == "orbit" { f.atmosphere_orbit } else { 0.5 };
            let wn = if COAST_VIEWS.contains(&v) { f.water_nodes_coast } else { 10 };
            for _ in 0..f.runs {
                runs.push(run(&format!("lit/{v}"), f, atmo, 1.2, wn));
            }
        }
        for p in FLIGHT_PATHS {
            runs.push(run(&format!("lit/{p}"), f, 1.4, 2.4, 300));
        }
        report_json_full(&adapter(), Some(SCHEME), &[], &validity(f), &protocol(f), &runs, &[], &[]).into_bytes()
    }

    fn formats_json(f: &Fx) -> Vec<u8> {
        let mut runs = Vec::new();
        for v in FORMAT_VIEWS {
            runs.push(run(&format!("lit-formats/{v}/vertex"), f, 0.5, 1.0, 100));
            let mut c = run(&format!("lit-formats/{v}/compact"), f, 0.5, 1.0, 100);
            c.counts.push(("ratio_terrain_permille".into(), 912));
            c.counts.push(("ratio_atmosphere_permille".into(), 1004));
            runs.push(c);
        }
        report_json_full(&adapter(), Some(SCHEME), &[], &validity(f), &protocol(f), &runs, &[], &[]).into_bytes()
    }

    fn stream_frames(scene: &str, wall: f64) -> Vec<FrameRecord> {
        let n = if scene.starts_with("dive") { DIVE_FRAMES } else { HOLD_FRAMES };
        (0..n)
            .map(|k| FrameRecord {
                wall_ms: wall + (k % 3) as f64 * 0.5,
                lag: Some(if scene.starts_with("dive") && k < DIVE_DESCENT_FRAMES { 0.4 } else { 0.0 }),
                upload_bytes: if scene.starts_with("look") { 0 } else { 100_000 },
                resident: 100,
                allocated_bytes: 1 << 30,
                ..FrameRecord::default()
            })
            .collect()
    }

    fn stream_files(scene: &str, f: &Fx, wall: f64) -> SceneFiles {
        let frames = stream_frames(scene, wall);
        let n = frames.len();
        let walls: Vec<f64> = frames.iter().map(|x| x.wall_ms).collect();
        let counts: Vec<(String, u64)> = vec![
            ("frames".into(), n as u64),
            ("blank_frames".into(), 0),
            ("upload_bytes_max".into(), frames.iter().map(|x| x.upload_bytes).max().unwrap_or(0)),
            ("allocated_bytes_max".into(), 1 << 30),
            ("clock_samples".into(), 12),
            ("clock_min_mhz".into(), 1470),
            ("pool_slots".into(), 4000),
            ("share_slots".into(), 9000),
            ("max_resident_slots".into(), 100),
            ("scene_alone".into(), 1),
            ("process_peak_bytes".into(), 5 << 30),
            ("driver_vram_bytes".into(), 3 << 30),
        ];
        let marker = if scene == "dive-cold" { prefill::COLD_MARKER.to_string() } else { prefill::prefilled_marker(3, 4) };
        let base = f.stream_source.clone().unwrap_or_else(|| f.source.clone());
        let r = RunReport {
            path_name: format!("stream-lit/{scene}"),
            vram_cap_gb: 8,
            stats: summarize(&walls, 0).unwrap(),
            clocks_before: Some(sample(1700)),
            clocks_after: Some(sample(1700)),
            metrics: vec![],
            counts,
        };
        let p = Protocol {
            warmup_frames: 0,
            measured_frames: 0,
            runs: 1,
            field: Some(format!("{PRODUCTION_FIELD}; lit path (atmosphere, water), {} tiles", f.stream_tiles)),
            source: Some(prefill::labelled(&base, &marker)),
            ..Protocol::default()
        };
        let json = report_json_full(&adapter(), Some(SCHEME), &[], &validity(f), &p, &[r], &[], &[]);
        SceneFiles { scene: scene.into(), json: Some(json.into_bytes()), frames: Some(frames_text(&frames)), last_valid_present: false }
    }

    fn window(f: &Fx) -> WindowFiles {
        WindowFiles {
            lit: Some(lit_json(f)),
            formats: Some(formats_json(f)),
            streams: ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km", "dive-cold"]
                .iter()
                .map(|s| stream_files(s, f, 8.0))
                .collect(),
        }
    }

    fn judged(e: Evaluation5) -> Judgement5 {
        match e {
            Evaluation5::Judged(j) => *j,
            Evaluation5::Refused(r) => panic!("expected a verdict, got a refusal: {r:?}"),
        }
    }

    fn refused(e: Evaluation5) -> String {
        match e {
            Evaluation5::Refused(r) => r.join("\n"),
            Evaluation5::Judged(j) => panic!("expected a refusal, got {:?}: {:?} {:?}", j.outcome, j.failing, j.incomplete),
        }
    }

    // spec: TEST-034
    #[test]
    fn a_view_recorded_below_the_floor_at_a_power_capped_p0_is_derived_valid_and_the_difference_is_printed() {
        // Window 9's coast 3 m: the run recorded `gpu_valid` false at P0 1,290 MHz; its samples carry the power cap only.
        let sample = |mhz: u32, pstate: &str| {
            format!("{{\"graphics_mhz\":{mhz},\"pstate\":\"{pstate}\",\"temperature_c\":70,\"power_w\":90,\"throttle_reasons\":\"0x0000000000000004\"}}")
        };
        let log = |pstate: &str| {
            let run = format!(
                "{{\"path\":\"lit/coast-3m\",\"median_ms\":5.8,\"gpu_valid\":false,\"gpu_clock_state\":\"P0 1290 MHz, below the 1400 MHz floor\",\"clocks_before\":{},\"clocks_after\":{}}}",
                sample(1290, pstate),
                sample(1300, pstate)
            );
            parse(
                format!("{{\"valid_for_budgets\":true,\"frame_valid\":true,\"gpu_session_valid\":true,\"warnings\":[],\"runs\":[{run}]}}")
                    .as_bytes(),
            )
            .unwrap()
        };
        let root = log("P0");
        let runs = runs_of(&root, "lit/coast-3m");
        let (gpu, note) = gpu_validity_of(&root, &runs);
        assert!(gpu.is_valid(), "{gpu:?} {:?}", crate::validity::session_verdict_notes(&root));
        assert!(note.is_some_and(|n| n.contains("judged again")));
        // In P2 (the cap beside another state) the verdict stays below the floor, as recorded: no difference to print.
        let root = log("P2");
        let runs = runs_of(&root, "lit/coast-3m");
        let (gpu, note) = gpu_validity_of(&root, &runs);
        assert!(matches!(gpu, GpuValidity::Below(..)) && note.is_none(), "{gpu:?} {note:?}");
    }

    // spec: TEST-026
    #[test]
    fn a_valid_window_within_the_budgets_passes_and_the_report_has_the_gated_and_report_only_parts_and_the_slots() {
        let j = judged(evaluate(&window(&fx())));
        assert_eq!(j.outcome, Outcome::Pass, "{:?} {:?}", j.failing, j.incomplete);
        assert_eq!(j.views.len(), 5);
        assert!(j.streams.iter().all(|s| s.facts[0].starts_with("adapter ")), "the cross-check reads the adapter line first");
        assert!(j.flights.iter().all(Result::is_ok) && j.formats.is_ok() && j.fast.is_ok() && j.cold.is_ok(), "{:?}", j.fast);
        // Flights are over the atmosphere and water budgets and still gate nothing.
        let glide = j.flights[0].as_ref().unwrap();
        assert!(glide.rows.iter().any(|r| r.line("g").contains("over (not gated)")), "{:?}", glide.rows);
        let r = render_report(&j, "| step | pass | 1 |\n", "2026-10-20");
        for needle in [
            "# M5 Atmosphere & ocean: report (2026-10-20)",
            "## Verdict on the hardware criteria: PASS",
            "## Criteria (gated): the lit views",
            "## Criteria (gated): the streaming scenes on the lit path",
            "## Tracked and reported, not gated",
            "## GPU time per pass",
            "## Coastal flights (report only",
            "## Compact against vertex (report only)",
            "orbit: compact / vertex GPU time ratio",
            "lit/orbit | atmosphere (sky view + aerial perspective + composite) median | pass | 0.800 ms against 1 ms",
            "lit/coast-3m | water (pass + scene copy) median | pass | 1.500 ms against 2.2 ms",
            "dive-lit | ",
            "## dive-fast on the lit path",
            "## dive-cold on the lit path",
            "| step | pass | 1 |",
        ] {
            assert!(r.contains(needle), "missing `{needle}` in:\n{r}");
        }
        assert_eq!(r.matches("| TO FILL |").count(), FINDING_SLOTS.len(), "every finding slot is TO FILL");
        assert!(r.contains("Look review (ADR 0037)") && r.contains("ADR 0059") && r.contains("Compact against vertex GPU ratio"));
    }

    // spec: TEST-026
    #[test]
    fn a_gated_row_over_its_budget_fails_the_window_and_is_named() {
        let j = judged(evaluate(&window(&Fx { atmosphere_orbit: 1.3, ..fx() })));
        assert_eq!(j.outcome, Outcome::Fail);
        assert!(
            j.failing.iter().any(|l| l.contains("lit/orbit") && l.contains("atmosphere") && l.contains("1.300 ms") && l.ends_with("OVER")),
            "{:?}",
            j.failing
        );
        assert_eq!(j.failing.len(), 1, "{:?}", j.failing);
        // A slow lit streaming scene fails through the harness's own verdicts (frames and statistics from one run).
        let mut w = window(&fx());
        w.streams[0] = stream_files("dive", &fx(), 30.0);
        let j = judged(evaluate(&w));
        assert_eq!(j.outcome, Outcome::Fail);
        assert!(j.failing.iter().any(|l| l.contains("dive-lit") && l.contains("frame")), "{:?}", j.failing);
        // The log's statistics and a per-frame log of another run disagree: refused rather than judged from a mixed pair.
        let mut w = window(&fx());
        w.streams[0].frames = stream_files("dive", &fx(), 30.0).frames;
        assert!(refused(evaluate(&w)).contains("disagree"));
    }

    // spec: TEST-026
    #[test]
    fn a_view_below_the_clock_floor_or_after_an_idle_start_is_incomplete_not_failed_and_not_passed() {
        // ADR 0057 (it changes this test's first half): at 1,150 MHz, below the floor, figures within their budgets pass ...
        let j = judged(evaluate(&window(&Fx { mhz: 1150, ..fx() })));
        assert_eq!(j.outcome, Outcome::Pass, "{:?} {:?}", j.failing, j.incomplete);
        assert!(j.views.iter().any(|v| v.rows.iter().any(|r| matches!(r.status, Status::OkBelowFloor(_)))));
        let r = render_report(&j, "", "d");
        assert!(r.contains("ok at a low clock") && r.contains("GPU thermal:"), "the clock and the heat stay beside the rows");
        // ... and a figure over its budget at that clock is inconclusive, as before the amendment.
        let j = judged(evaluate(&window(&Fx { mhz: 1150, atmosphere_orbit: 1.3, ..fx() })));
        assert_eq!(j.outcome, Outcome::Incomplete, "{:?}", j.failing);
        assert!(j.failing.is_empty() && !j.incomplete.is_empty());
        assert!(
            j.incomplete.iter().any(|l| l.contains("NOT VALID for GPU budgets") && l.contains("inconclusive") && l.contains("1150 MHz")),
            "{:?}",
            j.incomplete
        );
        let r = render_report(&j, "", "d");
        assert!(r.contains("Verdict on the hardware criteria: INCOMPLETE") && r.contains("could not be judged"));
        // An idle GPU start voids the GPU verdicts of the whole session; the frame rows are still judged.
        let j = judged(evaluate(&window(&Fx { p8_start: true, ..fx() })));
        assert_eq!(j.outcome, Outcome::Incomplete);
        assert!(j.views[0].rows.iter().any(|r| r.measure == "frame (waited)" && r.status == Status::Ok));
        assert!(j.incomplete.iter().all(|l| l.contains("NOT VALID")), "{:?}", j.incomplete);
    }

    // spec: TEST-026
    #[test]
    fn a_coast_view_without_water_is_incomplete_and_the_other_views_need_none() {
        let j = judged(evaluate(&window(&Fx { water_nodes_coast: 0, ..fx() })));
        assert_eq!(j.outcome, Outcome::Incomplete);
        assert!(
            j.incomplete.iter().any(|l| l.contains("lit/coast-2km") && l.contains("a coast view must hold water")),
            "{:?}",
            j.incomplete
        );
        assert!(!j.incomplete.iter().any(|l| l.contains("ground-1m")), "{:?}", j.incomplete);
    }

    // spec: TEST-026
    #[test]
    fn logs_that_cannot_be_trusted_are_refused_with_every_reason_and_the_commands() {
        let cases: Vec<(&str, Fx, &str)> = vec![
            ("throttled session", Fx { throttled: true, ..fx() }, "INVALID for frame verdicts"),
            ("a smoke size", Fx { size: (640, 360), ..fx() }, "not 2560x1440"),
            (
                "in-memory bake",
                Fx { source: "in-memory 128 cells (NOT the production source)".into(), ..fx() },
                "not from the production store",
            ),
            (
                "vertex tiles",
                Fx { field: format!("{PRODUCTION_FIELD}; lit path (atmosphere, water), vertex tiles"), ..fx() },
                "protocol.field",
            ),
            ("one run per view", Fx { runs: 1, ..fx() }, "where the protocol takes 3"),
            (
                "vertex tiles in the streams",
                Fx { stream_tiles: "vertex", ..fx() },
                "protocol.field is `height field cpu-erosion-v1 over a 16384-cell bake; lit path (atmosphere, water), vertex tiles`",
            ),
            ("a missing view", Fx { skip_view: Some("coast-3m"), ..fx() }, "`lit/coast-3m` has 0 run(s)"),
            (
                "another source for the streams",
                Fx { stream_source: Some("store D:/other, 16384 cells".into()), ..fx() },
                "one window, one source",
            ),
        ];
        for (name, f, needle) in cases {
            let text = refused(evaluate(&window(&f)));
            assert!(text.contains(needle), "{name}: {text}");
        }
        let mut w = window(&fx());
        w.lit = None;
        w.streams.retain(|s| s.scene != "look-1m");
        let text = refused(evaluate(&w));
        assert!(text.contains("lit: no log `lit.json`") && text.contains("look-1m-lit: no log was given"), "{text}");
        let r = refusal_text(&[text], "perf/logs");
        assert!(
            r.contains("accept M5 REFUSES")
                && r.contains("cargo xtask perf m5 --store <store> --out perf/logs")
                && r.contains("cargo xtask accept M5"),
            "{r}"
        );
        // A gated dive from a cold run is not the gated dive.
        let mut w = window(&fx());
        w.streams[0] = {
            let mut s = stream_files("dive", &fx(), 8.0);
            let text = String::from_utf8(s.json.take().unwrap()).unwrap();
            s.json = Some(text.replace(&prefill::prefilled_marker(3, 4), prefill::COLD_MARKER).into_bytes());
            s
        };
        assert!(refused(evaluate(&w)).contains("dive-lit: the log's source"));
    }

    // spec: TEST-026
    #[test]
    fn flights_formats_dive_fast_and_dive_cold_are_optional_and_never_refuse() {
        let mut w = window(&fx());
        w.formats = None;
        w.streams.retain(|s| s.scene != "dive-fast" && s.scene != "dive-cold");
        let mut j = judged(evaluate(&w));
        assert_eq!(j.outcome, Outcome::Pass);
        assert!(j.formats.is_err() && j.fast.is_err() && j.cold.is_err());
        let r = render_report(&j, "", "d");
        assert!(
            r.contains("lit-formats was not run") && r.contains("dive-fast-lit was not run") && r.contains("dive-cold-lit was not run"),
            "{r}"
        );
        // Flights absent from the log: noted, not refused.
        let f = fx();
        let mut runs = Vec::new();
        for v in STATIC_VIEWS {
            for _ in 0..3 {
                runs.push(run(&format!("lit/{v}"), &f, 0.5, 1.2, 400));
            }
        }
        w.lit = Some(report_json_full(&adapter(), Some(SCHEME), &[], &validity(&f), &protocol(&f), &runs, &[], &[]).into_bytes());
        j = judged(evaluate(&w));
        assert!(j.flights.iter().all(Result::is_err) && j.outcome == Outcome::Pass);
    }
}
