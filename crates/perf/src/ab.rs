//! The A/B helper's rules (BUILD-033, ADR 0087): interleaved runs of two builds of one perf command and one verdict. Pure: the orchestration (worktrees, child
//! processes, sleeping) lives in `xtask`; this module parses the options, orders the rounds, reads the metrics of a run's log, retries gate refusals through a
//! [`Runner`] and prints the table, so each rule has a tier-A test on fake runs.

use crate::loaded::{self, Kind};

/// The relative change that counts as a difference between the two builds (the measured quiet noise, ADR 0087 point 1).
pub const AB_MARGIN: f64 = 0.05;
/// Rounds of a comparison unless `--rounds` says otherwise.
pub const DEFAULT_ROUNDS: usize = 5;
/// Gate refusals tolerated per run unless `--retries` says otherwise (the helper waits between them).
pub const DEFAULT_RETRIES: usize = 40;

/// What the helper knows about a metric it reads from a run's log (BUILD-033: the metric table).
#[derive(Clone, Copy, Debug)]
pub struct MetricInfo {
    /// The key in the log: a field of the run (`median_ms`, `p99_ms`) or a key of its `counts`.
    pub name: &'static str,
    pub label: &'static str,
    /// The kind of time, which says how an absolute reading is judged under load (BUILD-031).
    pub kind: Kind,
    /// The budget an absolute reading is judged against, `None` for a metric that only has a relative reading.
    pub budget: Option<f64>,
}

/// The metrics of the streaming scenes (`perf stream`), lower is better for all of them.
pub const METRICS: [MetricInfo; 9] = [
    MetricInfo { name: "load_run_us_median", label: "tile-ready: load run median (us)", kind: Kind::Cpu { pinned: false }, budget: None },
    MetricInfo { name: "load_wait_us_median", label: "tile load wait median (us)", kind: Kind::Cpu { pinned: false }, budget: None },
    MetricInfo { name: "load_run_us_p99", label: "tile-ready: load run p99 (us)", kind: Kind::Cpu { pinned: false }, budget: None },
    MetricInfo { name: "median_ms", label: "frame median (ms)", kind: Kind::FrameStreaming, budget: Some(16.6) },
    MetricInfo { name: "p99_ms", label: "frame p99 (ms)", kind: Kind::FrameStreaming, budget: Some(20.0) },
    MetricInfo {
        name: "reserve_terrain_main_us_median",
        label: "reserve: terrain main-thread median (us)",
        kind: Kind::Cpu { pinned: false },
        budget: Some(10_000.0),
    },
    MetricInfo { name: "reserve_terrain_gpu_us_median", label: "reserve: terrain GPU median (us)", kind: Kind::Gpu, budget: Some(5_000.0) },
    MetricInfo {
        name: "reserve_terrain_peak_threads",
        label: "reserve: terrain peak busy threads",
        kind: Kind::Cpu { pinned: false },
        budget: Some(12.0),
    },
    MetricInfo {
        name: "reserve_spin_us_median",
        label: "reserve: the loads' main-thread spin median (us)",
        kind: Kind::Cpu { pinned: false },
        budget: None,
    },
];

/// The metrics compared unless `--metrics` says otherwise: tile-ready and the frame p99 (planner, 2026-11-01).
pub const DEFAULT_METRICS: [&str; 2] = ["load_run_us_median", "p99_ms"];

/// A parsed `perf --ab` command line.
#[derive(Clone, Debug, PartialEq)]
pub struct AbSpec {
    /// The ref the working tree is compared with (a branch, tag or commit).
    pub reference: String,
    pub rounds: usize,
    pub retries: usize,
    pub metrics: Vec<String>,
    /// The file of `perf/logs` the command writes, without the directory.
    pub log: String,
    /// The arguments of the perf command, as given after the helper's own options.
    pub command: Vec<String>,
}

/// Parse `--ab <ref> [--rounds N] [--retries N] [--metrics a,b] [--log FILE] <perf command...>` (the arguments after `perf`, starting at `--ab`).
pub fn parse(args: &[String]) -> Result<AbSpec, String> {
    let mut it = args.iter();
    if it.next().map(String::as_str) != Some("--ab") {
        return Err("the A/B helper starts with --ab <ref>".into());
    }
    let reference = it.next().filter(|r| !r.starts_with("--")).ok_or("--ab needs the ref to compare with, e.g. --ab origin/main")?.clone();
    let (mut rounds, mut retries, mut metrics, mut log) = (DEFAULT_ROUNDS, DEFAULT_RETRIES, None::<Vec<String>>, None::<String>);
    let mut command: Vec<String> = Vec::new();
    while let Some(a) = it.next() {
        let mut number = |what: &str| -> Result<usize, String> {
            let v = it.next().ok_or_else(|| format!("{what} needs a count"))?;
            v.parse::<usize>().map_err(|_| format!("{what} must be a count, got '{v}'"))
        };
        match a.as_str() {
            "--rounds" => rounds = number("--rounds")?,
            "--retries" => retries = number("--retries")?,
            "--metrics" => {
                let v = it.next().ok_or("--metrics needs a comma-separated list")?;
                metrics = Some(v.split(',').map(str::to_string).collect());
            }
            "--log" => log = Some(it.next().ok_or("--log needs the file name in perf/logs")?.clone()),
            _ => command.push(a.clone()),
        }
    }
    if rounds == 0 {
        return Err("--rounds must be at least 1".into());
    }
    if command.is_empty() {
        return Err("--ab needs the perf command to run, e.g. perf --ab origin/main stream dive --lit --store DIR".into());
    }
    let metrics = metrics.unwrap_or_else(|| DEFAULT_METRICS.iter().map(ToString::to_string).collect());
    if let Some(bad) = metrics.iter().find(|m| !METRICS.iter().any(|i| i.name == m.as_str())) {
        return Err(format!("unknown metric '{bad}' (known: {})", METRICS.map(|m| m.name).join(", ")));
    }
    let log = match log.or_else(|| default_log(&command)) {
        Some(l) => l,
        None => return Err("cannot tell which file of perf/logs this command writes: give --log FILE (stream scenes are known)".into()),
    };
    Ok(AbSpec { reference, rounds, retries, metrics, log, command })
}

/// The log file of a `stream <scene>` command: `<scene>.json`, `<scene>-lit.json` with `--lit`.
fn default_log(command: &[String]) -> Option<String> {
    if command.first().map(String::as_str) != Some("stream") {
        return None;
    }
    let scene = command.get(1).filter(|s| !s.starts_with("--") && s.as_str() != "all")?;
    let lit = command.iter().any(|a| a == "--lit");
    Some(format!("{scene}{}.json", if lit { "-lit" } else { "" }))
}

/// Which build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The ref.
    Base,
    /// The working tree.
    Tree,
}

/// The order of the two runs of a round: alternating, so a drift of the machine cannot favour one build.
pub fn order(round: usize) -> [Side; 2] {
    if round.is_multiple_of(2) {
        [Side::Base, Side::Tree]
    } else {
        [Side::Tree, Side::Base]
    }
}

/// The value of `name` in the first run of a log: a field of the run or a key of its `counts`.
pub fn metric_value(log: &str, name: &str) -> Option<f64> {
    let json = crate::compare::parse(log.as_bytes()).ok()?;
    let crate::compare::Json::Arr(runs) = json.get("runs")? else { return None };
    let run = runs.first()?;
    run.get(name).and_then(|v| v.num()).or_else(|| run.get("counts")?.get(name)?.num())
}

/// Whether the output of a perf run says the readiness gate refused it, and the conditions it names (one per line).
pub fn gate_refusal(output: &str) -> Option<Vec<String>> {
    let at = output.find("the machine is not ready")?;
    let conditions: Vec<String> =
        output[at..].lines().skip(1).take_while(|l| l.starts_with("  ") && !l.trim().is_empty()).map(|l| l.trim().to_string()).collect();
    Some(conditions)
}

/// What one run of the command came to.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// The gate refused; the conditions it named.
    Refused(Vec<String>),
    /// The command failed for another reason.
    Failed(String),
    /// The text of the log the command wrote.
    Log(String),
}

/// Runs the command on one build (a worktree and a child process in `xtask`, fake runs in the tests).
pub trait Runner {
    fn run(&mut self, side: Side, round: usize) -> Outcome;
    /// Wait before a refused run is tried again.
    fn wait(&mut self);
}

/// One finished run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub side: Side,
    pub round: usize,
    /// The values of the spec's metrics, in its order.
    pub values: Vec<Option<f64>>,
    /// The log text, kept for the record.
    pub log: String,
}

/// The finished comparison and the gate refusals met on the way (each condition with how often it was named).
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    pub runs: Vec<Run>,
    pub refusals: Vec<(String, usize)>,
    /// Refused runs in total (a run refused three times before it ran counts three).
    pub refused_runs: usize,
}

/// Run `spec.rounds` rounds, each of the two builds in the order of [`order`], retrying a refused run up to `spec.retries` times.
pub fn execute(spec: &AbSpec, runner: &mut dyn Runner) -> Result<Comparison, String> {
    let mut out = Comparison::default();
    for round in 0..spec.rounds {
        for side in order(round) {
            let mut tries = 0;
            loop {
                match runner.run(side, round) {
                    Outcome::Log(log) => {
                        let values = spec.metrics.iter().map(|m| metric_value(&log, m)).collect();
                        out.runs.push(Run { side, round, values, log });
                        break;
                    }
                    Outcome::Failed(why) => return Err(format!("round {} ({side:?}): the command failed: {why}", round + 1)),
                    Outcome::Refused(conditions) => {
                        out.refused_runs += 1;
                        for c in conditions {
                            match out.refusals.iter_mut().find(|(n, _)| *n == c) {
                                Some((_, n)) => *n += 1,
                                None => out.refusals.push((c, 1)),
                            }
                        }
                        tries += 1;
                        if tries > spec.retries {
                            return Err(format!(
                                "round {} ({side:?}): the readiness gate still refused after {} retries ({})",
                                round + 1,
                                spec.retries,
                                out.refusals.iter().map(|(c, n)| format!("{c} x{n}")).collect::<Vec<_>>().join("; ")
                            ));
                        }
                        runner.wait();
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The median of the values (the mean of the middle two for an even count), `None` for none.
pub fn median(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 })
}

/// The verdict on one metric: the working tree against the ref, lower is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Within,
    Better,
    Worse,
    /// A side has no reading.
    Unknown,
}

/// Compare the medians: within the margin, better or worse.
pub fn change(base: Option<f64>, tree: Option<f64>) -> (Option<f64>, Change) {
    match (base, tree) {
        (Some(b), Some(t)) if b > 0.0 => {
            let rel = (t - b) / b;
            let c = if rel.abs() <= AB_MARGIN {
                Change::Within
            } else if rel < 0.0 {
                Change::Better
            } else {
                Change::Worse
            };
            (Some(rel), c)
        }
        _ => (None, Change::Unknown),
    }
}

/// The table of a comparison: per metric both medians, the change and the verdict, and the rule that judges the absolute reading under load.
pub fn table(spec: &AbSpec, cmp: &Comparison) -> Vec<String> {
    let mut lines = vec![format!(
        "A/B of `perf {}` against {}: {} rounds, order alternating, margin +-{:.0} %",
        spec.command.join(" "),
        spec.reference,
        spec.rounds,
        AB_MARGIN * 100.0
    )];
    lines.push("| metric | ref | tree | change | verdict | absolute reading under load |".to_string());
    lines.push("|---|---|---|---|---|---|".to_string());
    for (i, name) in spec.metrics.iter().enumerate() {
        let info = METRICS.iter().find(|m| m.name == name).expect("checked by parse");
        let med = |side| median(&cmp.runs.iter().filter(|r| r.side == side).filter_map(|r| r.values[i]).collect::<Vec<_>>());
        let (b, t) = (med(Side::Base), med(Side::Tree));
        let (rel, c) = change(b, t);
        let num = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{v:.2}"));
        let verdict = match c {
            Change::Within => "within the margin",
            Change::Better => "better",
            Change::Worse => "WORSE",
            Change::Unknown => "no reading",
        };
        let rule = match (loaded::margin(info.kind), info.budget) {
            (None, _) => "needs a quiet window (BUILD-031)".to_string(),
            (Some(m), Some(b)) => format!("loaded pass at {:.0} % under {b}", m * 100.0),
            (Some(m), None) => format!("relative only; a CPU time needs {:.0} % margin", m * 100.0),
        };
        lines.push(format!(
            "| {} | {} | {} | {} | {verdict} | {rule} |",
            info.label,
            num(b),
            num(t),
            rel.map_or("-".to_string(), |r| format!("{:+.1} %", r * 100.0))
        ));
    }
    let refused = if cmp.refused_runs == 0 {
        "the gate never refused".to_string()
    } else {
        format!(
            "the gate refused {} time(s): {}",
            cmp.refused_runs,
            cmp.refusals.iter().map(|(c, n)| format!("{c} x{n}")).collect::<Vec<_>>().join("; ")
        )
    };
    lines.push(refused);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(a: &[&str]) -> Vec<String> {
        a.iter().map(|x| x.to_string()).collect()
    }

    fn log(tile: f64, p99: f64) -> String {
        format!(
            "{{\"runs\":[{{\"path\":\"stream/dive\",\"median_ms\":1.5,\"p99_ms\":{p99},\"counts\":{{\"load_run_us_median\":{tile}}}}}]}}"
        )
    }

    // spec: BUILD-033
    #[test]
    fn the_command_line_gives_the_ref_the_options_the_command_and_the_log_of_a_stream_scene() {
        let spec = parse(&s(&["--ab", "origin/main", "stream", "dive", "--lit", "--store", "D:/bake"])).unwrap();
        assert_eq!((spec.reference.as_str(), spec.rounds, spec.retries), ("origin/main", DEFAULT_ROUNDS, DEFAULT_RETRIES));
        assert_eq!(spec.command, s(&["stream", "dive", "--lit", "--store", "D:/bake"]));
        assert_eq!(spec.log, "dive-lit.json");
        assert_eq!(spec.metrics, s(&["load_run_us_median", "p99_ms"]));
        let spec =
            parse(&s(&["--ab", "v1", "--rounds", "3", "stream", "teleport-2km", "--metrics", "median_ms", "--retries", "2"])).unwrap();
        assert_eq!((spec.rounds, spec.retries, spec.log.as_str(), spec.metrics), (3, 2, "teleport-2km.json", s(&["median_ms"])));
        let spec = parse(&s(&["--ab", "v1", "--log", "x.json", "global-bake"])).unwrap();
        assert_eq!(spec.log, "x.json");
    }

    // spec: BUILD-033
    #[test]
    fn a_bad_command_line_is_refused_with_the_remedy() {
        for (args, needle) in [
            (vec!["--ab"], "needs the ref"),
            (vec!["--ab", "main"], "needs the perf command"),
            (vec!["--ab", "main", "--rounds", "0", "stream", "dive"], "at least 1"),
            (vec!["--ab", "main", "--rounds", "x", "stream", "dive"], "must be a count"),
            (vec!["--ab", "main", "stream", "dive", "--metrics", "bogus"], "unknown metric 'bogus'"),
            (vec!["--ab", "main", "global-bake"], "give --log FILE"),
            (vec!["--ab", "main", "stream", "all"], "give --log FILE"),
        ] {
            let e = parse(&s(&args)).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
    }

    // spec: BUILD-033
    #[test]
    fn the_order_alternates_each_round_and_a_log_gives_its_metrics_from_the_run_or_its_counts() {
        assert_eq!(order(0), [Side::Base, Side::Tree]);
        assert_eq!(order(1), [Side::Tree, Side::Base]);
        assert_eq!(order(2), order(0));
        let l = log(3500.0, 7.5);
        assert_eq!(
            (metric_value(&l, "load_run_us_median"), metric_value(&l, "p99_ms"), metric_value(&l, "nope")),
            (Some(3500.0), Some(7.5), None)
        );
        assert_eq!(metric_value("not json", "p99_ms"), None);
    }

    // spec: BUILD-033
    #[test]
    fn a_gate_refusal_is_told_from_other_failures_and_names_its_conditions() {
        let out = "readiness gate: sampling\nerror: refusing to measure: the machine is not ready (ADR 0020):\n  quiet machine: no build or test processes: cargo.exe (pid 7)\n  CPU load: 40.0 %\nFix these and retry";
        assert_eq!(gate_refusal(out), Some(s(&["quiet machine: no build or test processes: cargo.exe (pid 7)", "CPU load: 40.0 %"])));
        assert_eq!(gate_refusal("error: something else"), None);
    }

    /// Fake runs: the ref reads 100, the tree 110 (a tile-ready metric); the frame p99 is 8 for both. `refuse` runs are refused first.
    struct Fake {
        calls: Vec<(Side, usize)>,
        waits: usize,
        refuse_next: usize,
        fail: bool,
    }

    impl Runner for Fake {
        fn run(&mut self, side: Side, round: usize) -> Outcome {
            self.calls.push((side, round));
            if self.fail {
                return Outcome::Failed("exit 1".into());
            }
            if self.refuse_next > 0 {
                self.refuse_next -= 1;
                return Outcome::Refused(s(&["quiet machine: no build or test processes: cargo.exe (pid 7)"]));
            }
            Outcome::Log(log(if side == Side::Base { 100.0 } else { 110.0 }, 8.0))
        }

        fn wait(&mut self) {
            self.waits += 1;
        }
    }

    // spec: BUILD-033
    #[test]
    fn five_rounds_alternate_keep_every_log_retry_refusals_and_give_one_table() {
        let spec = parse(&s(&["--ab", "origin/main", "stream", "dive"])).unwrap();
        let mut fake = Fake { calls: vec![], waits: 0, refuse_next: 3, fail: false };
        let cmp = execute(&spec, &mut fake).unwrap();
        assert_eq!(cmp.runs.len(), 10, "every round ran both builds");
        assert_eq!(cmp.runs.iter().filter(|r| r.side == Side::Tree).count(), 5);
        assert!(cmp.runs.iter().all(|r| !r.log.is_empty()), "each log is kept");
        let sides: Vec<Side> = fake.calls.iter().map(|c| c.0).collect();
        assert_eq!(&sides[3..7], &[Side::Base, Side::Tree, Side::Tree, Side::Base], "refused tries repeat the same run; rounds alternate");
        assert_eq!((fake.waits, cmp.refused_runs), (3, 3));
        assert_eq!(cmp.refusals, vec![("quiet machine: no build or test processes: cargo.exe (pid 7)".to_string(), 3)]);
        let t = table(&spec, &cmp);
        assert!(t[0].contains("5 rounds") && t[0].contains("origin/main"), "{t:?}");
        let tile = t.iter().find(|l| l.contains("tile-ready: load run median")).unwrap();
        assert!(tile.contains("| 100.00 | 110.00 | +10.0 % | WORSE |"), "{tile}");
        let p99 = t.iter().find(|l| l.contains("frame p99")).unwrap();
        assert!(p99.contains("| 8.00 | 8.00 | +0.0 % | within the margin | needs a quiet window"), "{p99}");
        assert!(t.last().unwrap().contains("the gate refused 3 time(s)"), "{t:?}");
    }

    // spec: BUILD-033
    #[test]
    fn a_gate_that_keeps_refusing_or_a_failing_command_stops_the_helper_with_the_reasons() {
        let spec = AbSpec { retries: 2, ..parse(&s(&["--ab", "x", "stream", "dive"])).unwrap() };
        let mut stuck = Fake { calls: vec![], waits: 0, refuse_next: 99, fail: false };
        let e = execute(&spec, &mut stuck).unwrap_err();
        assert!(e.contains("still refused after 2 retries") && e.contains("cargo.exe (pid 7) x3"), "{e}");
        let mut broken = Fake { calls: vec![], waits: 0, refuse_next: 0, fail: true };
        assert!(execute(&spec, &mut broken).unwrap_err().contains("the command failed: exit 1"));
    }

    // spec: BUILD-033
    #[test]
    fn medians_and_changes_follow_the_margin_in_both_directions() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert_eq!(change(Some(100.0), Some(105.0)).1, Change::Within);
        assert_eq!(change(Some(100.0), Some(105.1)).1, Change::Worse);
        assert_eq!(change(Some(100.0), Some(60.0)).1, Change::Better);
        assert_eq!(change(None, Some(1.0)).1, Change::Unknown);
        assert_eq!(change(Some(0.0), Some(1.0)).1, Change::Unknown);
    }
}
