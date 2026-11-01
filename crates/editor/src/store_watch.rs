//! The revisit trigger of the item store (ADR 0066 point 6, AUTH-006): more than 20,000 items in one body, or an item load over 2 s, decides
//! whether the loose-file backend stays. Nothing else counted or timed it, so nothing would have said when the trigger was reached.
//!
//! [`StoreWatch`] is plain data: the editor tells it the count of a body and the duration of a load, it answers with the warnings to log (once
//! per crossing) and the line the diagnostics show. The duration comes from a [`LoadTimer`], which a test replaces: no assertion depends on a
//! clock.

use planet_world_def::BodyId;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// More items than this in one body is the count half of the trigger.
pub const ITEM_LIMIT: usize = 20_000;
/// A load of one body slower than this is the time half of the trigger.
pub const LOAD_LIMIT: Duration = Duration::from_secs(2);

/// Times a load: `begin` before it, `end` after it.
pub trait LoadTimer {
    fn begin(&mut self);
    fn end(&mut self) -> Duration;
}

/// The wall clock.
pub struct WallTimer(Option<Instant>);

impl WallTimer {
    pub fn new() -> WallTimer {
        WallTimer(None)
    }
}

impl Default for WallTimer {
    fn default() -> Self {
        WallTimer::new()
    }
}

impl LoadTimer for WallTimer {
    fn begin(&mut self) {
        self.0 = Some(Instant::now());
    }
    fn end(&mut self) -> Duration {
        self.0.take().map_or(Duration::ZERO, |t| t.elapsed())
    }
}

/// A timer that says what it is told (tests).
pub struct FixedTimer(pub Duration);

impl LoadTimer for FixedTimer {
    fn begin(&mut self) {}
    fn end(&mut self) -> Duration {
        self.0
    }
}

/// The counts and the last load of the store the editor works over.
#[derive(Clone, Debug, Default)]
pub struct StoreWatch {
    counts: BTreeMap<BodyId, usize>,
    last_load: Option<(BodyId, Duration)>,
    /// The bodies whose count has been reported over the limit: reported once, again after the count fell back under it.
    over: BTreeSet<BodyId>,
    /// Every warning issued so far, in order (also written to the error stream).
    issued: Vec<String>,
}

impl StoreWatch {
    /// The warning of a count over the limit, or `None`.
    fn count_warning(body: BodyId, count: usize) -> String {
        format!(
            "item store: body {} holds {count} items, over the {ITEM_LIMIT} of the revisit trigger (ADR 0066 point 6: revisit the backend)",
            body.0
        )
    }

    /// The count of `body` is now `count`: the warning when it has just crossed the limit (once per crossing).
    pub fn record_count(&mut self, body: BodyId, count: usize) -> Option<String> {
        self.counts.insert(body, count);
        if count > ITEM_LIMIT {
            self.over.insert(body).then(|| Self::count_warning(body, count))
        } else {
            self.over.remove(&body);
            None
        }
    }

    /// A load of `body` that found `count` items took `elapsed`: the warnings it earned, the count one first.
    pub fn record_load(&mut self, body: BodyId, count: usize, elapsed: Duration) -> Vec<String> {
        let mut out: Vec<String> = self.record_count(body, count).into_iter().collect();
        self.last_load = Some((body, elapsed));
        if elapsed > LOAD_LIMIT {
            out.push(format!(
                "item store: loading body {} took {} ms, over the {} ms of the revisit trigger (ADR 0066 point 6: revisit the backend)",
                body.0,
                elapsed.as_millis(),
                LOAD_LIMIT.as_millis()
            ));
        }
        out
    }

    /// Log a warning on the error stream and keep it.
    pub fn issue(&mut self, warning: &str) {
        eprintln!("warning: {warning}");
        self.issued.push(warning.to_string());
    }

    /// The warnings issued so far.
    pub fn issued(&self) -> &[String] {
        &self.issued
    }

    /// Whether a body is over the limit now (so that a fall back under it is noticed).
    pub fn any_over(&self) -> bool {
        !self.over.is_empty()
    }

    /// The most items any body holds.
    pub fn max_items(&self) -> usize {
        self.counts.values().copied().max().unwrap_or(0)
    }

    /// The duration of the last load, if there was one.
    pub fn last_load(&self) -> Option<Duration> {
        self.last_load.map(|(_, d)| d)
    }

    /// The diagnostics line: both values of the trigger with their limits, and a mark when one is over.
    pub fn summary(&self) -> String {
        let load = self
            .last_load()
            .map_or("no load yet".to_string(), |d| format!("last load {} ms (limit {} ms)", d.as_millis(), LOAD_LIMIT.as_millis()));
        let over = self.max_items() > ITEM_LIMIT || self.last_load().is_some_and(|d| d > LOAD_LIMIT);
        format!(
            "store: {} items (limit {ITEM_LIMIT}), {load}{}",
            self.max_items(),
            if over { " - REVISIT THE BACKEND (ADR 0066 point 6)" } else { "" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: BodyId = BodyId(0);

    // spec: AUTH-006
    #[test]
    fn the_count_warns_once_above_twenty_thousand_and_not_at_or_below() {
        let mut w = StoreWatch::default();
        assert_eq!(w.record_count(B, 19_999), None);
        assert_eq!(w.record_count(B, ITEM_LIMIT), None, "20,000 is the limit, not over it");
        let warning = w.record_count(B, 20_001).expect("one more is over");
        assert!(
            warning.contains("20001 items") && warning.contains("20000") && warning.contains("ADR 0066 point 6: revisit the backend"),
            "{warning}"
        );
        assert_eq!(w.record_count(B, 20_002), None, "once per crossing");
        assert_eq!(w.record_count(B, 100), None);
        assert!(w.record_count(B, 20_001).is_some(), "again after it fell back under the limit");
    }

    // spec: AUTH-006
    #[test]
    fn a_load_over_two_seconds_warns_with_the_time_and_one_at_two_seconds_does_not() {
        let mut w = StoreWatch::default();
        assert!(w.record_load(B, 10, LOAD_LIMIT).is_empty());
        let warnings = w.record_load(B, 10, Duration::from_millis(2_001));
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("took 2001 ms") && warnings[0].contains("2000 ms") && warnings[0].contains("revisit the backend"),
            "{warnings:?}"
        );
        let both = w.record_load(BodyId(1), 20_001, Duration::from_secs(3));
        assert_eq!(both.len(), 2, "the count and the time are two warnings: {both:?}");
        assert!(both[0].contains("20001") && both[1].contains("3000 ms"));
    }

    // spec: AUTH-006
    #[test]
    fn the_diagnostics_line_shows_both_values_and_marks_a_crossing() {
        let mut w = StoreWatch::default();
        assert_eq!(w.summary(), "store: 0 items (limit 20000), no load yet");
        w.record_load(B, 19_999, Duration::from_millis(40));
        assert_eq!(w.summary(), "store: 19999 items (limit 20000), last load 40 ms (limit 2000 ms)");
        w.record_load(B, 20_001, Duration::from_millis(40));
        assert!(w.summary().ends_with("REVISIT THE BACKEND (ADR 0066 point 6)"), "{}", w.summary());
        w.record_load(B, 5, Duration::from_millis(2_500));
        assert!(w.summary().contains("last load 2500 ms") && w.summary().contains("REVISIT"), "{}", w.summary());
    }

    // spec: AUTH-006
    #[test]
    fn the_fixed_timer_gives_the_duration_it_is_told_and_the_wall_timer_never_goes_backwards() {
        let mut t = FixedTimer(Duration::from_millis(7));
        t.begin();
        assert_eq!(t.end(), Duration::from_millis(7));
        let mut w = WallTimer::new();
        assert_eq!(w.end(), Duration::ZERO, "ended before it began");
        w.begin();
        let _ = w.end();
    }
}
