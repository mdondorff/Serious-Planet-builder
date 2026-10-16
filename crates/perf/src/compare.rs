//! Comparison of a perf session with the last valid one for the same scene (TEST-012, CON-21): a median that rose by more
//! than [`REGRESSION_THRESHOLD`] is reported. Report only: the architect records it, nothing here fails a run.
//!
//! Pure over the JSON text of two logs ([`crate::report_json`]); a small reader keeps this crate dependency-free.

use crate::clock::{equal_clock_state, gpu_validity, view_clock_state, ClockPoint, ClockState, GpuValidity};
use std::collections::BTreeMap;

/// The metrics that need a GPU-valid view (ADR 0034 as amended by the owner, 2026-10-15; the clock floor per ADR 0036): `frame_ms` is judged and compared at any clock state.
pub const GPU_METRICS: [&str; 1] = ["gpu_ms"];
/// Metrics that are not times (levels of detail lag): kept in the log, never compared as a regression in milliseconds.
pub const NON_TIME_METRICS: [&str; 1] = ["detail_lag"];

/// A median that rose by more than this fraction of the last valid log's median is a regression.
pub const REGRESSION_THRESHOLD: f64 = 0.15;
/// A rise smaller than this many milliseconds is never reported: the sub-0.1 ms medians of the light views (orbit) move by
/// more than the threshold from one valid session to the next without meaning anything.
pub const MIN_RISE_MS: f64 = 0.1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub(crate) fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        while self.b.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", c as char, self.i))
        }
    }
    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(format!("unexpected text at byte {}", self.i))
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hex = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
                            let code = u32::from_str_radix(&String::from_utf8_lossy(hex), 16).map_err(|e| e.to_string())?;
                            self.i += 4;
                            let ch = char::from_u32(code).unwrap_or('\u{fffd}');
                            out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                        }
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        // The power scheme name arrives in the OEM code page and may be garbled: keep going with replacement characters.
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i).ok_or("unexpected end of the log")? {
            b'n' => self.word("null", Json::Null),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'"' => Ok(Json::Str(self.string()?)),
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(v));
                }
                loop {
                    v.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(v));
                        }
                        _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(v));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    v.push((k, self.value()?));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(v));
                        }
                        _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                    }
                }
            }
            _ => {
                let start = self.i;
                while self.b.get(self.i).is_some_and(|c| matches!(c, b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')) {
                    self.i += 1;
                }
                let text = String::from_utf8_lossy(&self.b[start..self.i]).into_owned();
                text.parse::<f64>().map(Json::Num).map_err(|_| format!("not a value at byte {start}"))
            }
        }
    }
}

pub(crate) fn parse(text: &[u8]) -> Result<Json, String> {
    let mut r = Reader { b: text, i: 0 };
    let v = r.value()?;
    r.ws();
    if r.i != text.len() {
        return Err(format!("trailing text at byte {}", r.i));
    }
    Ok(v)
}

/// What a comparison needs from a log: validity, the frame size and, per path, the median over the runs of each
/// per-run median (`frame_ms`, then every named metric) and the counts.
#[derive(Clone, Debug, PartialEq)]
pub struct LogSummary {
    pub valid: bool,
    pub resolution: (u64, u64),
    /// What the session measured (`protocol.field`); `None` for a log that names none.
    pub field: Option<String>,
    /// Where the bake was read from (`protocol.source`); `None` for a log that names none (the terrain scene).
    pub source: Option<String>,
    /// The tile generation pool of the session (`protocol.gen`, `GenSettings::describe`); `None` for a log that records none
    /// (logs from before it, the scenes without a pool). Informational unless both logs have it (TEST-012).
    pub gen: Option<String>,
    pub medians: BTreeMap<String, BTreeMap<String, f64>>,
    /// Per path: whether its GPU figures are valid and the clock state, read from the recorded samples (ADR 0034).
    pub gpu: BTreeMap<String, GpuValidity>,
    /// Per path: the clock state of the view, or why there is none (`no clock sample`); printed beside frame
    /// medians, which are compared at any state.
    pub clock: BTreeMap<String, Result<ClockState, String>>,
    pub counts: BTreeMap<String, BTreeMap<String, u64>>,
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

/// Read the figures of a log written by [`crate::report_json`].
pub fn summarize_log(text: &[u8]) -> Result<LogSummary, String> {
    let root = parse(text)?;
    // Frame, CPU and count figures count when the frame verdicts do (an idle GPU start voids only the GPU figures, per view).
    let valid = crate::validity::log_frame_valid(&root);
    let res = match root.get("protocol").and_then(|p| p.get("internal_resolution")) {
        Some(Json::Arr(a)) if a.len() == 2 => (a[0].num().unwrap_or(0.0) as u64, a[1].num().unwrap_or(0.0) as u64),
        _ => return Err("no protocol.internal_resolution".into()),
    };
    let Some(Json::Arr(runs)) = root.get("runs") else { return Err("no runs".into()) };
    let mut samples: BTreeMap<String, BTreeMap<String, Vec<f64>>> = BTreeMap::new();
    let mut counts: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut points: BTreeMap<String, Vec<Option<ClockPoint>>> = BTreeMap::new();
    for run in runs {
        let Some(Json::Str(path)) = run.get("path") else { return Err("a run without a path".into()) };
        let entry = samples.entry(path.clone()).or_default();
        for key in ["clocks_before", "clocks_after"] {
            let point = run.get(key).and_then(|c| match (c.get("pstate"), c.get("graphics_mhz").and_then(Json::num)) {
                (Some(Json::Str(p)), Some(mhz)) => Some(ClockPoint { pstate: p.clone(), mhz }),
                _ => None,
            });
            points.entry(path.clone()).or_default().push(point);
        }
        if let Some(m) = run.get("median_ms").and_then(Json::num) {
            entry.entry("frame_ms".into()).or_default().push(m);
        }
        if let Some(Json::Obj(metrics)) = run.get("metrics") {
            for (name, stats) in metrics {
                if let Some(m) = stats.get("median").and_then(Json::num) {
                    entry.entry(name.clone()).or_default().push(m);
                }
            }
        }
        if let Some(Json::Obj(cs)) = run.get("counts") {
            for (name, v) in cs {
                if let Some(n) = v.num() {
                    counts.entry(path.clone()).or_default().insert(name.clone(), n as u64);
                }
            }
        }
    }
    let medians = samples.into_iter().map(|(p, m)| (p, m.into_iter().map(|(name, v)| (name, median_of(v))).collect())).collect();
    let field = match root.get("protocol").and_then(|p| p.get("field")) {
        Some(Json::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let source = match root.get("protocol").and_then(|p| p.get("source")) {
        Some(Json::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let gen = root.get("protocol").and_then(|p| p.get("gen")).and_then(|g| {
        let n = |k: &str| g.get(k).and_then(Json::num).map(|v| v as usize);
        let below_normal = matches!(g.get("priority"), Some(Json::Str(s)) if s == "below-normal");
        Some(crate::GenSettings { workers: n("workers")?, ahead: n("ahead")?, reserve: n("reserve"), below_normal }.describe())
    });
    let gpu = points.iter().map(|(p, v)| (p.clone(), gpu_validity(v))).collect();
    let clock = points.iter().map(|(p, v)| (p.clone(), view_clock_state(v))).collect();
    Ok(LogSummary { valid, resolution: res, field, source, gen, medians, gpu, clock, counts })
}

/// Where the last valid log of the scene whose log is `log` is kept: `terrain.json` -> `terrain.last-valid.json` beside it.
pub fn baseline_path(log: &std::path::Path) -> std::path::PathBuf {
    let stem = log.file_stem().map_or_else(|| "perf".into(), |s| s.to_string_lossy().into_owned());
    log.with_file_name(format!("{stem}.last-valid.json"))
}

/// The log to compare with: the stored baseline, or, before one exists, the previous log of the scene.
pub fn choose_baseline(stored: Option<Vec<u8>>, previous_log: Option<Vec<u8>>) -> Option<Vec<u8>> {
    stored.or(previous_log)
}

/// Whether a session's log replaces the baseline: only a valid session does.
pub fn replaces_baseline(new: &[u8]) -> bool {
    summarize_log(new).is_ok_and(|s| s.valid)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rise {
    pub path: String,
    pub metric: String,
    pub old: f64,
    pub new: f64,
}

impl Rise {
    pub fn percent(&self) -> f64 {
        (self.new / self.old - 1.0) * 100.0
    }
}

/// Every median of `new` that exceeds the one of `old` for the same path and metric by more than `threshold` of the old
/// median and by at least [`MIN_RISE_MS`].
pub fn rises(old: &LogSummary, new: &LogSummary, threshold: f64) -> Vec<Rise> {
    rises_where(old, new, threshold, |_, _| true)
}

/// [`rises`] restricted to the (path, metric) pairs `include` accepts.
pub fn rises_where(old: &LogSummary, new: &LogSummary, threshold: f64, include: impl Fn(&str, &str) -> bool) -> Vec<Rise> {
    let mut out = Vec::new();
    for (path, metrics) in &new.medians {
        let Some(old_metrics) = old.medians.get(path) else { continue };
        for (metric, &n) in metrics {
            let Some(&o) = old_metrics.get(metric) else { continue };
            if !include(path, metric) {
                continue;
            }
            if o > 0.0 && n > o * (1.0 + threshold) && n - o >= MIN_RISE_MS {
                out.push(Rise { path: path.clone(), metric: metric.clone(), old: o, new: n });
            }
        }
    }
    out
}

/// Where the baseline of the GPU figures is kept: `terrain.json` -> `terrain.last-gpu-valid.json` beside it (ADR 0034).
pub fn gpu_baseline_path(log: &std::path::Path) -> std::path::PathBuf {
    let stem = log.file_stem().map_or_else(|| "perf".into(), |s| s.to_string_lossy().into_owned());
    log.with_file_name(format!("{stem}.last-gpu-valid.json"))
}

/// Whether a session's log replaces the GPU baseline: it is valid and at least one view is valid for GPU figures.
pub fn replaces_gpu_baseline(new: &[u8]) -> bool {
    summarize_log(new).is_ok_and(|s| s.valid && s.gpu.values().any(GpuValidity::is_valid))
}

/// The baseline as a summary if it can be compared with `new` (valid, same field, same frame size), else the console line.
fn usable_baseline(old_text: &[u8], new: &LogSummary) -> Result<LogSummary, String> {
    let old = match summarize_log(old_text) {
        Ok(s) if s.valid => s,
        Ok(_) => {
            return Err("regression check skipped: the stored baseline is not valid for budgets; this session becomes the baseline".into())
        }
        Err(e) => {
            return Err(format!("regression check skipped: the stored baseline could not be read ({e}); this session becomes the baseline"))
        }
    };
    if old.field != new.field {
        let name = |f: &Option<String>| f.clone().unwrap_or_else(|| "none recorded (before logs named it)".into());
        return Err(format!(
            "regression check skipped: the baseline measured field {}, this session {}; the figures are not comparable across a field change, so this session becomes the baseline",
            name(&old.field),
            name(&new.field)
        ));
    }
    if old.source != new.source {
        let name = |f: &Option<String>| f.clone().unwrap_or_else(|| "none recorded".into());
        return Err(format!(
            "regression check skipped: the baseline read its bake from {}, this session from {}; streaming figures are not comparable across a source change, so this session becomes the baseline",
            name(&old.source),
            name(&new.source)
        ));
    }
    // Frame and upload medians depend on how many threads generate tiles next to the frame thread (perf window 2): sessions that
    // recorded different pools are not compared. A log that records none (before the field) is compared and the pools are named.
    if let (Some(a), Some(b)) = (&old.gen, &new.gen) {
        if a != b {
            return Err(format!(
                "regression check skipped: gen settings differ: {a} vs {b}; frame and upload figures are not comparable across a change of the generation pool, so this session becomes the baseline"
            ));
        }
    }
    if old.resolution != new.resolution {
        return Err(format!(
            "regression check skipped: the baseline was measured at {}x{}, this session at {}x{}",
            old.resolution.0, old.resolution.1, new.resolution.0, new.resolution.1
        ));
    }
    Ok(old)
}

fn regression_line(r: &Rise) -> String {
    format!(
        "REGRESSION vs the last valid log: {} {} median {:.2} -> {:.2} ms ({:+.0} %, limit {:.0} %)",
        r.path,
        r.metric,
        r.old,
        r.new,
        r.percent(),
        REGRESSION_THRESHOLD * 100.0
    )
}

fn count_compared(old: &LogSummary, new: &LogSummary, include: impl Fn(&str, &str) -> bool) -> usize {
    new.medians
        .iter()
        .map(|(p, m)| m.keys().filter(|k| include(p, k) && old.medians.get(p).is_some_and(|o| o.contains_key(*k))).count())
        .sum()
}

/// The console lines that compare the new log with the last valid one (`None`: there is none yet). Never fails a run.
/// The GPU figures are compared against the same log; see [`comparison_lines_gpu`] for a separate GPU baseline.
pub fn comparison_lines(last_valid: Option<&[u8]>, new: &[u8]) -> Vec<String> {
    comparison_lines_gpu(last_valid, last_valid, new)
}

/// The label of a view's clock state in the report: the state, or the reason there is none.
fn clock_label(s: &LogSummary, path: &str) -> String {
    match s.clock.get(path) {
        Some(Ok(c)) => c.to_string(),
        Some(Err(e)) => e.clone(),
        None => "no clock sample".into(),
    }
}

/// Frame medians are compared at any clock state; the states of both views are printed beside them (ADR 0034 as amended).
fn frame_state_lines(old: &LogSummary, new: &LogSummary) -> Vec<String> {
    let mut out = Vec::new();
    for (path, m) in &new.medians {
        if !m.contains_key("frame_ms") || !old.medians.get(path).is_some_and(|o| o.contains_key("frame_ms")) {
            continue;
        }
        // ADR 0036 point 2: equal = both views GPU-valid and clocks within the tolerance (the P-state is printed, not compared).
        let equal = matches!((old.gpu.get(path), new.gpu.get(path)), (Some(GpuValidity::Valid(a)), Some(GpuValidity::Valid(b))) if equal_clock_state(a, b));
        if !equal {
            out.push(format!(
                "frame medians of {path} compared across unequal clock states (the frame time is what a user sees): clock state {} vs {}",
                clock_label(old, path),
                clock_label(new, path)
            ));
        }
    }
    out
}

/// [`comparison_lines`] with the baseline of the GPU figure (`gpu_ms`) apart from that of the frame medians,
/// CPU figures and counts (ADR 0034). A view's `gpu_ms` is compared only if both views are valid for GPU
/// figures at equal clock state; otherwise the line says why (`not comparable: clock state X vs Y`). Frame medians are
/// compared whatever the states and print both.
pub fn comparison_lines_gpu(last_valid: Option<&[u8]>, last_gpu_valid: Option<&[u8]>, new: &[u8]) -> Vec<String> {
    let new = match summarize_log(new) {
        Ok(s) => s,
        Err(e) => return vec![format!("regression check skipped: this session's log could not be read ({e})")],
    };
    if !new.valid {
        return vec![
            "regression check skipped: this session is not valid for budgets, so it is neither compared nor kept as the baseline".into()
        ];
    }
    let mut lines = Vec::new();
    let mut compared = 0usize;
    let mut found = 0usize;
    match last_valid {
        None => lines.push("regression check: no earlier valid log for this scene; this session becomes the baseline".into()),
        Some(text) => match usable_baseline(text, &new) {
            Err(line) => lines.push(line),
            Ok(old) => {
                let cpu = |_: &str, m: &str| !GPU_METRICS.contains(&m) && !NON_TIME_METRICS.contains(&m);
                let rs = rises_where(&old, &new, REGRESSION_THRESHOLD, cpu);
                found += rs.len();
                lines.extend(rs.iter().map(|r| {
                    let l = regression_line(r);
                    if r.metric == "frame_ms" {
                        format!("{l} [clock state {} vs {}]", clock_label(&old, &r.path), clock_label(&new, &r.path))
                    } else {
                        l
                    }
                }));
                lines.extend(frame_state_lines(&old, &new));
                if old.gen.is_some() != new.gen.is_some() {
                    let name = |g: &Option<String>| g.clone().unwrap_or_else(|| "none recorded".into());
                    lines.push(format!(
                        "gen settings: baseline {}, this session {} (recorded only; the figures were compared)",
                        name(&old.gen),
                        name(&new.gen)
                    ));
                }
                compared += count_compared(&old, &new, cpu);
                for (path, counts) in &new.counts {
                    for (name, &n) in counts {
                        if RECORDED_ONLY_COUNTS.contains(&name.as_str()) || name.starts_with(crate::streaming::LOAD_COUNT_PREFIX) {
                            continue;
                        }
                        if let Some(&o) = old.counts.get(path).and_then(|c| c.get(name)) {
                            if o != n {
                                lines.push(format!("count changed vs the last valid log: {path} {name} {o} -> {n}"));
                            }
                        }
                    }
                }
            }
        },
    }
    // The GPU figures, view by view.
    let gpu_old = last_gpu_valid.map(|t| usable_baseline(t, &new));
    for (path, validity) in &new.gpu {
        let has_gpu = new.medians.get(path).is_some_and(|m| GPU_METRICS.iter().any(|g| m.contains_key(*g)));
        if !has_gpu {
            continue;
        }
        let new_state = match validity {
            GpuValidity::Valid(s) => s,
            GpuValidity::Invalid(r) => {
                lines.push(format!("GPU medians of {path} not compared: NOT VALID for GPU budgets ({r})"));
                continue;
            }
        };
        let old = match &gpu_old {
            None => continue,
            Some(Err(line)) => {
                if !lines.contains(line) {
                    lines.push(line.clone());
                }
                continue;
            }
            Some(Ok(o)) => o,
        };
        match old.gpu.get(path) {
            None => continue,
            Some(GpuValidity::Invalid(r)) => {
                lines.push(format!("GPU medians of {path} not compared: the baseline view was not valid for GPU budgets ({r})"))
            }
            Some(GpuValidity::Valid(old_state)) if !equal_clock_state(old_state, new_state) => {
                lines.push(format!("GPU medians of {path} not comparable: clock state {old_state} vs {new_state}"))
            }
            Some(GpuValidity::Valid(_)) => {
                let only = |p: &str, m: &str| p == path && GPU_METRICS.contains(&m);
                let rs = rises_where(old, &new, REGRESSION_THRESHOLD, only);
                found += rs.len();
                lines.extend(rs.iter().map(regression_line));
                compared += count_compared(old, &new, only);
            }
        }
    }
    if found == 0 && compared > 0 {
        lines.push(format!(
            "regression check: {compared} medians compared with the last valid log, none up by more than {:.0} %",
            REGRESSION_THRESHOLD * 100.0
        ));
    }
    lines
}

/// Counts that are recorded in the log but never compared: they differ from run to run by nature (the CPU's effective clock at
/// the start and end of a case, the most slots resident in any frame), so a change is not news.
const RECORDED_ONLY_COUNTS: [&str; 4] = ["cpu_clock_before_mhz", "cpu_clock_after_mhz", "max_resident_slots", "scene_alone"];

#[cfg(test)]
mod tests {
    use super::*;

    const P0: &str = "{\"pstate\":\"P0\",\"graphics_mhz\":1470}";

    fn log(valid: bool, res: (u32, u32), ground_gpu: [f64; 3], orbit_gpu: [f64; 3], nodes: u64) -> String {
        let run = |path: &str, frame: f64, gpu: f64, nodes: u64| {
            format!(
                "{{\"path\":\"{path}\",\"median_ms\":{frame},\"clocks_before\":{P0},\"clocks_after\":{P0},\"metrics\":{{\"gpu_ms\":{{\"median\":{gpu},\"p99\":9}}}},\"counts\":{{\"nodes\":{nodes}}}}}"
            )
        };
        let mut runs = Vec::new();
        for i in 0..3 {
            runs.push(run("terrain/ground-1m", 7.0, ground_gpu[i], nodes));
            runs.push(run("terrain/orbit-20000km", 1.0, orbit_gpu[i], 52));
        }
        format!(
            "{{\"power_scheme\":\"H\u{fffd}chst \\\"x\\\" \\u00e4\",\"valid_for_budgets\":{valid},\"warnings\":[],\"protocol\":{{\"internal_resolution\":[{},{}]}},\"runs\":[{}]}}",
            res.0,
            res.1,
            runs.join(",")
        )
    }

    // spec: TEST-012
    #[test]
    fn a_median_up_by_more_than_the_threshold_is_reported_with_both_figures() {
        let old = log(true, (2560, 1440), [3.0, 3.1, 2.9], [0.06, 0.06, 0.06], 8687);
        let new = log(true, (2560, 1440), [4.2, 4.1, 4.3], [0.07, 0.07, 0.07], 8687);
        let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("REGRESSION") && lines[0].contains("terrain/ground-1m gpu_ms median 3.00 -> 4.20 ms (+40 %"),
            "{lines:?}"
        );
    }

    // spec: TEST-012
    #[test]
    fn a_rise_within_the_threshold_or_below_a_tenth_of_a_millisecond_is_not_reported() {
        let old = log(true, (2560, 1440), [3.0, 3.0, 3.0], [0.06, 0.06, 0.06], 8687);
        // +14 % on the heavy view; +50 % but only 0.03 ms on the light one.
        let new = log(true, (2560, 1440), [3.42, 3.42, 3.42], [0.09, 0.09, 0.09], 8687);
        let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("regression check:") && lines[0].contains("none up by more than 15 %"), "{lines:?}");
        // Just under +15 % is not reported, just over it is.
        let (o, under, over) = (
            summarize_log(old.as_bytes()).unwrap(),
            summarize_log(log(true, (2560, 1440), [3.44; 3], [0.06; 3], 8687).as_bytes()).unwrap(),
            summarize_log(log(true, (2560, 1440), [3.46; 3], [0.06; 3], 8687).as_bytes()).unwrap(),
        );
        assert!(rises(&o, &under, REGRESSION_THRESHOLD).is_empty());
        assert_eq!(rises(&o, &over, REGRESSION_THRESHOLD).len(), 1);
    }

    // spec: TEST-012
    #[test]
    fn the_median_over_the_runs_decides_not_one_outlier_run() {
        let old = log(true, (2560, 1440), [3.0, 3.0, 3.0], [0.06; 3], 8687);
        let new = log(true, (2560, 1440), [3.0, 9.0, 3.0], [0.06; 3], 8687);
        let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
        assert!(lines.iter().all(|l| !l.contains("REGRESSION")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn the_cpu_clock_counts_are_recorded_but_never_compared_and_a_log_without_them_stays_readable() {
        let plain = log(true, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        let with = |before: u32, after: u32| {
            plain.replace(
                "\"counts\":{\"nodes\":8687}",
                &format!("\"counts\":{{\"nodes\":8687,\"cpu_clock_before_mhz\":{before},\"cpu_clock_after_mhz\":{after},\"max_resident_slots\":{before}}}"),
            )
        };
        let (a, b) = (with(3100, 3000), with(2200, 1800));
        assert_ne!(a, plain);
        for (old, new) in [(&plain, &a), (&a, &plain), (&a, &b)] {
            let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
            assert!(lines.iter().all(|l| !l.contains("count changed") && !l.contains("could not be read")), "{lines:?}");
        }
        // A real count still counts.
        let lines = comparison_lines(Some(a.as_bytes()), with(3100, 3000).replace("8687", "9000").as_bytes());
        assert!(lines.iter().any(|l| l.contains("nodes 8687 -> 9000")), "{lines:?}");
    }

    // spec: STRM-022
    #[test]
    fn the_tile_load_counts_are_recorded_but_never_reported_as_changed_counts() {
        let plain = log(true, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        let with = |median: u64, jobs: u64| {
            plain.replace(
                "\"counts\":{\"nodes\":8687}",
                &format!("\"counts\":{{\"nodes\":8687,\"load_run_us_median\":{median},\"load_jobs\":{jobs}}}"),
            )
        };
        assert_ne!(with(1, 1), plain);
        for (old, new) in [(with(26_000, 14_000), with(52_000, 7_000)), (plain.clone(), with(1, 1)), (with(1, 1), plain.clone())] {
            let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
            assert!(lines.iter().all(|l| !l.contains("count changed") && !l.contains("could not be read")), "{lines:?}");
        }
    }

    // spec: TEST-019
    #[test]
    fn scene_alone_is_recorded_but_never_reported_as_a_changed_count() {
        let plain = log(true, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        let with =
            |alone: u32| plain.replace("\"counts\":{\"nodes\":8687}", &format!("\"counts\":{{\"nodes\":8687,\"scene_alone\":{alone}}}"));
        let lines = comparison_lines(Some(with(1).as_bytes()), with(0).as_bytes());
        assert!(lines.iter().all(|l| !l.contains("count changed")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn changed_counts_are_listed_beside_the_timing() {
        let old = log(true, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        let new = log(true, (2560, 1440), [4.0; 3], [0.06; 3], 9000);
        let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("terrain/ground-1m nodes 8687 -> 9000")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("REGRESSION")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn an_invalid_session_an_invalid_baseline_a_missing_baseline_and_another_frame_size_compare_nothing() {
        let good = log(true, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        let bad = log(false, (2560, 1440), [9.0; 3], [0.06; 3], 8687);
        let small = log(true, (1280, 720), [9.0; 3], [0.06; 3], 8687);
        let one = |old: Option<&str>, new: &str| comparison_lines(old.map(str::as_bytes), new.as_bytes());
        assert!(one(Some(&good), &bad)[0].contains("not valid for budgets"));
        assert!(one(Some(&bad), &good)[0].contains("stored baseline is not valid"));
        assert!(one(None, &good)[0].contains("becomes the baseline"));
        assert!(one(Some(&good), &small)[0].contains("2560x1440") && one(Some(&good), &small)[0].contains("1280x720"));
        assert!(one(Some("{not json"), &good)[0].contains("could not be read"));
        assert!(one(Some(&good), "{not json")[0].contains("could not be read"));
    }

    // spec: TEST-012
    #[test]
    fn only_a_valid_session_replaces_the_baseline_and_the_previous_log_stands_in_before_one_exists() {
        let good = log(true, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        let bad = log(false, (2560, 1440), [3.0; 3], [0.06; 3], 8687);
        assert!(replaces_baseline(good.as_bytes()));
        assert!(!replaces_baseline(bad.as_bytes()) && !replaces_baseline(b"{not json"));
        assert_eq!(choose_baseline(Some(b"a".to_vec()), Some(b"b".to_vec())), Some(b"a".to_vec()));
        assert_eq!(choose_baseline(None, Some(b"b".to_vec())), Some(b"b".to_vec()));
        assert_eq!(choose_baseline(None, None), None);
    }

    // spec: TEST-012
    #[test]
    fn the_baseline_sits_beside_the_log() {
        use std::path::Path;
        assert_eq!(baseline_path(Path::new("perf/logs/terrain.json")), Path::new("perf/logs/terrain.last-valid.json"));
        assert_eq!(baseline_path(Path::new("x.json")), Path::new("x.last-valid.json"));
    }

    // spec: TEST-012
    #[test]
    fn a_real_session_log_round_trips_through_the_reader() {
        use crate::{report_json, summarize, AdapterFacts, DeviceKind, Protocol, RunReport};
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let run = |gpu: &[f64]| RunReport {
            path_name: "terrain/ground-1m".into(),
            vram_cap_gb: 8,
            stats: summarize(&[5.0, 6.0, 7.0], 0).unwrap(),
            clocks_before: None,
            clocks_after: None,
            metrics: vec![("gpu_ms".into(), summarize(gpu, 0).unwrap())],
            counts: vec![("nodes".into(), 8687)],
        };
        let json = report_json(
            &a,
            Some("GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (H\u{fffd}chstleistung)"),
            &[],
            &crate::gate::Validity::default(),
            &Protocol::default(),
            &[run(&[3.0, 4.0, 5.0]), run(&[3.0, 4.0, 5.0])],
        );
        let s = summarize_log(json.as_bytes()).unwrap();
        assert!(s.valid);
        assert_eq!(s.medians["terrain/ground-1m"]["gpu_ms"], 4.0);
        assert_eq!(s.medians["terrain/ground-1m"]["frame_ms"], 6.0);
        assert_eq!(s.counts["terrain/ground-1m"]["nodes"], 8687);
        // Bytes that are not UTF-8 (the OEM code page) do not stop the reader.
        let mut raw = json.into_bytes();
        let at = raw.windows(3).position(|w| w == "\u{fffd}".as_bytes()).unwrap();
        raw.splice(at..at + 3, [0xe4u8]);
        assert!(summarize_log(&raw).unwrap().valid);
    }
}

#[cfg(test)]
mod field_tests {
    use super::*;
    use crate::{report_json, summarize, AdapterFacts, DeviceKind, Protocol, RunReport};

    fn session(field: Option<&str>, gpu: f64, nodes: u64) -> String {
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let run = RunReport {
            path_name: "terrain/ground-1m".into(),
            vram_cap_gb: 8,
            stats: summarize(&[5.0, 6.0, 7.0], 0).unwrap(),
            clocks_before: crate::parse_nvidia_smi("X, 1470, 810, 100, 60, P0, 0x0"),
            clocks_after: crate::parse_nvidia_smi("X, 1470, 810, 100, 60, P0, 0x0"),
            metrics: vec![("gpu_ms".into(), summarize(&[gpu, gpu, gpu], 0).unwrap())],
            counts: vec![("nodes".into(), nodes)],
        };
        let protocol = Protocol { field: field.map(str::to_string), ..Protocol::default() };
        report_json(&a, Some("High performance"), &[], &crate::gate::Validity::default(), &protocol, &[run])
    }

    // spec: TEST-012
    #[test]
    fn a_log_records_the_field_it_measured_and_is_not_compared_across_a_field_change() {
        let eroded = "cpu-erosion-v1 over a 256-cell bake";
        let new = session(Some(eroded), 4.0, 900);
        assert_eq!(summarize_log(new.as_bytes()).unwrap().field.as_deref(), Some(eroded), "the log names the field");
        // The baseline of the reference field (recorded, or from before logs named a field) is not compared: a 40 % rise and a
        // changed count would otherwise be reported as a regression of the code.
        for old in [session(Some("cpu-ref-height-v2"), 2.8, 8000), session(None, 2.8, 8000)] {
            let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert!(lines[0].contains("field") && lines[0].contains(eroded) && !lines[0].contains("REGRESSION"), "{lines:?}");
        }
        // Another bake of the same field is another measurement, too.
        let other_bake = session(Some("cpu-erosion-v1 over a 512-cell bake"), 2.8, 8000);
        assert!(comparison_lines(Some(other_bake.as_bytes()), new.as_bytes())[0].contains("512-cell"));
        // The same field compares as before.
        let same = session(Some(eroded), 2.8, 800);
        let lines = comparison_lines(Some(same.as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("REGRESSION")) && lines.iter().any(|l| l.contains("nodes 800 -> 900")), "{lines:?}");
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;
    use crate::{report_json_with_warmups, summarize, AdapterFacts, DeviceKind, Protocol, RunReport};

    /// A recorded session: one run each of ground-1m and orbit at the given states and GPU medians.
    fn session(ground: (&str, u32, f64), orbit: (&str, u32, f64)) -> String {
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let run = |name: &str, (pstate, mhz, gpu): (&str, u32, f64), cpu: f64| {
            let c = crate::parse_nvidia_smi(&format!("X, {mhz}, 810, 100, 60, {pstate}, 0x0"));
            RunReport {
                path_name: format!("terrain/{name}"),
                vram_cap_gb: 8,
                stats: summarize(&[gpu + 1.0; 3], 0).unwrap(),
                clocks_before: c.clone(),
                clocks_after: c,
                metrics: vec![("gpu_ms".into(), summarize(&[gpu; 3], 0).unwrap()), ("cpu_ms".into(), summarize(&[cpu; 3], 0).unwrap())],
                counts: vec![("nodes".into(), 100)],
            }
        };
        let runs = [run("ground-1m", ground, 6.0), run("orbit-20000km", orbit, 0.5)];
        report_json_with_warmups(&a, Some("High performance"), &[], &crate::gate::Validity::default(), &Protocol::default(), &runs, &[])
    }

    // spec: TEST-012
    #[test]
    fn a_log_marks_each_view_valid_for_gpu_figures_only_at_the_floor_and_keeps_the_session_valid() {
        let log = session(("P0", 1470, 3.0), ("P2", 1150, 0.5));
        assert!(log.contains("\"valid_for_budgets\":true"), "the CPU side of the session stays valid");
        let s = summarize_log(log.as_bytes()).unwrap();
        assert!(s.valid && s.gpu["terrain/ground-1m"].is_valid() && !s.gpu["terrain/orbit-20000km"].is_valid());
        assert!(log.contains("\"gpu_valid\":true") && log.contains("\"gpu_valid\":false"), "{log}");
        assert!(log.contains("\"gpu_clock_state\":\"P2 1150 MHz, below the 1400 MHz floor\""), "{log}");
        assert!(replaces_gpu_baseline(log.as_bytes()));
        assert!(!replaces_gpu_baseline(session(("P2", 1150, 4.0), ("P3", 1100, 0.5)).as_bytes()), "no GPU-valid view: no GPU baseline");
        // A light view in P2 at 1,710 MHz (the 2026-10-15 window) is GPU-valid.
        let light = session(("P2", 1710, 3.0), ("P2", 1710, 0.5));
        let s = summarize_log(light.as_bytes()).unwrap();
        assert!(s.gpu.values().all(GpuValidity::is_valid) && light.contains("\"gpu_clock_state\":\"P2 1710 MHz\""), "{light}");
        assert!(replaces_gpu_baseline(light.as_bytes()));
    }

    // spec: TEST-012
    #[test]
    fn gpu_medians_are_compared_only_between_views_at_equal_clock_state() {
        let old = session(("P0", 1470, 3.0), ("P0", 1470, 0.5));
        // Same state within the tolerance: a +40 % rise is reported.
        let near = session(("P0", 1450, 4.2), ("P0", 1470, 0.5));
        let lines = comparison_lines(Some(old.as_bytes()), near.as_bytes());
        assert!(lines.iter().any(|l| l.contains("terrain/ground-1m gpu_ms median 3.00 -> 4.20")), "{lines:?}");
        // Another loaded clock beyond the tolerance (P2 at 1,710 MHz is 14 % above): not comparable, and the line names both
        // states; the P-state itself is not what decides.
        let far = session(("P2", 1710, 4.2), ("P0", 1470, 0.5));
        let lines = comparison_lines(Some(old.as_bytes()), far.as_bytes());
        assert!(lines.iter().all(|l| !l.contains("gpu_ms")), "no GPU regression across unequal states: {lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("not comparable: clock state P0 1470 MHz vs P2 1710 MHz") && l.contains("ground-1m")),
            "{lines:?}"
        );
        // P0 against P2 at close clocks is the same state: compared.
        let p2_close = session(("P2", 1500, 4.2), ("P0", 1470, 0.5));
        let lines = comparison_lines(Some(old.as_bytes()), p2_close.as_bytes());
        assert!(lines.iter().any(|l| l.contains("terrain/ground-1m gpu_ms median 3.00 -> 4.20")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn the_p2_session_of_2026_10_13_against_the_p0_session_of_2026_10_11_is_not_a_regression() {
        let p0 = session(("P0", 1470, 3.0), ("P0", 1470, 0.5));
        let p2 = session(("P2", 1150, 4.2), ("P0", 1470, 0.5));
        let lines = comparison_lines(Some(p0.as_bytes()), p2.as_bytes());
        assert!(lines.iter().all(|l| !l.contains("gpu_ms")), "{lines:?}");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("ground-1m") && l.contains("NOT VALID for GPU budgets (P2 1150 MHz, below the 1400 MHz floor)")),
            "{lines:?}"
        );
        // The other way round: the baseline view was not valid.
        let lines = comparison_lines(Some(p2.as_bytes()), p0.as_bytes());
        assert!(
            lines.iter().any(|l| l.contains("baseline view was not valid for GPU budgets (P2 1150 MHz, below the 1400 MHz floor)")),
            "{lines:?}"
        );
        // The untouched P0 orbit view is still compared.
        assert!(lines.iter().any(|l| l.contains("regression check:") && l.contains("compared")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn cpu_figures_and_counts_are_compared_whatever_the_clock_state() {
        let a = |cpu: f64, nodes: u64| {
            let s = session(("P2", 1150, 4.0), ("P2", 1150, 0.5));
            // Rewrite the recorded cpu median and node count of the fixture.
            s.replace("\"cpu_ms\":{\"median\":6", &format!("\"cpu_ms\":{{\"median\":{cpu}"))
                .replace("\"nodes\":100", &format!("\"nodes\":{nodes}"))
        };
        let (old, new) = (a(6.0, 100), a(9.0, 120));
        let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("terrain/ground-1m cpu_ms median 6.00 -> 9.00")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("nodes 100 -> 120")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn a_log_without_samples_is_read_as_not_valid_for_gpu_and_a_separate_gpu_baseline_is_used() {
        // Old-format log: no clock samples at all.
        let old_fmt = session(("P0", 1470, 3.0), ("P0", 1470, 0.5))
            .replace("\"clocks_before\"", "\"x_before\"")
            .replace("\"clocks_after\"", "\"x_after\"");
        let s = summarize_log(old_fmt.as_bytes()).unwrap();
        assert!(s.gpu.values().all(|g| *g == GpuValidity::Invalid("no clock sample".into())), "{:?}", s.gpu);
        // The CPU baseline is the P2 session; the GPU baseline is the older P0 one: the GPU rise is still found.
        let cpu_base = session(("P2", 1150, 4.0), ("P0", 1470, 0.5));
        let gpu_base = session(("P0", 1470, 3.0), ("P0", 1470, 0.5));
        let new = session(("P0", 1470, 4.5), ("P0", 1470, 0.5));
        let lines = comparison_lines_gpu(Some(cpu_base.as_bytes()), Some(gpu_base.as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("terrain/ground-1m gpu_ms median 3.00 -> 4.50")), "{lines:?}");
        assert_eq!(
            gpu_baseline_path(std::path::Path::new("perf/logs/terrain.json")),
            std::path::Path::new("perf/logs/terrain.last-gpu-valid.json")
        );
    }

    // spec: TEST-012
    #[test]
    fn frame_medians_are_compared_across_unequal_clock_states_with_both_states_printed() {
        let p0 = session(("P0", 1470, 3.0), ("P0", 1470, 0.5));
        let p2 = session(("P2", 1150, 4.2), ("P0", 1470, 0.5));
        // Frame median 4.0 -> 5.2 ms (+30 %): reported although the states differ, with both states; the GPU figure is not.
        let lines = comparison_lines(Some(p0.as_bytes()), p2.as_bytes());
        let frame = lines.iter().find(|l| l.contains("REGRESSION") && l.contains("frame_ms")).unwrap_or_else(|| panic!("{lines:?}"));
        assert!(
            frame.contains("ground-1m") && frame.contains("4.00 -> 5.20") && frame.contains("clock state P0 1470 MHz vs P2 1150 MHz"),
            "{frame}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("frame medians of terrain/ground-1m compared")
                    && l.contains("clock state P0 1470 MHz vs P2 1150 MHz")),
            "{lines:?}"
        );
        assert!(lines.iter().all(|l| !l.contains("REGRESSION") || !l.contains("gpu_ms")), "{lines:?}");
        // The other way round, and P2 against P2 (equal states: no extra state line).
        let back = comparison_lines(Some(p2.as_bytes()), p0.as_bytes());
        assert!(back.iter().any(|l| l.contains("clock state P2 1150 MHz vs P0 1470 MHz")), "{back:?}");
        let (a, b) = (session(("P2", 1710, 4.0), ("P2", 1710, 0.5)), session(("P2", 1710, 5.5), ("P2", 1710, 0.5)));
        let same = comparison_lines(Some(a.as_bytes()), b.as_bytes());
        assert!(
            same.iter().any(|l| l.contains("REGRESSION") && l.contains("frame_ms") && l.contains("P2 1710 MHz vs P2 1710 MHz")),
            "{same:?}"
        );
        assert!(same.iter().all(|l| !l.starts_with("frame medians of")), "{same:?}");
        // Equal readings below the floor are not an equal state: both views must be GPU-valid.
        let (c, d) = (session(("P2", 1150, 4.0), ("P2", 1150, 0.5)), session(("P2", 1150, 5.5), ("P2", 1150, 0.5)));
        let low = comparison_lines(Some(c.as_bytes()), d.as_bytes());
        assert!(low.iter().any(|l| l.starts_with("frame medians of") && l.contains("P2 1150 MHz vs P2 1150 MHz")), "{low:?}");
    }

    // spec: TEST-012
    #[test]
    fn a_log_without_samples_still_compares_its_frame_medians() {
        let old_fmt = session(("P0", 1470, 3.0), ("P0", 1470, 0.5))
            .replace("\"clocks_before\"", "\"x_before\"")
            .replace("\"clocks_after\"", "\"x_after\"");
        let new = session(("P0", 1470, 4.5), ("P0", 1470, 0.5));
        let lines = comparison_lines(Some(old_fmt.as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("frame_ms") && l.contains("no clock sample vs P0 1470 MHz")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("GPU medians of terrain/ground-1m not compared")), "the GPU figure is withheld: {lines:?}");
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::{report_json, summarize, AdapterFacts, DeviceKind, Protocol, RunReport};

    fn session(scene: &str, source: Option<&str>, wall: f64, lag: f64) -> String {
        session_gen(scene, source, wall, lag, None)
    }

    fn session_gen(scene: &str, source: Option<&str>, wall: f64, lag: f64, gen: Option<crate::GenSettings>) -> String {
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let run = RunReport {
            path_name: format!("stream/{scene}"),
            vram_cap_gb: 8,
            stats: summarize(&[wall; 3], 0).unwrap(),
            clocks_before: crate::parse_nvidia_smi("X, 1470, 810, 100, 60, P0, 0x0"),
            clocks_after: crate::parse_nvidia_smi("X, 1470, 810, 100, 60, P0, 0x0"),
            metrics: vec![("detail_lag".into(), summarize(&[lag; 3], 0).unwrap())],
            counts: vec![("blank_frames".into(), 0)],
        };
        let protocol = Protocol {
            field: Some("height field cpu-erosion-v1 over a 16384-cell bake".into()),
            source: source.map(str::to_string),
            gen,
            ..Protocol::default()
        };
        report_json(&a, Some("High performance"), &[], &crate::gate::Validity::default(), &protocol, &[run])
    }

    // spec: TEST-012, STRM-020
    #[test]
    fn a_log_records_its_generation_settings_and_logs_with_other_settings_are_not_compared_as_equal() {
        let store = "store C:/bake, 16384 cells";
        let g18 = crate::GenSettings { workers: 18, ahead: 18, reserve: Some(2), below_normal: false };
        let g20 = crate::GenSettings { workers: 20, ahead: 20, reserve: Some(0), below_normal: false };
        let new = session_gen("dive", Some(store), 12.0, 0.4, Some(g18));
        let text = String::from_utf8(new.clone().into_bytes()).unwrap();
        assert!(text.contains("\"gen\":{\"workers\":18,\"ahead\":18,\"reserve\":2,\"priority\":\"normal\"}"), "{text}");
        assert_eq!(summarize_log(new.as_bytes()).unwrap().gen.as_deref(), Some("workers 18, ahead 18, reserve 2"));
        // The workers' priority is part of the settings (ADR 0046): a log taken below normal is not the same pool as one at normal.
        let low = session_gen("dive", Some(store), 12.0, 0.4, Some(crate::GenSettings { below_normal: true, ..g18 }));
        assert!(low.contains("\"priority\":\"below-normal\""), "{low}");
        assert_eq!(summarize_log(low.as_bytes()).unwrap().gen.as_deref(), Some("workers 18, ahead 18, reserve 2, below-normal priority"));
        let lines = comparison_lines(Some(new.as_bytes()), low.as_bytes());
        assert!(lines[0].contains("gen settings differ") && lines[0].contains("below-normal priority"), "{lines:?}");
        // A size fixed outright has no reserve to record.
        let fixed = session_gen(
            "dive",
            Some(store),
            12.0,
            0.4,
            Some(crate::GenSettings { workers: 8, ahead: 0, reserve: None, below_normal: false }),
        );
        assert_eq!(summarize_log(fixed.as_bytes()).unwrap().gen.as_deref(), Some("workers 8, ahead 0"));
        // Another pool: not compared, the line names both, the session becomes the baseline.
        let lines = comparison_lines(Some(session_gen("dive", Some(store), 6.0, 0.4, Some(g20)).as_bytes()), new.as_bytes());
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("gen settings differ: workers 20, ahead 20, reserve 0 vs workers 18, ahead 18, reserve 2"), "{lines:?}");
        assert!(!lines[0].contains("REGRESSION"), "{lines:?}");
        // The same pool compares, silently as to the pool.
        let lines = comparison_lines(Some(session_gen("dive", Some(store), 6.0, 0.4, Some(g18)).as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("stream/dive frame_ms") && l.contains("REGRESSION")), "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains("gen settings")), "{lines:?}");
        // A log from before the field still reads and compares; the pools are named beside the figures, never refused.
        let old = session_gen("dive", Some(store), 6.0, 0.4, None);
        assert_eq!(summarize_log(old.as_bytes()).unwrap().gen, None);
        let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("stream/dive frame_ms") && l.contains("REGRESSION")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("gen settings: baseline none recorded, this session workers 18")), "{lines:?}");
    }

    // spec: TEST-012
    #[test]
    fn a_streaming_log_records_its_source_and_is_compared_only_with_the_same_scene_and_source() {
        let store = "store C:/bake, 16384 cells";
        let new = session("dive", Some(store), 12.0, 0.4);
        assert_eq!(summarize_log(new.as_bytes()).unwrap().source.as_deref(), Some(store), "the log names the source");
        // Another source (an in-memory bake, a copy of the store elsewhere, or a log that names none) is not compared.
        for other in [Some("in-memory 256 cells (NOT the production source)"), Some("store D:/copy, 16384 cells"), None] {
            let old = session("dive", other, 6.0, 0.4);
            let lines = comparison_lines(Some(old.as_bytes()), new.as_bytes());
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert!(lines[0].contains("source") && !lines[0].contains("REGRESSION"), "{lines:?}");
        }
        // The same source compares: a frame median that rose by half is reported, a lag in levels is never a millisecond rise.
        let lines = comparison_lines(Some(session("dive", Some(store), 6.0, 0.01).as_bytes()), new.as_bytes());
        assert!(lines.iter().any(|l| l.contains("stream/dive frame_ms") && l.contains("REGRESSION")), "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains("detail_lag") && l.contains("REGRESSION")), "{lines:?}");
        // Another scene's log shares no path with this one: nothing is compared, so no scene is held to another's figures.
        let other_scene = session("teleport-1m", Some(store), 1.0, 0.0);
        let lines = comparison_lines(Some(other_scene.as_bytes()), new.as_bytes());
        assert!(!lines.iter().any(|l| l.contains("REGRESSION")), "{lines:?}");
    }
}
