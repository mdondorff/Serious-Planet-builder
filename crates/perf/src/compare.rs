//! Comparison of a perf session with the last valid one for the same scene (TEST-012, CON-21): a median that rose by more
//! than [`REGRESSION_THRESHOLD`] is reported. Report only: the architect records it, nothing here fails a run.
//!
//! Pure over the JSON text of two logs ([`crate::report_json`]); a small reader keeps this crate dependency-free.

use std::collections::BTreeMap;

/// A median that rose by more than this fraction of the last valid log's median is a regression.
pub const REGRESSION_THRESHOLD: f64 = 0.15;
/// A rise smaller than this many milliseconds is never reported: the sub-0.1 ms medians of the light views (orbit) move by
/// more than the threshold from one valid session to the next without meaning anything.
pub const MIN_RISE_MS: f64 = 0.1;

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn num(&self) -> Option<f64> {
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

fn parse(text: &[u8]) -> Result<Json, String> {
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
    pub medians: BTreeMap<String, BTreeMap<String, f64>>,
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
    let valid = matches!(root.get("valid_for_budgets"), Some(Json::Bool(true)));
    let res = match root.get("protocol").and_then(|p| p.get("internal_resolution")) {
        Some(Json::Arr(a)) if a.len() == 2 => (a[0].num().unwrap_or(0.0) as u64, a[1].num().unwrap_or(0.0) as u64),
        _ => return Err("no protocol.internal_resolution".into()),
    };
    let Some(Json::Arr(runs)) = root.get("runs") else { return Err("no runs".into()) };
    let mut samples: BTreeMap<String, BTreeMap<String, Vec<f64>>> = BTreeMap::new();
    let mut counts: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for run in runs {
        let Some(Json::Str(path)) = run.get("path") else { return Err("a run without a path".into()) };
        let entry = samples.entry(path.clone()).or_default();
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
    Ok(LogSummary { valid, resolution: res, field, medians, counts })
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
    let mut out = Vec::new();
    for (path, metrics) in &new.medians {
        let Some(old_metrics) = old.medians.get(path) else { continue };
        for (metric, &n) in metrics {
            let Some(&o) = old_metrics.get(metric) else { continue };
            if o > 0.0 && n > o * (1.0 + threshold) && n - o >= MIN_RISE_MS {
                out.push(Rise { path: path.clone(), metric: metric.clone(), old: o, new: n });
            }
        }
    }
    out
}

/// The console lines that compare the new log with the last valid one (`None`: there is none yet). Never fails a run.
pub fn comparison_lines(last_valid: Option<&[u8]>, new: &[u8]) -> Vec<String> {
    let new = match summarize_log(new) {
        Ok(s) => s,
        Err(e) => return vec![format!("regression check skipped: this session's log could not be read ({e})")],
    };
    if !new.valid {
        return vec![
            "regression check skipped: this session is not valid for budgets, so it is neither compared nor kept as the baseline".into()
        ];
    }
    let Some(old_text) = last_valid else {
        return vec!["regression check: no earlier valid log for this scene; this session becomes the baseline".into()];
    };
    let old = match summarize_log(old_text) {
        Ok(s) if s.valid => s,
        Ok(_) => {
            return vec!["regression check skipped: the stored baseline is not valid for budgets; this session becomes the baseline".into()]
        }
        Err(e) => {
            return vec![format!(
                "regression check skipped: the stored baseline could not be read ({e}); this session becomes the baseline"
            )]
        }
    };
    if old.field != new.field {
        let name = |f: &Option<String>| f.clone().unwrap_or_else(|| "none recorded (before logs named it)".into());
        return vec![format!(
            "regression check skipped: the baseline measured field {}, this session {}; the figures are not comparable across a field change, so this session becomes the baseline",
            name(&old.field),
            name(&new.field)
        )];
    }
    if old.resolution != new.resolution {
        return vec![format!(
            "regression check skipped: the baseline was measured at {}x{}, this session at {}x{}",
            old.resolution.0, old.resolution.1, new.resolution.0, new.resolution.1
        )];
    }
    let mut lines = Vec::new();
    let found = rises(&old, &new, REGRESSION_THRESHOLD);
    for r in &found {
        lines.push(format!(
            "REGRESSION vs the last valid log: {} {} median {:.2} -> {:.2} ms ({:+.0} %, limit {:.0} %)",
            r.path,
            r.metric,
            r.old,
            r.new,
            r.percent(),
            REGRESSION_THRESHOLD * 100.0
        ));
    }
    for (path, counts) in &new.counts {
        for (name, &n) in counts {
            if let Some(&o) = old.counts.get(path).and_then(|c| c.get(name)) {
                if o != n {
                    lines.push(format!("count changed vs the last valid log: {path} {name} {o} -> {n}"));
                }
            }
        }
    }
    if found.is_empty() {
        let compared: usize =
            new.medians.iter().map(|(p, m)| m.keys().filter(|k| old.medians.get(p).is_some_and(|o| o.contains_key(*k))).count()).sum();
        lines.push(format!(
            "regression check: {compared} medians compared with the last valid log, none up by more than {:.0} %",
            REGRESSION_THRESHOLD * 100.0
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(valid: bool, res: (u32, u32), ground_gpu: [f64; 3], orbit_gpu: [f64; 3], nodes: u64) -> String {
        let run = |path: &str, frame: f64, gpu: f64, nodes: u64| {
            format!(
                "{{\"path\":\"{path}\",\"median_ms\":{frame},\"metrics\":{{\"gpu_ms\":{{\"median\":{gpu},\"p99\":9}}}},\"counts\":{{\"nodes\":{nodes}}}}}"
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
            clocks_before: None,
            clocks_after: None,
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
