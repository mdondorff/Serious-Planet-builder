//! System-wide CPU load and the competing processes during a perf case (TEST-023; M4 window lessons, ADR 0020, ADR 0034, ADR 0036).
//!
//! The readiness gate (ADR 0020) looks at the machine for 10 s *before* a run; nothing looked at it *during* one, so a window that
//! lost a scene to a browser, an indexer or a build could not say so afterwards. A sampler takes the per-process CPU time counters
//! of the whole machine (`Win32_PerfRawData_PerfProc_Process`: cumulative 100 ns units, locale-independent) every few seconds
//! beside the case and turns each pair of readings into a [`LoadSample`]: the machine's busy share, the harness's own share (the
//! process and its helpers: the sampler's own `powershell`, `nvidia-smi`) and the rest, the *competing* load, with the top
//! processes by name. Everything below is pure over those readings and tested on recorded fixtures; only [`LoadSampler`] touches
//! the platform.
//!
//! Recorded only (owner and planner, M5 item 8): nothing here voids a verdict. A session whose competing load or busy processes
//! exceeded the readiness limits during a case carries a *note* (`load.notes` in the log, a line on the console), which uses the
//! numbers of ADR 0020 (15 % CPU load, the busy-process names) and does not invent a new limit; whether such a note should void
//! a session is the architect's call after the first window that shows one.

use crate::gate::{BUSY_PROCESSES, MAX_CPU_LOAD_PCT};
use std::process::Command;

/// Top processes kept per sample and per case.
pub const TOP_N: usize = 5;
/// Seconds between two readings while a case runs.
pub const SAMPLE_PERIOD_S: u64 = 5;
/// Samples kept in a log per case (one per period: 5 minutes of case at the default period).
pub const MAX_SAMPLES: usize = 64;
/// The competing load that earns a note: ADR 0020's readiness limit on the machine's mean CPU load, applied to the part of the load
/// that is not the harness's.
pub const NOTE_COMPETING_MEAN_PCT: f64 = MAX_CPU_LOAD_PCT;
/// A busy process (ADR 0020's list) that used at least this share of the machine in a sample earns a note.
pub const NOTE_BUSY_PROCESS_PCT: f64 = 1.0;
/// Names of the harness's own helpers (lower case, no `.exe`): the sampler's PowerShell, `nvidia-smi`, the WMI host that answers
/// the sampler's query and the console host. They count as the harness's load, never as a competitor.
pub const HELPER_PROCESSES: [&str; 5] = ["powershell", "pwsh", "nvidia-smi", "wmiprvse", "conhost"];

/// One process's cumulative CPU time at a reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcRaw {
    pub pid: u32,
    pub name: String,
    /// Cumulative CPU time in 100 ns units.
    pub cpu_100ns: u64,
}

/// A reading of every process's CPU time, with the system clock of the reading in 100 ns units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawSample {
    pub at_100ns: u64,
    pub procs: Vec<ProcRaw>,
}

/// The name of a process without `.exe` and without the `#2` suffix the performance counters add to a second instance.
fn clean_name(raw: &str) -> String {
    let n = raw.trim();
    let n = n.split('#').next().unwrap_or(n);
    n.strip_suffix(".exe").unwrap_or(n).to_string()
}

/// Parse the query's lines `<timestamp>,<pid>,<cpu time>,<name>`; `None` when nothing parses (the query failed).
pub fn parse_raw(csv: &str) -> Option<RawSample> {
    let mut at = 0u64;
    let mut procs = Vec::new();
    for line in csv.lines() {
        let mut it = line.trim().splitn(4, ',');
        let (Some(ts), Some(pid), Some(cpu), Some(name)) = (it.next(), it.next(), it.next(), it.next()) else { continue };
        let (Ok(ts), Ok(pid), Ok(cpu)) = (ts.trim().parse::<u64>(), pid.trim().parse::<u32>(), cpu.trim().parse::<u64>()) else {
            continue;
        };
        at = at.max(ts);
        procs.push(ProcRaw { pid, name: clean_name(name), cpu_100ns: cpu });
    }
    (!procs.is_empty() && at > 0).then_some(RawSample { at_100ns: at, procs })
}

/// One process (or all instances of one name) and its share of the machine, percent of all logical processors.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcLoad {
    pub name: String,
    pub pct: f64,
}

/// The load between two readings.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadSample {
    /// Seconds since the sampler started, at the end of the interval.
    pub at_s: f64,
    /// Length of the interval, seconds.
    pub span_s: f64,
    /// The machine's busy share, percent of all logical processors (every process but `Idle`).
    pub system_pct: f64,
    /// The harness's share: its own process and its helpers.
    pub harness_pct: f64,
    /// `system - harness`, at least 0: what ran beside the case.
    pub competing_pct: f64,
    /// The competing processes by name, largest first, at most [`TOP_N`].
    pub top: Vec<ProcLoad>,
    /// Processes of ADR 0020's busy list that used at least [`NOTE_BUSY_PROCESS_PCT`], chosen before the top is cut.
    pub busy: Vec<ProcLoad>,
}

fn is_helper(name: &str) -> bool {
    let n = name.to_lowercase();
    HELPER_PROCESSES.contains(&n.as_str())
}

/// The load between readings `a` and `b` (`b` later) on a machine with `logical` processors; `own_pid` is the harness. `None` when the
/// clock did not advance. A process that appeared between the readings counts its whole time (it ran only inside the interval), one
/// that exited in between is not seen (the load is then slightly under-counted, never over-counted).
pub fn between(a: &RawSample, b: &RawSample, logical: usize, own_pid: u32, at_s: f64) -> Option<LoadSample> {
    let dt = b.at_100ns.checked_sub(a.at_100ns).filter(|d| *d > 0)?;
    let denom = dt as f64 * logical.max(1) as f64;
    let before: std::collections::HashMap<u32, &ProcRaw> = a.procs.iter().map(|p| (p.pid, p)).collect();
    let (mut system, mut harness) = (0.0f64, 0.0f64);
    let mut by_name: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for p in &b.procs {
        let lower = p.name.to_lowercase();
        if p.pid == 0 || lower == "idle" || lower == "_total" {
            continue;
        }
        // A reused pid with less time than before is a new process: its whole time counts.
        let used = match before.get(&p.pid) {
            Some(prev) if prev.name == p.name && p.cpu_100ns >= prev.cpu_100ns => p.cpu_100ns - prev.cpu_100ns,
            _ => p.cpu_100ns,
        };
        let pct = used as f64 / denom * 100.0;
        system += pct;
        if p.pid == own_pid || is_helper(&p.name) {
            harness += pct;
        } else {
            *by_name.entry(p.name.clone()).or_insert(0.0) += pct;
        }
    }
    let mut top: Vec<ProcLoad> = by_name.into_iter().map(|(name, pct)| ProcLoad { name, pct }).collect();
    let busy: Vec<ProcLoad> = top
        .iter()
        .filter(|p| BUSY_PROCESSES.contains(&p.name.to_lowercase().as_str()) && p.pct >= NOTE_BUSY_PROCESS_PCT)
        .cloned()
        .collect();
    top.sort_by(|x, y| y.pct.total_cmp(&x.pct).then_with(|| x.name.cmp(&y.name)));
    top.truncate(TOP_N);
    Some(LoadSample {
        at_s,
        span_s: dt as f64 / 1.0e7,
        system_pct: system,
        harness_pct: harness,
        competing_pct: (system - harness).max(0.0),
        top,
        busy,
    })
}

/// A process name seen over a case: its mean and largest share of the samples it appeared in.
#[derive(Clone, Debug, PartialEq)]
pub struct TopOverCase {
    pub name: String,
    /// Mean share over all samples (a sample without it counts 0).
    pub mean_pct: f64,
    pub max_pct: f64,
    /// Samples the name was among the top of.
    pub seen: usize,
}

/// What a case's samples say, in one place for the log, the console and the report.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseLoad {
    /// `false` where the host cannot be read (not Windows, a failed query, a smoke run): the fields below are empty and the log says
    /// `not measured`.
    pub measured: bool,
    pub samples: Vec<LoadSample>,
}

impl CaseLoad {
    pub fn not_measured() -> CaseLoad {
        CaseLoad { measured: false, samples: Vec::new() }
    }

    pub fn from_samples(mut samples: Vec<LoadSample>) -> CaseLoad {
        // A longer case keeps its first samples and its last one, so `start` and `end` stay the case's.
        if samples.len() > MAX_SAMPLES {
            let last = samples.pop().expect("non-empty");
            samples.truncate(MAX_SAMPLES - 1);
            samples.push(last);
        }
        CaseLoad { measured: !samples.is_empty(), samples }
    }

    fn mean_of(&self, f: impl Fn(&LoadSample) -> f64) -> Option<f64> {
        (!self.samples.is_empty()).then(|| self.samples.iter().map(f).sum::<f64>() / self.samples.len() as f64)
    }

    fn max_of(&self, f: impl Fn(&LoadSample) -> f64) -> Option<f64> {
        self.samples.iter().map(f).fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v))))
    }

    pub fn system_mean(&self) -> Option<f64> {
        self.mean_of(|s| s.system_pct)
    }
    pub fn system_max(&self) -> Option<f64> {
        self.max_of(|s| s.system_pct)
    }
    pub fn competing_mean(&self) -> Option<f64> {
        self.mean_of(|s| s.competing_pct)
    }
    pub fn competing_max(&self) -> Option<f64> {
        self.max_of(|s| s.competing_pct)
    }
    pub fn harness_mean(&self) -> Option<f64> {
        self.mean_of(|s| s.harness_pct)
    }

    /// The competing processes over the whole case, largest mean share first, at most [`TOP_N`].
    pub fn top_over_case(&self) -> Vec<TopOverCase> {
        let n = self.samples.len().max(1) as f64;
        let mut acc: std::collections::BTreeMap<&str, (f64, f64, usize)> = std::collections::BTreeMap::new();
        for s in &self.samples {
            for p in &s.top {
                let e = acc.entry(p.name.as_str()).or_insert((0.0, 0.0, 0));
                e.0 += p.pct;
                e.1 = e.1.max(p.pct);
                e.2 += 1;
            }
        }
        let mut v: Vec<TopOverCase> = acc
            .into_iter()
            .map(|(name, (sum, max, seen))| TopOverCase { name: name.to_string(), mean_pct: sum / n, max_pct: max, seen })
            .collect();
        v.sort_by(|a, b| b.mean_pct.total_cmp(&a.mean_pct).then_with(|| a.name.cmp(&b.name)));
        v.truncate(TOP_N);
        v
    }

    /// The notes of the case (empty when the case ran beside nothing worth saying): competing load above ADR 0020's limit on
    /// average, and any busy process of ADR 0020's list that took [`NOTE_BUSY_PROCESS_PCT`] of the machine in a sample. A case whose
    /// load was not measured has no notes (the console says `not measured`, which is not a pass either).
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if let Some(m) = self.competing_mean().filter(|m| *m > NOTE_COMPETING_MEAN_PCT) {
            let top =
                self.top_over_case().into_iter().take(3).map(|t| format!("{} {:.1} %", t.name, t.mean_pct)).collect::<Vec<_>>().join(", ");
            notes.push(format!(
                "competing CPU load averaged {m:.1} % over {} samples (ADR 0020's readiness limit is {MAX_CPU_LOAD_PCT:.0} %; the harness's own load is not counted): {top}",
                self.samples.len()
            ));
        }
        let mut busy: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
        for s in &self.samples {
            for p in &s.busy {
                let e = busy.entry(p.name.clone()).or_insert(0.0);
                *e = e.max(p.pct);
            }
        }
        for (name, pct) in busy {
            notes.push(format!(
                "a busy process ran beside the case: {name} used up to {pct:.1} % of the machine (ADR 0020 refuses these before a run)"
            ));
        }
        notes
    }

    /// The console line of the case: the means and maxima, the harness's own share, the top processes, and `not measured`.
    pub fn line(&self) -> String {
        if !self.measured {
            return "host load: not measured".to_string();
        }
        let f = |v: Option<f64>| v.map_or("n/a".to_string(), |v| format!("{v:.1}"));
        let top = self.top_over_case();
        let top = if top.is_empty() {
            "none".to_string()
        } else {
            top.iter().map(|t| format!("{} {:.1} % (max {:.1})", t.name, t.mean_pct, t.max_pct)).collect::<Vec<_>>().join(", ")
        };
        format!(
            "host load ({} samples, recorded not judged): system mean {} % max {} %; harness {} %; competing mean {} % max {} %; top competitors: {top}",
            self.samples.len(),
            f(self.system_mean()),
            f(self.system_max()),
            f(self.harness_mean()),
            f(self.competing_mean()),
            f(self.competing_max())
        )
    }
}

/// The case's load as the log's `load` object: the aggregates, the first and last samples (start and end), every sample, the notes.
pub fn load_json(l: &CaseLoad) -> String {
    use crate::{json_f, json_str};
    if !l.measured {
        return "{\"measured\":false}".to_string();
    }
    let opt = |v: Option<f64>| v.map_or("null".to_string(), json_f);
    let tops = |t: &[ProcLoad]| {
        t.iter().map(|p| format!("{{\"name\":{},\"pct\":{}}}", json_str(&p.name), json_f(p.pct))).collect::<Vec<_>>().join(",")
    };
    let sample = |s: &LoadSample| {
        format!(
            "{{\"at_s\":{},\"span_s\":{},\"system_pct\":{},\"harness_pct\":{},\"competing_pct\":{},\"top\":[{}]}}",
            json_f(s.at_s),
            json_f(s.span_s),
            json_f(s.system_pct),
            json_f(s.harness_pct),
            json_f(s.competing_pct),
            tops(&s.top)
        )
    };
    let over = l
        .top_over_case()
        .iter()
        .map(|t| {
            format!(
                "{{\"name\":{},\"mean_pct\":{},\"max_pct\":{},\"seen\":{}}}",
                json_str(&t.name),
                json_f(t.mean_pct),
                json_f(t.max_pct),
                t.seen
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"measured\":true,\"samples\":{},\"system_mean_pct\":{},\"system_max_pct\":{},\"harness_mean_pct\":{},\"competing_mean_pct\":{},\"competing_max_pct\":{},\"top\":[{}],\"start\":{},\"end\":{},\"series\":[{}],\"notes\":[{}]}}",
        l.samples.len(),
        opt(l.system_mean()),
        opt(l.system_max()),
        opt(l.harness_mean()),
        opt(l.competing_mean()),
        opt(l.competing_max()),
        over,
        l.samples.first().map_or("null".to_string(), sample),
        l.samples.last().map_or("null".to_string(), sample),
        l.samples.iter().map(sample).collect::<Vec<_>>().join(","),
        l.notes().iter().map(|n| json_str(n)).collect::<Vec<_>>().join(",")
    )
}

/// The PowerShell that prints one line per process: `<Timestamp_Sys100NS>,<pid>,<cpu 100 ns>,<name>`. The raw class carries the
/// cumulative time, so the difference of two readings is exact whatever the interval; the class name is the same in every locale.
pub const QUERY_SCRIPT: &str = "Get-CimInstance Win32_PerfRawData_PerfProc_Process | ForEach-Object { \"$($_.Timestamp_Sys100NS),$($_.IDProcess),$($_.PercentProcessorTime),$($_.Name)\" }";

/// One reading of every process's CPU time now (about a second; never on a frame path). `None` where it cannot be read.
pub fn query_raw() -> Option<RawSample> {
    let out = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", QUERY_SCRIPT]).output().ok()?;
    out.status.success().then(|| parse_raw(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// The competing CPU load (percent of the whole machine) between two readings `seconds` apart: every process but `own_pid` and
/// the helpers that read the machine. `None` where a reading fails or the clock did not advance. Blocks for `seconds`.
pub fn competing_load(seconds: u32, own_pid: u32) -> Option<f64> {
    let logical = std::thread::available_parallelism().map_or(1, |n| n.get());
    let a = query_raw()?;
    std::thread::sleep(std::time::Duration::from_secs(u64::from(seconds)));
    let b = query_raw()?;
    competing_between(&a, &b, logical, own_pid)
}

/// [`competing_load`] from two readings already taken.
pub fn competing_between(a: &RawSample, b: &RawSample, logical: usize, own_pid: u32) -> Option<f64> {
    between(a, b, logical, own_pid, 0.0).map(|s| s.competing_pct)
}

/// A background sampler of the host's load beside a case. Start it before the measured frames, stop it after; it reads on its own
/// thread, so the frame loop never waits for a process spawn.
pub struct LoadSampler {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<Vec<LoadSample>>>,
}

impl LoadSampler {
    /// Start sampling every `period_s` seconds (the first reading is taken at once; each later one makes a [`LoadSample`]).
    pub fn start(period_s: u64) -> LoadSampler {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop2 = std::sync::Arc::clone(&stop);
        let own = std::process::id();
        let logical = std::thread::available_parallelism().map_or(1, |n| n.get());
        let handle = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let mut samples = Vec::new();
            let Some(mut prev) = query_raw() else { return samples };
            let mut last = std::time::Instant::now();
            let take = |prev: &mut RawSample, samples: &mut Vec<LoadSample>| {
                if let Some(next) = query_raw() {
                    if let Some(s) = between(prev, &next, logical, own, started.elapsed().as_secs_f64()) {
                        samples.push(s);
                    }
                    *prev = next;
                }
            };
            while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if last.elapsed().as_secs() >= period_s {
                    last = std::time::Instant::now();
                    take(&mut prev, &mut samples);
                }
            }
            // The end of the case: one more reading, so the last interval is the case's end.
            take(&mut prev, &mut samples);
            samples
        });
        LoadSampler { stop, handle: Some(handle) }
    }

    /// Stop and return what the case saw (`not measured` when the host could not be read).
    pub fn stop(mut self) -> CaseLoad {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let samples = self.handle.take().and_then(|h| h.join().ok()).unwrap_or_default();
        CaseLoad::from_samples(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(at: u64, rows: &[(u32, &str, u64)]) -> RawSample {
        RawSample {
            at_100ns: at,
            procs: rows.iter().map(|(pid, n, c)| ProcRaw { pid: *pid, name: (*n).to_string(), cpu_100ns: *c }).collect(),
        }
    }

    // spec: TEST-023
    #[test]
    fn the_query_output_is_parsed_with_instance_suffixes_and_exe_removed_and_garbage_skipped() {
        let csv = "133000000000,0,900000000,Idle\n133000000000,4,1000,System\n133000000000,812,5000,chrome#3\n133000000000,900,7,Code.exe\nnot a row\n,,,\n";
        let s = parse_raw(csv).unwrap();
        assert_eq!(s.at_100ns, 133_000_000_000);
        let names: Vec<&str> = s.procs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Idle", "System", "chrome", "Code"]);
        assert!(parse_raw("").is_none() && parse_raw("garbage").is_none(), "a failed query is not a sample");
    }

    // spec: TEST-032
    #[test]
    fn the_gate_and_the_settle_poll_read_the_competing_load_and_not_the_harness_start() {
        // A child that has just started: it uses 30 % of an 8-processor machine itself (store open, world build) and its sampler
        // helper 1 %; a browser uses 6 %. The gate's reading is the browser's 6 %, whatever the harness is doing.
        let a = raw(1_000_000_000, &[(0, "Idle", 0), (100, "planet", 0), (300, "powershell", 0), (200, "chrome", 0)]);
        let b = raw(
            1_100_000_000,
            &[(0, "Idle", 5_000_000_000), (100, "planet", 240_000_000), (300, "powershell", 8_000_000), (200, "chrome", 48_000_000)],
        );
        let gate = competing_between(&a, &b, 8, 100).unwrap();
        assert!((gate - 6.0).abs() < 1e-9, "{gate}");
        // The settle poll runs in the xtask process (pid 7) between children: the same two readings give the same reading when the
        // harness child is a competitor to it (its tail), and a quiet machine reads zero.
        let poll = competing_between(&a, &b, 8, 7).unwrap();
        assert!((poll - 36.0).abs() < 1e-9, "the previous child's tail counts for the poll: {poll}");
        let quiet = raw(1_100_000_000, &[(0, "Idle", 5_000_000_000), (100, "planet", 0), (300, "powershell", 0), (200, "chrome", 0)]);
        assert_eq!(competing_between(&a, &quiet, 8, 100), Some(0.0));
        assert!(competing_between(&b, &a, 8, 100).is_none(), "time must advance");
    }

    /// By hand on Windows (`cargo test -p planet-perf live_competing -- --ignored --nocapture`): the live reading is a sane percentage.
    // spec: TEST-032
    #[test]
    #[ignore = "reads the live machine through PowerShell"]
    fn live_competing_load_is_a_percentage() {
        let v = competing_load(2, std::process::id()).expect("a reading");
        println!("competing load over 2 s: {v:.1} %");
        assert!((0.0..=100.0).contains(&v), "{v}");
    }

    // spec: TEST-023
    #[test]
    fn load_between_two_readings_splits_the_machine_into_harness_helpers_and_competitors() {
        // 8 logical processors, a 10 s interval = 1e8 units each: 8e8 units of capacity.
        let a = raw(1_000_000_000, &[(0, "Idle", 0), (100, "planet", 0), (200, "chrome", 0), (300, "powershell", 0), (400, "msedge", 0)]);
        let b = raw(
            1_100_000_000,
            &[
                (0, "Idle", 5_000_000_000),
                (100, "planet", 200_000_000),   // 25 % of the machine
                (200, "chrome", 160_000_000),   // 20 %
                (300, "powershell", 8_000_000), // 1 %: the sampler's own helper
                (400, "msedge", 40_000_000),    // 5 %
                (500, "chrome", 80_000_000),    // a second chrome, new: 10 %
            ],
        );
        let s = between(&a, &b, 8, 100, 10.0).unwrap();
        assert!((s.span_s - 10.0).abs() < 1e-9);
        assert!((s.system_pct - 61.0).abs() < 1e-9, "{s:?}");
        assert!((s.harness_pct - 26.0).abs() < 1e-9, "the process and its helper: {s:?}");
        assert!((s.competing_pct - 35.0).abs() < 1e-9, "{s:?}");
        let top: Vec<(&str, f64)> = s.top.iter().map(|p| (p.name.as_str(), p.pct)).collect();
        assert_eq!(top, [("chrome", 30.0), ("msedge", 5.0)], "instances of one name add up, the harness is not listed");
        assert!(between(&b, &a, 8, 100, 0.0).is_none() && between(&a, &a, 8, 100, 0.0).is_none(), "time must advance");
    }

    // spec: TEST-023
    #[test]
    fn the_top_list_is_cut_at_five_and_ordered_by_share_then_name() {
        let names = ["a", "b", "c", "d", "e", "f", "g"];
        let a = raw(10, &names.iter().enumerate().map(|(i, n)| (10 + i as u32, *n, 0u64)).collect::<Vec<_>>());
        let b = raw(
            10 + 1_000_000,
            &names
                .iter()
                .enumerate()
                .map(|(i, n)| (10 + i as u32, *n, if i < 2 { 5_000u64 } else { (i as u64 + 1) * 1_000 }))
                .collect::<Vec<_>>(),
        );
        let s = between(&a, &b, 1, 1, 1.0).unwrap();
        assert_eq!(s.top.len(), TOP_N);
        assert_eq!(s.top.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["g", "f", "a", "b", "e"], "{:?}", s.top);
    }

    fn sample(at: f64, competing: f64, top: &[(&str, f64)]) -> LoadSample {
        LoadSample {
            at_s: at,
            span_s: 5.0,
            system_pct: competing + 20.0,
            harness_pct: 20.0,
            competing_pct: competing,
            top: top.iter().map(|(n, p)| ProcLoad { name: (*n).to_string(), pct: *p }).collect(),
            busy: top
                .iter()
                .filter(|(n, p)| BUSY_PROCESSES.contains(n) && *p >= NOTE_BUSY_PROCESS_PCT)
                .map(|(n, p)| ProcLoad { name: (*n).to_string(), pct: *p })
                .collect(),
        }
    }

    // spec: TEST-023
    #[test]
    fn the_case_summary_has_means_maxima_the_top_over_the_case_and_notes_only_above_adr_0020s_numbers() {
        let quiet = CaseLoad::from_samples(vec![sample(5.0, 2.0, &[("Teams", 2.0)]), sample(10.0, 4.0, &[("Teams", 3.0), ("Code", 1.0)])]);
        assert_eq!(quiet.competing_mean(), Some(3.0));
        assert_eq!(quiet.competing_max(), Some(4.0));
        assert_eq!(quiet.system_mean(), Some(23.0));
        assert_eq!(quiet.harness_mean(), Some(20.0));
        let top = quiet.top_over_case();
        assert_eq!((top[0].name.as_str(), top[0].mean_pct, top[0].max_pct, top[0].seen), ("Teams", 2.5, 3.0, 2));
        assert_eq!((top[1].name.as_str(), top[1].mean_pct), ("Code", 0.5), "a sample without the name counts 0");
        assert!(quiet.notes().is_empty(), "{:?}", quiet.notes());
        assert!(quiet.line().contains("recorded not judged") && quiet.line().contains("Teams"));

        // Competing load above the readiness limit on average: a note, with the numbers and the names.
        let loaded = CaseLoad::from_samples(vec![sample(5.0, 30.0, &[("chrome", 28.0)]), sample(10.0, 10.0, &[("chrome", 9.0)])]);
        let notes = loaded.notes();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("20.0 %") && notes[0].contains("15 %") && notes[0].contains("chrome"), "{notes:?}");
        // Exactly the limit is not above it.
        assert!(CaseLoad::from_samples(vec![sample(5.0, 15.0, &[])]).notes().is_empty());

        // A busy process of ADR 0020's list taking 1 % or more earns its own note, whatever the mean.
        let build = CaseLoad::from_samples(vec![sample(5.0, 3.0, &[("rustc", 2.5)]), sample(10.0, 1.0, &[("cargo", 0.4)])]);
        let notes = build.notes();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("rustc") && notes[0].contains("2.5 %") && notes[0].contains("ADR 0020"), "{notes:?}");
    }

    // spec: TEST-023
    #[test]
    fn an_unmeasured_case_says_so_and_has_no_numbers_and_no_notes() {
        let n = CaseLoad::not_measured();
        assert_eq!(n.line(), "host load: not measured");
        assert!(n.notes().is_empty() && n.system_mean().is_none() && n.top_over_case().is_empty());
        assert_eq!(load_json(&n), "{\"measured\":false}");
        assert!(!CaseLoad::from_samples(vec![]).measured, "no samples is not a measurement");
    }

    // spec: TEST-023
    #[test]
    fn the_log_object_parses_and_carries_the_aggregates_start_end_series_and_notes() {
        let l =
            CaseLoad::from_samples(vec![sample(5.0, 30.0, &[("chrome", 28.0)]), sample(10.0, 10.0, &[("chrome", 9.0), ("Teams", 1.0)])]);
        let json = load_json(&l);
        let j = crate::compare::parse(json.as_bytes()).unwrap();
        let num = |k: &str| match j.get(k) {
            Some(crate::compare::Json::Num(n)) => *n,
            other => panic!("{k}: {other:?}"),
        };
        assert_eq!(num("samples"), 2.0);
        assert_eq!(num("competing_mean_pct"), 20.0);
        assert_eq!(num("competing_max_pct"), 30.0);
        assert_eq!(num("harness_mean_pct"), 20.0);
        for k in ["start", "end"] {
            assert!(matches!(j.get(k), Some(crate::compare::Json::Obj(_))), "{k}");
        }
        assert!(matches!(j.get("series"), Some(crate::compare::Json::Arr(a)) if a.len() == 2));
        assert!(matches!(j.get("notes"), Some(crate::compare::Json::Arr(a)) if a.len() == 1));
        assert!(matches!(j.get("top"), Some(crate::compare::Json::Arr(a)) if a.len() == 2));
    }

    // spec: TEST-023
    #[test]
    fn the_session_log_carries_the_load_of_each_run_and_the_notes_and_changes_no_validity_answer() {
        use crate::{gate, parse_nvidia_smi, summarize, AdapterFacts, DeviceKind, Protocol, RunReport};
        let rtx =
            AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let run = |path: &str| RunReport {
            path_name: path.into(),
            vram_cap_gb: 8,
            stats: summarize(&[1.0, 2.0], 0).unwrap(),
            clocks_before: parse_nvidia_smi("X, 1700, 405, 15, 50, P0, 0x0"),
            clocks_after: parse_nvidia_smi("X, 1700, 405, 15, 50, P0, 0x0"),
            metrics: vec![],
            counts: vec![],
        };
        let runs = [run("stream/dive"), run("stream/look-1m")];
        let scheme = Some("Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)");
        let loaded = CaseLoad::from_samples(vec![sample(5.0, 40.0, &[("chrome", 38.0)])]);
        let loads = vec![("stream/dive".to_string(), loaded)];
        let with = crate::report_json_full(&rtx, scheme, &[], &gate::Validity::default(), &Protocol::default(), &runs, &[], &loads);
        let without = crate::report_json(&rtx, scheme, &[], &gate::Validity::default(), &Protocol::default(), &runs);
        let j = crate::compare::parse(with.as_bytes()).unwrap();
        let crate::compare::Json::Arr(rs) = j.get("runs").unwrap() else { panic!("runs") };
        assert!(rs[0].get("load").is_some_and(|l| matches!(l.get("measured"), Some(crate::compare::Json::Bool(true)))), "{with}");
        assert!(rs[1].get("load").is_none(), "a run without an entry carries none");
        let crate::compare::Json::Arr(notes) = j.get("load_notes").unwrap() else { panic!("load_notes") };
        assert_eq!(notes.len(), 1);
        assert!(matches!(&notes[0], crate::compare::Json::Str(s) if s.starts_with("stream/dive: competing CPU load averaged 40.0 %")));
        // Recorded only: both validity answers and the warnings are what they are without the load.
        for key in ["valid_for_budgets", "frame_valid", "gpu_session_valid"] {
            assert_eq!(j.get(key), crate::compare::parse(without.as_bytes()).unwrap().get(key), "{key}");
        }
        assert!(matches!(j.get("valid_for_budgets"), Some(crate::compare::Json::Bool(true))));
        assert!(crate::compare::summarize_log(with.as_bytes()).unwrap().valid, "the regression report still reads a log with load");
    }

    /// The platform query on this host (Windows): about two seconds a reading. Run by hand: `cargo test -p planet-perf -- --ignored live`.
    // spec: TEST-023
    #[test]
    #[ignore = "queries the host through PowerShell"]
    fn live_the_sampler_reads_this_host_and_the_harness_is_among_its_own_load() {
        let sampler = LoadSampler::start(1);
        let mut x = 0u64;
        let t = std::time::Instant::now();
        while t.elapsed().as_secs() < 4 {
            x = x.wrapping_add(std::hint::black_box(1));
        }
        let load = sampler.stop();
        println!("{} (spun {x})", load.line());
        assert!(load.measured, "no samples on this host");
        assert!(load.harness_mean().unwrap() > 0.0, "the spinning thread is the harness's own load");
    }

    // spec: TEST-023
    #[test]
    fn a_busy_process_hidden_below_the_top_five_still_earns_its_note_and_a_long_case_keeps_its_last_sample() {
        let names = ["a", "b", "c", "d", "e", "rustc"];
        let a = raw(10, &names.iter().enumerate().map(|(i, n)| (10 + i as u32, *n, 0u64)).collect::<Vec<_>>());
        let b = raw(
            10 + 1_000_000,
            &names
                .iter()
                .enumerate()
                .map(|(i, n)| (10 + i as u32, *n, if *n == "rustc" { 15_000u64 } else { 100_000 + i as u64 }))
                .collect::<Vec<_>>(),
        );
        let s = between(&a, &b, 1, 1, 1.0).unwrap();
        assert!(!s.top.iter().any(|p| p.name == "rustc"), "cut from the top five");
        let notes = CaseLoad::from_samples(vec![s]).notes();
        assert!(notes.iter().any(|n| n.contains("rustc") && n.contains("1.5 %")), "{notes:?}");
        let many: Vec<LoadSample> = (0..100).map(|i| sample(f64::from(i), 1.0, &[])).collect();
        let c = CaseLoad::from_samples(many);
        assert_eq!(c.samples.len(), MAX_SAMPLES);
        assert_eq!(c.samples.last().unwrap().at_s, 99.0, "the end of the case is kept");
        assert_eq!(c.samples[0].at_s, 0.0);
    }
}
