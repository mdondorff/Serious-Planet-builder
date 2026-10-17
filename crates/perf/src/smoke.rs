//! The streaming smoke checks of tier B (TEST-035): the log of a short `test-render --perf-smoke` dive on a small in-memory world,
//! read for order-of-magnitude findings. Not a performance verdict (the adapter is the software one and the world a toy): it
//! catches a configuration cap, the M4 kind of mistake (a worker count or an `ahead` limit that leaves most of the streamer idle, a
//! pool a tenth of its size, a scheduler or upload step ten times its cost), without a perf window. Pure over the log text.
//!
//! The thresholds are about a tenth of the first measurements on the software adapter (WARP, 2026-10-17, dive, 48 frames, 6
//! workers, a 256-cell in-memory bake; three runs): 187 tile jobs, at most 6 and a median of 4 running at once, 189 resident slots,
//! 231,090 slots in the pool, medians of 0.044 ms (scheduler) and 0.16 ms (upload). The concurrency is judged against the workers
//! the run was asked for, not against a constant.

use crate::accept::count;
use crate::compare::{parse, Json};

/// Fewest tile jobs a 48-frame dive must complete (measured 187).
pub const MIN_JOBS: u64 = 18;
/// Fewest resident slots at the peak (measured 189).
pub const MIN_RESIDENT_SLOTS: u64 = 18;
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
    need("load_jobs", MIN_JOBS, "the streamer finished too few tile jobs (a tenth of the first measurement)", &mut out);
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
        let f = streaming_smoke_findings(&log(6, 9, 5, 20_000, 0.6, 2.0), 6);
        for name in ["load_jobs", "max_resident_slots", "pool_slots", "scheduler_ms", "upload_ms"] {
            assert!(f.iter().any(|l| l.contains(name)), "{name}: {f:?}");
        }
        // The limits are inclusive: exactly at them passes.
        let edge = log(2, MIN_JOBS, MIN_RESIDENT_SLOTS, MIN_POOL_SLOTS, MAX_SCHEDULER_MEDIAN_MS, MAX_UPLOAD_MEDIAN_MS);
        assert!(streaming_smoke_findings(&edge, 6).is_empty(), "{:?}", streaming_smoke_findings(&edge, 6));
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
