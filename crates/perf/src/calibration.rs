//! The CPU calibration decides whether a perf scene counts (TEST-039, ADR 0065): a scene whose fixed-kernel times (STRM-022) lie outside the
//! limits below, before or after the case, gets no verdict, with the reading, the limit and the moment named. Logs without a calibration
//! are not judged. A pure function over the logged counts, used by the harness (the session verdict) and by `accept` (judging a log
//! again), so that they cannot disagree.
//!
//! The limits are the maximum of the quiet-machine readings (Rasierklinge, 18 threads on 20 logical CPUs, the native-UI coder
//! session, competing processes 0.8 to 3.8 % of the CPU) plus about 5 %, rounded:
//!
//! | reading (us)             | quiet machine (windows 7 and 11, three smoke dives) | slow planner terminal (2026-10-16) | limit   |
//! |--------------------------|------------------------------------------------------|------------------------------------|---------|
//! | before, single           | 46,170 to 49,913                                     | 56,696 to 78,076                   | 52,000  |
//! | before, parallel median  | 85,042 to 106,885                                    | 127,845 to 131,044                 | 112,000 |
//! | after, single            | 46,329 to 51,080 (59,335 with `git` at 17 %)         | 47,226 to 52,879                   | 56,000  |
//! | after, parallel median   | 90,357 to 103,874                                    | 128,346 to 137,871                 | 112,000 |
//!
//! The first limit set (single 50,000, parallel 100,000, before and after) would have voided about one clean scene in five: before
//! parallel spreads 26 % on a quiet machine, and after single drifts upward after a long scene (the CPU clock falls, the post-pass
//! still runs). The parallel limits hold for the reference thread count only ([`REFERENCE_THREADS`], the generation pool of the
//! 20-thread machine); a log with another count is judged on the single readings alone.

/// The thread count the parallel limits were measured with: 20 logical CPUs less the generation reserve of 2.
pub const REFERENCE_THREADS: u64 = 18;
/// `load_calib_before_single_us` at most this.
pub const BEFORE_SINGLE_MAX_US: u64 = 52_000;
/// `load_calib_before_parallel_median_us` at most this.
pub const BEFORE_PARALLEL_MEDIAN_MAX_US: u64 = 112_000;
/// `load_calib_after_single_us` at most this.
pub const AFTER_SINGLE_MAX_US: u64 = 56_000;
/// `load_calib_after_parallel_median_us` at most this.
pub const AFTER_PARALLEL_MEDIAN_MAX_US: u64 = 112_000;

/// The start of every reason this module words.
pub const REASON_PREFIX: &str = "no verdict: CPU calibration";

/// What the calibration of one scene says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Judgement {
    /// No calibration reading in the counts (a smoke run, a log from before the lit scenes recorded it): not judged, not invalid.
    NotJudged,
    /// Every reading present is within its limit.
    Within,
    /// The readings outside their limits, one text each: `<when> <kind> <reading> us over <limit> us`.
    Outside(Vec<String>),
}

fn with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Judge the calibration of the scene `path` from its counts; `count(name)` reads a count by its log name (`load_calib_before_single_us`,
/// `load_calib_after_parallel_median_us`, `load_calib_before_threads`, ...).
pub fn judge(count: &dyn Fn(&str) -> Option<u64>) -> Judgement {
    let mut seen = false;
    let mut over = Vec::new();
    for (when, single_max, parallel_max) in
        [("before", BEFORE_SINGLE_MAX_US, BEFORE_PARALLEL_MEDIAN_MAX_US), ("after", AFTER_SINGLE_MAX_US, AFTER_PARALLEL_MEDIAN_MAX_US)]
    {
        if let Some(v) = count(&format!("load_calib_{when}_single_us")) {
            seen = true;
            if v > single_max {
                over.push(format!("{when} single {} us over {} us", with_commas(v), with_commas(single_max)));
            }
        }
        // The parallel limits belong to the reference thread count: another count (or none logged) is not judged on them.
        if count(&format!("load_calib_{when}_threads")) == Some(REFERENCE_THREADS) {
            if let Some(v) = count(&format!("load_calib_{when}_parallel_median_us")) {
                seen = true;
                if v > parallel_max {
                    over.push(format!("{when} parallel median {} us over {} us", with_commas(v), with_commas(parallel_max)));
                }
            }
        }
    }
    match (seen, over.is_empty()) {
        (false, _) => Judgement::NotJudged,
        (true, true) => Judgement::Within,
        (true, false) => Judgement::Outside(over),
    }
}

/// The reason text of a scene outside the limits: the log's warning and the console's cause.
pub fn reason(path: &str, over: &[String]) -> String {
    format!("{REASON_PREFIX} of '{path}' outside the limits (ADR 0065): {}", over.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scene's counts: threads, then before and after (single, parallel median).
    fn counts(threads: u64, b: (u64, u64), a: (u64, u64)) -> impl Fn(&str) -> Option<u64> {
        move |k: &str| match k {
            "load_calib_before_threads" | "load_calib_after_threads" => Some(threads),
            "load_calib_before_single_us" => Some(b.0),
            "load_calib_before_parallel_median_us" => Some(b.1),
            "load_calib_after_single_us" => Some(a.0),
            "load_calib_after_parallel_median_us" => Some(a.1),
            _ => None,
        }
    }

    // spec: TEST-039
    #[test]
    fn a_reading_at_its_limit_counts_and_one_microsecond_over_does_not_for_each_of_the_four_readings() {
        let ok = (BEFORE_SINGLE_MAX_US, BEFORE_PARALLEL_MEDIAN_MAX_US);
        let ok_after = (AFTER_SINGLE_MAX_US, AFTER_PARALLEL_MEDIAN_MAX_US);
        assert_eq!(judge(&counts(18, ok, ok_after)), Judgement::Within);
        let cases = [
            (counts(18, (ok.0 + 1, ok.1), ok_after), "before single 52,001 us over 52,000 us"),
            (counts(18, (ok.0, ok.1 + 1), ok_after), "before parallel median 112,001 us over 112,000 us"),
            (counts(18, ok, (ok_after.0 + 1, ok_after.1)), "after single 56,001 us over 56,000 us"),
            (counts(18, ok, (ok_after.0, ok_after.1 + 1)), "after parallel median 112,001 us over 112,000 us"),
        ];
        for (c, text) in cases {
            assert_eq!(judge(&c), Judgement::Outside(vec![text.to_string()]));
        }
    }

    // spec: TEST-039
    #[test]
    fn the_quiet_machine_counts_and_the_slow_state_and_a_disturbed_scene_do_not() {
        // The extremes of windows 7 and 11 and the smoke dives (quiet): all within.
        for (b, a) in [
            ((46_170, 106_885), (48_446, 93_364)),
            ((49_913, 85_042), (51_080, 90_357)),
            ((46_984, 105_071), (46_918, 99_689)),
            ((46_501, 95_713), (53_343, 92_236)),
        ] {
            assert_eq!(judge(&counts(18, b, a)), Judgement::Within, "{b:?} {a:?}");
        }
        // The slow planner terminal (2026-10-16): the dive and look-2km.
        let slow = judge(&counts(18, (66_877, 130_386), (48_553, 133_755)));
        assert_eq!(
            slow,
            Judgement::Outside(vec![
                "before single 66,877 us over 52,000 us".to_string(),
                "before parallel median 130,386 us over 112,000 us".to_string(),
                "after parallel median 133,755 us over 112,000 us".to_string(),
            ])
        );
        // The smoke dive with `git` at up to 17 % of the CPU: its after single reading is out.
        let git = judge(&counts(18, (46_984, 105_071), (59_335, 91_121)));
        assert_eq!(git, Judgement::Outside(vec!["after single 59,335 us over 56,000 us".to_string()]));
        let text = reason("stream-lit/dive", &["after single 59,335 us over 56,000 us".to_string()]);
        assert_eq!(
            text,
            "no verdict: CPU calibration of 'stream-lit/dive' outside the limits (ADR 0065): after single 59,335 us over 56,000 us"
        );
        assert!(text.starts_with(REASON_PREFIX));
    }

    // spec: TEST-039
    #[test]
    fn a_log_without_a_calibration_is_not_judged_and_another_thread_count_is_judged_on_the_single_readings_only() {
        assert_eq!(judge(&|_| None), Judgement::NotJudged);
        // Only the before readings logged: they are judged, the after ones are absent.
        let only_before = |k: &str| match k {
            "load_calib_before_threads" => Some(18),
            "load_calib_before_single_us" => Some(60_000),
            "load_calib_before_parallel_median_us" => Some(100_000),
            _ => None,
        };
        assert_eq!(judge(&only_before), Judgement::Outside(vec!["before single 60,000 us over 52,000 us".to_string()]));
        // Eight threads: the parallel median is far from the reference's and is not judged; the single reading still is.
        assert_eq!(judge(&counts(8, (46_000, 300_000), (46_000, 300_000))), Judgement::Within);
        assert!(matches!(judge(&counts(8, (53_000, 1), (1, 1))), Judgement::Outside(v) if v.len() == 1));
        // No thread count logged: the same.
        let no_threads = |k: &str| (k == "load_calib_before_parallel_median_us").then_some(900_000);
        assert_eq!(judge(&no_threads), Judgement::NotJudged);
        assert_eq!(with_commas(0), "0");
        assert_eq!(with_commas(999), "999");
        assert_eq!(with_commas(1_000), "1,000");
        assert_eq!(with_commas(1_234_567), "1,234,567");
    }
}
