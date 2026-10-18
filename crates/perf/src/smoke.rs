//! The streaming smoke checks of tier B (TEST-035): the log of a short `test-render --perf-smoke` dive on a small in-memory world,
//! read for order-of-magnitude findings. Not a performance verdict (the adapter is the software one and the world a toy): it
//! catches a configuration cap, the M4 kind of mistake (a worker count or an `ahead` limit that leaves most of the streamer idle, a
//! pool a tenth of its size, a scheduler or upload step ten times its cost), without a perf window. Pure over the log text.
//!
//! The thresholds are about a tenth of the first measurements on the software adapter (WARP, 2026-10-17, dive, 48 frames, 6
//! workers, a 256-cell in-memory bake; three runs): at most 6 and a median of 4 jobs running at once,
//! 231,090 slots in the pool, medians of 0.044 ms (scheduler) and 0.16 ms (upload). The concurrency is judged against the workers
//! the run was asked for, not against a constant. The number of tile jobs and of resident slots is NOT judged against a tenth of the
//! first run (187 jobs, 189 slots): the frame loop of a software renderer paces the dive, and the first CI run (a 4-core runner)
//! finished 5 jobs and 11 slots in the same 48 frames. Those two only show that the streamer works at all.

use crate::accept::count;
use crate::compare::{parse, Json};

/// Fewest tile jobs a 48-frame dive must complete (187 on the development machine, 5 on a 4-core CI runner): the streamer is alive.
pub const MIN_JOBS: u64 = 3;
/// Fewest resident slots at the peak (189 and 11): tiles reached the pool.
pub const MIN_RESIDENT_SLOTS: u64 = 3;
/// Fewest slots the pool offers (measured 231,090 for the tier's share).
pub const MIN_POOL_SLOTS: u64 = 23_109;
/// Most the median of the scheduler step may cost, ms (measured 0.044).
pub const MAX_SCHEDULER_MEDIAN_MS: f64 = 0.5;
/// Most the median of the upload step may cost, ms (measured 0.16).
pub const MAX_UPLOAD_MEDIAN_MS: f64 = 1.6;

/// The fewest jobs that must have run at once, at the peak, for a streamer asked to run `workers` workers: a third of them, and at
/// least two. One worker running alone is a configuration cap, whatever the machine.
pub fn min_concurrent_jobs(workers: usize) -> u64 {
    (workers as u64 / 3).max(2)
}

/// Whether the streamer of a smoke log has shown it is alive: at least [`MIN_JOBS`] tile jobs finished and [`MIN_RESIDENT_SLOTS`] slots
/// were resident at the peak. The stop rule of the tier-B smoke: the dive is run until this holds (with a wall-time cap whose expiry is
/// the failure), so that the check judges liveness, not how many frames a loaded runner paced in a fixed number. An unreadable or
/// incomplete log is not alive.
pub fn liveness_met(log: &str) -> bool {
    let Ok(root) = parse(log.as_bytes()) else { return false };
    let counts = match root.get("runs") {
        Some(Json::Arr(a)) => a.first().and_then(|r| r.get("counts")),
        _ => None,
    };
    counts.is_some_and(|c| {
        count(c, "load_jobs").is_some_and(|v| v >= MIN_JOBS) && count(c, "max_resident_slots").is_some_and(|v| v >= MIN_RESIDENT_SLOTS)
    })
}

/// The findings of a smoke log of a streaming scene run with `workers` generation workers; empty means every check holds. Each
/// finding names the measure, the value and the limit.
pub fn streaming_smoke_findings(log: &str, workers: usize) -> Vec<String> {
    let root = match parse(log.as_bytes()) {
        Ok(r) => r,
        Err(e) => return vec![format!("the log is not readable JSON: {e}")],
    };
    let Some(run) = (match root.get("runs") {
        Some(Json::Arr(a)) => a.first(),
        _ => None,
    }) else {
        return vec!["the log has no run".to_string()];
    };
    let (Some(counts), Some(metrics)) = (run.get("counts"), run.get("metrics")) else {
        return vec!["the run has no counts or metrics".to_string()];
    };
    let mut out = Vec::new();
    let need = |name: &str, min: u64, why: &str, out: &mut Vec<String>| match count(counts, name) {
        None => out.push(format!("`{name}` is missing from the log")),
        Some(v) if v < min => out.push(format!("{why}: `{name}` is {v}, at least {min} expected")),
        Some(_) => {}
    };
    let want = min_concurrent_jobs(workers);
    need(
        "load_running_max",
        want,
        &format!("the streamer ran fewer jobs at once than {workers} workers allow (a worker or ahead cap?)"),
        &mut out,
    );
    need("load_jobs", MIN_JOBS, "the streamer finished almost no tile jobs", &mut out);
    need("max_resident_slots", MIN_RESIDENT_SLOTS, "too few slots became resident", &mut out);
    need("pool_slots", MIN_POOL_SLOTS, "the pool is a tenth of its size or less (a pool cap?)", &mut out);
    for (name, max) in [("scheduler_ms", MAX_SCHEDULER_MEDIAN_MS), ("upload_ms", MAX_UPLOAD_MEDIAN_MS)] {
        match metrics.get(name).and_then(|m| m.get("median")).and_then(Json::num) {
            None => out.push(format!("`{name}` is missing from the log")),
            Some(v) if v > max => {
                out.push(format!("the {name} median is {v:.3} ms, at most {max} ms expected (ten times the first measurement)"))
            }
            Some(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log with the counters and medians of the first measurement, one value replaced by `(counter, value)`.
    fn log(running_max: u64, jobs: u64, resident: u64, pool: u64, sched: f64, upload: f64) -> String {
        format!(
            "{{\"runs\":[{{\"counts\":{{\"load_running_max\":{running_max},\"load_jobs\":{jobs},\"max_resident_slots\":{resident},\"pool_slots\":{pool}}},\"metrics\":{{\"scheduler_ms\":{{\"median\":{sched}}},\"upload_ms\":{{\"median\":{upload}}}}}}}]}}"
        )
    }

    // spec: TEST-035
    #[test]
    fn the_first_measurement_passes_and_each_configuration_cap_is_found_by_name() {
        let ok = log(6, 187, 189, 231_090, 0.044, 0.16);
        assert!(streaming_smoke_findings(&ok, 6).is_empty());
        // A worker cap to 1: the run reaches one job at a time (the real run with `--gen-workers 1` reads 1).
        let f = streaming_smoke_findings(&log(1, 58, 63, 231_090, 0.033, 0.14), 6);
        assert!(f.len() == 1 && f[0].contains("load_running_max") && f[0].contains("6 workers"), "{f:?}");
        // A pool of a tenth of its size, too few jobs, too few resident slots, a scheduler and an upload step ten times dearer.
        let f = streaming_smoke_findings(&log(6, 1, 1, 20_000, 0.6, 2.0), 6);
        for name in ["load_jobs", "max_resident_slots", "pool_slots", "scheduler_ms", "upload_ms"] {
            assert!(f.iter().any(|l| l.contains(name)), "{name}: {f:?}");
        }
        // The limits are inclusive: exactly at them passes.
        let edge = log(2, MIN_JOBS, MIN_RESIDENT_SLOTS, MIN_POOL_SLOTS, MAX_SCHEDULER_MEDIAN_MS, MAX_UPLOAD_MEDIAN_MS);
        assert!(streaming_smoke_findings(&edge, 6).is_empty(), "{:?}", streaming_smoke_findings(&edge, 6));
    }

    // spec: TEST-035
    #[test]
    fn the_stop_rule_is_liveness_at_the_limits_and_an_unreadable_log_is_not_alive() {
        assert!(liveness_met(&log(6, 187, 189, 231_090, 0.044, 0.16)));
        // Exactly the limits are alive; one short of either is not (the runner that finished 2 jobs in 48 frames).
        assert!(liveness_met(&log(1, MIN_JOBS, MIN_RESIDENT_SLOTS, 0, 9.0, 9.0)), "inclusive");
        assert!(!liveness_met(&log(6, MIN_JOBS - 1, 189, 231_090, 0.044, 0.16)));
        assert!(!liveness_met(&log(6, 187, MIN_RESIDENT_SLOTS - 1, 231_090, 0.044, 0.16)));
        // Only liveness stops the dive: a capped streamer (one worker, pool or timing findings) is alive and its findings stay.
        assert!(liveness_met(&log(1, 58, 63, 20_000, 0.6, 2.0)));
        assert!(!streaming_smoke_findings(&log(1, 58, 63, 231_090, 0.033, 0.14), 6).is_empty());
        // No log, no run, no counts, missing counters: not alive.
        assert!(!liveness_met("not json") && !liveness_met("{\"runs\":[]}") && !liveness_met("{\"runs\":[{\"counts\":{}}]}"));
        assert!(!liveness_met("{\"runs\":[{\"counts\":{\"load_jobs\":9}}]}"), "both counters are needed");
    }

    // spec: TEST-035
    #[test]
    fn the_concurrency_asked_for_follows_the_workers_and_an_unreadable_log_is_a_finding() {
        assert_eq!([1, 2, 3, 6, 18].map(min_concurrent_jobs), [2, 2, 2, 2, 6]);
        // With 18 workers asked for, 6 at once is the least that passes, and 5 is a finding.
        assert!(!streaming_smoke_findings(&log(1, 187, 189, 231_090, 0.04, 0.15), 18).is_empty());
        assert!(streaming_smoke_findings(&log(5, 187, 189, 231_090, 0.04, 0.15), 18).iter().any(|l| l.contains("18 workers")));
        assert!(!streaming_smoke_findings("not json", 6).is_empty());
        assert!(streaming_smoke_findings("{\"runs\":[]}", 6)[0].contains("no run"));
        let missing = streaming_smoke_findings("{\"runs\":[{\"counts\":{},\"metrics\":{}}]}", 6);
        assert!(missing.len() == 6 && missing.iter().all(|l| l.contains("missing")), "{missing:?}");
    }
}
