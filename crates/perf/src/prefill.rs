//! How a streaming log says what the tile pack held at the start of its scene (STRM-021, TEST-019, TEST-021, ADR 0045 point 3).
//!
//! `protocol.source` is `<where the bake was read from>; <marker>`: the marker is `prefilled coarse pack (levels 0-N present at
//! start)` when the harness checked that the pack held every tile of levels 0 to N (and the scene reads them), `cold: ...` when the
//! reader ignores the pack's coarse tiles (the `dive-cold` scene), and `no pre-filled coarse pack (...)` when the pack lacked some.
//! A log without a marker was written before the pre-fill existed: its scenes started from an empty tile cache, which is what
//! `cold` means, but the log does not say so, so it is `Unstated` and `accept M4` treats it as cold for the gated dive.

/// The levels the gated dive requires pre-filled (owner, 2026-10-16: levels 0 to 4).
pub const GATED_PREFILL_LEVELS: u8 = 4;

/// The marker of a scene that read the pre-filled coarse tiles, all of levels `0..=levels` checked present at its start.
pub fn prefilled_marker(levels: u8) -> String {
    format!("prefilled coarse pack (levels 0-{levels} present at start)")
}

/// The marker of a scene whose reader ignores the pack's coarse tiles.
pub const COLD_MARKER: &str = "cold: the pack's coarse tiles are ignored, every tile is generated on demand";

/// The marker of a scene that found the pack without all the tiles of levels `0..=levels` (and so reads what is there).
pub fn absent_marker(present: usize, total: usize, levels: u8) -> String {
    format!("no pre-filled coarse pack ({present} of {total} tiles of levels 0-{levels} present)")
}

/// `source` of a log: the place the bake was read from and the marker.
pub fn labelled(base: &str, marker: &str) -> String {
    format!("{base}; {marker}")
}

/// What a log's source says about the pack's coarse tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefillState {
    /// Every tile of levels 0 to this one was present at the start and the scene read them.
    Prefilled(u8),
    /// The reader ignored the pack's coarse tiles.
    Cold,
    /// The pack lacked tiles of the levels checked.
    Absent,
    /// No marker: a log from before the pre-fill (an empty tile cache at start, not stated).
    Unstated,
}

/// Split a log's `protocol.source` into the place the bake was read from and what it says about the pack.
pub fn parse(source: &str) -> (&str, PrefillState) {
    let Some((base, marker)) = source.split_once("; ") else { return (source, PrefillState::Unstated) };
    let state = if let Some(rest) = marker.strip_prefix("prefilled coarse pack (levels 0-") {
        rest.split_once(' ')
            .and_then(|(n, tail)| (tail == "present at start)").then_some(n))
            .and_then(|n| n.parse().ok())
            .map_or(PrefillState::Unstated, PrefillState::Prefilled)
    } else if marker.starts_with("cold:") {
        PrefillState::Cold
    } else if marker.starts_with("no pre-filled coarse pack") {
        PrefillState::Absent
    } else {
        PrefillState::Unstated
    };
    (base, state)
}

impl PrefillState {
    /// Words for a report or a refusal.
    pub fn describe(self) -> String {
        match self {
            PrefillState::Prefilled(n) => format!("pre-filled coarse pack (levels 0-{n} present at start)"),
            PrefillState::Cold => "cold (the pack's coarse tiles ignored)".to_string(),
            PrefillState::Absent => "no pre-filled coarse pack (some tiles of the checked levels were missing)".to_string(),
            PrefillState::Unstated => {
                "not stated (a log from before the pre-fill: it started from an empty tile cache, which is cold)".to_string()
            }
        }
    }

    /// Whether the gated dive may be judged from a log with this state: pre-filled to at least [`GATED_PREFILL_LEVELS`].
    pub fn counts_for_the_gated_dive(self) -> bool {
        matches!(self, PrefillState::Prefilled(n) if n >= GATED_PREFILL_LEVELS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-021
    #[test]
    fn a_source_round_trips_its_marker_and_a_log_without_one_is_unstated() {
        let base = "store D:/bake, 16384 cells";
        for (marker, state) in [
            (prefilled_marker(4), PrefillState::Prefilled(4)),
            (prefilled_marker(6), PrefillState::Prefilled(6)),
            (COLD_MARKER.to_string(), PrefillState::Cold),
            (absent_marker(126, 2046, 4), PrefillState::Absent),
        ] {
            assert_eq!(parse(&labelled(base, &marker)), (base, state), "{marker}");
        }
        assert_eq!(parse(base), (base, PrefillState::Unstated));
        assert_eq!(parse(&labelled(base, "something else")), (base, PrefillState::Unstated));
        assert_eq!(parse(&labelled(base, "prefilled coarse pack (levels 0-x present at start)")), (base, PrefillState::Unstated));
        assert_eq!(
            parse(&labelled(base, "prefilled coarse pack (levels 0-4 present)")),
            (base, PrefillState::Unstated),
            "a garbled marker proves nothing"
        );
        assert!(prefilled_marker(4).contains("prefilled coarse pack (levels 0-4 present at start)"));
    }

    // spec: TEST-021
    #[test]
    fn only_a_pack_pre_filled_to_the_gated_levels_counts_for_the_gated_dive() {
        assert!(PrefillState::Prefilled(4).counts_for_the_gated_dive() && PrefillState::Prefilled(5).counts_for_the_gated_dive());
        assert!(![PrefillState::Prefilled(3), PrefillState::Cold, PrefillState::Absent, PrefillState::Unstated]
            .iter()
            .any(|s| s.counts_for_the_gated_dive()));
    }
}
