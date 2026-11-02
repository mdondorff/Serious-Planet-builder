//! The perf window runner's rules (BUILD-034, ADR 0087 point 7, SPIKE-95): a declared plan of configurations and flights, run in rotated
//! rounds with the duty idle between runs (BUILD-035), bounded retries, a stop rule that ends a configuration once its verdict against
//! the reference is settled, and the start, end and wall time of every attempt. Pure: `xtask` builds once, runs the window's preflight,
//! starts the children and sleeps; this module parses the plan, orders the runs, judges each log and settles the verdicts, so every
//! rule has a tier-A test on fake runs.

use crate::ab::{self, Outcome, AB_MARGIN, DEFAULT_METRICS, METRICS};

/// A configuration settles as worse after one valid round beyond this multiple of the margin (ADR 0087 point 7: a reading far from
/// the reference cannot come back within the margin in the rounds left, with this machine's round-to-round spread of up to 10 %).
pub const STOP_FAR_MULTIPLE: f64 = 3.0;
/// A configuration settles as within the margin after this many valid rounds within it on every metric.
pub const STOP_WITHIN_ROUNDS: usize = 2;
/// Rounds of a window unless the plan says otherwise.
pub const DEFAULT_ROUNDS: usize = 3;
/// Retries of a run that came out invalid (the CPU calibration, throttling) or was refused by the gate, unless the plan says otherwise.
pub const DEFAULT_RETRIES: usize = 2;

/// One configuration: a name and the environment its runs get.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub name: String,
    pub env: Vec<(String, String)>,
}

/// One flight: a name, the arguments of `cargo xtask perf ...` and the file of `perf/logs` it writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub name: String,
    pub command: Vec<String>,
    pub log: String,
}

/// A parsed window plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub rounds: usize,
    pub retries: usize,
    /// The index of the reference configuration in `configs`.
    pub reference: usize,
    pub configs: Vec<Config>,
    pub flights: Vec<Flight>,
    pub metrics: Vec<String>,
}

/// Parse a plan: one statement per line, `#` starts a comment.
/// `rounds N`, `retries N`, `metrics a,b`, `reference NAME`, `config NAME [KEY=VALUE ...]`, `flight NAME <perf arguments...>`
/// (`flight dive stream dive --store DIR`; `--log FILE` names the log of a command that is not `stream <scene>`).
pub fn parse_plan(text: &str) -> Result<Plan, String> {
    let (mut rounds, mut retries, mut metrics, mut reference) = (DEFAULT_ROUNDS, DEFAULT_RETRIES, None::<Vec<String>>, None::<String>);
    let (mut configs, mut flights) = (Vec::<Config>::new(), Vec::<Flight>::new());
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let at = |e: String| format!("plan line {}: {e}", n + 1);
        let mut words = line.split_whitespace();
        let key = words.next().unwrap_or("");
        let rest: Vec<String> = words.map(str::to_string).collect();
        let count = |what: &str| -> Result<usize, String> {
            match rest.as_slice() {
                [v] => v.parse::<usize>().map_err(|_| at(format!("{what} must be a count, got '{v}'"))),
                _ => Err(at(format!("{what} takes one count"))),
            }
        };
        match key {
            "rounds" => rounds = count("rounds")?,
            "retries" => retries = count("retries")?,
            "metrics" => metrics = Some(rest.join(",").split(',').filter(|m| !m.is_empty()).map(str::to_string).collect()),
            "reference" => reference = Some(rest.first().ok_or_else(|| at("reference needs a configuration name".into()))?.clone()),
            "config" => {
                let (name, env) = rest.split_first().ok_or_else(|| at("config needs a name".into()))?;
                let env = env
                    .iter()
                    .map(|kv| {
                        kv.split_once('=')
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .ok_or_else(|| at(format!("'{kv}' is not KEY=VALUE")))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if configs.iter().any(|c| c.name == *name) {
                    return Err(at(format!("configuration '{name}' twice")));
                }
                configs.push(Config { name: name.clone(), env });
            }
            "flight" => {
                let (name, args) = rest.split_first().ok_or_else(|| at("flight needs a name and a perf command".into()))?;
                let mut command = Vec::new();
                let mut log = None;
                let mut it = args.iter();
                while let Some(a) = it.next() {
                    if a == "--log" {
                        log = Some(it.next().ok_or_else(|| at("--log needs a file name".into()))?.clone());
                    } else {
                        command.push(a.clone());
                    }
                }
                if command.is_empty() {
                    return Err(at(format!("flight '{name}' has no perf command")));
                }
                let log = log
                    .or_else(|| ab::default_log(&command))
                    .ok_or_else(|| at(format!("flight '{name}': give --log FILE (stream scenes are known)")))?;
                flights.push(Flight { name: name.clone(), command, log });
            }
            other => return Err(at(format!("unknown statement '{other}' (rounds, retries, metrics, reference, config, flight)"))),
        }
    }
    if rounds == 0 {
        return Err("rounds must be at least 1".into());
    }
    if configs.is_empty() {
        configs.push(Config { name: "default".into(), env: Vec::new() });
    }
    if flights.is_empty() {
        return Err("the plan has no flight".into());
    }
    let reference = match reference {
        Some(r) => configs.iter().position(|c| c.name == r).ok_or_else(|| format!("reference '{r}' is not a configuration"))?,
        None => 0,
    };
    let metrics = metrics.unwrap_or_else(|| DEFAULT_METRICS.iter().map(ToString::to_string).collect());
    if metrics.is_empty() {
        return Err("metrics names no metric".into());
    }
    if let Some(bad) = metrics.iter().find(|m| !METRICS.iter().any(|i| i.name == m.as_str())) {
        return Err(format!("unknown metric '{bad}' (known: {})", METRICS.map(|m| m.name).join(", ")));
    }
    Ok(Plan { rounds, retries, reference, configs, flights, metrics })
}

/// The configurations of round `round` in run order: rotated by one each round, so heat and drift spread over them.
pub fn rotation(round: usize, n: usize) -> Vec<usize> {
    (0..n).map(|k| (k + round) % n.max(1)).collect()
}

/// What the stop rule says about a configuration against the reference in one flight.
#[derive(Clone, Debug, PartialEq)]
pub enum Settle {
    /// Not yet settled: run the next round.
    Open,
    /// Worse than the reference by more than [`STOP_FAR_MULTIPLE`] times the margin on a metric (named, with its change).
    FarWorse(String),
    /// Within the margin on every metric in [`STOP_WITHIN_ROUNDS`] valid rounds.
    Within,
}

/// The stop rule (ADR 0087 point 7): `reference` and `config` are the metric values of their valid rounds (one vector per round, in
/// the plan's metric order). Lower is better for every metric.
pub fn settle(metrics: &[String], reference: &[Vec<Option<f64>>], config: &[Vec<Option<f64>>]) -> Settle {
    if reference.is_empty() || config.is_empty() {
        return Settle::Open;
    }
    let median_of =
        |runs: &[Vec<Option<f64>>], m: usize| ab::median(&runs.iter().filter_map(|r| r.get(m).copied().flatten()).collect::<Vec<_>>());
    let mut all_within = config.len() >= STOP_WITHIN_ROUNDS && reference.len() >= STOP_WITHIN_ROUNDS;
    for (m, name) in metrics.iter().enumerate() {
        let (r, c) = (median_of(reference, m), median_of(config, m));
        let Some(rel) = ab::change(r, c).0 else {
            all_within = false;
            continue;
        };
        if rel > STOP_FAR_MULTIPLE * AB_MARGIN {
            return Settle::FarWorse(format!("{name} {:+.1} %", rel * 100.0));
        }
        if rel.abs() > AB_MARGIN {
            all_within = false;
        }
    }
    if all_within {
        Settle::Within
    } else {
        Settle::Open
    }
}

/// How one attempt of a run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum AttemptOutcome {
    /// A valid log: the plan's metric values.
    Valid(Vec<Option<f64>>),
    /// A log whose frame verdicts do not count, with the reasons.
    Invalid(String),
    /// The readiness gate refused, with its conditions.
    Refused(String),
}

/// One attempt, timed (ADR 0087 point 7: start, end and active time per scene).
#[derive(Clone, Debug, PartialEq)]
pub struct Attempt {
    pub flight: String,
    pub config: String,
    pub round: usize,
    pub attempt: usize,
    pub outcome: AttemptOutcome,
    /// Seconds since the Unix epoch.
    pub start_s: f64,
    pub end_s: f64,
    /// The duty idle kept before it (BUILD-035).
    pub idle_before_s: f64,
}

/// One child run: what it wrote and how long it took.
pub struct ChildRun {
    pub outcome: Outcome,
    pub start_s: f64,
    pub end_s: f64,
}

/// Starts the children (`cargo xtask perf ...` with the configuration's environment in `xtask`, fakes in the tests).
pub trait WindowRunner {
    fn run(&mut self, flight: &Flight, config: &Config, tag: &str) -> ChildRun;
    /// Keep `seconds` of duty idle before the next run.
    fn rest(&mut self, seconds: f64);
    /// Wait before a refused run is tried again.
    fn wait_refused(&mut self);
}

/// The result of a window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub attempts: Vec<Attempt>,
    /// Per flight and configuration (not the reference): the stop rule's verdict and the round it settled after.
    pub settled: Vec<(String, String, Settle, Option<usize>)>,
    /// The run that failed and why, which stopped the window; the attempts before it are kept.
    pub failure: Option<String>,
}

/// Judge a child's log: valid with its metric values, or invalid with the reasons its frame verdicts do not count.
pub fn judge_log(log: &str, metrics: &[String]) -> AttemptOutcome {
    let Ok(root) = crate::compare::parse(log.as_bytes()) else { return AttemptOutcome::Invalid("the log is not JSON".into()) };
    if crate::validity::log_frame_valid(&root) {
        AttemptOutcome::Valid(metrics.iter().map(|m| ab::metric_value(log, m)).collect())
    } else {
        AttemptOutcome::Invalid(crate::validity::log_frame_void_reasons(&root).join("; "))
    }
}

/// Run the plan: rounds of rotated configurations per flight, each run retried while invalid or refused up to the plan's retries,
/// the duty idle between two runs, and no further rounds for a configuration the stop rule settled. A child that fails for another
/// reason stops the window: the report keeps the attempts before it and names the failure.
pub fn execute(plan: &Plan, runner: &mut dyn WindowRunner) -> Report {
    let mut report = Report::default();
    let nf = plan.flights.len();
    let nc = plan.configs.len();
    // valid[f][c] = the metric vectors of the valid rounds.
    let mut valid: Vec<Vec<Vec<Vec<Option<f64>>>>> = vec![vec![Vec::new(); nc]; nf];
    let mut settled: Vec<Vec<Option<(Settle, usize)>>> = vec![vec![None; nc]; nf];
    let mut last_run_s: Option<f64> = None;
    // A retry after a refusal waits for the gate instead of the duty idle (BUILD-035): the refused run did not load the machine.
    let mut after_refusal = false;
    'rounds: for round in 0..plan.rounds {
        for f in 0..nf {
            let open = |c: usize, settled: &Vec<Vec<Option<(Settle, usize)>>>| settled[f][c].is_none();
            // The reference runs while any other configuration of the flight is open (and alone, with one configuration).
            let any_open = (0..nc).any(|c| c != plan.reference && open(c, &settled)) || nc == 1;
            for c in rotation(round, nc) {
                let wanted = if c == plan.reference { any_open } else { open(c, &settled) };
                if !wanted {
                    continue;
                }
                let (flight, config) = (&plan.flights[f], &plan.configs[c]);
                for attempt in 0..=plan.retries {
                    let idle = if after_refusal { 0.0 } else { crate::clock::duty_idle_s(last_run_s) };
                    if idle > 0.0 {
                        runner.rest(idle);
                    }
                    let tag = format!("r{}-{}-{}-t{}", round + 1, flight.name, config.name, attempt + 1);
                    let child = runner.run(flight, config, &tag);
                    last_run_s = Some(child.end_s - child.start_s);
                    let outcome = match child.outcome {
                        Outcome::Log(log) => judge_log(&log, &plan.metrics),
                        Outcome::Refused(conditions) => AttemptOutcome::Refused(conditions.join("; ")),
                        Outcome::Failed(why) => {
                            report.failure = Some(format!("{tag}: the run failed: {why}"));
                            break 'rounds;
                        }
                    };
                    let done = matches!(outcome, AttemptOutcome::Valid(_));
                    if let AttemptOutcome::Valid(values) = &outcome {
                        valid[f][c].push(values.clone());
                    }
                    let refused = matches!(outcome, AttemptOutcome::Refused(_));
                    after_refusal = refused;
                    report.attempts.push(Attempt {
                        flight: flight.name.clone(),
                        config: config.name.clone(),
                        round,
                        attempt,
                        outcome,
                        start_s: child.start_s,
                        end_s: child.end_s,
                        idle_before_s: idle,
                    });
                    if done {
                        break;
                    }
                    if refused && attempt < plan.retries {
                        runner.wait_refused();
                    }
                }
            }
            let still_open: Vec<usize> = (0..nc).filter(|&c| c != plan.reference && settled[f][c].is_none()).collect();
            for c in still_open {
                match settle(&plan.metrics, &valid[f][plan.reference], &valid[f][c]) {
                    Settle::Open => {}
                    s => settled[f][c] = Some((s, round)),
                }
            }
        }
    }
    for (f, flight) in plan.flights.iter().enumerate() {
        for (c, config) in plan.configs.iter().enumerate().filter(|(c, _)| *c != plan.reference) {
            let (s, at) = match &settled[f][c] {
                Some((s, r)) => (s.clone(), Some(*r)),
                None => (Settle::Open, None),
            };
            report.settled.push((flight.name.clone(), config.name.clone(), s, at));
        }
    }
    report
}

/// The window's table: every attempt with its times, then per flight and configuration the medians against the reference, the
/// verdict by the margin, and whether the stop rule settled it early; the wall time of the window and the minutes per valid run.
pub fn table(plan: &Plan, report: &Report) -> Vec<String> {
    let mut out = vec![
        format!(
            "perf window: {} configuration(s) x {} flight(s), up to {} round(s), reference '{}', margin {:.0} %",
            plan.configs.len(),
            plan.flights.len(),
            plan.rounds,
            plan.configs[plan.reference].name,
            AB_MARGIN * 100.0
        ),
        String::new(),
        "| Flight | Config | Round | Try | Outcome | Start (s) | Wall (s) | Idle before (s) |".into(),
        "|---|---|---|---|---|---|---|---|".into(),
    ];
    let t0 = report.attempts.first().map_or(0.0, |a| a.start_s);
    for a in &report.attempts {
        let what = match &a.outcome {
            AttemptOutcome::Valid(_) => "valid".to_string(),
            AttemptOutcome::Invalid(r) => format!("INVALID: {r}"),
            AttemptOutcome::Refused(r) => format!("refused: {r}"),
        };
        out.push(format!(
            "| {} | {} | {} | {} | {what} | {:.0} | {:.0} | {:.0} |",
            a.flight,
            a.config,
            a.round + 1,
            a.attempt + 1,
            a.start_s - t0,
            a.end_s - a.start_s,
            a.idle_before_s
        ));
    }
    out.push(String::new());
    let values = |f: &str, c: &str, m: usize| -> Vec<f64> {
        report
            .attempts
            .iter()
            .filter(|a| a.flight == f && a.config == c)
            .filter_map(|a| match &a.outcome {
                AttemptOutcome::Valid(v) => v.get(m).copied().flatten(),
                _ => None,
            })
            .collect()
    };
    out.push("| Flight | Config | Metric | Reference median | Median | Change | Verdict | Stop rule |".into());
    out.push("|---|---|---|---|---|---|---|---|".into());
    let reference = &plan.configs[plan.reference].name;
    for (flight, config, settle, at) in &report.settled {
        for (m, metric) in plan.metrics.iter().enumerate() {
            let (r, c) = (ab::median(&values(flight, reference, m)), ab::median(&values(flight, config, m)));
            let (rel, verdict) = ab::change(r, c);
            let stop = match (settle, at) {
                (Settle::FarWorse(w), Some(k)) => format!("settled worse after round {} ({w})", k + 1),
                (Settle::Within, Some(k)) => format!("settled within after round {}", k + 1),
                _ => "all rounds".to_string(),
            };
            let num = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{v:.2}"));
            out.push(format!(
                "| {flight} | {config} | {metric} | {} | {} | {} | {} | {stop} |",
                num(r),
                num(c),
                rel.map_or("-".to_string(), |v| format!("{:+.1} %", v * 100.0)),
                verdict.word()
            ));
        }
    }
    let wall = report.attempts.last().map_or(0.0, |a| a.end_s) - t0;
    let valid = report.attempts.iter().filter(|a| matches!(a.outcome, AttemptOutcome::Valid(_))).count();
    out.push(String::new());
    if let Some(f) = &report.failure {
        out.push(format!("STOPPED: {f}"));
    }
    out.push(format!(
        "window: {} attempt(s), {valid} valid, {:.1} min wall, {} min per valid run",
        report.attempts.len(),
        wall / 60.0,
        if valid > 0 { format!("{:.1}", wall / 60.0 / valid as f64) } else { "-".to_string() }
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = "# SPIKE-90 window 2, shortened\nrounds 3\nretries 1\nreference c\nconfig c PLANET_DUAL_POOL=1\nconfig d PLANET_DUAL_POOL=1 PLANET_DUAL_POOL_LOAD=8\nconfig e PLANET_DUAL_POOL=1 PLANET_DUAL_POOL_BG=2\nflight dive stream dive --store C:/store --gen-workers 5  # the 40 s flight\n";

    // spec: BUILD-034
    #[test]
    fn a_plan_names_rounds_configurations_with_their_environment_and_flights_with_their_logs() {
        let p = parse_plan(PLAN).unwrap();
        assert_eq!((p.rounds, p.retries, p.reference), (3, 1, 0));
        assert_eq!(p.configs[1].env, vec![("PLANET_DUAL_POOL".into(), "1".into()), ("PLANET_DUAL_POOL_LOAD".into(), "8".into())]);
        assert_eq!(p.flights[0].command, ["stream", "dive", "--store", "C:/store", "--gen-workers", "5"]);
        assert_eq!(p.flights[0].log, "dive.json");
        assert_eq!(p.metrics, DEFAULT_METRICS.map(String::from));
        let lit = parse_plan("flight t stream teleport-1m --lit\n").unwrap();
        assert_eq!((lit.flights[0].log.as_str(), lit.configs[0].name.as_str()), ("teleport-1m-lit.json", "default"));
        for (bad, says) in [
            ("rounds 0\nflight a stream dive", "at least 1"),
            ("flight a lit", "--log FILE"),
            ("config c X\nflight a stream dive", "KEY=VALUE"),
            ("reference z\nflight a stream dive", "not a configuration"),
            ("metrics nope\nflight a stream dive", "unknown metric"),
            ("frobnicate 3", "unknown statement"),
            ("rounds 3", "no flight"),
            (
                "metrics
flight a stream dive",
                "no metric",
            ),
        ] {
            let e = parse_plan(bad).unwrap_err();
            assert!(e.contains(says), "{bad}: {e}");
        }
        assert_eq!(rotation(0, 3), [0, 1, 2]);
        assert_eq!(rotation(1, 3), [1, 2, 0]);
        assert_eq!(rotation(2, 3), [2, 0, 1]);
    }

    fn v(tile: f64, p99: f64) -> Vec<Option<f64>> {
        vec![Some(tile), Some(p99)]
    }

    // spec: BUILD-034
    #[test]
    fn the_stop_rule_settles_a_far_miss_after_one_round_and_a_match_after_two_and_leaves_the_rest_open() {
        let m: Vec<String> = DEFAULT_METRICS.map(String::from).to_vec();
        // Window 2: every knob raised dive's p99 by 39-65 % against c: settled worse after one valid round.
        assert_eq!(settle(&m, &[v(4.24, 8.79)], &[v(4.41, 14.23)]), Settle::FarWorse("p99_ms +61.9 %".into()));
        // 15 % is the threshold itself: still open.
        assert_eq!(settle(&m, &[v(100.0, 10.0)], &[v(115.0, 10.0)]), Settle::Open);
        // Within the margin: open after one round, settled after two.
        assert_eq!(settle(&m, &[v(100.0, 10.0)], &[v(103.0, 10.2)]), Settle::Open);
        assert_eq!(settle(&m, &[v(100.0, 10.0), v(101.0, 10.0)], &[v(103.0, 10.2), v(99.0, 10.1)]), Settle::Within);
        // Between the margin and three times it: open (every round runs).
        assert_eq!(settle(&m, &[v(100.0, 10.0), v(100.0, 10.0)], &[v(110.0, 10.0), v(110.0, 10.0)]), Settle::Open);
        // Better by more than the margin is not "within": open.
        assert_eq!(settle(&m, &[v(100.0, 10.0), v(100.0, 10.0)], &[v(80.0, 10.0), v(80.0, 10.0)]), Settle::Open);
        // No valid round of either side: open.
        assert_eq!(settle(&m, &[], &[v(1.0, 1.0)]), Settle::Open);
        assert_eq!((STOP_FAR_MULTIPLE, STOP_WITHIN_ROUNDS), (3.0, 2));
    }

    /// A log with one run whose tile-ready and p99 are given, valid or voided by a calibration reason.
    fn log(tile: f64, p99: f64, valid: bool) -> String {
        let w = if valid {
            String::new()
        } else {
            "\"no verdict: CPU calibration of 'stream/dive' outside the limits (ADR 0065): before single 53,000 us over 52,000 us\"".into()
        };
        format!(
            "{{\"valid_for_budgets\":{valid},\"frame_valid\":{valid},\"gpu_session_valid\":true,\"warnings\":[{w}],\"runs\":[{{\"path\":\"stream/dive\",\"median_ms\":5.0,\"p99_ms\":{p99},\"counts\":{{\"load_run_us_median\":{tile}}}}}]}}"
        )
    }

    /// Fake children: config c reads (4.0, 9.0), d reads (5.0, 14.0) (far worse), e reads (4.1, 9.1) (within); `invalid_first`
    /// voids the first attempt of each listed config, `refuse` refuses the first run of all; each run takes 40 s.
    struct Fake {
        clock: f64,
        calls: Vec<String>,
        rests: Vec<f64>,
        waits: usize,
        invalid_first: Vec<&'static str>,
        refuse: bool,
        seen: Vec<String>,
    }

    impl WindowRunner for Fake {
        fn run(&mut self, _flight: &Flight, config: &Config, tag: &str) -> ChildRun {
            self.calls.push(tag.to_string());
            let start_s = self.clock;
            self.clock += 40.0;
            let outcome = if self.refuse && self.calls.len() == 1 {
                Outcome::Refused(vec!["quiet machine: cargo.exe (pid 7)".into()])
            } else {
                let first = !self.seen.contains(&config.name);
                self.seen.push(config.name.clone());
                let (t, p) = match config.name.as_str() {
                    "c" => (4.0, 9.0),
                    "d" => (5.0, 14.0),
                    _ => (4.1, 9.1),
                };
                Outcome::Log(log(t, p, !(first && self.invalid_first.contains(&config.name.as_str()))))
            };
            ChildRun { outcome, start_s, end_s: self.clock }
        }
        fn rest(&mut self, seconds: f64) {
            self.rests.push(seconds);
            self.clock += seconds;
        }
        fn wait_refused(&mut self) {
            self.waits += 1;
            self.clock += 30.0;
        }
    }

    fn fake() -> Fake {
        Fake { clock: 1_000.0, calls: vec![], rests: vec![], waits: 0, invalid_first: vec![], refuse: false, seen: vec![] }
    }

    // spec: BUILD-034, BUILD-035
    #[test]
    fn a_window_rotates_rests_between_runs_stops_settled_configurations_and_times_every_attempt() {
        let plan = parse_plan(PLAN).unwrap();
        let mut f = fake();
        let r = execute(&plan, &mut f);
        assert_eq!(r.failure, None);
        // Round 1 runs c, d, e; d settles worse at once, e is open after one round. Round 2 runs e and c (rotated); e settles
        // within. Round 3 runs nothing: every configuration is settled, so the reference stops too.
        assert_eq!(f.calls, ["r1-dive-c-t1", "r1-dive-d-t1", "r1-dive-e-t1", "r2-dive-e-t1", "r2-dive-c-t1"]);
        assert_eq!(f.rests, vec![20.0; 4], "half of each 40 s run, never before the first");
        assert_eq!(r.settled[0], ("dive".into(), "d".into(), Settle::FarWorse("load_run_us_median +25.0 %".into()), Some(0)));
        assert_eq!(r.settled[1], ("dive".into(), "e".into(), Settle::Within, Some(1)));
        assert_eq!(r.attempts[1].start_s, 1_060.0, "start, end and the idle before are recorded");
        assert_eq!((r.attempts[1].end_s, r.attempts[1].idle_before_s), (1_100.0, 20.0));
        let t = table(&plan, &r);
        assert!(
            t.iter()
                .any(|l| l.starts_with("| dive | d | p99_ms |") && l.contains("settled worse after round 1 (load_run_us_median +25.0 %)")),
            "{t:?}"
        );
        assert!(
            t.iter().any(|l| l.starts_with("| dive | e | load_run_us_median |") && l.contains("settled within after round 2")),
            "{t:?}"
        );
        assert!(t.last().unwrap().starts_with("window: 5 attempt(s), 5 valid, 4.7 min wall, 0.9 min per valid run"), "{t:?}");
    }

    // spec: BUILD-034
    #[test]
    fn an_invalid_or_refused_run_is_retried_within_the_plan_and_a_failed_child_stops_the_window() {
        let plan = parse_plan(PLAN).unwrap();
        let mut f = Fake { invalid_first: vec!["c"], refuse: true, ..fake() };
        let r = execute(&plan, &mut f);
        // The first run is refused (a wait, then the retry), and that retry of c is invalid: retries 1 is used up, so round 1 has
        // no valid c and nothing settles in round 1.
        assert_eq!(&f.calls[..3], ["r1-dive-c-t1", "r1-dive-c-t2", "r1-dive-d-t1"]);
        assert_eq!(f.waits, 1);
        assert_eq!(f.rests.first(), Some(&20.0), "no duty idle before the refused run's retry; the next run after the invalid one rests");
        assert_eq!(r.attempts[1].idle_before_s, 0.0, "the retry after the refusal waited for the gate, not the duty idle");
        assert!(matches!(&r.attempts[0].outcome, AttemptOutcome::Refused(w) if w.contains("cargo.exe")));
        assert!(matches!(&r.attempts[1].outcome, AttemptOutcome::Invalid(w) if w.contains("CPU calibration")));
        assert!(r.settled.iter().all(|s| s.3 != Some(0)), "no valid reference in round 1: nothing settles: {:?}", r.settled);
        struct Broken;
        impl WindowRunner for Broken {
            fn run(&mut self, _: &Flight, _: &Config, _: &str) -> ChildRun {
                ChildRun { outcome: Outcome::Failed("exit 101".into()), start_s: 0.0, end_s: 1.0 }
            }
            fn rest(&mut self, _: f64) {}
            fn wait_refused(&mut self) {}
        }
        let r = execute(&plan, &mut Broken);
        assert!(r.failure.as_deref().is_some_and(|f| f.contains("r1-dive-c-t1: the run failed: exit 101")), "{:?}", r.failure);
        assert!(table(&plan, &r).iter().any(|l| l.starts_with("STOPPED: r1-dive-c-t1")));
        // A failure after valid runs keeps them.
        struct FailSecond(usize);
        impl WindowRunner for FailSecond {
            fn run(&mut self, _: &Flight, _: &Config, _: &str) -> ChildRun {
                self.0 += 1;
                let outcome = if self.0 == 1 { Outcome::Log(log(4.0, 9.0, true)) } else { Outcome::Failed("exit 101".into()) };
                ChildRun { outcome, start_s: 0.0, end_s: 1.0 }
            }
            fn rest(&mut self, _: f64) {}
            fn wait_refused(&mut self) {}
        }
        let r = execute(&plan, &mut FailSecond(0));
        assert!(r.attempts.len() == 1 && r.failure.is_some(), "{r:?}");
    }
}
