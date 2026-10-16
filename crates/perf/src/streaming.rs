//! Verdicts of the streaming perf scenes (ADR 0032 point 4, TEST-012, TEST-018): `dive`, `dive-fast`, `teleport-1m`,
//! `teleport-2km`, `look-1m`, `look-2km`. GPU-free: the harness records one [`FrameRecord`] per frame of a case and the memory
//! facts of the run; this module turns them into verdict lines in the style of the terrain scene's. The frame-time verdicts
//! hold at any clock state (ADR 0034 as amended); a GPU verdict needs the clock floor (ADR 0036), and these scenes measure no
//! GPU time (the pipelined loop submits without waiting, so there is no per-frame timestamp), which the lines say.

use crate::{summarize, Stats, FRAME_P99_BUDGET_MS};

/// The scene names, in the order of ADR 0032 point 8, then `dive-cold` (the dive over a reader that ignores the pack's pre-filled coarse tiles: tracked, report-only, ADR 0045 point 3).
pub const SCENES: [&str; 7] = ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km", "dive-cold"];

/// CON-21: the median frame is at most one 60 Hz frame (16.6 ms).
pub const FRAME_MEDIAN_BUDGET_MS: f64 = 16.6;
/// ADR 0032 point 4: no frame over two frame budgets.
pub const FRAME_MAX_BUDGET_MS: f64 = 40.0;
/// ADR 0033 point 9 (the same number as `planet_frame::upload::DEFAULT_TILE_BYTES_PER_FRAME`, checked by a test in the app).
pub const UPLOAD_BYTES_BUDGET: u64 = 1 << 20;
/// CON-21: upload time per frame (judged on the p99 of the frames; the maximum is printed).
pub const UPLOAD_MS_BUDGET: f64 = 2.0;
/// CON-21: the process holds at most 8 GB.
pub const RAM_BUDGET_BYTES: u64 = 8 << 30;
/// Detail lag below this is zero (the lag is a weighted mean of whole levels).
pub const LAG_ZERO: f64 = 1e-9;

/// What the harness knows about one frame of a case (ADR 0032 point 6: the columns apart, so a frame over budget is
/// attributed to its largest part). Times are milliseconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameRecord {
    /// Plan time: the worker's time of the plan delivered in this frame (off the frame's critical path, STRM-016).
    pub plan_ms: f64,
    pub scheduler_ms: f64,
    pub upload_ms: f64,
    /// CPU time to record and submit.
    pub record_ms: f64,
    /// GPU time; `None` where it is not measured (the pipelined submit has no timestamp readback).
    pub gpu_ms: Option<f64>,
    /// The frame's work time from the start of the iteration to the submit (waits for the GPU included, the pacing sleep
    /// excluded): the frame time of the verdicts.
    pub wall_ms: f64,
    /// Detail lag (levels) against the plan selected afresh for the frame's own camera with every tile resident (TEST-020), from
    /// the post-pass over the recorded poses and drawn sets. Every frame has one: `None` is missing data (the case failed to
    /// record the frame), which every lag verdict reports as OVER, never as "not measured".
    pub lag: Option<f64>,
    /// Nodes of that fresh selection with no drawn tile at, above or below them (counted in the lag as level + 1): ADR 0032's hard
    /// measure is "a tile, root or ancestor, for every visible node", so they are blank tiles, gated like the draw's own blank count
    /// (`CaseSpec::uncovered`).
    pub uncovered_nodes: usize,
    /// The raw ids of those nodes (empty when `uncovered_nodes` is 0).
    pub uncovered_tiles: Vec<u64>,
    /// Target nodes with no resident tile of their own or of an ancestor, and whether nothing at all was drawn.
    pub blank_nodes: usize,
    pub upload_bytes: u64,
    /// Queue calls the frame's tile writes made (REND-016): the cost the bytes do not show.
    pub upload_calls: u64,
    pub resident: usize,
    pub in_flight: usize,
    pub queued: usize,
    pub dispatched: u64,
    pub uploaded: u64,
    pub cancelled: u64,
    pub degraded: usize,
    /// Bytes the renderer has allocated for terrain after the frame.
    pub allocated_bytes: u64,
}

/// How a lag rule counts towards the verdict (the owner's decision of 2026-10-15, ADR 0032).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Over budget fails the case.
    Gated,
    /// Reported with its budget, never failing (the teleport; the dive's ground segment until the model confirms it).
    Tracked,
    /// Reported only, no budget (`dive-fast`'s ground segment).
    ReportOnly,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LagRule {
    /// Mean lag over `frames` at most `mean_max`, and no frame's lag over `frame_max` levels.
    Mean { name: &'static str, frames: std::ops::Range<usize>, mean_max: f64, frame_max: f64, gate: Gate },
    /// Lag is zero from `within_s` seconds after frame `from` to the end.
    SettledBy { name: &'static str, from: usize, within_s: f64, gate: Gate },
    /// From the first frame: lag under one level for good within `under1_s`, zero for good within `complete_s`.
    Recovery { under1_s: f64, complete_s: f64, gate: Gate },
    /// Lag zero in every frame it is known, and no request, cancellation or upload in the case (rotation never fetches).
    Still { gate: Gate },
    /// The mean and worst lag over `frames`, no budget.
    Report { name: &'static str, frames: std::ops::Range<usize> },
}

/// What a case is and what it is held to.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseSpec {
    pub scene: &'static str,
    pub frames: usize,
    /// Script seconds per frame.
    pub step_s: f64,
    pub rules: Vec<LagRule>,
    /// How nodes of the fresh selection with nothing drawn at, above or below them count (ADR 0032's hard measure: a tile for
    /// every visible node): gated, zero in every frame, for every scene but `dive-fast`, whose criteria are report-only.
    pub uncovered: Gate,
}

/// The rules of `scene` (ADR 0032 point 4). `descent_frames` is the dive's descent length in frames, `frames` the length of the
/// measured part and `step_s` the script's frame step. `None` for a name that is not a streaming scene.
pub fn case_spec(scene: &str, frames: usize, step_s: f64, descent_frames: usize) -> Option<CaseSpec> {
    let name = *SCENES.iter().find(|s| **s == scene)?;
    let rules = match name {
        "dive" => vec![
            LagRule::Mean { name: "descent", frames: 0..descent_frames.min(frames), mean_max: 1.0, frame_max: 3.0, gate: Gate::Gated },
            LagRule::SettledBy { name: "ground segment", from: descent_frames, within_s: 2.0, gate: Gate::Tracked },
        ],
        "dive-fast" => vec![
            LagRule::Mean { name: "descent", frames: 0..descent_frames.min(frames), mean_max: 1.0, frame_max: 3.0, gate: Gate::Gated },
            LagRule::Report { name: "ground segment", frames: descent_frames.min(frames)..frames },
        ],
        // The cold dive is the case the pre-fill removes from the gates: its lag is reported with no budget (ADR 0045 point 3).
        "dive-cold" => vec![
            LagRule::Report { name: "descent", frames: 0..descent_frames.min(frames) },
            LagRule::Report { name: "ground segment", frames: descent_frames.min(frames)..frames },
        ],
        "teleport-1m" => vec![LagRule::Recovery { under1_s: 2.0, complete_s: 9.0, gate: Gate::Tracked }],
        "teleport-2km" => vec![LagRule::Recovery { under1_s: 1.0, complete_s: 4.0, gate: Gate::Tracked }],
        _ => vec![LagRule::Still { gate: Gate::Gated }],
    };
    let uncovered = if name == "dive-fast" || name == "dive-cold" { Gate::ReportOnly } else { Gate::Gated };
    Some(CaseSpec { scene: name, frames, step_s, rules, uncovered })
}

/// Memory facts of a run. `None` is "not measured" (the line says so; nothing is invented).
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryFacts {
    /// Terrain's share of the VRAM cap (ADR 0033 point 1) and the tier's cap, bytes.
    pub share_bytes: u64,
    pub vram_cap_bytes: u64,
    /// Driver-reported VRAM in use after the case (`nvidia-smi memory.used`, the whole GPU including other processes).
    pub driver_vram_bytes: Option<u64>,
    /// The peak working set of the process that ran the scene, bytes. With one scene per process (`perf stream all` spawns a child
    /// per scene, CON-21) it is the scene's own peak, its world open included; `scene_alone` says so.
    pub process_peak_bytes: Option<u64>,
    /// Whether the process ran this scene alone (a child of `perf stream all`, or a single named scene) and the figure is therefore
    /// the scene's own; false when one process ran several scenes and the peak covers them all.
    pub scene_alone: bool,
    /// Slots of the terrain pool the renderer was built with (its ceiling), the slots the tier's share holds at the pool's slot
    /// size, and the most slots resident in any frame: the allocator's side of the VRAM line (ADR 0033).
    pub pool_slots: u64,
    pub share_slots: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// A gated measure over budget: fails the case.
    Over,
    /// A tracked measure within / over its budget (never fails).
    TrackedWithin,
    TrackedOver,
    Reported,
    NotMeasured,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

impl Verdict {
    fn new(name: &'static str, status: Status, detail: String) -> Self {
        Verdict { name, status, detail }
    }

    /// One console line, in the style of the terrain scene's (`  scene: measure ... word`).
    pub fn line(&self, scene: &str) -> String {
        let word = match self.status {
            Status::Ok => "ok",
            Status::Over => "OVER",
            Status::TrackedWithin => "within (tracked, not gated)",
            Status::TrackedOver => "over (tracked, not gated)",
            Status::Reported => "reported",
            Status::NotMeasured => "not measured",
        };
        format!("  {scene}: {}: {} {word}", self.name, self.detail)
    }

    pub fn fails(&self) -> bool {
        self.status == Status::Over
    }
}

fn word(ok: bool) -> Status {
    if ok {
        Status::Ok
    } else {
        Status::Over
    }
}

fn tracked(ok: bool) -> Status {
    if ok {
        Status::TrackedWithin
    } else {
        Status::TrackedOver
    }
}

fn gated(gate: Gate, ok: bool) -> Status {
    match gate {
        Gate::Gated => word(ok),
        Gate::Tracked => tracked(ok),
        Gate::ReportOnly => Status::Reported,
    }
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 20) as f64
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 30) as f64
}

/// Seconds (script time) from the start to the first frame from which `ok` holds for every later frame, or `None` if it never
/// holds for good (a frame without a known lag is not ok).
pub fn holds_from(lags: &[Option<f64>], step_s: f64, ok: impl Fn(f64) -> bool) -> Option<f64> {
    let mut from = lags.len();
    for (i, l) in lags.iter().enumerate().rev() {
        if l.is_some_and(&ok) {
            from = i;
        } else {
            break;
        }
    }
    (from < lags.len()).then_some(from as f64 * step_s)
}

fn mean_and_worst(lags: &[Option<f64>]) -> Option<(f64, f64)> {
    let known: Vec<f64> = lags.iter().flatten().copied().collect();
    if known.is_empty() {
        return None;
    }
    Some((known.iter().sum::<f64>() / known.len() as f64, known.iter().copied().fold(0.0, f64::max)))
}

/// Every verdict of a case. `blank_frames` lists the frames with a blank tile and their tile ids (raw `TileId` values; empty
/// when only the node count is known). Frame-time verdicts do not depend on the clock state; the caller prints that beside them.
pub fn case_verdicts(spec: &CaseSpec, frames: &[FrameRecord], blank_frames: &[(usize, Vec<u64>)], mem: &MemoryFacts) -> Vec<Verdict> {
    let mut v = Vec::new();
    // Hard: no blank tile, ever.
    let blank_total = frames.iter().filter(|f| f.blank_nodes > 0).count();
    if blank_total == 0 {
        v.push(Verdict::new("blank tiles", Status::Ok, "none in any of the frames".into()));
    } else {
        let first = blank_frames.first().map_or_else(
            || frames.iter().position(|f| f.blank_nodes > 0).map_or_else(String::new, |k| format!("first at frame {k}")),
            |(k, ids)| {
                format!("first at frame {k}, tiles {}", ids.iter().take(8).map(|t| format!("{t:#018x}")).collect::<Vec<_>>().join(" "))
            },
        );
        v.push(Verdict::new("blank tiles", Status::Over, format!("{blank_total} frames drew a blank node ({first}); budget none")));
    }
    // Frame time (valid at any clock state).
    let wall: Vec<f64> = frames.iter().map(|f| f.wall_ms).collect();
    match summarize(&wall, 0) {
        Some(s) => {
            v.push(Verdict::new(
                "frame median",
                word(s.median <= FRAME_MEDIAN_BUDGET_MS),
                format!("{:.2} ms (budget {FRAME_MEDIAN_BUDGET_MS})", s.median),
            ));
            v.push(Verdict::new(
                "frame p99",
                word(s.p99 <= FRAME_P99_BUDGET_MS),
                format!("{:.2} ms (budget {FRAME_P99_BUDGET_MS})", s.p99),
            ));
            let worst = wall.iter().position(|w| *w == s.max).unwrap_or(0);
            v.push(Verdict::new(
                "frame max",
                word(s.max <= FRAME_MAX_BUDGET_MS),
                format!("{:.2} ms at frame {worst} (cap {FRAME_MAX_BUDGET_MS}); {}", s.max, attribution(&frames[worst])),
            ));
        }
        None => v.push(Verdict::new("frame time", Status::NotMeasured, "no frames".into())),
    }
    // Upload.
    let max_bytes = frames.iter().map(|f| f.upload_bytes).max().unwrap_or(0);
    v.push(Verdict::new(
        "upload bytes",
        word(max_bytes <= UPLOAD_BYTES_BUDGET),
        format!("at most {:.3} MiB per frame (cap {} MiB)", mb(max_bytes), mb(UPLOAD_BYTES_BUDGET)),
    ));
    let up: Vec<f64> = frames.iter().map(|f| f.upload_ms).collect();
    if let Some(s) = summarize(&up, 0) {
        v.push(Verdict::new(
            "upload time",
            word(s.p99 <= UPLOAD_MS_BUDGET),
            format!("p99 {:.2} ms, max {:.2} ms (budget {UPLOAD_MS_BUDGET} ms on the p99)", s.p99, s.max),
        ));
    }
    // Memory.
    let alloc = frames.iter().map(|f| f.allocated_bytes).max().unwrap_or(0);
    v.push(Verdict::new(
        "terrain bytes",
        word(alloc <= mem.share_bytes),
        format!("at most {:.3} GiB allocated, every frame (share {:.1} GiB)", gib(alloc), gib(mem.share_bytes)),
    ));
    v.push(Verdict::new(
        "terrain pool VRAM",
        word(alloc <= mem.vram_cap_bytes),
        format!("{:.3} GiB in the terrain pools (tier cap {:.0} GiB)", gib(alloc), gib(mem.vram_cap_bytes)),
    ));
    let max_resident = frames.iter().map(|f| f.resident).max().unwrap_or(0);
    v.push(Verdict::new(
        "terrain slots",
        word(mem.pool_slots <= mem.share_slots),
        format!(
            "{} slots allocated of {} share slots (pool ceiling against the share less its fixed items; at most {max_resident} resident; share {:.1} GiB)",
            mem.pool_slots,
            mem.share_slots,
            gib(mem.share_bytes)
        ),
    ));
    v.push(match mem.driver_vram_bytes {
        Some(b) => Verdict::new(
            "driver VRAM",
            word(b <= mem.vram_cap_bytes),
            format!(
                "{:.2} GiB in use on the whole GPU after the case, all processes: an upper bound for the scene, beside its {} of {} share slots (tier cap {:.0} GiB)",
                gib(b),
                mem.pool_slots,
                mem.share_slots,
                gib(mem.vram_cap_bytes)
            ),
        ),
        None => Verdict::new("driver VRAM", Status::NotMeasured, "no nvidia-smi reading".into()),
    });
    v.push(match mem.process_peak_bytes {
        Some(b) => Verdict::new(
            "RAM",
            word(b <= RAM_BUDGET_BYTES),
            format!(
                "{:.2} GiB {} (budget {:.0} GiB, CON-21)",
                gib(b),
                if mem.scene_alone {
                    crate::session::RAM_LABEL
                } else {
                    "peak working set of the process over all its scenes, bake and store included, NOT the scene's own"
                },
                gib(RAM_BUDGET_BYTES)
            ),
        ),
        None => Verdict::new("RAM", Status::NotMeasured, "the process working set could not be read".into()),
    });
    // Detail lag.
    let lags: Vec<Option<f64>> = frames.iter().map(|f| f.lag).collect();
    for rule in &spec.rules {
        v.push(lag_verdict(rule, &lags, frames, spec.step_s));
    }
    // Coverage of the fresh selection (ADR 0032's hard measure: a tile, root or ancestor, for every visible node): a node with
    // nothing drawn at, above or below it is a blank tile. Also counted in the lag as level + 1.
    let uncovered = frames.iter().filter(|f| f.uncovered_nodes > 0).count();
    match frames.iter().position(|f| f.uncovered_nodes > 0) {
        None => v.push(Verdict::new("uncovered nodes", Status::Ok, "none in any of the frames".into())),
        Some(first) => {
            let ids: Vec<String> = frames[first].uncovered_tiles.iter().take(8).map(|t| format!("{t:#018x}")).collect();
            v.push(Verdict::new(
                "uncovered nodes",
                gated(spec.uncovered, false),
                format!(
                    "{uncovered} of {} frames had nodes of their fresh selection with nothing drawn at, above or below them (first at frame {first}, {} nodes, tiles {}; counted in the lag as level + 1; budget none{})",
                    frames.len(),
                    frames[first].uncovered_nodes,
                    ids.join(" "),
                    if spec.uncovered == Gate::ReportOnly { "; report-only in this scene" } else { "" }
                ),
            ));
        }
    }
    v
}

/// A lag rule judged on a slice with a frame that has no lag is a harness failure, said loudly: every frame has a lag (TEST-020), so
/// a missing one means the pose or the drawn set of that frame was not recorded. Not "not measured", and never a pass.
fn missing(name: &str, first_frame: usize, lags: &[Option<f64>]) -> Option<Verdict> {
    let first = lags.iter().position(Option::is_none)?;
    let n = lags.iter().filter(|l| l.is_none()).count();
    Some(Verdict::new(
        "detail lag",
        Status::Over,
        format!(
            "{name}: {n} of {} frames have no lag (first at frame {}): missing data, the post-pass must give every frame one;",
            lags.len(),
            first_frame + first
        ),
    ))
}

/// The largest column of a frame (ADR 0032 point 6), for the line of the worst frame.
fn attribution(f: &FrameRecord) -> String {
    let cols = [("scheduler", f.scheduler_ms), ("upload", f.upload_ms), ("record", f.record_ms)];
    let (name, ms) = cols.iter().copied().fold(("other", 0.0), |a, c| if c.1 > a.1 { c } else { a });
    format!(
        "largest column {name} {ms:.2} ms (plan {:.2} ms off the path, scheduler {:.2}, upload {:.2}, record {:.2})",
        f.plan_ms, f.scheduler_ms, f.upload_ms, f.record_ms
    )
}

fn lag_verdict(rule: &LagRule, lags: &[Option<f64>], frames: &[FrameRecord], step_s: f64) -> Verdict {
    match rule {
        LagRule::Mean { name, frames: range, mean_max, frame_max, gate } => {
            let slice = &lags[range.start.min(lags.len())..range.end.min(lags.len())];
            if let Some(v) = missing(name, range.start, slice) {
                return v;
            }
            match mean_and_worst(slice) {
                None => Verdict::new("detail lag", Status::NotMeasured, format!("{name}: the rule covers no frame")),
                Some((mean, worst)) => Verdict::new(
                    "detail lag",
                    gated(*gate, mean <= *mean_max && worst <= *frame_max),
                    format!(
                        "{name}: mean {mean:.3} levels (budget {mean_max}), worst frame {worst:.2} (budget {frame_max}), over {} frames",
                        slice.len()
                    ),
                ),
            }
        }
        LagRule::SettledBy { name, from, within_s, gate } => {
            let tail = &lags[(*from).min(lags.len())..];
            if let Some(v) = missing(name, *from, tail) {
                return v;
            }
            let at = holds_from(tail, step_s, |l| l <= LAG_ZERO);
            let ok = at.is_some_and(|t| t <= *within_s);
            let shown = at.map_or("never".to_string(), |t| format!("{t:.2} s"));
            Verdict::new("detail lag", gated(*gate, ok), format!("{name}: zero for good after {shown} (budget {within_s} s)"))
        }
        LagRule::Recovery { under1_s, complete_s, gate } => {
            if let Some(v) = missing("recovery", 0, lags) {
                return v;
            }
            let under = holds_from(lags, step_s, |l| l < 1.0);
            let zero = holds_from(lags, step_s, |l| l <= LAG_ZERO);
            let fmt = |t: Option<f64>| t.map_or("never".to_string(), |t| format!("{t:.2} s"));
            let ok = under.is_some_and(|t| t <= *under1_s) && zero.is_some_and(|t| t <= *complete_s);
            let first = lags.first().copied().flatten().map_or("no plan yet".to_string(), |l| format!("{l:.2}"));
            Verdict::new(
                "detail lag",
                gated(*gate, ok),
                format!("lag {first} at frame 0, under 1 level for good after {} (budget {under1_s} s), draw set complete after {} (budget {complete_s} s)", fmt(under), fmt(zero)),
            )
        }
        LagRule::Still { gate } => {
            if let Some(v) = missing("rotation", 0, lags) {
                return Verdict { name: "rotation", ..v };
            }
            let worst = lags.iter().flatten().copied().fold(0.0, f64::max);
            let fetched: u64 = frames.iter().map(|f| f.dispatched + f.cancelled + f.uploaded).sum();
            Verdict::new(
                "rotation",
                gated(*gate, worst <= LAG_ZERO && fetched == 0),
                format!("worst lag {worst:.3} levels (budget 0), {fetched} requests, cancellations and uploads (budget 0)"),
            )
        }
        LagRule::Report { name, frames: range } => {
            let slice = &lags[range.start.min(lags.len())..range.end.min(lags.len())];
            if let Some(v) = missing(name, range.start, slice) {
                return v;
            }
            match mean_and_worst(slice) {
                None => Verdict::new("detail lag", Status::NotMeasured, format!("{name}: the rule covers no frame")),
                Some((mean, worst)) => Verdict::new(
                    "detail lag",
                    Status::Reported,
                    format!("{name}: mean {mean:.3} levels, worst frame {worst:.2}, over {} frames (no budget)", slice.len()),
                ),
            }
        }
    }
}

/// `(metric name, stats)` of the time columns of a case, for the log (`metrics`): wall time is the run's frame statistic and is
/// not repeated here. Columns that were not measured are left out (the log does not invent them).
pub fn column_metrics(frames: &[FrameRecord]) -> Vec<(String, Stats)> {
    let col = |name: &str, f: &dyn Fn(&FrameRecord) -> Option<f64>| {
        let v: Vec<f64> = frames.iter().filter_map(f).collect();
        (v.len() == frames.len()).then(|| summarize(&v, 0)).flatten().map(|s| (name.to_string(), s))
    };
    [
        col("plan_ms", &|f| Some(f.plan_ms)),
        col("scheduler_ms", &|f| Some(f.scheduler_ms)),
        col("upload_ms", &|f| Some(f.upload_ms)),
        col("record_ms", &|f| Some(f.record_ms)),
        col("gpu_ms", &|f| f.gpu_ms),
        col("detail_lag", &|f| f.lag),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The per-frame log as text (one line per frame): the columns of TEST-012 and the counts of the frame.
pub fn frames_text(frames: &[FrameRecord]) -> String {
    let mut s = String::from(
        "# frame plan_ms scheduler_ms upload_ms record_ms gpu_ms wall_ms lag uncovered blank upload_bytes resident in_flight queued dispatched uploaded cancelled degraded allocated_bytes upload_calls\n",
    );
    for (k, f) in frames.iter().enumerate() {
        let opt = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{v:.3}"));
        s.push_str(&format!(
            "{k} {:.3} {:.3} {:.3} {:.3} {} {:.3} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
            f.plan_ms,
            f.scheduler_ms,
            f.upload_ms,
            f.record_ms,
            opt(f.gpu_ms),
            f.wall_ms,
            opt(f.lag),
            f.uncovered_nodes,
            f.blank_nodes,
            f.upload_bytes,
            f.resident,
            f.in_flight,
            f.queued,
            f.dispatched,
            f.uploaded,
            f.cancelled,
            f.degraded,
            f.allocated_bytes,
            f.upload_calls
        ));
    }
    s
}

/// Parse `nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits` (MiB) into bytes.
pub fn parse_memory_used(csv: &str) -> Option<u64> {
    let mib: f64 = csv.lines().find(|l| !l.trim().is_empty())?.trim().parse().ok()?;
    (mib.is_finite() && mib >= 0.0).then_some((mib * (1u64 << 20) as f64) as u64)
}

/// Driver-reported VRAM in use, bytes (the whole GPU). `None` when `nvidia-smi` is missing or fails.
pub fn query_gpu_memory_used() -> Option<u64> {
    let out = std::process::Command::new("nvidia-smi").args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"]).output().ok()?;
    out.status.success().then(|| parse_memory_used(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// Parse the `VmHWM:` line of `/proc/self/status` (kB) into bytes.
pub fn parse_vm_hwm(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.trim_start_matches("VmHWM:").trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb * 1024)
}

/// The process's peak working set so far, bytes: `VmHWM` on Linux, `PeakWorkingSet64` through PowerShell on Windows (one short
/// call after a case, never during one). `None` where neither can be read.
pub fn query_process_peak_bytes() -> Option<u64> {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        return parse_vm_hwm(&status);
    }
    let pid = std::process::id();
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(Get-Process -Id {pid}).PeakWorkingSet64")])
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().parse().ok()).flatten()
}

/// Prefix of the log counts that describe the tile source's load (STRM-022): recorded for the reader of the log, never compared
/// with an earlier log (they are wall-clock readings and cache behaviour, which differ run to run by nature).
pub const LOAD_COUNT_PREFIX: &str = "load_";

/// One tile job as its worker saw it (the streamer's `JobSample`, restated here so this crate stays free of the streamer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadJob {
    pub wait_us: u64,
    pub run_us: u64,
    /// Jobs inside their load when this one started, itself included.
    pub running: u32,
    pub done: bool,
    /// The level of the tile.
    pub level: u8,
}

/// What the tile source's caches and the pack did during the measured part of a case (differences of two readings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadCounters {
    pub block_decodes: u64,
    pub block_lookups: u64,
    pub block_hits: u64,
    pub block_evictions: u64,
    pub block_decode_waits: u64,
    pub block_lock_wait_us: u64,
    pub block_fetch_us: u64,
    pub block_decode_us: u64,
    /// Blocks in the shared cache at the end, and its budget in blocks.
    pub block_resident: u64,
    pub block_budget: u64,
    pub pack_gets: u64,
    pub pack_found: u64,
    pub pack_get_us: u64,
    pub prefill_lookups: u64,
    pub prefill_found: u64,
    pub prefill_skipped: u64,
    pub prefill_us: u64,
    /// Microseconds the tile jobs together spent getting blocks from the shared cache (a part of their load time).
    pub job_block_wait_us: u64,
    /// Blocks the tile jobs decoded themselves (the others were decoded for the plan worker).
    pub job_block_decodes: u64,
    pub field_builds: u64,
    pub field_us: u64,
}

/// The concurrency bands the job time is split by: jobs that started with at most 4, 5-9, 10-14 and 15 or more loads running.
const LOAD_BANDS: [(&str, u32, u32); 4] = [("1_4", 1, 4), ("5_9", 5, 9), ("10_14", 10, 14), ("15_up", 15, u32::MAX)];

/// The level bands the job time is split by.
const LEVEL_BANDS: [(&str, u8, u8); 4] = [("0_8", 0, 8), ("9_14", 9, 14), ("15_18", 15, 18), ("19_up", 19, u8::MAX)];

/// A fixed amount of integer and floating-point work that touches no memory beyond registers (STRM-022): the same value on every
/// call and machine, so only its duration varies. `rounds` of 1,000 steps; 20,000 rounds take about 34 ms on the reference machine.
pub fn calibration_kernel(rounds: u32) -> u64 {
    let (mut x, mut f) = (0x9E37_79B9_7F4A_7C15u64, 1.000_001f64);
    for _ in 0..rounds {
        for _ in 0..1000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            f = (f * 1.000_000_1 + 1e-9) * 0.999_999_9;
        }
    }
    x ^ f.to_bits()
}

/// Rounds of [`calibration_kernel`] one calibration measurement runs.
pub const CALIBRATION_ROUNDS: u32 = 20_000;

/// How fast the CPU runs a fixed job alone and with `threads` copies at once (STRM-022): the kernel's time on one thread, and the
/// median and the slowest of `threads` threads running it together. The ratio of the median to the single time is what the
/// threads cost each other (hyperthreads, efficiency cores, clocks), measured outside the code under test, so a log's per-tile
/// times can be read against the machine's state of the moment. Wall-clock readings, recorded only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuCalibration {
    pub threads: usize,
    pub single_us: u64,
    pub parallel_median_us: u64,
    pub parallel_max_us: u64,
}

/// Measure [`CpuCalibration`] now. Takes about `2 x` the kernel's time; call it when the case is not running.
pub fn calibrate_cpu(threads: usize) -> CpuCalibration {
    let threads = threads.max(1);
    let time = || {
        let started = std::time::Instant::now();
        std::hint::black_box(calibration_kernel(std::hint::black_box(CALIBRATION_ROUNDS)));
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
    };
    let single_us = time();
    let start = std::sync::Barrier::new(threads);
    let mut each: Vec<u64> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    start.wait();
                    time()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("a calibration thread")).collect()
    });
    each.sort_unstable();
    CpuCalibration { threads, single_us, parallel_median_us: quantile_u64(&each, 0.5), parallel_max_us: each.last().copied().unwrap_or(0) }
}

/// The calibration as log counts: `load_calib_<when>_single_us`, `..._parallel_median_us`, `..._parallel_max_us`, `..._threads`.
pub fn calibration_counts(when: &str, c: &CpuCalibration) -> Vec<(String, u64)> {
    vec![
        (format!("load_calib_{when}_threads"), c.threads as u64),
        (format!("load_calib_{when}_single_us"), c.single_us),
        (format!("load_calib_{when}_parallel_median_us"), c.parallel_median_us),
        (format!("load_calib_{when}_parallel_max_us"), c.parallel_max_us),
    ]
}

fn quantile_u64(sorted: &[u64], q: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let at = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[at.min(sorted.len() - 1)]
}

/// The log counts of a case's tile loads (STRM-022), all prefixed [`LOAD_COUNT_PREFIX`]: jobs that produced a tile; the load
/// time of a tile (median, p99, max, microseconds) and the time it waited in the pool's queue; the same median by the number of
/// loads running beside it; the block cache (decodes, lookups, hits, evictions, waits for another thread's decode, the lock wait,
/// fetch and decode time, residency at the end) and decodes per 1000 tiles; the pack's lookups and the pre-filled tile lookups with
/// their time. Pure: the same inputs give the same counts.
pub fn load_counts(jobs: &[LoadJob], c: &LoadCounters) -> Vec<(String, u64)> {
    let done: Vec<&LoadJob> = jobs.iter().filter(|j| j.done).collect();
    let sorted = |f: &dyn Fn(&LoadJob) -> u64| -> Vec<u64> {
        let mut v: Vec<u64> = done.iter().map(|j| f(j)).collect();
        v.sort_unstable();
        v
    };
    let (run, wait, running) = (sorted(&|j| j.run_us), sorted(&|j| j.wait_us), sorted(&|j| u64::from(j.running)));
    let mut out: Vec<(String, u64)> = vec![
        ("load_jobs".into(), done.len() as u64),
        ("load_jobs_not_done".into(), (jobs.len() - done.len()) as u64),
        ("load_run_us_median".into(), quantile_u64(&run, 0.5)),
        ("load_run_us_p99".into(), quantile_u64(&run, 0.99)),
        ("load_run_us_max".into(), run.last().copied().unwrap_or(0)),
        ("load_wait_us_median".into(), quantile_u64(&wait, 0.5)),
        ("load_wait_us_p99".into(), quantile_u64(&wait, 0.99)),
        ("load_running_median".into(), quantile_u64(&running, 0.5)),
        ("load_running_max".into(), running.last().copied().unwrap_or(0)),
    ];
    for (name, lo, hi) in LOAD_BANDS {
        let mut v: Vec<u64> = done.iter().filter(|j| (lo..=hi).contains(&j.running)).map(|j| j.run_us).collect();
        v.sort_unstable();
        out.push((format!("load_jobs_running_{name}"), v.len() as u64));
        out.push((format!("load_run_us_median_running_{name}"), quantile_u64(&v, 0.5)));
    }
    for (name, lo, hi) in LEVEL_BANDS {
        let mut v: Vec<u64> = done.iter().filter(|j| (lo..=hi).contains(&j.level)).map(|j| j.run_us).collect();
        v.sort_unstable();
        out.push((format!("load_jobs_level_{name}"), v.len() as u64));
        out.push((format!("load_run_us_median_level_{name}"), quantile_u64(&v, 0.5)));
    }
    out.push(("load_run_us_sum".into(), run.iter().sum()));
    out.push(("load_wait_us_sum".into(), wait.iter().sum()));
    out.push(("load_job_block_wait_us".into(), c.job_block_wait_us));
    out.push(("load_job_block_decodes".into(), c.job_block_decodes));
    out.push(("load_field_builds".into(), c.field_builds));
    out.push(("load_field_us".into(), c.field_us));
    let per_1000 = |n: u64| if done.is_empty() { 0 } else { n * 1000 / done.len() as u64 };
    out.extend([
        ("load_block_decodes".into(), c.block_decodes),
        ("load_block_decodes_per_1000_tiles".into(), per_1000(c.block_decodes)),
        ("load_block_lookups".into(), c.block_lookups),
        ("load_block_lookups_per_1000_tiles".into(), per_1000(c.block_lookups)),
        ("load_block_hits".into(), c.block_hits),
        ("load_block_evictions".into(), c.block_evictions),
        ("load_block_decode_waits".into(), c.block_decode_waits),
        ("load_block_lock_wait_us".into(), c.block_lock_wait_us),
        ("load_block_fetch_us".into(), c.block_fetch_us),
        ("load_block_decode_us".into(), c.block_decode_us),
        ("load_block_resident".into(), c.block_resident),
        ("load_block_budget".into(), c.block_budget),
        ("load_pack_gets".into(), c.pack_gets),
        ("load_pack_found".into(), c.pack_found),
        ("load_pack_get_us".into(), c.pack_get_us),
        ("load_prefill_lookups".into(), c.prefill_lookups),
        ("load_prefill_found".into(), c.prefill_found),
        ("load_prefill_skipped".into(), c.prefill_skipped),
        ("load_prefill_us".into(), c.prefill_us),
    ]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(wall: f64) -> FrameRecord {
        FrameRecord {
            plan_ms: 0.0,
            scheduler_ms: 0.1,
            upload_ms: 0.2,
            record_ms: 0.3,
            gpu_ms: None,
            wall_ms: wall,
            lag: Some(0.0),
            uncovered_nodes: 0,
            uncovered_tiles: Vec::new(),
            blank_nodes: 0,
            upload_bytes: 0,
            upload_calls: 0,
            resident: 10,
            in_flight: 0,
            queued: 0,
            dispatched: 0,
            uploaded: 0,
            cancelled: 0,
            degraded: 0,
            allocated_bytes: 1 << 30,
        }
    }

    fn mem() -> MemoryFacts {
        MemoryFacts {
            share_bytes: 2 << 30,
            vram_cap_bytes: 8 << 30,
            driver_vram_bytes: Some(3 << 30),
            process_peak_bytes: Some(4 << 30),
            scene_alone: true,
            pool_slots: 100,
            share_slots: 120,
        }
    }

    fn run(scene: &str, frames: &[FrameRecord]) -> Vec<Verdict> {
        let spec = case_spec(scene, frames.len(), 1.0 / 60.0, 1800.min(frames.len())).unwrap();
        case_verdicts(&spec, frames, &[], &mem())
    }

    fn find<'a>(v: &'a [Verdict], name: &str) -> &'a Verdict {
        v.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("no verdict {name}: {v:?}"))
    }

    // spec: TEST-018
    #[test]
    fn the_registry_lists_the_six_scenes_and_the_cold_dive_and_refuses_other_names() {
        assert_eq!(SCENES, ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km", "dive-cold"]);
        for s in SCENES {
            assert_eq!(case_spec(s, 600, 1.0 / 60.0, 0).unwrap().scene, s);
        }
        assert!(case_spec("terrain", 600, 1.0 / 60.0, 0).is_none());
    }

    // spec: TEST-018
    #[test]
    fn a_case_with_a_blank_tile_fails_and_names_the_frame_and_the_tiles() {
        let mut frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        frames[41].blank_nodes = 2;
        let spec = case_spec("look-1m", 600, 1.0 / 60.0, 0).unwrap();
        let v = case_verdicts(&spec, &frames, &[(41, vec![0x1234, 0x5678])], &mem());
        let b = find(&v, "blank tiles");
        assert!(b.fails() && b.detail.contains("first at frame 41") && b.detail.contains("0x0000000000001234"), "{b:?}");
        assert!(b.line("look-1m").contains("OVER"));
        assert!(find(&run("look-1m", &frames[..40]), "blank tiles").status == Status::Ok);
    }

    // spec: TEST-018
    #[test]
    fn frame_time_is_judged_on_the_median_the_p99_and_the_40_ms_cap() {
        let mut frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        let v = run("look-2km", &frames);
        assert!(["frame median", "frame p99", "frame max"].iter().all(|n| find(&v, n).status == Status::Ok), "{v:?}");
        // Seven frames at 21 ms: p99 (nearest rank 594 of 600) is 21 ms, over; the median and the cap hold.
        for f in frames.iter_mut().take(7) {
            f.wall_ms = 21.0;
        }
        let v = run("look-2km", &frames);
        assert_eq!(
            (find(&v, "frame p99").status, find(&v, "frame max").status, find(&v, "frame median").status),
            (Status::Over, Status::Ok, Status::Ok)
        );
        // One frame at 41 ms breaks the cap only, and the line attributes it to its largest column.
        let mut frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        frames[3].wall_ms = 41.0;
        frames[3].upload_ms = 30.0;
        let v = run("look-2km", &frames);
        let m = find(&v, "frame max");
        assert!(m.fails() && m.detail.contains("at frame 3") && m.detail.contains("largest column upload 30.00 ms"), "{m:?}");
        assert_eq!(find(&v, "frame p99").status, Status::Ok);
        // A slow median fails on its own budget.
        let slow: Vec<FrameRecord> = (0..600).map(|_| frame(17.0)).collect();
        assert_eq!(find(&run("look-2km", &slow), "frame median").status, Status::Over);
    }

    // spec: TEST-018
    #[test]
    fn upload_bytes_time_and_the_share_are_hard_measures() {
        let mut frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        frames[9].upload_bytes = UPLOAD_BYTES_BUDGET;
        assert_eq!(find(&run("look-1m", &frames), "upload bytes").status, Status::Ok);
        frames[9].upload_bytes = UPLOAD_BYTES_BUDGET + 1;
        assert_eq!(find(&run("look-1m", &frames), "upload bytes").status, Status::Over);
        for f in &mut frames {
            f.upload_ms = 2.5;
        }
        assert_eq!(find(&run("look-1m", &frames), "upload time").status, Status::Over);
        frames[0].allocated_bytes = (2 << 30) + 1;
        let v = run("look-1m", &frames);
        assert_eq!((find(&v, "terrain bytes").status, find(&v, "terrain pool VRAM").status), (Status::Over, Status::Ok));
    }

    // spec: TEST-018
    #[test]
    fn unknown_memory_is_not_measured_and_known_memory_over_budget_fails() {
        let frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        let spec = case_spec("look-1m", 600, 1.0 / 60.0, 0).unwrap();
        let unknown = MemoryFacts { driver_vram_bytes: None, process_peak_bytes: None, ..mem() };
        let v = case_verdicts(&spec, &frames, &[], &unknown);
        for n in ["driver VRAM", "RAM"] {
            let x = find(&v, n);
            assert!(x.status == Status::NotMeasured && !x.fails() && x.line("look-1m").ends_with("not measured"), "{x:?}");
        }
        let big = MemoryFacts { process_peak_bytes: Some(RAM_BUDGET_BYTES + 1), driver_vram_bytes: Some((8 << 30) + 1), ..mem() };
        let v = case_verdicts(&spec, &frames, &[], &big);
        assert!(find(&v, "RAM").fails() && find(&v, "driver VRAM").fails());
    }

    // spec: TEST-019
    #[test]
    fn the_ram_line_names_the_scenes_own_peak_and_the_vram_line_sits_beside_the_slots_against_the_share() {
        let frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        let spec = case_spec("look-1m", 600, 1.0 / 60.0, 0).unwrap();
        let v = case_verdicts(&spec, &frames, &[], &mem());
        let ram = find(&v, "RAM").line("look-1m");
        assert!(ram.contains("scene's own peak working set (child process)") && ram.contains("budget 8 GiB, CON-21"), "{ram}");
        let slots = find(&v, "terrain slots");
        assert_eq!(slots.status, Status::Ok);
        assert!(
            slots.detail.starts_with("100 slots allocated of 120 share slots (") && slots.detail.contains("at most 10 resident"),
            "{}",
            slots.detail
        );
        let vram = find(&v, "driver VRAM").line("look-1m");
        assert!(vram.contains("whole GPU") && vram.contains("upper bound") && vram.contains("100 of 120 share slots"), "{vram}");
        // A pool above the share's slots is OVER; a process that ran several scenes is not called the scene's own.
        let over = MemoryFacts { pool_slots: 121, scene_alone: false, ..mem() };
        let v = case_verdicts(&spec, &frames, &[], &over);
        assert!(find(&v, "terrain slots").fails());
        assert!(find(&v, "RAM").detail.contains("NOT the scene's own"), "{}", find(&v, "RAM").detail);
    }

    // spec: TEST-018
    #[test]
    fn the_dive_descent_lag_is_gated_and_the_teleport_lag_is_tracked() {
        // Dive: 1,800 descent frames at mean lag 1.5 fail the gate; at 0.5 they pass; one frame over 3 levels fails.
        let mut dive: Vec<FrameRecord> = (0..2400).map(|_| frame(5.0)).collect();
        for f in &mut dive[..1800] {
            f.lag = Some(1.5);
        }
        let d = find(&run("dive", &dive), "detail lag").clone();
        assert!(d.fails() && d.detail.starts_with("descent"), "{d:?}");
        for f in &mut dive[..1800] {
            f.lag = Some(0.5);
        }
        assert_eq!(find(&run("dive", &dive), "detail lag").status, Status::Ok);
        dive[10].lag = Some(3.5);
        assert!(find(&run("dive", &dive), "detail lag").fails(), "no frame over 3 levels");
        // The ground segment is tracked: a lag that never settles is reported over budget, not failing.
        let mut dive: Vec<FrameRecord> = (0..2400).map(|_| frame(5.0)).collect();
        for f in &mut dive[1800..] {
            f.lag = Some(2.0);
        }
        let v = run("dive", &dive);
        let ground = v.iter().find(|x| x.detail.starts_with("ground segment")).unwrap();
        assert_eq!(ground.status, Status::TrackedOver);
        assert!(!v.iter().any(Verdict::fails), "{v:?}");
        // dive-fast reports its ground segment with no budget.
        let v = case_verdicts(&case_spec("dive-fast", 2400, 1.0 / 60.0, 1800).unwrap(), &dive, &[], &mem());
        assert_eq!(v.iter().find(|x| x.detail.starts_with("ground segment")).unwrap().status, Status::Reported);
        // Teleport: recovery times are tracked: late recovery is over budget and does not fail.
        let mut tp: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        for (k, f) in tp.iter_mut().enumerate() {
            f.lag = Some(if k < 60 {
                5.0
            } else if k < 200 {
                0.5
            } else {
                0.0
            });
        }
        let v = run("teleport-1m", &tp);
        let l = find(&v, "detail lag");
        assert_eq!(l.status, Status::TrackedWithin, "under 1 after 1.0 s (budget 2), complete after 3.3 s (budget 9): {l:?}");
        assert!(l.detail.contains("lag 5.00 at frame 0") && l.detail.contains("1.00 s") && l.detail.contains("3.33 s"), "{l:?}");
        let v = run("teleport-2km", &tp);
        assert_eq!(find(&v, "detail lag").status, Status::TrackedWithin, "1.0 s is at the 1 s budget, 3.33 s under 4 s");
        for f in tp.iter_mut().skip(300) {
            f.lag = Some(0.5); // a late stretch that falls back under one level but not to zero is not recovery
        }
        let v = run("teleport-1m", &tp);
        assert_eq!(find(&v, "detail lag").status, Status::TrackedOver);
        assert!(!v.iter().any(Verdict::fails));
    }

    // spec: TEST-018
    #[test]
    fn a_still_look_needs_zero_lag_and_no_fetching() {
        let mut frames: Vec<FrameRecord> = (0..600).map(|_| frame(5.0)).collect();
        assert_eq!(find(&run("look-1m", &frames), "rotation").status, Status::Ok);
        frames[100].dispatched = 1;
        let r = find(&run("look-1m", &frames), "rotation").clone();
        assert!(r.fails() && r.detail.contains("1 requests, cancellations and uploads"), "{r:?}");
        frames[100].dispatched = 0;
        frames[200].lag = Some(0.4);
        assert!(find(&run("look-1m", &frames), "rotation").fails());
        frames[200].lag = Some(0.0);
        assert_eq!(find(&run("look-1m", &frames), "rotation").status, Status::Ok);
    }

    // spec: TEST-018
    #[test]
    fn the_columns_are_logged_apart_and_an_unmeasured_gpu_column_is_left_out() {
        let frames: Vec<FrameRecord> = (0..10).map(|k| frame(5.0 + k as f64)).collect();
        let names: Vec<String> = column_metrics(&frames).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["plan_ms", "scheduler_ms", "upload_ms", "record_ms", "detail_lag"], "no gpu_ms: not measured");
        let mut with_gpu = frames.clone();
        for f in &mut with_gpu {
            f.gpu_ms = Some(2.0);
        }
        assert!(column_metrics(&with_gpu).iter().any(|(n, _)| n == "gpu_ms"));
        let t = frames_text(&frames);
        assert!(t.lines().count() == 11 && t.lines().nth(1).unwrap().contains(" - "), "{t}");
    }

    // spec: TEST-012
    #[test]
    fn memory_readings_are_parsed_and_unreadable_ones_are_none() {
        assert_eq!(parse_memory_used("3072\n"), Some(3 << 30));
        assert_eq!(parse_memory_used("garbage"), None);
        assert_eq!(parse_vm_hwm("Name: x\nVmHWM:\t  2048 kB\nVmRSS: 1 kB\n"), Some(2 << 20));
        assert_eq!(parse_vm_hwm("Name: x\n"), None);
    }
}

#[cfg(test)]
mod fresh_lag_tests {
    use super::*;

    fn frames(n: usize, lag: Option<f64>) -> Vec<FrameRecord> {
        (0..n)
            .map(|_| FrameRecord {
                plan_ms: 0.0,
                scheduler_ms: 0.0,
                upload_ms: 0.0,
                record_ms: 0.0,
                gpu_ms: None,
                wall_ms: 5.0,
                lag,
                uncovered_nodes: 0,
                uncovered_tiles: Vec::new(),
                blank_nodes: 0,
                upload_bytes: 0,
                upload_calls: 0,
                resident: 0,
                in_flight: 0,
                queued: 0,
                dispatched: 0,
                uploaded: 0,
                cancelled: 0,
                degraded: 0,
                allocated_bytes: 0,
            })
            .collect()
    }

    fn mem() -> MemoryFacts {
        MemoryFacts {
            share_bytes: 1 << 30,
            vram_cap_bytes: 8 << 30,
            driver_vram_bytes: None,
            process_peak_bytes: None,
            scene_alone: true,
            pool_slots: 0,
            share_slots: 0,
        }
    }

    fn lag_of(scene: &str, f: &[FrameRecord], descent: usize) -> Verdict {
        let spec = case_spec(scene, f.len(), 1.0 / 60.0, descent).unwrap();
        let v = case_verdicts(&spec, f, &[], &mem());
        v.into_iter().find(|x| x.name == "detail lag" || x.name == "rotation").unwrap()
    }

    // spec: TEST-020
    #[test]
    fn a_gated_lag_judges_every_frame_even_when_only_one_has_a_lag_over_budget() {
        // The old coverage guard called a gated rule "not measured" on fewer than half the frames; now every frame has a lag and a
        // single bad frame fails a gated rule, a tracked one only reports.
        let mut f = frames(600, Some(0.0));
        assert_eq!(lag_of("look-1m", &f, 0).status, Status::Ok);
        f[599].lag = Some(0.25);
        let r = lag_of("look-1m", &f, 0);
        assert!(r.fails() && r.detail.contains("worst lag 0.250"), "{r:?}");
        let mut dive = frames(2400, Some(0.2));
        dive[1799].lag = Some(3.5);
        assert!(lag_of("dive", &dive, 1800).fails(), "no frame of the descent over 3 levels, the last frame too");
        let mut tp = frames(600, Some(0.0));
        tp[599].lag = Some(2.0);
        let r = lag_of("teleport-1m", &tp, 0);
        assert_eq!(r.status, Status::TrackedOver, "a tracked rule never fails: {r:?}");
    }

    // spec: TEST-018, TEST-021
    #[test]
    fn the_cold_dive_reports_its_lag_with_no_budget_and_never_fails_on_it() {
        // A cold start cannot meet the gated descent (ADR 0045 point 3): `dive-cold` is the case that keeps the number visible.
        let mut cold = frames(2400, Some(0.0));
        for f in &mut cold[..1800] {
            f.lag = Some(7.5);
        }
        let v = case_verdicts(&case_spec("dive-cold", 2400, 1.0 / 60.0, 1800).unwrap(), &cold, &[], &mem());
        let descent = v.iter().find(|x| x.detail.starts_with("descent")).unwrap_or_else(|| panic!("{v:?}"));
        assert_eq!(descent.status, Status::Reported);
        assert!(descent.detail.contains("7.5"), "{descent:?}");
        assert!(!v.iter().any(Verdict::fails), "{v:?}");
        // The gated dive with the same frames fails.
        let v = case_verdicts(&case_spec("dive", 2400, 1.0 / 60.0, 1800).unwrap(), &cold, &[], &mem());
        assert!(v.iter().any(Verdict::fails), "{v:?}");
    }

    // spec: TEST-020
    #[test]
    fn a_frame_without_a_lag_is_missing_data_that_fails_every_lag_rule_and_is_never_not_measured() {
        for (scene, descent) in [("look-2km", 0), ("dive", 1800), ("dive-fast", 1800), ("teleport-2km", 0)] {
            let n = if scene.starts_with("dive") { 2400 } else { 600 };
            let mut f = frames(n, Some(0.0));
            f[7].lag = None;
            let spec = case_spec(scene, n, 1.0 / 60.0, descent).unwrap();
            let v = case_verdicts(&spec, &f, &[], &mem());
            let lag: Vec<&Verdict> = v.iter().filter(|x| x.name == "detail lag" || x.name == "rotation").collect();
            let hit =
                lag.iter().find(|x| x.fails()).unwrap_or_else(|| panic!("{scene}: no lag rule failed on a frame without a lag: {lag:?}"));
            assert!(hit.detail.contains("no lag (first at frame 7)"), "{scene}: {hit:?}");
            assert!(lag.iter().all(|x| x.status != Status::NotMeasured), "{lag:?}");
        }
        // Every frame has a lag, none of them zero-filled: 600 frames at 0.0 pass, and a single missing one in the descent fails.
        let mut dive = frames(2400, Some(0.1));
        assert_eq!(lag_of("dive", &dive, 1800).status, Status::Ok);
        dive[1000].lag = None;
        let r = lag_of("dive", &dive, 1800);
        assert!(r.fails() && r.detail.contains("first at frame 1000") && r.line("dive").contains("OVER"), "{r:?}");
    }

    // spec: TEST-018, TEST-020
    #[test]
    fn nodes_of_the_fresh_selection_with_nothing_drawn_are_blank_tiles_gated_in_every_scene_but_dive_fast() {
        for scene in ["look-1m", "look-2km", "teleport-1m", "teleport-2km", "dive"] {
            let n = if scene == "dive" { 2400 } else { 600 };
            let mut f = frames(n, Some(0.0));
            f[40].uncovered_nodes = 3;
            f[40].uncovered_tiles = vec![0x1234, 0x5678, 0x9abc];
            f[41].uncovered_nodes = 1;
            f[41].uncovered_tiles = vec![0x1234];
            let spec = case_spec(scene, n, 1.0 / 60.0, if scene == "dive" { 1800 } else { 0 }).unwrap();
            let v = case_verdicts(&spec, &f, &[], &mem());
            let u = v.iter().find(|x| x.name == "uncovered nodes").unwrap();
            assert!(
                u.fails()
                    && u.detail.contains("2 of")
                    && u.detail.contains("first at frame 40, 3 nodes, tiles 0x0000000000001234 0x0000000000005678 0x0000000000009abc")
                    && u.line(scene).ends_with(" OVER"),
                "{scene}: {u:?}"
            );
        }
        // dive-fast is report-only: the same frames are reported, with the ids, and do not fail.
        let mut fast = frames(2400, Some(0.0));
        fast[40].uncovered_nodes = 3;
        fast[40].uncovered_tiles = vec![0x1234, 0x5678, 0x9abc];
        let spec = case_spec("dive-fast", 2400, 1.0 / 60.0, 1800).unwrap();
        let v = case_verdicts(&spec, &fast, &[], &mem());
        let u = v.iter().find(|x| x.name == "uncovered nodes").unwrap();
        assert!(!u.fails() && u.status == Status::Reported && u.detail.contains("tiles 0x0000000000001234"), "{u:?}");
        // Nothing uncovered: an ok line, never absent.
        let spec = case_spec("look-1m", 600, 1.0 / 60.0, 0).unwrap();
        let none = case_verdicts(&spec, &frames(600, Some(0.0)), &[], &mem());
        assert_eq!(none.iter().find(|x| x.name == "uncovered nodes").unwrap().status, Status::Ok);
        let mut f = frames(600, Some(0.0));
        f[41].uncovered_nodes = 3;
        let t = frames_text(&f);
        assert!(t.lines().next().unwrap().contains("lag uncovered blank") && t.lines().nth(42).unwrap().contains(" 0.000 3 0 "), "{t}");
    }
}

#[cfg(test)]
mod load_tests {
    use super::*;

    fn job(run_us: u64, running: u32, level: u8) -> LoadJob {
        LoadJob { wait_us: run_us / 10, run_us, running, done: true, level }
    }

    fn get(counts: &[(String, u64)], name: &str) -> u64 {
        counts.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no count {name}: {counts:?}")).1
    }

    // spec: STRM-022
    #[test]
    fn the_load_counts_are_the_medians_tails_bands_and_cache_figures_of_fixed_jobs() {
        // 100 jobs: run time 1..=100 ms at 18 running, levels 5 (first 50) and 20 (last 50), and two that did not finish.
        let mut jobs: Vec<LoadJob> = (1..=100u64).map(|i| job(i * 1000, 18, if i <= 50 { 5 } else { 20 })).collect();
        jobs.push(LoadJob { done: false, ..job(999_999, 3, 5) });
        jobs.push(LoadJob { done: false, ..job(999_999, 3, 5) });
        let c = LoadCounters {
            block_decodes: 25,
            block_lookups: 400,
            block_hits: 300,
            block_evictions: 20,
            block_decode_waits: 7,
            block_lock_wait_us: 11,
            block_fetch_us: 22,
            block_decode_us: 33,
            block_resident: 31,
            block_budget: 31,
            pack_gets: 130,
            pack_found: 100,
            pack_get_us: 44,
            prefill_lookups: 105,
            prefill_found: 75,
            prefill_skipped: 9,
            prefill_us: 55,
            job_block_wait_us: 66,
            job_block_decodes: 20,
            field_builds: 1234,
            field_us: 77,
        };
        let counts = load_counts(&jobs, &c);
        assert!(counts.iter().all(|(n, _)| n.starts_with(LOAD_COUNT_PREFIX)), "{counts:?}");
        let names: std::collections::BTreeSet<&str> = counts.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names.len(), counts.len(), "no name twice");
        assert_eq!((get(&counts, "load_jobs"), get(&counts, "load_jobs_not_done")), (100, 2));
        // Sorted 1000..=100000: index round(99 q).
        assert_eq!(get(&counts, "load_run_us_median"), 51_000);
        assert_eq!(get(&counts, "load_run_us_p99"), 99_000);
        assert_eq!(get(&counts, "load_run_us_max"), 100_000);
        assert_eq!(get(&counts, "load_run_us_sum"), 5_050_000);
        assert_eq!(get(&counts, "load_wait_us_sum"), 505_000);
        assert_eq!((get(&counts, "load_running_median"), get(&counts, "load_running_max")), (18, 18));
        assert_eq!((get(&counts, "load_jobs_running_15_up"), get(&counts, "load_jobs_running_1_4")), (100, 0));
        assert_eq!(get(&counts, "load_run_us_median_running_15_up"), 51_000);
        assert_eq!((get(&counts, "load_jobs_level_0_8"), get(&counts, "load_jobs_level_19_up")), (50, 50));
        assert_eq!(get(&counts, "load_run_us_median_level_0_8"), 26_000);
        assert_eq!(get(&counts, "load_run_us_median_level_19_up"), 76_000);
        assert_eq!(get(&counts, "load_block_decodes_per_1000_tiles"), 250);
        assert_eq!(get(&counts, "load_block_lookups_per_1000_tiles"), 4000);
        for (name, want) in [
            ("load_block_hits", 300),
            ("load_block_evictions", 20),
            ("load_block_decode_waits", 7),
            ("load_block_resident", 31),
            ("load_block_budget", 31),
            ("load_pack_gets", 130),
            ("load_pack_found", 100),
            ("load_prefill_lookups", 105),
            ("load_prefill_found", 75),
            ("load_prefill_skipped", 9),
            ("load_job_block_wait_us", 66),
            ("load_job_block_decodes", 20),
            ("load_field_builds", 1234),
        ] {
            assert_eq!(get(&counts, name), want, "{name}");
        }
        // The same inputs give the same counts.
        assert_eq!(load_counts(&jobs, &c), counts);
    }

    // spec: STRM-022
    #[test]
    fn no_jobs_give_zeros_and_never_divide_by_zero() {
        let counts = load_counts(&[], &LoadCounters { block_decodes: 5, ..Default::default() });
        assert_eq!(
            (get(&counts, "load_jobs"), get(&counts, "load_run_us_median"), get(&counts, "load_block_decodes_per_1000_tiles")),
            (0, 0, 0)
        );
        assert_eq!(get(&counts, "load_block_decodes"), 5);
    }

    // spec: STRM-022
    #[test]
    fn the_calibration_kernel_does_fixed_work_and_the_calibration_reports_its_threads() {
        // Fixed work: the same value on every call and thread count, different for different amounts of work.
        assert_eq!(calibration_kernel(3), calibration_kernel(3));
        assert_ne!(calibration_kernel(3), calibration_kernel(4));
        assert_eq!(calibration_kernel(0), calibration_kernel(0));
        let c = calibrate_cpu(3);
        assert_eq!(c.threads, 3);
        assert!(c.parallel_median_us <= c.parallel_max_us, "{c:?}");
        let counts = calibration_counts("before", &c);
        assert_eq!(get(&counts, "load_calib_before_threads"), 3);
        assert_eq!(get(&counts, "load_calib_before_single_us"), c.single_us);
        assert_eq!(get(&counts, "load_calib_before_parallel_max_us"), c.parallel_max_us);
        assert_eq!(calibrate_cpu(0).threads, 1, "at least one thread");
    }
}
