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
//! | a run started with the GPU in P5 or lower (idle clock) | NO               | yes                |
//! | a view's clock sample below the 1,400 MHz floor    | NO (per view)        | yes (that view)    |
//! | software power cap 0x4                             | no (recorded only)   | no                 |
//!
//! Frame verdicts (frame time, CPU times, plan time, counts, RAM, slots, lag) hold at any clock state of the GPU (owner,
//! 2026-10-15); only `gpu_ms`, the GPU budget and GPU comparisons need a GPU-valid view.

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
    c
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

/// Whether a log's frame verdicts count. New logs carry `frame_valid`. A log from before it carried only `valid_for_budgets`,
/// where an idle GPU start voided the whole session: it is re-read as frame-valid when every warning is that idle-start warning
/// (the warnings list every cause, so nothing else voided it). A log with neither field is not valid.
pub(crate) fn log_frame_valid(root: &Json) -> bool {
    if let Some(Json::Bool(b)) = root.get("frame_valid") {
        return *b;
    }
    if matches!(root.get("valid_for_budgets"), Some(Json::Bool(true))) {
        return true;
    }
    let ws = log_warnings(root);
    !ws.is_empty() && ws.iter().all(|w| is_low_start_warning(w)) && root.get("valid_for_budgets").is_some()
}

/// Whether a log's session is fit for GPU verdicts (old logs: exactly when `valid_for_budgets` is true).
pub(crate) fn log_gpu_session_valid(root: &Json) -> bool {
    match root.get("gpu_session_valid") {
        Some(Json::Bool(b)) => *b,
        _ => matches!(root.get("valid_for_budgets"), Some(Json::Bool(true))),
    }
}

/// The warnings of a log that void its frame verdicts (all but the idle-start warning).
pub(crate) fn log_frame_void_reasons(root: &Json) -> Vec<String> {
    log_warnings(root).into_iter().filter(|w| !is_low_start_warning(w)).collect()
}

/// The warnings of a log that void GPU verdicts only.
pub(crate) fn log_gpu_only_reasons(root: &Json) -> Vec<String> {
    log_warnings(root).into_iter().filter(|w| is_low_start_warning(w)).collect()
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
}
