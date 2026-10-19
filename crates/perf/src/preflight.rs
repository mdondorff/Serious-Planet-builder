//! The perf preflight (TEST-040): a short check before a perf window that says whether the machine is fit, and why not.
//!
//! The decision is a pure function over measured facts: the readiness gate's failures (ADR 0020), the CPU calibration readings
//! (STRM-022) judged against the limits of ADR 0065 for a reading taken before a case, and the host facts the report records (AC
//! state, power plan, the top competing processes). The scene in the binary measures; this module decides and words.

use crate::calibration::{self, Judgement};
use crate::gate::GateFailure;
use crate::load::TopOverCase;
use crate::streaming::{calibration_counts, CpuCalibration};

/// Calibration rounds of one preflight.
pub const ROUNDS: usize = 3;
/// Seconds between two rounds.
pub const ROUND_SPACING_S: u64 = 20;

/// Everything the decision looks at.
#[derive(Clone, Debug)]
pub struct PreflightFacts {
    /// The readiness gate's failures (empty: ready).
    pub gate: Vec<GateFailure>,
    /// `None`: the AC state could not be read.
    pub ac_power: Option<bool>,
    /// The active power plan as the host names it (`None`: could not be read).
    pub power_plan: Option<String>,
    /// The calibration readings, one per round; empty: none could be measured.
    pub calibrations: Vec<CpuCalibration>,
    /// The top competing processes over the sampled period, largest mean share first.
    pub top: Vec<TopOverCase>,
}

/// The decision and the report that goes with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preflight {
    /// One line per cause; empty: the machine is fit.
    pub failures: Vec<String>,
    /// The report lines: the host facts, the readings with their limits, the verdict.
    pub report: Vec<String>,
}

impl Preflight {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

fn reading_line(i: usize, c: &CpuCalibration) -> String {
    format!(
        "round {}: single {} us (limit {}), parallel median {} us on {} threads (limit {}, judged at {} threads), parallel max {} us",
        i + 1,
        c.single_us,
        calibration::BEFORE_SINGLE_MAX_US,
        c.parallel_median_us,
        c.threads,
        calibration::BEFORE_PARALLEL_MEDIAN_MAX_US,
        calibration::REFERENCE_THREADS,
        c.parallel_max_us
    )
}

/// Decide: every gate failure, a missing calibration, and every round whose reading lies over a limit of ADR 0065 for a reading before
/// a case (the worst round decides, so one slow round refuses).
pub fn decide(f: &PreflightFacts) -> Preflight {
    let mut failures: Vec<String> = f.gate.iter().map(|g| format!("readiness gate, {g}")).collect();
    if f.calibrations.is_empty() {
        failures.push("CPU calibration could not be measured".to_string());
    }
    for (i, c) in f.calibrations.iter().enumerate() {
        let counts = calibration_counts("before", c);
        let count = |name: &str| counts.iter().find(|(k, _)| k == name).map(|(_, v)| *v);
        if let Judgement::Outside(over) = calibration::judge(&count) {
            failures.push(format!("CPU calibration round {}: {} (ADR 0065)", i + 1, over.join("; ")));
        }
    }

    let mut report = Vec::new();
    report.push(format!(
        "AC power: {}",
        match f.ac_power {
            Some(true) => "connected",
            Some(false) => "BATTERY",
            None => "could not be read",
        }
    ));
    report.push(format!("power plan: {}", f.power_plan.as_deref().map_or("could not be read", str::trim)));
    if f.top.is_empty() {
        report.push("top competing processes: not measured".to_string());
    } else {
        report.push("top competing processes (mean share of the machine over the preflight):".to_string());
        for t in &f.top {
            report.push(format!("  {} {:.1} % (max {:.1} %)", t.name, t.mean_pct, t.max_pct));
        }
    }
    for (i, c) in f.calibrations.iter().enumerate() {
        report.push(reading_line(i, c));
    }
    if failures.is_empty() {
        report.push("PREFLIGHT OK".to_string());
    } else {
        report.push("PREFLIGHT REFUSED: the window must not start".to_string());
        for l in &failures {
            report.push(format!("  - {l}"));
        }
    }
    Preflight { failures, report }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calib(single: u64, parallel: u64) -> CpuCalibration {
        CpuCalibration {
            threads: calibration::REFERENCE_THREADS as usize,
            single_us: single,
            parallel_median_us: parallel,
            parallel_max_us: parallel + 5_000,
        }
    }

    fn quiet() -> PreflightFacts {
        PreflightFacts {
            gate: Vec::new(),
            ac_power: Some(true),
            power_plan: Some("High performance".into()),
            calibrations: vec![calib(47_000, 90_000), calib(48_000, 100_000), calib(49_000, 95_000)],
            top: vec![TopOverCase { name: "chrome".into(), mean_pct: 1.2, max_pct: 3.0, seen: 10 }],
        }
    }

    // spec: TEST-040
    #[test]
    fn a_quiet_machine_passes_and_the_report_has_the_host_facts_the_top_processes_and_every_reading() {
        let p = decide(&quiet());
        assert!(p.passed(), "{:?}", p.failures);
        let text = p.report.join("\n");
        for needle in ["AC power: connected", "power plan: High performance", "chrome 1.2 %", "round 1:", "round 3:", "PREFLIGHT OK"] {
            assert!(text.contains(needle), "{needle} in\n{text}");
        }
    }

    // spec: TEST-040
    #[test]
    fn a_gate_failure_refuses_and_is_named_with_its_measured_value() {
        let mut f = quiet();
        f.gate.push(GateFailure { condition: "AC power connected", measured: "running on battery".into() });
        f.ac_power = Some(false);
        let p = decide(&f);
        assert!(!p.passed());
        assert!(p.failures.iter().any(|l| l.contains("AC power connected") && l.contains("running on battery")), "{:?}", p.failures);
        let text = p.report.join("\n");
        assert!(text.contains("BATTERY") && text.contains("PREFLIGHT REFUSED"), "{text}");
    }

    // spec: TEST-040
    #[test]
    fn no_calibration_refuses() {
        let mut f = quiet();
        f.calibrations.clear();
        let p = decide(&f);
        assert!(p.failures.iter().any(|l| l.contains("could not be measured")), "{:?}", p.failures);
    }

    // spec: TEST-040
    #[test]
    fn the_worst_of_the_three_readings_decides_and_names_the_round() {
        let mut f = quiet();
        f.calibrations[2] = calib(56_696, 100_000);
        let p = decide(&f);
        assert_eq!(p.failures.len(), 1, "{:?}", p.failures);
        assert!(p.failures[0].contains("round 3") && p.failures[0].contains("single 56,696 us over 52,000 us"), "{:?}", p.failures);
        f.calibrations[2] = calib(50_000, 127_845);
        let p = decide(&f);
        assert!(p.failures[0].contains("parallel median 127,845 us over 112,000 us"), "{:?}", p.failures);
    }

    // spec: TEST-040
    #[test]
    fn the_limits_are_those_of_a_reading_before_a_case_and_the_boundary_passes() {
        let mut f = quiet();
        f.calibrations = vec![calib(calibration::BEFORE_SINGLE_MAX_US, calibration::BEFORE_PARALLEL_MEDIAN_MAX_US)];
        assert!(decide(&f).passed());
        f.calibrations = vec![calib(calibration::BEFORE_SINGLE_MAX_US + 1, 90_000)];
        assert!(!decide(&f).passed());
    }

    // spec: TEST-040
    #[test]
    fn the_parallel_limit_holds_for_the_reference_thread_count_only() {
        let mut f = quiet();
        let mut other = calib(47_000, 400_000);
        other.threads = 4;
        f.calibrations = vec![other];
        assert!(decide(&f).passed(), "another thread count is judged on the single reading alone");
    }

    // spec: TEST-040
    #[test]
    fn unreadable_host_facts_are_reported_not_hidden() {
        let mut f = quiet();
        f.ac_power = None;
        f.power_plan = None;
        f.top.clear();
        let text = decide(&f).report.join("\n");
        assert!(text.contains("AC power: could not be read") && text.contains("power plan: could not be read"), "{text}");
        assert!(text.contains("top competing processes: not measured"), "{text}");
    }
}
