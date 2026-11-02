//! Session validity, in one place (TEST-012, TEST-021; ADR 0020, ADR 0034 as accepted, ADR 0036): which cause voids the
//! frame verdicts of a session and which only the GPU verdicts. A pure function over measured facts, used by the harness, the log
//! writer, the console and `cargo xtask accept M4`, so that they cannot disagree.
//!
//! | cause                                              | voids frame verdicts | voids GPU verdicts |
//! |----------------------------------------------------|----------------------|--------------------|
//! | smoke run                                          | yes                  | yes                |
//! | not the reference adapter                          | yes                  | yes                |
//! | not a performance power scheme / unreadable        | yes                  | yes                |
//! | run validity rule (thermal/hardware throttling, noise, not the production source) | yes | yes |
//! | owner override of the readiness gate               | yes                  | yes                |
//! | a scene's CPU calibration outside the limits (ADR 0065, TEST-039) | yes   | yes                |
//! | loaded mode (`--loaded`, BUILD-030): the quiet verdicts do not count; the CPU calibration is recorded and does not void | yes | yes |
//! | a run started with the GPU in P5 or lower (idle clock) | NO               | yes                |
//! | a view's clock sample below the 1,400 MHz floor    | NO (per view)        | yes (that view)    |
//! | software power cap 0x4                             | no (recorded only)   | no                 |
//!
//! Frame verdicts (frame time, CPU times, plan time, counts, RAM, slots, lag) hold at any clock state of the GPU (owner,
//! 2026-10-15); only `gpu_ms`, the GPU budget and GPU comparisons need a GPU-valid view.

use crate::calibration::{self, Judgement};
use crate::compare::Json;
use crate::{check_reference_adapter, clock, gate, is_performance_scheme, AdapterFacts, RunReport};

/// What kind of cause made a session (partly) invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CauseKind {
    Smoke,
    WrongAdapter,
    PowerScheme,
    /// The after-the-run rules of [`gate::validity`] and the source rule: throttling, noise, not the production source.
    Run,
    OwnerOverride,
    /// A run started with the GPU in P5 or lower: the workload did not raise the clocks.
    LowStartClock,
    /// The scene's CPU calibration lies outside the limits of [`calibration`] before or after the case (ADR 0065): no verdict.
    CpuCalibration,
    /// The session ran in loaded mode (BUILD-030): its readings are recorded, and the quiet budget verdicts do not count; the loaded rules of BUILD-031 judge it.
    Loaded,
}

impl CauseKind {
    /// Whether the cause voids the frame, CPU, plan, count and RAM verdicts. Only the idle start does not.
    pub fn voids_frames(self) -> bool {
        !matches!(self, CauseKind::LowStartClock)
    }
    /// Whether the cause voids the GPU verdicts: every cause does (a view's own clock floor is judged per view).
    pub fn voids_gpu(self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cause {
    pub kind: CauseKind,
    pub text: String,
}

/// The causes found for a session, with what each voids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionVerdict {
    pub causes: Vec<Cause>,
}

/// The marker of the idle-start warning in a log written before `frame_valid` existed.
const LOW_START_MARKER: &str = "started with the GPU in";

/// Whether a warning text of a log is the idle-start cause (the only one that does not void frame verdicts).
pub fn is_low_start_warning(w: &str) -> bool {
    w.starts_with("run '") && w.contains(LOW_START_MARKER)
}

/// Whether a run takes GPU time at all: the pipelined streaming scenes (`stream/<scene>`, `stream-lit/<scene>`) do not, so the clock
/// they started at says nothing about any verdict (recorded, not judged; TEST-037).
pub fn takes_gpu_rows(path_name: &str) -> bool {
    !(path_name.starts_with("stream/") || path_name.starts_with("stream-lit/"))
}

/// The session-level causes from the machine and the runs (not the after-the-run rules): smoke, adapter, power scheme, idle start.
pub fn session_causes(adapter: &AdapterFacts, power_scheme: Option<&str>, runs: &[RunReport], smoke: bool) -> Vec<Cause> {
    let mut c = Vec::new();
    let mut push = |kind, text: String| c.push(Cause { kind, text });
    if smoke {
        push(
            CauseKind::Smoke,
            "this was a smoke run (reduced frame size and counts): it exercises the code path and is never a measurement".to_string(),
        );
    }
    if let Err(r) = check_reference_adapter(adapter) {
        push(CauseKind::WrongAdapter, format!("not the reference adapter: {r}"));
    }
    match power_scheme {
        Some(s) if is_performance_scheme(s) => {}
        Some(s) => push(
            CauseKind::PowerScheme,
            format!("power scheme is '{}', not a performance scheme; set the performance profile before measuring", s.trim()),
        ),
        None => push(CauseKind::PowerScheme, "the active power scheme could not be read".to_string()),
    }
    for r in runs.iter().filter(|r| takes_gpu_rows(&r.path_name)) {
        if let Some(cl) = &r.clocks_before {
            let low = cl.pstate.trim_start_matches('P').parse::<u32>().is_ok_and(|n| n >= 5);
            if low {
                push(
                    CauseKind::LowStartClock,
                    format!(
                        "run '{}' started with the GPU in {} at {} MHz: the workload is too light or the warm-up too short to reach steady clocks (GPU verdicts only; frame verdicts stand)",
                        r.path_name, cl.pstate, cl.graphics_mhz
                    ),
                );
                break;
            }
        }
    }
    // ADR 0065: a scene whose CPU calibration lies outside the limits gets no verdict (a scene without a calibration is not judged).
    let mut judged_paths: Vec<&str> = Vec::new();
    for r in runs {
        if judged_paths.contains(&r.path_name.as_str()) {
            continue;
        }
        let count = |k: &str| r.counts.iter().find(|(n, _)| n == k).map(|(_, v)| *v);
        if let Judgement::Outside(over) = calibration::judge(&count) {
            judged_paths.push(&r.path_name);
            push(CauseKind::CpuCalibration, calibration::reason(&r.path_name, &over));
        }
    }
    c
}

/// The CPU calibration verdict of a session (ADR 0065) for the log: `Some(true)` when every scene that logged a calibration is within the
/// limits, `Some(false)` with the reasons (one per scene) when one is not, `None` when no run logged a calibration (not judged).
pub fn calibration_status(runs: &[RunReport]) -> (Option<bool>, Vec<String>) {
    let (mut judged, mut reasons, mut seen) = (false, Vec::new(), Vec::<&str>::new());
    for r in runs {
        if seen.contains(&r.path_name.as_str()) {
            continue;
        }
        let count = |k: &str| r.counts.iter().find(|(n, _)| n == k).map(|(_, v)| *v);
        match calibration::judge(&count) {
            Judgement::NotJudged => {}
            Judgement::Within => judged = true,
            Judgement::Outside(over) => {
                judged = true;
                seen.push(&r.path_name);
                reasons.push(calibration::reason(&r.path_name, &over));
            }
        }
    }
    (judged.then_some(reasons.is_empty()), reasons)
}

impl SessionVerdict {
    /// All causes: the session-level ones, the after-the-run rules and the owner's override, in that order.
    pub fn assess(
        adapter: &AdapterFacts,
        power_scheme: Option<&str>,
        runs: &[RunReport],
        smoke: bool,
        validity: &gate::Validity,
        owner_override: &[String],
    ) -> SessionVerdict {
        let mut causes = session_causes(adapter, power_scheme, runs, smoke);
        // Loaded mode (ADR 0087, BUILD-030): the calibration limits of ADR 0065 are recorded, not a cause; the quiet verdicts do not count.
        if let Some(load) = &validity.load {
            causes.retain(|c| c.kind != CauseKind::CpuCalibration);
            causes.push(Cause {
                kind: CauseKind::Loaded,
                text: format!(
                    "loaded mode ({}): the quiet budget verdicts do not count; judge it by the loaded rules (BUILD-031)",
                    load.text()
                ),
            });
        }
        causes.extend(validity.reasons.iter().map(|r| Cause { kind: CauseKind::Run, text: r.clone() }));
        if !owner_override.is_empty() {
            causes.push(Cause {
                kind: CauseKind::OwnerOverride,
                text: format!("the owner overrode the readiness gate (failed: {})", owner_override.join("; ")),
            });
        }
        SessionVerdict { causes }
    }

    /// Whether frame, CPU, plan, count and RAM verdicts count.
    pub fn frame_valid(&self) -> bool {
        self.causes.iter().all(|c| !c.kind.voids_frames())
    }

    /// Whether the session as a whole is fit for GPU verdicts (each view is still judged against the clock floor).
    pub fn gpu_session_valid(&self) -> bool {
        self.causes.iter().all(|c| !c.kind.voids_gpu())
    }

    /// Every cause's text (the log's `warnings`).
    pub fn warnings(&self) -> Vec<String> {
        self.causes.iter().map(|c| c.text.clone()).collect()
    }

    /// Texts of the causes that void the GPU verdicts only (the log's `gpu_only_causes`).
    pub fn gpu_only(&self) -> Vec<String> {
        self.causes.iter().filter(|c| !c.kind.voids_frames()).map(|c| c.text.clone()).collect()
    }

    /// The console lines of the verdict, printed once before any budget verdict. The only place that words the session's validity.
    pub fn lines(&self) -> Vec<String> {
        self.lines_for(true)
    }

    /// [`lines`](SessionVerdict::lines) for a session whose runs take GPU rows (`gpu_rows`) or do not (the streaming scenes: "no GPU
    /// rows" instead of a GPU verdict that does not exist).
    pub fn lines_for(&self, gpu_rows: bool) -> Vec<String> {
        if self.causes.is_empty() && !gpu_rows {
            return vec!["SESSION VALID for budgets: frame verdicts: yes; no GPU rows (the pipelined scenes take no GPU time)".to_string()];
        }
        if self.causes.is_empty() {
            return vec![format!(
                "SESSION VALID for budgets: frame verdicts: yes; GPU verdicts: yes (each view judged against the {:.0} MHz clock floor)",
                clock::GPU_CLOCK_FLOOR_MHZ
            )];
        }
        if self.frame_valid() {
            return vec![format!("SESSION VALID for frame verdicts: yes; GPU verdicts: NOT VALID ({})", self.gpu_only().join("; "))];
        }
        let mut l = vec!["SESSION INVALID for budgets: frame verdicts: NO; GPU verdicts: NO; the verdicts below do not count".to_string()];
        for c in &self.causes {
            l.push(format!("  INVALID SESSION: {}", c.text));
        }
        l
    }
}

/// Whether a log has a `warnings` list and a recorded verdict field (`valid_for_budgets` or `frame_valid`): the inputs to judge it again.
/// A log that records the session as not valid without any reason in its warnings cannot be judged again (nothing says why): its
/// recorded flags stand.
fn has_verdict_inputs(root: &Json) -> bool {
    let unexplained =
        ["valid_for_budgets", "frame_valid", "gpu_session_valid"].iter().any(|k| matches!(root.get(k), Some(Json::Bool(false))))
            && log_warnings(root).is_empty();
    matches!(root.get("warnings"), Some(Json::Arr(_)))
        && (root.get("frame_valid").is_some() || root.get("valid_for_budgets").is_some())
        && !unexplained
}

/// The warnings of a log judged again with today's rules (accept derives every verdict from the logged data, not from the flags
/// written at run time). The throttling reasons are rebuilt from the logged throttle masks of every run and the noise reasons from
/// the logged run medians of every view with at least two runs, both through [`gate::validity`], the function the harness used; every
/// other cause (smoke, adapter, power scheme, source, owner override, idle start) stays as recorded. A kind whose inputs are not
/// all in the log keeps its recorded reasons. The second value lists what was kept for that reason.
fn recomputed_warnings(root: &Json) -> (Vec<String>, Vec<String>) {
    let recorded = log_warnings(root);
    let runs: Vec<&Json> = match root.get("runs") {
        Some(Json::Arr(a)) => a.iter().collect(),
        _ => Vec::new(),
    };
    let path_of = |r: &Json| match r.get("path") {
        Some(Json::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let mut kept = Vec::new();
    // Throttling: every run needs both clock samples with a throttle mask.
    let mask = |r: &Json, k: &str| match r.get(k).and_then(|c| c.get("throttle_reasons")) {
        Some(Json::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let throttle_complete =
        !runs.is_empty() && runs.iter().all(|r| mask(r, "clocks_before").is_some() && mask(r, "clocks_after").is_some());
    let throttled: Vec<String> = runs
        .iter()
        .filter(|r| ["clocks_before", "clocks_after"].iter().any(|k| mask(r, k).is_some_and(|m| gate::is_throttling(&m))))
        .map(|r| path_of(r))
        .collect();
    // Noise: the views with two or more runs (the streaming scenes are not noise-checked by the harness).
    let mut by_path: Vec<(String, Vec<f64>)> = Vec::new();
    for r in &runs {
        let (p, m) = (path_of(r), r.get("median_ms").and_then(Json::num));
        let (Some(m), true) = (m, takes_gpu_rows(&p)) else { continue };
        match by_path.iter_mut().find(|(q, _)| *q == p) {
            Some((_, v)) => v.push(m),
            None => by_path.push((p, vec![m])),
        }
    }
    by_path.retain(|(_, v)| v.len() >= 2);
    let noisy: Vec<(String, f64)> = by_path
        .iter()
        .filter(|(_, v)| gate::noise_needs_retry(v))
        .map(|(p, v)| (p.clone(), crate::run_to_run_noise(v).unwrap_or(f64::NAN)))
        .collect();
    let fresh = gate::validity(if throttle_complete { &throttled } else { &[] }, &noisy).reasons;
    // The CPU calibration (ADR 0065) is judged again from the logged counts of every run; a log with no calibration readings keeps what it
    // recorded (it was not judged then either).
    let (mut calibrated, mut cal_fresh, mut cal_paths) = (false, Vec::new(), Vec::<String>::new());
    for r in &runs {
        let count = |k: &str| r.get("counts").and_then(|c| c.get(k)).and_then(Json::num).map(|n| n as u64);
        match calibration::judge(&count) {
            Judgement::NotJudged => {}
            Judgement::Within => calibrated = true,
            Judgement::Outside(over) => {
                calibrated = true;
                let p = path_of(r);
                if !cal_paths.contains(&p) {
                    cal_fresh.push(calibration::reason(&p, &over));
                    cal_paths.push(p);
                }
            }
        }
    }
    let mut out = Vec::new();
    for w in &recorded {
        if calibrated && w.starts_with(calibration::REASON_PREFIX) {
            continue;
        }
        let is_throttle = w.starts_with(gate::THROTTLE_REASON_PREFIX);
        let is_noise = w.starts_with(gate::NOISE_REASON_PREFIX);
        let noise_path = is_noise.then(|| w.split(" on '").nth(1).and_then(|s| s.split('\'').next()).unwrap_or("").to_string());
        let rederived = (is_throttle && throttle_complete) || noise_path.as_ref().is_some_and(|p| by_path.iter().any(|(q, _)| q == p));
        if rederived {
            continue;
        }
        if is_throttle || is_noise {
            kept.push(format!("kept as recorded (inputs not in the log): {w}"));
        }
        out.push(w.clone());
    }
    out.extend(fresh);
    out.extend(cal_fresh);
    (out, kept)
}

fn recomputed_or_recorded(root: &Json) -> Vec<String> {
    if has_verdict_inputs(root) {
        recomputed_warnings(root).0
    } else {
        log_warnings(root)
    }
}

/// Where the session verdict derived from the logged data differs from the one recorded at run time, and which reasons stayed as
/// recorded for want of inputs. One line each; empty when they agree.
pub(crate) fn session_verdict_notes(root: &Json) -> Vec<String> {
    if !has_verdict_inputs(root) {
        return vec![
            "no `warnings` list, no verdict field or no reason for a recorded invalid session: the recorded flags are used as written"
                .to_string(),
        ];
    }
    let (now, kept) = recomputed_warnings(root);
    let recorded = log_warnings(root);
    let mut notes: Vec<String> = Vec::new();
    for w in recorded.iter().filter(|w| !now.contains(w)) {
        notes.push(format!("recorded at run time, not true when judged again: {w}"));
    }
    for w in now.iter().filter(|w| !recorded.contains(w)) {
        notes.push(format!("not recorded at run time, true when judged again: {w}"));
    }
    let flag = |k: &str| match root.get(k) {
        Some(Json::Bool(b)) => Some(*b),
        _ => None,
    };
    let (frame, gpu) = (log_frame_valid(root), log_gpu_session_valid(root));
    if let Some(r) = flag("frame_valid").filter(|r| *r != frame) {
        notes.push(format!("frame_valid: recorded {r}, judged again {frame}"));
    }
    if let Some(r) = flag("gpu_session_valid").filter(|r| *r != gpu) {
        notes.push(format!("gpu_session_valid: recorded {r}, judged again {gpu}"));
    }
    notes.extend(kept);
    notes
}

/// The GPU validity of a view from the logged clock samples of its runs (`clocks_before` and `clocks_after`), judged again by
/// [`clock::gpu_validity`] against today's floor. `None` when a run has no readable sample: the recorded verdict is used then.
pub(crate) fn view_gpu_validity_from_log(runs: &[&Json]) -> Option<clock::GpuValidity> {
    let point = |r: &Json, k: &str| -> Option<clock::ClockPoint> {
        let c = r.get(k)?;
        let pstate = match c.get("pstate") {
            Some(Json::Str(s)) => s.clone(),
            _ => return None,
        };
        let throttle = match c.get("throttle_reasons") {
            Some(Json::Str(t)) => t.clone(),
            _ => String::new(),
        };
        Some(clock::ClockPoint { pstate, mhz: c.get("graphics_mhz")?.num()?, throttle })
    };
    if runs.is_empty() {
        return None;
    }
    let mut points = Vec::new();
    for r in runs {
        for k in ["clocks_before", "clocks_after"] {
            points.push(Some(point(r, k)?));
        }
    }
    Some(clock::gpu_validity(&points))
}

/// Whether a log's frame verdicts count, derived from its warnings judged again ([`recomputed_warnings`]): none but the idle-start
/// warning. A log without `warnings` falls back to its recorded flags: `frame_valid`, else `valid_for_budgets`, else (an old log that
/// voided the whole session for an idle start) every warning being that one. A log with no verdict field is not valid.
pub(crate) fn log_frame_valid(root: &Json) -> bool {
    if has_verdict_inputs(root) {
        return recomputed_warnings(root).0.iter().all(|w| is_low_start_warning(w));
    }
    if let Some(Json::Bool(b)) = root.get("frame_valid") {
        return *b;
    }
    if matches!(root.get("valid_for_budgets"), Some(Json::Bool(true))) {
        return true;
    }
    let ws = log_warnings(root);
    !ws.is_empty() && ws.iter().all(|w| is_low_start_warning(w)) && root.get("valid_for_budgets").is_some()
}

/// Whether a log's session is fit for GPU verdicts: no warning left after judging them again (without `warnings`: the recorded
/// `gpu_session_valid`, else exactly when `valid_for_budgets` is true).
pub(crate) fn log_gpu_session_valid(root: &Json) -> bool {
    if has_verdict_inputs(root) {
        return recomputed_warnings(root).0.is_empty();
    }
    match root.get("gpu_session_valid") {
        Some(Json::Bool(b)) => *b,
        _ => matches!(root.get("valid_for_budgets"), Some(Json::Bool(true))),
    }
}

/// The warnings of a log, judged again, that void its frame verdicts (all but the idle-start warning).
pub(crate) fn log_frame_void_reasons(root: &Json) -> Vec<String> {
    recomputed_or_recorded(root).into_iter().filter(|w| !is_low_start_warning(w)).collect()
}

/// The warnings of a log, judged again, that void GPU verdicts only.
pub(crate) fn log_gpu_only_reasons(root: &Json) -> Vec<String> {
    recomputed_or_recorded(root).into_iter().filter(|w| is_low_start_warning(w)).collect()
}

fn log_warnings(root: &Json) -> Vec<String> {
    match root.get("warnings") {
        Some(Json::Arr(a)) => a.iter().filter_map(|w| if let Json::Str(s) = w { Some(s.clone()) } else { None }).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::parse;
    use crate::{parse_nvidia_smi, report_json, summarize, DeviceKind, Protocol};

    fn rtx() -> AdapterFacts {
        AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete }
    }
    const GOOD: &str = "Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)";

    fn run(pstate: &str, mhz: u32) -> RunReport {
        let c = parse_nvidia_smi(&format!("X, {mhz}, 405, 15, 50, {pstate}, 0x0")).unwrap();
        RunReport {
            // A view that takes GPU rows (the lit and terrain views); the streaming scenes do not (`takes_gpu_rows`).
            path_name: "lit/coast-2km".into(),
            vram_cap_gb: 8,
            stats: summarize(&[1.0, 2.0], 0).unwrap(),
            clocks_before: Some(c),
            clocks_after: None,
            metrics: vec![],
            counts: vec![],
        }
    }

    fn assess(runs: &[RunReport], smoke: bool, v: &gate::Validity, ov: &[String]) -> SessionVerdict {
        SessionVerdict::assess(&rtx(), Some(GOOD), runs, smoke, v, ov)
    }

    // spec: TEST-012
    #[test]
    fn an_idle_start_voids_only_the_gpu_verdicts_and_the_console_says_so_once() {
        let v = assess(&[run("P8", 210)], false, &gate::Validity::default(), &[]);
        assert!(v.frame_valid() && !v.gpu_session_valid());
        let l = v.lines();
        assert_eq!(l.len(), 1, "{l:?}");
        assert!(l[0].starts_with("SESSION VALID for frame verdicts: yes; GPU verdicts: NOT VALID (") && l[0].contains("P8"), "{l:?}");
        assert!(!l[0].contains("INVALID for budgets"), "the lines never say valid and invalid of the same thing: {l:?}");
    }

    // spec: TEST-037
    #[test]
    fn a_streaming_scene_started_at_an_idle_clock_voids_nothing_and_the_console_says_no_gpu_rows() {
        let mut r = run("P8", 210);
        for path in ["stream/dive", "stream-lit/teleport-1m"] {
            r.path_name = path.into();
            let v = assess(std::slice::from_ref(&r), false, &gate::Validity::default(), &[]);
            assert!(v.frame_valid() && v.gpu_session_valid() && v.warnings().is_empty(), "{path}: {:?}", v.warnings());
            let l = v.lines_for(false);
            assert_eq!(l.len(), 1, "{l:?}");
            assert!(l[0].contains("no GPU rows") && !l[0].contains("NOT VALID"), "{l:?}");
        }
        assert!(!takes_gpu_rows("stream/dive") && !takes_gpu_rows("stream-lit/look-2km"));
        assert!(takes_gpu_rows("lit/coast-glide") && takes_gpu_rows("terrain/ground-1m"));
        // A view that takes GPU rows still has its idle start voiding the GPU verdicts (the test above), and a real cause still
        // voids a streaming session.
        let throttled = gate::validity(&["stream/dive".into()], &[]);
        r.path_name = "stream/dive".into();
        let v = assess(std::slice::from_ref(&r), false, &throttled, &[]);
        assert!(!v.frame_valid());
    }

    // spec: TEST-012
    #[test]
    fn every_other_cause_voids_both_and_the_console_agrees() {
        let throttled = gate::validity(&["terrain/x".into()], &[]);
        let cases: Vec<(&str, SessionVerdict)> = vec![
            ("smoke", assess(&[run("P0", 1500)], true, &gate::Validity::default(), &[])),
            ("throttling", assess(&[run("P0", 1500)], false, &throttled, &[])),
            ("override", assess(&[run("P0", 1500)], false, &gate::Validity::default(), &["AC power connected".to_string()])),
            ("power scheme", SessionVerdict::assess(&rtx(), Some("Balanced"), &[run("P0", 1500)], false, &gate::Validity::default(), &[])),
            (
                "adapter",
                SessionVerdict::assess(
                    &AdapterFacts { name: "llvmpipe".into(), backend: "Vulkan".into(), kind: DeviceKind::Cpu },
                    Some(GOOD),
                    &[],
                    false,
                    &gate::Validity::default(),
                    &[],
                ),
            ),
        ];
        for (name, v) in cases {
            assert!(!v.frame_valid() && !v.gpu_session_valid(), "{name}");
            assert!(v.lines()[0].starts_with("SESSION INVALID for budgets"), "{name}: {:?}", v.lines());
        }
        // An idle start beside a real cause: the real cause voids the frames, and both are listed.
        let both = assess(&[run("P8", 210)], false, &throttled, &[]);
        assert!(!both.frame_valid() && both.causes.len() == 2);
    }

    // spec: TEST-012
    #[test]
    fn a_busy_start_and_the_power_cap_leave_the_session_valid_for_both() {
        let v = assess(&[run("P2", 1710)], false, &gate::validity(&[], &[]), &[]);
        assert!(v.frame_valid() && v.gpu_session_valid() && v.lines()[0].starts_with("SESSION VALID for budgets"));
    }

    // spec: TEST-012
    #[test]
    fn the_log_carries_both_validities_and_old_logs_are_re_read_from_their_warnings() {
        let proto = Protocol::default();
        let new = report_json(&rtx(), Some(GOOD), &[], &gate::Validity::default(), &proto, &[run("P8", 210)]);
        let j = parse(new.as_bytes()).unwrap();
        assert!(matches!(j.get("valid_for_budgets"), Some(Json::Bool(false))), "its old meaning is unchanged: everything valid");
        assert!(matches!(j.get("frame_valid"), Some(Json::Bool(true))) && matches!(j.get("gpu_session_valid"), Some(Json::Bool(false))));
        assert!(log_frame_valid(&j) && !log_gpu_session_valid(&j) && log_gpu_only_reasons(&j).len() == 1);
        let s = crate::compare::summarize_log(new.as_bytes()).unwrap();
        assert!(
            s.valid && !s.gpu.values().any(clock::GpuValidity::is_valid),
            "the regression report keeps the frame figures and loses the GPU ones"
        );
        // A log written before `frame_valid`: only valid_for_budgets and the warnings.
        let old_p8 = new.replace("\"frame_valid\":true", "\"x1\":0").replace("\"gpu_session_valid\":false", "\"x2\":0");
        let j = parse(old_p8.as_bytes()).unwrap();
        assert!(j.get("frame_valid").is_none());
        assert!(
            log_frame_valid(&j) && !log_gpu_session_valid(&j),
            "an old log whose only warning is the idle start keeps its frame verdicts"
        );
        // An old log with a real cause (a power scheme) stays invalid, with or without the idle start beside it.
        let bal = report_json(&rtx(), Some("Balanced"), &[], &gate::Validity::default(), &proto, &[run("P8", 210)]);
        let j = parse(bal.replace("\"frame_valid\":false", "\"x1\":0").as_bytes()).unwrap();
        assert!(!log_frame_valid(&j) && log_frame_void_reasons(&j).len() == 1);
        // No fields at all: not valid.
        assert!(!log_frame_valid(&parse(b"{\"runs\":[]}").unwrap()));
    }

    /// A log with the given recorded flags and warnings and one run per `(path, median_ms, before_mhz, after_mhz, throttle mask)`.
    fn log_of(frame_valid: bool, warnings: &[&str], runs: &[(&str, f64, f64, Option<f64>, &str)]) -> Json {
        let sample = |mhz: f64, mask: &str| {
            format!("{{\"graphics_mhz\":{mhz},\"pstate\":\"P0\",\"temperature_c\":60,\"power_w\":80,\"throttle_reasons\":\"{mask}\"}}")
        };
        let runs: Vec<String> = runs
            .iter()
            .map(|(p, m, b, a, mask)| {
                let after = a.map_or("null".to_string(), |a| sample(a, mask));
                format!("{{\"path\":\"{p}\",\"median_ms\":{m},\"clocks_before\":{},\"clocks_after\":{after}}}", sample(*b, mask))
            })
            .collect();
        let warnings: Vec<String> = warnings.iter().map(|w| format!("\"{w}\"")).collect();
        let text = format!(
            "{{\"valid_for_budgets\":{frame_valid},\"frame_valid\":{frame_valid},\"gpu_session_valid\":{frame_valid},\"warnings\":[{}],\"runs\":[{}]}}",
            warnings.join(","),
            runs.join(",")
        );
        parse(text.as_bytes()).unwrap()
    }

    // spec: TEST-012
    #[test]
    fn accept_judges_the_noise_again_from_the_logged_run_medians_and_says_where_it_differs() {
        // Window 9's orbit: invalid at run time (0.149 on a 1.5 ms frame), valid under ADR 0059's floor of 0.5 ms.
        let w = "run-to-run noise 0.149 on 'lit/orbit' is above 0.1 after one retry";
        let orbit: Vec<_> = [1.708, 1.557, 1.475].iter().map(|m| ("lit/orbit", *m, 1500.0, Some(1500.0), "0x4")).collect();
        let j = log_of(false, &[w], &orbit);
        assert!(log_frame_valid(&j) && log_gpu_session_valid(&j) && log_frame_void_reasons(&j).is_empty());
        let notes = session_verdict_notes(&j);
        assert!(notes.iter().any(|n| n.contains("not true when judged again") && n.contains("lit/orbit")), "{notes:?}");
        assert!(notes.iter().any(|n| n.starts_with("frame_valid: recorded false, judged again true")), "{notes:?}");
        // A 10 ms frame with a 1.5 ms spread that the run did not flag is invalid when judged again, with the harness's own text.
        let big: Vec<_> = [10.0, 11.5, 10.2].iter().map(|m| ("lit/aerial-20km", *m, 1500.0, Some(1500.0), "0x4")).collect();
        let j = log_of(true, &[], &big);
        assert!(!log_frame_valid(&j) && !log_gpu_session_valid(&j));
        assert_eq!(log_frame_void_reasons(&j), gate::validity(&[], &[("lit/aerial-20km".into(), 1.5 / 10.2)]).reasons);
        // A view with a single run (the flights) has no noise; the streaming scenes are not noise-checked.
        let one = [("lit/coast-glide", 13.2, 1500.0, Some(1500.0), "0x4")];
        assert!(log_frame_valid(&log_of(true, &[], &one)));
        let stream: Vec<_> = [10.0, 30.0, 10.0].iter().map(|m| ("stream/dive", *m, 1500.0, Some(1500.0), "0x0")).collect();
        assert!(log_frame_valid(&log_of(true, &[], &stream)));
    }

    // spec: TEST-012
    #[test]
    fn accept_judges_throttling_and_the_clock_floor_again_from_the_logged_samples() {
        // A thermal mask in a sample the run did not flag voids the session; the power cap alone does not.
        let hot = [("lit/coast-2km", 4.7, 1500.0, Some(1500.0), "0x8"), ("lit/coast-2km", 4.7, 1500.0, Some(1500.0), "0x4")];
        let j = log_of(true, &[], &hot);
        assert!(!log_frame_valid(&j) && log_frame_void_reasons(&j).iter().all(|r| r.starts_with(gate::THROTTLE_REASON_PREFIX)));
        let capped = [("lit/coast-2km", 4.7, 1500.0, Some(1500.0), "0x4")];
        assert!(log_frame_valid(&log_of(true, &[], &capped)));
        // A throttling reason the run recorded and the samples do not support goes away.
        let j = log_of(false, &["GPU throttling reported during 'lit/coast-2km'"], &capped);
        assert!(log_frame_valid(&j) && session_verdict_notes(&j).iter().any(|n| n.contains("not true when judged again")));
        // The clock floor is applied to the samples, whatever the run recorded: 1,395 MHz is below 1,400, 1,400 is at it.
        let view = |after: f64, mask: &str| log_of(true, &[], &[("lit/coast-2km", 4.7, 1500.0, Some(after), mask)]);
        let (below, at) = (view(1395.0, "0x0"), view(1400.0, "0x0"));
        fn runs_of(j: &Json) -> Vec<&Json> {
            match j.get("runs") {
                Some(Json::Arr(a)) => a.iter().collect(),
                _ => vec![],
            }
        }
        assert!(matches!(view_gpu_validity_from_log(&runs_of(&below)), Some(clock::GpuValidity::Below(..))));
        assert!(matches!(view_gpu_validity_from_log(&runs_of(&at)), Some(clock::GpuValidity::Valid(_))));
        // ADR 0060: the same 1,395 MHz at P0 with the power cap as the only reason is valid when judged again.
        let capped = view(1395.0, "0x4");
        assert!(matches!(view_gpu_validity_from_log(&runs_of(&capped)), Some(clock::GpuValidity::Valid(_))));
        // ...but not with a thermal bit beside the cap.
        let hot = view(1395.0, "0x44");
        assert!(matches!(view_gpu_validity_from_log(&runs_of(&hot)), Some(clock::GpuValidity::Below(..))));
    }

    // spec: TEST-012
    #[test]
    fn a_log_missing_the_inputs_keeps_the_recorded_verdict_and_says_so() {
        // No `after` sample: the throttling and the clock cannot be judged again; the recorded throttling reason stays.
        let w = "GPU throttling reported during 'lit/coast-2km'";
        let j = log_of(false, &[w], &[("lit/coast-2km", 4.7, 1500.0, None, "0x4")]);
        assert!(!log_frame_valid(&j) && log_frame_void_reasons(&j) == vec![w.to_string()]);
        assert!(session_verdict_notes(&j).iter().any(|n| n.starts_with("kept as recorded (inputs not in the log)")));
        let runs = match j.get("runs") {
            Some(Json::Arr(a)) => a.iter().collect::<Vec<_>>(),
            _ => vec![],
        };
        assert!(view_gpu_validity_from_log(&runs).is_none());
        // A recorded noise reason of a view whose runs are not in the log stays as well.
        let n = "run-to-run noise 0.250 on 'terrain/x' is above 0.1 and 0.5 ms after one retry";
        let j = log_of(false, &[n], &[]);
        assert!(!log_frame_valid(&j));
        // No warnings list: the recorded flags are used as written, and the notes say so.
        let j = parse(b"{\"valid_for_budgets\":true,\"runs\":[]}").unwrap();
        assert!(log_frame_valid(&j) && session_verdict_notes(&j)[0].contains("recorded flags are used as written"));
    }

    /// The counts of a scene's calibration (18 threads), before and after.
    fn calib(b: (u64, u64), a: (u64, u64)) -> Vec<(String, u64)> {
        let mut c = Vec::new();
        for (when, (single, parallel)) in [("before", b), ("after", a)] {
            c.push((format!("load_calib_{when}_threads"), 18));
            c.push((format!("load_calib_{when}_single_us"), single));
            c.push((format!("load_calib_{when}_parallel_median_us"), parallel));
        }
        c
    }

    // spec: TEST-039
    #[test]
    fn a_scene_outside_the_calibration_limits_has_no_verdict_and_the_log_says_which_reading() {
        let mut r = run("P0", 1500);
        r.path_name = "stream-lit/dive".into();
        // Within the limits (a quiet scene of window 11): valid, calibration valid in the log.
        r.counts = calib((46_296, 96_434), (46_902, 95_388));
        let v = assess(std::slice::from_ref(&r), false, &gate::Validity::default(), &[]);
        assert!(v.frame_valid() && v.warnings().is_empty());
        let proto = Protocol::default();
        let json = report_json(&rtx(), Some(GOOD), &[], &gate::Validity::default(), &proto, std::slice::from_ref(&r));
        assert!(json.contains("\"cpu_calibration_valid\":true,\"cpu_calibration_reasons\":[]"), "{json}");
        // The smoke dive disturbed by git: the after single reading is out.
        r.counts = calib((46_984, 105_071), (59_335, 91_121));
        let v = assess(std::slice::from_ref(&r), false, &gate::Validity::default(), &[]);
        assert!(!v.frame_valid() && !v.gpu_session_valid(), "no verdict: the whole scene does not count");
        assert_eq!(v.causes.len(), 1);
        assert_eq!(v.causes[0].kind, CauseKind::CpuCalibration);
        let reason =
            "no verdict: CPU calibration of 'stream-lit/dive' outside the limits (ADR 0065): after single 59,335 us over 56,000 us";
        assert_eq!(v.warnings(), vec![reason.to_string()]);
        assert!(v.lines().iter().any(|l| l.contains(reason)), "{:?}", v.lines());
        let json = report_json(&rtx(), Some(GOOD), &[], &gate::Validity::default(), &proto, std::slice::from_ref(&r));
        assert!(json.contains("\"frame_valid\":false") && json.contains("\"cpu_calibration_valid\":false"), "{json}");
        assert!(json.contains(&format!("\"cpu_calibration_reasons\":[\"{reason}\"]")), "{json}");
        // Three runs of one view carry the same calibration: one cause, not three.
        let three = vec![r.clone(), r.clone(), r.clone()];
        assert_eq!(assess(&three, false, &gate::Validity::default(), &[]).causes.len(), 1);
        // A scene without a calibration is not judged: valid, and `null` in the log.
        r.counts = vec![];
        let v = assess(std::slice::from_ref(&r), false, &gate::Validity::default(), &[]);
        assert!(v.frame_valid());
        let json = report_json(&rtx(), Some(GOOD), &[], &gate::Validity::default(), &proto, std::slice::from_ref(&r));
        assert!(json.contains("\"cpu_calibration_valid\":null,\"cpu_calibration_reasons\":[]"), "{json}");
    }

    // spec: BUILD-032
    #[test]
    fn a_one_off_log_is_stamped_with_its_mode_and_load_and_a_loaded_one_carries_verdicts() {
        let log = "{\n  \"scene\": \"global-bake\",\n  \"seconds\": 500.000,\n  \"budget_failures\": []\n}\n";
        let reading = [crate::OneOffReading { metric: "bake seconds (GEN-014)", reading: 500.0, budget: 600.0 }];
        let quiet = crate::stamp_one_off_json(log, &gate::Validity::default(), &reading);
        assert!(quiet.starts_with("{\n  \"mode\":\"quiet\",\n  \"scene\": \"global-bake\""), "{quiet}");
        assert!(!quiet.contains("load_record") && !quiet.contains("loaded_verdicts"), "{quiet}");
        let load = gate::LoadRecord {
            cpu_pct: Some(41.5),
            cpu_perf_pct: Some(88.0),
            gpu_util_pct: None,
            gpu_temp_c: None,
            processes: Some(vec![]),
        };
        let loaded = gate::Validity { reasons: vec![], load: Some(load) };
        let text = crate::stamp_one_off_json(log, &loaded, &reading);
        assert!(text.starts_with("{\n  \"mode\":\"loaded\",\"load_record\":{\"cpu_pct\":41.5,"), "{text}");
        // 500 s against 600 s is 16.7 % under: inside the 30 % margin of an unpinned CPU time, so the window queue decides.
        assert!(
            text.contains("\"loaded_verdicts\": [{\"metric\":\"bake seconds (GEN-014)\",\"reading\":500") && text.contains("window queue"),
            "{text}"
        );
        assert!(text.trim_end().ends_with("]\n}"), "the object still closes: {text}");
        let fast = [crate::OneOffReading { metric: "m", reading: 300.0, budget: 600.0 }];
        assert!(crate::stamp_one_off_json(log, &loaded, &fast).contains("loaded pass"));
        let over = [crate::OneOffReading { metric: "m", reading: 700.0, budget: 600.0 }];
        assert!(crate::stamp_one_off_json(log, &loaded, &over).contains("inconclusive"));
        assert_eq!(crate::mode_lines(&gate::Validity::default()), "mode: quiet\n");
        assert!(crate::mode_lines(&loaded).starts_with("mode: loaded\nload: "));
    }

    // spec: BUILD-030, BUILD-032
    #[test]
    fn a_loaded_session_records_its_calibration_and_load_and_its_quiet_verdicts_do_not_count() {
        let mut r = run("P0", 1500);
        r.path_name = "stream-lit/dive".into();
        r.counts = calib((46_984, 105_071), (59_335, 91_121));
        let load = gate::LoadRecord {
            cpu_pct: Some(41.5),
            cpu_perf_pct: Some(88.0),
            gpu_util_pct: Some(12.0),
            gpu_temp_c: Some(55.0),
            processes: Some(vec!["cargo.exe (pid 7)".into()]),
        };
        let loaded = gate::Validity { reasons: vec![], load: Some(load) };
        let v = assess(std::slice::from_ref(&r), false, &loaded, &[]);
        // The calibration over its limit is not a cause in loaded mode; the loaded mode is, and it carries the readings.
        assert_eq!(v.causes.len(), 1);
        assert_eq!(v.causes[0].kind, CauseKind::Loaded);
        assert!(v.causes[0].text.contains("CPU load 41.5 %") && v.causes[0].text.contains("cargo.exe (pid 7)"), "{}", v.causes[0].text);
        assert!(!v.frame_valid() && !v.gpu_session_valid());
        // The log names the mode on the session and on every row, with the load record, and still records the calibration reading.
        let json = report_json(&rtx(), Some(GOOD), &[], &loaded, &Protocol::default(), std::slice::from_ref(&r));
        assert!(json.starts_with("{\"mode\":\"loaded\",\"load_record\":{\"cpu_pct\":41.5,\"cpu_perf_pct\":88.0,"), "{json}");
        assert!(json.contains("\"processes\":[\"cargo.exe (pid 7)\"]") && json.contains("\"runs\":[{\"mode\":\"loaded\","), "{json}");
        assert!(json.contains("\"cpu_calibration_valid\":false"), "the reading stays recorded: {json}");
        assert!(
            json.contains("\"loaded_verdicts\":[{\"metric\":\"frame median ms\""),
            "a streaming row carries its loaded verdicts: {json}"
        );
        assert!(json.contains("\"verdict\":\"needs a quiet window\""), "{json}");
        let quiet = report_json(&rtx(), Some(GOOD), &[], &gate::Validity::default(), &Protocol::default(), std::slice::from_ref(&r));
        assert!(!quiet.contains("loaded_verdicts"), "a quiet row has none: {quiet}");
        assert!(quiet.starts_with("{\"mode\":\"quiet\",\"adapter\"") && quiet.contains("\"runs\":[{\"mode\":\"quiet\","), "{quiet}");
    }

    /// A log with one run whose counts are `counts`, recorded flags and warnings as given.
    fn log_with_counts(frame_valid: bool, warnings: &[&str], path: &str, counts: &[(String, u64)]) -> Json {
        let counts: Vec<String> = counts.iter().map(|(n, v)| format!("\"{n}\":{v}")).collect();
        let warnings: Vec<String> = warnings.iter().map(|w| format!("\"{w}\"")).collect();
        parse(
            format!(
                "{{\"valid_for_budgets\":{frame_valid},\"frame_valid\":{frame_valid},\"gpu_session_valid\":{frame_valid},\"warnings\":[{}],\"runs\":[{{\"path\":\"{path}\",\"median_ms\":5.0,\"counts\":{{{}}}}}]}}",
                warnings.join(","),
                counts.join(",")
            )
            .as_bytes(),
        )
        .unwrap()
    }

    // spec: TEST-039
    #[test]
    fn accept_judges_the_calibration_again_from_the_logged_counts_and_leaves_logs_without_one_alone() {
        // A log written before the rule, with the slow planner terminal's readings: now no verdict, with the reason.
        let slow = log_with_counts(true, &[], "stream/dive", &calib((66_877, 130_386), (48_553, 133_755)));
        assert!(!log_frame_valid(&slow) && !log_gpu_session_valid(&slow));
        let reasons = log_frame_void_reasons(&slow);
        assert_eq!(reasons.len(), 1);
        assert!(
            reasons[0].starts_with("no verdict: CPU calibration of 'stream/dive'")
                && reasons[0].contains("before single 66,877 us over 52,000 us"),
            "{reasons:?}"
        );
        assert!(session_verdict_notes(&slow).iter().any(|n| n.starts_with("not recorded at run time, true when judged again")));
        // A quiet scene of window 11 stays valid.
        assert!(log_frame_valid(&log_with_counts(true, &[], "stream-lit/dive", &calib((46_296, 96_434), (46_902, 95_388)))));
        // A reason recorded at run time that the logged counts do not support goes away; one the counts support stays (once).
        let w = "no verdict: CPU calibration of 'stream-lit/dive' outside the limits (ADR 0065): after single 59,335 us over 56,000 us";
        let ok = log_with_counts(false, &[w], "stream-lit/dive", &calib((46_296, 96_434), (46_902, 95_388)));
        assert!(log_frame_valid(&ok), "judged again with the counts of the log");
        let out = log_with_counts(false, &[w], "stream-lit/dive", &calib((46_984, 105_071), (59_335, 91_121)));
        assert!(!log_frame_valid(&out) && log_frame_void_reasons(&out) == vec![w.to_string()]);
        // A log without calibration counts (the lit scenes before this change) is not judged: valid, and a recorded reason stays as it was.
        let none = log_with_counts(true, &[], "stream-lit/dive", &[]);
        assert!(log_frame_valid(&none));
        let kept = log_with_counts(false, &[w], "stream-lit/dive", &[]);
        assert!(!log_frame_valid(&kept), "no counts to judge again: the recorded reason stands");
    }
}
