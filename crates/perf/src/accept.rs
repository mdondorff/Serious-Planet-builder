//! The hardware half of `cargo xtask accept M4` (TEST-021): reads the logs of a streaming perf window (`<scene>.json` and
//! `<scene>.frames.txt` per scene, TEST-019) and judges M4's exit criteria from them, or refuses. Pure over the file contents, so
//! tier A drives it with recorded fixtures; the xtask reads the files, runs the other steps and writes the report.
//!
//! M4's exit criteria (architecture report M4 row, amended by the owner 2026-10-15, ADR 0032): on the scenes `dive`,
//! `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km`: no blank tile (the draw's own count and the uncovered nodes of the
//! fresh selection, TEST-020), p99 frame at most 20 ms, VRAM within the terrain share and the tier, RAM at most 8 GiB (the scene's
//! own peak); the detail lag is gated for the dive descent and the still look, tracked for the teleport and the dive's ground
//! segment; `dive-fast` is report-only. The verdicts are the harness's own (`streaming::case_verdicts`), recomputed here from the
//! per-frame log, so the command and the perf scene cannot disagree; nothing is trusted that the log does not carry.

use crate::compare::{parse, Json};
use crate::streaming::{case_spec, case_verdicts, frames_text, FrameRecord, MemoryFacts, Status, Verdict, RAM_BUDGET_BYTES};
use crate::summarize;

/// The scenes that gate M4, in the order of ADR 0032 point 8.
pub const GATED_SCENES: [&str; 5] = ["dive", "teleport-1m", "teleport-2km", "look-1m", "look-2km"];
/// Run and reported, not part of the verdict (owner, 2026-10-15; ADR 0032 point 1).
pub const REPORT_ONLY_SCENE: &str = "dive-fast";
/// The production source: a stored bake of this many cells per face edge (GEN-014, TEST-019).
pub const PRODUCTION_CELLS: u64 = 16384;
/// What `protocol.field` must say: the world's default, eroded field over the production bake (GEN-018, GEN-017).
pub const PRODUCTION_FIELD: &str = "height field cpu-erosion-v1 over a 16384-cell bake";
/// ADR 0033 point 1: terrain's VRAM share at the mid tier (2 GiB of 8 GiB), the tier the perf scenes run (`vram_cap_gb` 8).
pub const MID_TIER_SHARE_BYTES: u64 = 2 << 30;
/// ADR 0033 point 1: the mid tier's VRAM cap.
pub const MID_TIER_CAP_BYTES: u64 = 8 << 30;
/// Frames of a case: the dive is 40 s (30 s descent, 10 s ground), the others 10 s at 1/60 s (TEST-018).
pub const DIVE_FRAMES: usize = 2400;
pub const DIVE_DESCENT_FRAMES: usize = 1800;
pub const HOLD_FRAMES: usize = 600;
/// The script's frame step (TEST-018).
pub const FRAME_STEP_S: f64 = 1.0 / 60.0;
/// How far the log's own frame statistics may differ from those recomputed from the per-frame log (`frames.txt` prints three
/// decimals).
const STATS_TOLERANCE_MS: f64 = 0.01;

/// The content of one scene's log files, as read from disk (`None`: the file is missing).
#[derive(Clone, Debug, Default)]
pub struct SceneFiles {
    pub scene: String,
    pub json: Option<Vec<u8>>,
    pub frames: Option<String>,
    /// Whether `<scene>.last-valid.json` exists beside the log (only mentioned in a refusal: it is never used).
    pub last_valid_present: bool,
}

/// One scene read from its log.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRead {
    pub scene: String,
    pub verdicts: Vec<Verdict>,
    /// Facts of the run that are reported and never judged (clock state, CPU clock, adapter, power plan, columns).
    pub facts: Vec<String>,
    pub source: String,
    pub field: String,
}

impl SceneRead {
    pub fn fails(&self) -> bool {
        self.verdicts.iter().any(Verdict::fails)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Judgement {
    /// The gated scenes, in order.
    pub scenes: Vec<SceneRead>,
    /// `dive-fast`: what its log said, or why it is unusable (never a reason to refuse).
    pub report_only: Result<SceneRead, Vec<String>>,
    pub pass: bool,
    pub source: String,
    pub field: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Evaluation {
    /// The logs cannot be judged (missing, invalid, not the production source, old format): no verdict, nothing to write.
    Refused(Vec<String>),
    Judged(Judgement),
}

/// Judge a window. `files` holds the logs of the gated scenes and, optionally, of `dive-fast`.
pub fn evaluate(files: &[SceneFiles]) -> Evaluation {
    let mut refusals: Vec<String> = Vec::new();
    let mut scenes = Vec::new();
    for scene in GATED_SCENES {
        match files.iter().find(|f| f.scene == scene) {
            None => refusals.push(format!("{scene}: no log was given")),
            Some(f) => match read_scene(f) {
                Ok(r) => scenes.push(r),
                Err(why) => refusals.extend(why.into_iter().map(|w| format!("{scene}: {w}"))),
            },
        }
    }
    if refusals.is_empty() {
        let (source, field) = (scenes[0].source.clone(), scenes[0].field.clone());
        for s in &scenes[1..] {
            if s.source != source || s.field != field || s.facts[0] != scenes[0].facts[0] {
                refusals.push(format!(
                    "{}: read `{}` / `{}` while dive read `{source}` / `{field}` ({} against {}): the five scenes must come from one window over one source, adapter and power scheme",
                    s.scene, s.source, s.field, s.facts[0], scenes[0].facts[0]
                ));
            }
        }
    }
    if !refusals.is_empty() {
        return Evaluation::Refused(refusals);
    }
    let report_only = match files.iter().find(|f| f.scene == REPORT_ONLY_SCENE) {
        Some(f) => read_scene(f),
        None => Err(vec![format!("{REPORT_ONLY_SCENE} was not run (report-only; no log was given)")]),
    };
    let pass = scenes.iter().all(|s| !s.fails());
    Evaluation::Judged(Judgement { source: scenes[0].source.clone(), field: scenes[0].field.clone(), scenes, report_only, pass })
}

fn str_at<'a>(j: &'a Json, key: &str) -> Option<&'a str> {
    match j.get(key) {
        Some(Json::Str(s)) => Some(s),
        _ => None,
    }
}

fn count(counts: &Json, key: &str) -> Option<u64> {
    counts.get(key).and_then(Json::num).map(|n| n as u64)
}

/// Parse a `frames.txt` into frame records. Columns that the file does not carry (the tile ids of blank and uncovered nodes)
/// stay empty; the counts are what the verdicts use.
pub fn parse_frames(text: &str) -> Result<Vec<FrameRecord>, String> {
    let expected = frames_text(&[]);
    let expected = expected.trim_end();
    let mut lines = text.lines();
    let header = lines.next().unwrap_or("").trim_end();
    if header != expected {
        return Err(format!("old-format frames.txt: columns are `{header}`, this build writes `{expected}`"));
    }
    let mut out = Vec::new();
    for (n, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() != 19 {
            return Err(format!("frames.txt line {}: {} columns, expected 19", n + 2, t.len()));
        }
        let bad = |what: &str| format!("frames.txt line {}: `{}` is not a {what}", n + 2, line);
        let f = |i: usize| t[i].parse::<f64>().map_err(|_| bad("number"));
        let u = |i: usize| t[i].parse::<u64>().map_err(|_| bad("count"));
        let opt = |i: usize| if t[i] == "-" { Ok(None) } else { f(i).map(Some) };
        if u(0)? != out.len() as u64 {
            return Err(format!("frames.txt line {}: frame index {} where {} was expected", n + 2, t[0], out.len()));
        }
        out.push(FrameRecord {
            plan_ms: f(1)?,
            scheduler_ms: f(2)?,
            upload_ms: f(3)?,
            record_ms: f(4)?,
            gpu_ms: opt(5)?,
            wall_ms: f(6)?,
            lag: opt(7)?,
            uncovered_nodes: u(8)? as usize,
            blank_nodes: u(9)? as usize,
            upload_bytes: u(10)?,
            resident: u(11)? as usize,
            in_flight: u(12)? as usize,
            queued: u(13)? as usize,
            dispatched: u(14)?,
            uploaded: u(15)?,
            cancelled: u(16)?,
            degraded: u(17)? as usize,
            allocated_bytes: u(18)?,
            ..FrameRecord::default()
        });
    }
    Ok(out)
}

/// Read and judge one scene's log; `Err` lists every reason it cannot be judged.
pub fn read_scene(f: &SceneFiles) -> Result<SceneRead, Vec<String>> {
    let scene = f.scene.as_str();
    let mut why: Vec<String> = Vec::new();
    let Some(json_bytes) = &f.json else {
        return Err(vec![format!("no log `{scene}.json`")]);
    };
    let Some(frames_txt) = &f.frames else {
        return Err(vec![format!("no per-frame log `{scene}.frames.txt` (a log from before the streaming scenes wrote it is old-format)")]);
    };
    let Some(spec_frames) = (match scene {
        "dive" | "dive-fast" => Some(DIVE_FRAMES),
        "teleport-1m" | "teleport-2km" | "look-1m" | "look-2km" => Some(HOLD_FRAMES),
        _ => None,
    }) else {
        return Err(vec![format!("`{scene}` is not a streaming scene")]);
    };
    let root = parse(json_bytes).map_err(|e| vec![format!("the log is not readable JSON: {e}")])?;
    // The session must count (ADR 0020 gate and throttling, ADR 0036; smoke, wrong adapter and power scheme are warnings too).
    if !matches!(root.get("valid_for_budgets"), Some(Json::Bool(true))) {
        let reasons = match root.get("warnings") {
            Some(Json::Arr(a)) => a.iter().filter_map(|w| if let Json::Str(s) = w { Some(s.as_str()) } else { None }).collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        why.push(format!(
            "the session is INVALID for budgets (ADR 0020, ADR 0036): {}{}",
            if reasons.is_empty() { "no valid_for_budgets in the log (old-format log?)".to_string() } else { reasons.join("; ") },
            if f.last_valid_present {
                "; an earlier `.last-valid.json` exists beside the log but is never used: a window's five scenes must come from one session"
            } else {
                ""
            }
        ));
    }
    let protocol = root.get("protocol");
    let source = protocol.and_then(|p| str_at(p, "source")).map(str::to_string);
    let field = protocol.and_then(|p| str_at(p, "field")).map(str::to_string);
    match &source {
        None => why.push("old-format log: no protocol.source (written before the streaming scenes recorded where the bake was read from)".into()),
        Some(s) if s.contains("NOT the production source") || !s.starts_with("store ") => {
            why.push(format!("the bake was read from `{s}`, not from the production store: the exit criteria are measured over a stored {PRODUCTION_CELLS}-cell bake"))
        }
        Some(s) if !s.ends_with(&format!(", {PRODUCTION_CELLS} cells")) => {
            why.push(format!("the bake was read from `{s}`: the production store has {PRODUCTION_CELLS} cells per face edge"))
        }
        Some(_) => {}
    }
    match &field {
        None => why.push("old-format log: no protocol.field".into()),
        Some(s) if s != PRODUCTION_FIELD => {
            why.push(format!("protocol.field is `{s}`, not `{PRODUCTION_FIELD}` (the world's eroded field)"))
        }
        Some(_) => {}
    }
    let run = match root.get("runs") {
        Some(Json::Arr(a)) if a.len() == 1 && str_at(&a[0], "path") == Some(&format!("stream/{scene}")) => Some(&a[0]),
        _ => {
            why.push(format!("the log does not hold exactly one run `stream/{scene}`"));
            None
        }
    };
    let (Some(run), Some(counts)) = (run, run.and_then(|r| r.get("counts"))) else { return Err(why) };
    let required = [
        "frames",
        "blank_frames",
        "upload_bytes_max",
        "allocated_bytes_max",
        "pool_slots",
        "share_slots",
        "max_resident_slots",
        "scene_alone",
    ];
    let absent: Vec<&str> = required.iter().copied().filter(|k| count(counts, k).is_none()).collect();
    if !absent.is_empty() {
        why.push(format!(
            "old-format log: counts {} are missing (a log from before the per-scene process and slot figures)",
            absent.join(", ")
        ));
    } else if count(counts, "scene_alone") != Some(1) {
        why.push("the RAM figure is the peak of a process that ran several scenes, not the scene's own: run `cargo xtask perf stream all` (one child per scene)".into());
    }
    let ram = count(counts, "process_peak_bytes");
    if ram.is_none() && absent.is_empty() {
        why.push(
            "RAM was not measured (the process working set could not be read): the RAM criterion is gated and may not be \"not measured\""
                .into(),
        );
    }
    let frames = match parse_frames(frames_txt) {
        Ok(fr) => fr,
        Err(e) => {
            why.push(e);
            return Err(why);
        }
    };
    if frames.len() != spec_frames {
        why.push(format!(
            "{} frames where the full script has {spec_frames} (a smoke or partial run is never a measurement)",
            frames.len()
        ));
    }
    for (key, mine) in [
        ("upload_bytes_max", frames.iter().map(|f| f.upload_bytes).max().unwrap_or(0)),
        ("allocated_bytes_max", frames.iter().map(|f| f.allocated_bytes).max().unwrap_or(0)),
    ] {
        if count(counts, key).is_some_and(|n| n != mine) {
            why.push(format!(
                "the log counts {key} {} but frames.txt holds {mine} (files from different runs?)",
                count(counts, key).unwrap_or(0)
            ));
        }
    }
    if count(counts, "frames").is_some_and(|n| n != frames.len() as u64) {
        why.push(format!("the log counts {} frames but frames.txt holds {}", count(counts, "frames").unwrap_or(0), frames.len()));
    }
    // The log's own frame statistics must be the ones of the per-frame log.
    let wall: Vec<f64> = frames.iter().map(|f| f.wall_ms).collect();
    let stat = |k: &str| run.get(k).and_then(Json::num);
    match (summarize(&wall, 0), stat("p99_ms"), stat("median_ms"), stat("max_ms")) {
        (Some(s), Some(p99), Some(med), Some(max)) => {
            if (s.p99 - p99).abs() > STATS_TOLERANCE_MS
                || (s.median - med).abs() > STATS_TOLERANCE_MS
                || (s.max - max).abs() > STATS_TOLERANCE_MS
            {
                why.push(format!(
                    "the log and frames.txt disagree: log p99 {p99:.3} / median {med:.3} / max {max:.3} ms, frames.txt p99 {:.3} / median {:.3} / max {:.3} ms (files from different runs?)",
                    s.p99, s.median, s.max
                ));
            }
        }
        (None, ..) => why.push("frames.txt holds no frames".into()),
        _ => why.push("the log has no frame statistics (p99_ms, median_ms)".into()),
    }
    if !why.is_empty() {
        return Err(why);
    }
    let ram = ram.expect("checked above");
    let pool_slots = count(counts, "pool_slots").unwrap_or(0);
    let share_slots = count(counts, "share_slots").unwrap_or(0);
    let mem = MemoryFacts {
        share_bytes: MID_TIER_SHARE_BYTES,
        vram_cap_bytes: MID_TIER_CAP_BYTES,
        driver_vram_bytes: count(counts, "driver_vram_bytes"),
        process_peak_bytes: Some(ram),
        scene_alone: true,
        pool_slots,
        share_slots,
    };
    let spec = case_spec(scene, frames.len(), FRAME_STEP_S, DIVE_DESCENT_FRAMES).expect("a streaming scene");
    let mut verdicts = case_verdicts(&spec, &frames, &[], &mem);
    let blank_logged = count(counts, "blank_frames").unwrap_or(0);
    if blank_logged > 0 {
        verdicts.push(Verdict {
            name: "blank frames (log count)",
            status: Status::Over,
            detail: format!("the run logged {blank_logged} frames that drew a blank tile; budget none"),
        });
    }
    let mut facts = Vec::new();
    facts.push(format!(
        "adapter {}; power scheme {}",
        root.get("adapter").and_then(|a| str_at(a, "name")).unwrap_or("unknown"),
        str_at(&root, "power_scheme").unwrap_or("not read")
    ));
    let gpu_valid = matches!(run.get("gpu_valid"), Some(Json::Bool(true)));
    facts.push(format!(
        "GPU clock state {}{} ({} samples, lowest {} MHz); frame verdicts stand at any clock state, GPU time is not measured in the pipelined scenes",
        str_at(run, "gpu_clock_state").unwrap_or("unknown"),
        if gpu_valid { "" } else { " (NOT valid for GPU budgets)" },
        count(counts, "clock_samples").map_or("?".to_string(), |n| n.to_string()),
        count(counts, "clock_min_mhz").map_or("?".to_string(), |n| n.to_string())
    ));
    facts.push(format!(
        "CPU effective clock before / after the case: {} / {} MHz (recorded, never judged)",
        count(counts, "cpu_clock_before_mhz").map_or("not measured".to_string(), |n| n.to_string()),
        count(counts, "cpu_clock_after_mhz").map_or("not measured".to_string(), |n| n.to_string())
    ));
    facts.push(format!(
        "terrain slots: {pool_slots} allocated of {share_slots} share slots, at most {} resident; upload at most {} bytes per frame; RAM {:.2} GiB of {:.0} GiB",
        count(counts, "max_resident_slots").unwrap_or(0),
        count(counts, "upload_bytes_max").unwrap_or(0),
        ram as f64 / (1u64 << 30) as f64,
        RAM_BUDGET_BYTES as f64 / (1u64 << 30) as f64
    ));
    Ok(SceneRead { scene: scene.to_string(), verdicts, facts, source: source.unwrap_or_default(), field: field.unwrap_or_default() })
}

/// The refusal printed (and the exit status) when the logs cannot be judged: what is missing and the exact commands.
pub fn refusal_text(reasons: &[String], logs_dir: &str) -> String {
    let mut s =
        String::from("accept M4 REFUSES to judge the hardware criteria: no valid perf window in the logs, and no verdict is written.\n\n");
    for r in reasons {
        s += &format!("  - {r}\n");
    }
    s += &format!("\n{}", window_commands(logs_dir));
    s
}

/// The commands for the architect's window and for the acceptance after it.
pub fn window_commands(logs_dir: &str) -> String {
    format!(
        "Architect, in a perf window on Rasierklinge (coders never run these; the readiness gate of ADR 0020 decides):\n  \
         cargo xtask perf stream all --store <the 16384-cell store, e.g. target/accept/global16384>\n  \
         (one child per scene, a log per scene in {logs_dir}/: <scene>.json and <scene>.frames.txt for dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km;\n  \
         a scene whose session is INVALID is re-run the same way; do not pass --allow-memory-bake or --smoke: those logs are refused)\n\
         Then:\n  cargo xtask accept M4\n  (reads {logs_dir}/, writes docs/milestones/M4-report.md from the logs when they are valid)\n"
    )
}

fn word(s: Status) -> &'static str {
    match s {
        Status::Ok => "pass",
        Status::Over => "FAIL",
        Status::TrackedWithin => "tracked, within",
        Status::TrackedOver => "tracked, over",
        Status::Reported => "reported",
        Status::NotMeasured => "not measured",
    }
}

/// The verdict lines of one scene as table rows.
fn rows(r: &SceneRead, only: impl Fn(&Verdict) -> bool) -> String {
    r.verdicts
        .iter()
        .filter(|v| only(v))
        .map(|v| format!("| {} | {} | {} | {} |\n", r.scene, v.name, word(v.status), v.detail.replace('|', "/")))
        .collect()
}

/// Findings the report must list that no log carries; the architect fills them after the window. The first two columns are
/// fixed text, the third is the slot.
pub const FINDING_SLOTS: [(&str, &str); 9] = [
    (
        "Cold start",
        "a cold viewer start waits for the root load: 10.4-10.9 s for six roots after #157 (103 s before); cold OS file cache unmeasured",
    ),
    ("Teleport detail lag (tracked)", "the recovery times of both teleport scenes, from the table above"),
    (
        "Dive ground segment (tracked)",
        "the first window sets the 1.5 m/s budget (owner decision 2026-10-15); the value is in the table above",
    ),
    (
        "Upload cap (ADR 0033 point 9, provisional)",
        "1 MiB per frame; upload time is a perf-window figure (p99 in the table above); is the cap right?",
    ),
    (
        "RAM split (ADR 0033 point 8, proposed)",
        "terrain's proposed 3 GB of the 8 GB; the scene's own peak above; accept or amend the split",
    ),
    ("Prefetch look-ahead (ADR 0031 question 2)", "2 s proposed; decide from the dive and walking scenes"),
    ("Compact vs vertex GPU ratio", "GPU time is not measured in the pipelined scenes; needs the terrain scene's A/B at equal clock state"),
    ("gpu_ms in pipelined scenes", "unmeasured (no timestamp readback without a wait); tracked, not needed for the M4 exit"),
    ("VRAM whole-GPU figure", "an upper bound for the scene (all processes); judged beside the allocator's slots against the share"),
];

/// The report written from valid logs (to `docs/milestones/M4-report.md`). `steps_md` is the automated steps' table.
pub fn render_report(j: &Judgement, steps_md: &str, date: &str) -> String {
    let mut s = format!("# M4 Streaming: report ({date})\n\n");
    s += "Generated by `cargo xtask accept M4` from the perf logs of one valid window; the criteria rows are recomputed from the per-frame logs, nothing is estimated. The architect completes the sections marked TO FILL and adds the owner's recommendation.\n\n";
    s += &format!("Source: `{}`; field: `{}`.\n\n", j.source, j.field);
    s += &format!(
        "## Verdict on the hardware criteria: {}\n\nExit criteria (architecture report M4 row as amended by the owner 2026-10-15, ADR 0032): on `dive`, `teleport-1m`, `teleport-2km`, `look-1m`, `look-2km`: no blank tiles (draw count and uncovered nodes), p99 frame <= 20 ms, VRAM within the terrain share and tier, RAM <= 8 GiB (the scene's own peak); detail lag gated for the dive descent and the still look, tracked for the teleports and the dive's ground segment; `dive-fast` report-only.\n\n",
        if j.pass { "PASS" } else { "FAIL" }
    );
    s += "## Criteria (gated)\n\n| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
    for r in &j.scenes {
        s += &rows(r, |v| !matches!(v.status, Status::TrackedWithin | Status::TrackedOver | Status::Reported));
    }
    s += "\n## Tracked, not gated\n\n| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
    for r in &j.scenes {
        s += &rows(r, |v| matches!(v.status, Status::TrackedWithin | Status::TrackedOver | Status::Reported));
    }
    s += "\n## Facts of the window (reported, never judged)\n\n";
    for r in &j.scenes {
        s += &format!("- **{}**\n", r.scene);
        for f in &r.facts {
            s += &format!("  - {f}\n");
        }
    }
    s += "\n## dive-fast (report-only, not part of the verdict)\n\n";
    match &j.report_only {
        Ok(r) => {
            s += "| Scene | Measure | Result | Evidence |\n|---|---|---|---|\n";
            s += &rows(r, |_| true);
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
    s += "\n## Open risks, learnings, proposed ADRs awaiting the owner, recommendation\n\nTO FILL by the architect (the `accept` skill, steps 3-6): the verifier's table over the milestone, open risks, learnings, the proposed ADRs awaiting the owner, and the recommendation for M5. M4 is not a go/no-go gate.\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gate, report_json_with_warmups, AdapterFacts, DeviceKind, Protocol, RunReport};

    const GOOD_SCHEME: &str = "Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)";
    const STORE: &str = "store D:/bake, 16384 cells";

    /// A recorded window's knobs: each fixture starts valid and a test breaks one thing.
    #[derive(Clone)]
    struct Fx {
        source: String,
        field: String,
        invalid: Vec<String>,
        ram: Option<u64>,
        scene_alone: u64,
        pool_slots: u64,
        share_slots: u64,
        driver_vram: Option<u64>,
        omit_source: bool,
        omit_count: Option<&'static str>,
        blank_logged: u64,
    }

    fn fx() -> Fx {
        Fx {
            source: STORE.into(),
            field: PRODUCTION_FIELD.into(),
            invalid: vec![],
            ram: Some(5 << 30),
            scene_alone: 1,
            pool_slots: 4000,
            share_slots: 9000,
            driver_vram: Some(3 << 30),
            omit_source: false,
            omit_count: None,
            blank_logged: 0,
        }
    }

    fn n_frames(scene: &str) -> usize {
        if scene.starts_with("dive") {
            DIVE_FRAMES
        } else {
            HOLD_FRAMES
        }
    }

    /// Good frames of a scene: 8 ms of work, lag within every budget, nothing blank, one MiB at most.
    fn good_frames(scene: &str) -> Vec<FrameRecord> {
        (0..n_frames(scene))
            .map(|k| FrameRecord {
                wall_ms: 8.0 + (k % 3) as f64 * 0.5,
                lag: Some(if scene.starts_with("dive") && k < DIVE_DESCENT_FRAMES { 0.4 } else { 0.0 }),
                upload_bytes: if scene.starts_with("look") { 0 } else { 100_000 },
                dispatched: 0,
                resident: 100,
                allocated_bytes: 1 << 30,
                ..FrameRecord::default()
            })
            .collect()
    }

    fn files(scene: &str, frames: &[FrameRecord], f: &Fx) -> SceneFiles {
        let sample = |mhz: u32| {
            crate::parse_nvidia_smi(&format!("NVIDIA GeForce RTX 3080 Ti Laptop GPU, {mhz}, 6000, 87.5, 61, P0, 0x0000000000000000"))
                .unwrap()
        };
        let wall: Vec<f64> = frames.iter().map(|x| x.wall_ms).collect();
        let mut counts: Vec<(String, u64)> = vec![
            ("frames".into(), frames.len() as u64),
            ("blank_frames".into(), f.blank_logged),
            ("upload_bytes_max".into(), frames.iter().map(|x| x.upload_bytes).max().unwrap_or(0)),
            ("allocated_bytes_max".into(), frames.iter().map(|x| x.allocated_bytes).max().unwrap_or(0)),
            ("clock_samples".into(), 12),
            ("clock_min_mhz".into(), 1470),
            ("pool_slots".into(), f.pool_slots),
            ("share_slots".into(), f.share_slots),
            ("max_resident_slots".into(), 100),
            ("scene_alone".into(), f.scene_alone),
            ("cpu_clock_before_mhz".into(), 3100),
            ("cpu_clock_after_mhz".into(), 3000),
        ];
        if let Some(b) = f.ram {
            counts.push(("process_peak_bytes".into(), b));
        }
        if let Some(b) = f.driver_vram {
            counts.push(("driver_vram_bytes".into(), b));
        }
        counts.retain(|(k, _)| Some(k.as_str()) != f.omit_count);
        let run = RunReport {
            path_name: format!("stream/{scene}"),
            vram_cap_gb: 8,
            stats: summarize(&wall, 0).unwrap(),
            clocks_before: Some(sample(1470)),
            clocks_after: Some(sample(1470)),
            metrics: vec![],
            counts,
        };
        let adapter =
            AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let protocol = Protocol {
            warmup_frames: 0,
            measured_frames: 0,
            runs: 1,
            field: Some(f.field.clone()),
            source: (!f.omit_source).then(|| f.source.clone()),
            ..Protocol::default()
        };
        let validity = gate::Validity { reasons: f.invalid.clone() };
        let json = report_json_with_warmups(&adapter, Some(GOOD_SCHEME), &[], &validity, &protocol, &[run], &[]);
        SceneFiles { scene: scene.into(), json: Some(json.into_bytes()), frames: Some(frames_text(frames)), last_valid_present: false }
    }

    fn window(f: &Fx) -> Vec<SceneFiles> {
        ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km"].iter().map(|s| files(s, &good_frames(s), f)).collect()
    }

    fn replace(w: &mut [SceneFiles], scene: &str, new: SceneFiles) {
        *w.iter_mut().find(|f| f.scene == scene).unwrap() = new;
    }

    fn refused(e: Evaluation) -> String {
        match e {
            Evaluation::Refused(r) => r.join("\n"),
            Evaluation::Judged(_) => panic!("expected a refusal"),
        }
    }

    fn judged(e: Evaluation) -> Judgement {
        match e {
            Evaluation::Judged(j) => j,
            Evaluation::Refused(r) => panic!("expected a verdict, got a refusal: {r:?}"),
        }
    }

    fn failing(j: &Judgement) -> Vec<String> {
        j.scenes.iter().flat_map(|s| s.verdicts.iter().filter(|v| v.fails()).map(move |v| v.line(&s.scene))).collect()
    }

    // spec: TEST-021
    #[test]
    fn a_valid_window_with_all_five_scenes_passing_passes_and_writes_a_report() {
        let j = judged(evaluate(&window(&fx())));
        assert!(j.pass, "{:?}", failing(&j));
        assert_eq!(j.scenes.len(), 5);
        let md = render_report(&j, "| step | pass |\n", "2026-10-16");
        for needle in [
            "PASS",
            "blank tiles",
            "uncovered nodes",
            "frame p99",
            "RAM",
            "terrain slots",
            "TO FILL",
            "Cold start",
            "10.4-10.9 s",
            "ADR 0033 point 9",
            "dive-fast",
            "| step | pass |",
        ] {
            assert!(md.contains(needle), "missing {needle}:\n{md}");
        }
    }

    // spec: TEST-021
    #[test]
    fn a_missing_scene_or_a_missing_per_frame_log_refuses() {
        let mut w = window(&fx());
        w.retain(|f| f.scene != "look-2km");
        let r = refused(evaluate(&w));
        assert!(r.contains("look-2km: no log was given"), "{r}");
        let mut w = window(&fx());
        w.iter_mut().find(|f| f.scene == "teleport-1m").unwrap().frames = None;
        assert!(refused(evaluate(&w)).contains("teleport-1m: no per-frame log"));
        let mut w = window(&fx());
        w.iter_mut().find(|f| f.scene == "dive").unwrap().json = None;
        assert!(refused(evaluate(&w)).contains("dive: no log `dive.json`"));
        assert!(refused(evaluate(&[])).contains("dive: no log was given"));
    }

    // spec: TEST-021
    #[test]
    fn dive_fast_is_report_only_absent_or_failing() {
        let mut w = window(&fx());
        let mut bad = good_frames("dive-fast");
        for f in bad.iter_mut().take(100) {
            f.wall_ms = 35.0;
            f.blank_nodes = 2;
        }
        replace(&mut w, "dive-fast", files("dive-fast", &bad, &fx()));
        let j = judged(evaluate(&w));
        assert!(j.pass, "dive-fast does not count: {:?}", failing(&j));
        assert!(j.report_only.as_ref().is_ok_and(SceneRead::fails), "but it is shown with its failures");
        w.retain(|f| f.scene != "dive-fast");
        let j = judged(evaluate(&w));
        assert!(j.pass && j.report_only.is_err());
        assert!(render_report(&j, "", "d").contains("dive-fast was not run"));
        // A broken dive-fast log is shown as unusable, not a refusal.
        let mut w = window(&fx());
        replace(
            &mut w,
            "dive-fast",
            files("dive-fast", &good_frames("dive-fast"), &Fx { source: "in-memory 256 cells (NOT the production source)".into(), ..fx() }),
        );
        assert!(judged(evaluate(&w)).report_only.is_err());
    }

    // spec: TEST-021
    #[test]
    fn a_log_over_an_in_memory_bake_or_another_field_is_refused() {
        for source in ["in-memory 16384 cells (NOT the production source)", "in-memory 16384 cells", "store D:/bake, 8192 cells"] {
            let mut w = window(&fx());
            replace(&mut w, "dive", files("dive", &good_frames("dive"), &Fx { source: source.into(), ..fx() }));
            let r = refused(evaluate(&w));
            assert!(r.contains("dive:") && r.contains(source), "{r}");
        }
        let mut w = window(&fx());
        replace(
            &mut w,
            "look-1m",
            files("look-1m", &good_frames("look-1m"), &Fx { field: "height field cpu-terrain-v1 over a 16384-cell bake".into(), ..fx() }),
        );
        assert!(refused(evaluate(&w)).contains("not `height field cpu-erosion-v1 over a 16384-cell bake`"));
        // Scenes of different windows (another store) cannot be mixed.
        let mut w = window(&fx());
        replace(&mut w, "look-1m", files("look-1m", &good_frames("look-1m"), &Fx { source: "store E:/other, 16384 cells".into(), ..fx() }));
        assert!(refused(evaluate(&w)).contains("one window over one source"));
    }

    // spec: TEST-021
    #[test]
    fn an_invalid_session_or_an_old_format_log_is_refused_with_the_reason() {
        let mut w = window(&fx());
        let gate = fx().clone();
        replace(
            &mut w,
            "teleport-2km",
            files(
                "teleport-2km",
                &good_frames("teleport-2km"),
                &Fx { invalid: vec!["readiness gate failed: GPU temperature 74 C".into()], ..gate },
            ),
        );
        let r = refused(evaluate(&w));
        assert!(r.contains("teleport-2km: the session is INVALID") && r.contains("GPU temperature 74 C"), "{r}");
        // The earlier last-valid log is mentioned and never used.
        w.iter_mut().find(|f| f.scene == "teleport-2km").unwrap().last_valid_present = true;
        assert!(refused(evaluate(&w)).contains("never used"));
        // A log from before the source was recorded.
        let mut w = window(&fx());
        replace(&mut w, "dive", files("dive", &good_frames("dive"), &Fx { omit_source: true, ..fx() }));
        assert!(refused(evaluate(&w)).contains("old-format log: no protocol.source"));
        // A log from before the per-scene figures.
        let mut w = window(&fx());
        replace(&mut w, "dive", files("dive", &good_frames("dive"), &Fx { omit_count: Some("scene_alone"), ..fx() }));
        let r = refused(evaluate(&w));
        assert!(r.contains("old-format log: counts scene_alone are missing"), "{r}");
        // A frames.txt with other columns.
        let mut w = window(&fx());
        let f = w.iter_mut().find(|f| f.scene == "dive").unwrap();
        f.frames = f.frames.as_ref().map(|t| t.replacen("lag uncovered ", "lag ", 1));
        assert!(refused(evaluate(&w)).contains("old-format frames.txt"));
    }

    // spec: TEST-021
    #[test]
    fn a_partial_run_or_files_from_different_runs_are_refused() {
        let mut w = window(&fx());
        let short = good_frames("look-1m")[..24].to_vec();
        replace(&mut w, "look-1m", files("look-1m", &short, &fx()));
        assert!(refused(evaluate(&w)).contains("24 frames where the full script has 600"));
        let mut w = window(&fx());
        let mut other = good_frames("look-1m");
        other[3].wall_ms = 30.0;
        other[4].wall_ms = 30.0;
        let mixed = SceneFiles { frames: files("look-1m", &other, &fx()).frames, ..files("look-1m", &good_frames("look-1m"), &fx()) };
        replace(&mut w, "look-1m", mixed);
        assert!(refused(evaluate(&w)).contains("disagree"));
    }

    // spec: TEST-021
    #[test]
    fn scenes_of_different_sessions_or_a_log_that_disagrees_with_its_frames_are_refused() {
        let mut w = window(&fx());
        let f = w.iter_mut().find(|f| f.scene == "look-1m").unwrap();
        let text = String::from_utf8(f.json.take().unwrap()).unwrap().replace("(High performance)", "(Balanced)");
        f.json = Some(text.into_bytes());
        assert!(refused(evaluate(&w)).contains("adapter and power scheme"));
        let mut w = window(&fx());
        let f = w.iter_mut().find(|f| f.scene == "dive").unwrap();
        let text = String::from_utf8(f.json.take().unwrap()).unwrap().replace("\"upload_bytes_max\":100000", "\"upload_bytes_max\":5");
        f.json = Some(text.into_bytes());
        assert!(refused(evaluate(&w)).contains("upload_bytes_max 5 but frames.txt holds 100000"));
    }

    // spec: TEST-021
    #[test]
    fn a_blank_tile_or_an_uncovered_node_fails() {
        for (what, edit) in [
            ("blank", (|f: &mut FrameRecord| f.blank_nodes = 1) as fn(&mut FrameRecord)),
            ("uncovered", |f: &mut FrameRecord| f.uncovered_nodes = 3),
        ] {
            let mut w = window(&fx());
            let mut fr = good_frames("teleport-1m");
            edit(&mut fr[0]);
            replace(&mut w, "teleport-1m", files("teleport-1m", &fr, &fx()));
            let j = judged(evaluate(&w));
            assert!(!j.pass, "{what}");
            assert!(
                failing(&j).iter().any(|l| l.contains("teleport-1m") && (l.contains("blank tiles") || l.contains("uncovered nodes"))),
                "{:?}",
                failing(&j)
            );
            assert!(render_report(&j, "", "d").contains("FAIL"));
        }
        // Blank frames the log counts fail even if the per-frame columns were clean.
        let mut w = window(&fx());
        replace(&mut w, "look-2km", files("look-2km", &good_frames("look-2km"), &Fx { blank_logged: 2, ..fx() }));
        assert!(failing(&judged(evaluate(&w))).iter().any(|l| l.contains("blank frames (log count)")));
    }

    // spec: TEST-021
    #[test]
    fn p99_over_20_ms_fails() {
        let mut w = window(&fx());
        let mut fr = good_frames("dive");
        for f in fr.iter_mut().step_by(25) {
            f.wall_ms = 25.0;
        }
        replace(&mut w, "dive", files("dive", &fr, &fx()));
        let j = judged(evaluate(&w));
        assert!(!j.pass && failing(&j).iter().any(|l| l.contains("dive: frame p99")), "{:?}", failing(&j));
    }

    // spec: TEST-021
    #[test]
    fn ram_over_8_gib_fails_and_unmeasured_or_shared_ram_is_refused() {
        let mut w = window(&fx());
        replace(&mut w, "look-1m", files("look-1m", &good_frames("look-1m"), &Fx { ram: Some((8 << 30) + 1), ..fx() }));
        let j = judged(evaluate(&w));
        assert!(!j.pass && failing(&j).iter().any(|l| l.contains("look-1m: RAM")), "{:?}", failing(&j));
        let mut w = window(&fx());
        replace(&mut w, "look-1m", files("look-1m", &good_frames("look-1m"), &Fx { ram: Some(8 << 30), ..fx() }));
        assert!(judged(evaluate(&w)).pass, "exactly 8 GiB is within the budget");
        let mut w = window(&fx());
        replace(&mut w, "look-1m", files("look-1m", &good_frames("look-1m"), &Fx { ram: None, ..fx() }));
        assert!(refused(evaluate(&w)).contains("RAM was not measured"));
        let mut w = window(&fx());
        replace(&mut w, "look-1m", files("look-1m", &good_frames("look-1m"), &Fx { scene_alone: 0, ..fx() }));
        assert!(refused(evaluate(&w)).contains("not the scene's own"));
    }

    // spec: TEST-021
    #[test]
    fn vram_or_slots_over_the_share_or_the_tier_fail() {
        let mut w = window(&fx());
        replace(&mut w, "dive", files("dive", &good_frames("dive"), &Fx { pool_slots: 9001, ..fx() }));
        let j = judged(evaluate(&w));
        assert!(!j.pass && failing(&j).iter().any(|l| l.contains("dive: terrain slots")), "{:?}", failing(&j));
        let mut w = window(&fx());
        let mut fr = good_frames("teleport-2km");
        fr[10].allocated_bytes = (2 << 30) + 1;
        replace(&mut w, "teleport-2km", files("teleport-2km", &fr, &fx()));
        let j = judged(evaluate(&w));
        assert!(!j.pass && failing(&j).iter().any(|l| l.contains("terrain bytes")), "{:?}", failing(&j));
        let mut w = window(&fx());
        replace(&mut w, "look-2km", files("look-2km", &good_frames("look-2km"), &Fx { driver_vram: Some((8 << 30) + 1), ..fx() }));
        assert!(failing(&judged(evaluate(&w))).iter().any(|l| l.contains("driver VRAM")));
        // Not reading the driver is shown, never invented, and does not fail the allocator's side.
        let mut w = window(&fx());
        replace(&mut w, "look-2km", files("look-2km", &good_frames("look-2km"), &Fx { driver_vram: None, ..fx() }));
        let j = judged(evaluate(&w));
        assert!(j.pass && j.scenes[4].verdicts.iter().any(|v| v.name == "driver VRAM" && v.status == Status::NotMeasured));
    }

    // spec: TEST-021
    #[test]
    fn gated_lag_over_budget_fails_in_the_dive_descent_and_in_a_look_while_tracked_lag_is_only_reported() {
        let mut w = window(&fx());
        let mut fr = good_frames("dive");
        for f in fr.iter_mut().take(DIVE_DESCENT_FRAMES) {
            f.lag = Some(1.6);
        }
        replace(&mut w, "dive", files("dive", &fr, &fx()));
        let j = judged(evaluate(&w));
        assert!(!j.pass && failing(&j).iter().any(|l| l.contains("dive: detail lag: descent")), "{:?}", failing(&j));
        let mut w = window(&fx());
        let mut fr = good_frames("look-1m");
        fr[300].lag = Some(0.5);
        replace(&mut w, "look-1m", files("look-1m", &fr, &fx()));
        let j = judged(evaluate(&w));
        assert!(!j.pass && failing(&j).iter().any(|l| l.contains("look-1m: rotation")), "{:?}", failing(&j));
        // A still look that fetches fails too.
        let mut w = window(&fx());
        let mut fr = good_frames("look-2km");
        fr[10].dispatched = 1;
        replace(&mut w, "look-2km", files("look-2km", &fr, &fx()));
        assert!(!judged(evaluate(&w)).pass);
        // Tracked: teleport recovery never completes, the dive's ground segment never settles: reported, window passes.
        let mut w = window(&fx());
        let mut tele = good_frames("teleport-1m");
        for f in tele.iter_mut() {
            f.lag = Some(2.0);
        }
        replace(&mut w, "teleport-1m", files("teleport-1m", &tele, &fx()));
        let mut dive = good_frames("dive");
        for f in dive.iter_mut().skip(DIVE_DESCENT_FRAMES) {
            f.lag = Some(0.8);
        }
        replace(&mut w, "dive", files("dive", &dive, &fx()));
        let j = judged(evaluate(&w));
        assert!(j.pass, "{:?}", failing(&j));
        let md = render_report(&j, "", "d");
        assert!(md.contains("tracked, over"), "{md}");
        let tracked: Vec<&Verdict> = j.scenes.iter().flat_map(|s| &s.verdicts).filter(|v| v.status == Status::TrackedOver).collect();
        assert_eq!(tracked.len(), 2, "{tracked:?}");
    }

    // spec: TEST-021
    #[test]
    fn the_refusal_names_the_commands_for_the_window_and_the_acceptance() {
        let t = refusal_text(&["dive: no log `dive.json`".to_string()], "perf/logs");
        for needle in
            ["REFUSES", "dive: no log", "cargo xtask perf stream all --store", "cargo xtask accept M4", "perf/logs", "--allow-memory-bake"]
        {
            assert!(t.contains(needle), "missing {needle}: {t}");
        }
    }

    // spec: TEST-021
    #[test]
    fn frames_text_round_trips_through_the_reader() {
        let mut fr = good_frames("look-1m");
        fr[0].gpu_ms = Some(1.25);
        fr[1].lag = None;
        fr[2].uncovered_nodes = 4;
        let back = parse_frames(&frames_text(&fr)).unwrap();
        assert_eq!(back.len(), fr.len());
        assert_eq!((back[0].gpu_ms, back[1].lag, back[2].uncovered_nodes, back[5].allocated_bytes), (Some(1.25), None, 4, 1 << 30));
        assert!(parse_frames("").unwrap_err().contains("old-format"));
        assert!(parse_frames(&format!("{}1 2 3\n", frames_text(&[]))).unwrap_err().contains("columns"));
    }
}
