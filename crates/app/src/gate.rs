//! The no-degradation gate (TEST-061): what a window run counts and the limits it fails on.
//!
//! The window records, with `PLANET_GATE` set, every frame's time with its phases, the tiles the plan holds back (ADR 0074) with whether an edit
//! reaches them, the panics caught and the horizon maps baked. [`Gate::failures`] judges them against [`FRAME_LIMIT_MS`] and the rule of the
//! owner's decision of 2026-10-28; GPU-free, so tier A tests it.

use planet_core::TileId;
use std::sync::atomic::{AtomicU64, Ordering};

/// A frame over this many milliseconds fails the gate.
pub const FRAME_LIMIT_MS: f64 = 100.0;

/// The frame phases that wait for the GPU (by the start of their names in the window's stall probe); they are not counted in a frame's time.
/// Frames at the start of a run that are not judged: shaders and pipelines compile on the first frames.
pub const WARMUP_FRAMES: u64 = 20;

/// The frame phases that wait for the GPU (by the start of their names in the window's stall probe); they are not counted in a frame's time.
const GPU_PHASES: [&str; 4] = ["wait for the GPU", "render submit", "present", "title, exposure readback"];

/// Panics seen by the process hook (any thread); the gate reads the count at the end of the run.
static PANICS: AtomicU64 = AtomicU64::new(0);

/// The panic hook calls this for every panic.
pub fn note_panic() {
    PANICS.fetch_add(1, Ordering::Relaxed);
}

/// Panics so far in this process.
pub fn panics() -> u64 {
    PANICS.load(Ordering::Relaxed)
}

/// A frame over the limit.
#[derive(Clone, Debug, PartialEq)]
pub struct SlowFrame {
    pub frame: u64,
    pub ms: f64,
    pub phase: String,
    pub phase_ms: f64,
}

/// A frame whose plan held back tiles with no edit in reach.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldFrame {
    pub frame: u64,
    pub tiles: Vec<TileId>,
}

/// What a run counted.
#[derive(Clone, Debug, Default)]
pub struct Gate {
    pub frames: u64,
    pub longest_ms: f64,
    pub slow: Vec<SlowFrame>,
    pub held_free: Vec<HeldFrame>,
    /// Most tiles held back under an edit at once (reported, not failed).
    pub held_under_edit_max: usize,
    pub held_frames: u64,
}

impl Gate {
    /// One finished frame: its phases (name, milliseconds). The frame's time is the sum of the phases that are not a wait for the GPU, so a slow
    /// adapter (the software one in CI) does not fail the gate for drawing slowly; a stall on the main thread does.
    pub fn frame(&mut self, frame: u64, phases: &[(&str, f64)]) {
        if frame < WARMUP_FRAMES {
            return;
        }
        let phases: Vec<(&str, f64)> = phases.iter().copied().filter(|(name, _)| !GPU_PHASES.iter().any(|g| name.starts_with(g))).collect();
        let ms: f64 = phases.iter().map(|p| p.1).sum();
        self.frames += 1;
        self.longest_ms = self.longest_ms.max(ms);
        if ms > FRAME_LIMIT_MS {
            let (phase, phase_ms) = phases.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or(("(no phase)", 0.0));
            self.slow.push(SlowFrame { frame, ms, phase: phase.to_string(), phase_ms });
        }
    }

    /// The tiles the plan of `frame` holds back, and which of them an edit reaches.
    pub fn held(&mut self, frame: u64, held: &[TileId], under_edit: &[TileId]) {
        if held.is_empty() {
            return;
        }
        self.held_frames += 1;
        let free: Vec<TileId> = held.iter().copied().filter(|t| !under_edit.contains(t)).collect();
        self.held_under_edit_max = self.held_under_edit_max.max(held.len() - free.len());
        if !free.is_empty() {
            self.held_free.push(HeldFrame { frame, tiles: free });
        }
    }

    /// With `judge_perf` false (the software adapter: its rendering shares the CPU with the main thread, and a 32-cell world lands too few chunks to bake a map) the
    /// frame time and the horizon maps are reported and not judged; held-back tiles and panics always are.
    /// The reasons the run fails, each naming the counter, the frame and the phase or the tiles; empty when it passes. `horizon_baked` is the
    /// maps baked over the run and `lit_with_stage` whether the run drew lit over a chunk stage (where none baked is a failure).
    pub fn failures(&self, horizon_baked: u64, lit_with_stage: bool, panics: u64, judge_perf: bool) -> Vec<String> {
        let mut f = Vec::new();
        if let Some(s) = self.slow.iter().filter(|_| judge_perf).max_by(|a, b| a.ms.total_cmp(&b.ms)) {
            f.push(format!(
                "frames over {FRAME_LIMIT_MS:.0} ms: {} (longest frame {}: {:.0} ms, most of it in \"{}\" at {:.0} ms)",
                self.slow.len(),
                s.frame,
                s.ms,
                s.phase,
                s.phase_ms
            ));
        }
        if let Some(first) = self.held_free.first() {
            let worst = self.held_free.iter().map(|h| h.tiles.len()).max().unwrap_or(0);
            let ids: Vec<String> = first.tiles.iter().take(8).map(|t| format!("{t:?}")).collect();
            f.push(format!(
                "held-back tiles with no edit in reach (ADR 0074): on {} frames, up to {worst} at once; frame {}: {}",
                self.held_free.len(),
                first.frame,
                ids.join(", ")
            ));
        }
        if judge_perf && lit_with_stage && horizon_baked == 0 {
            f.push("horizon maps baked: 0 over a lit run with a chunk stage".to_string());
        }
        if panics > 0 {
            f.push(format!("panics caught: {panics}"));
        }
        f
    }

    /// The counters, a few lines, for the smoke summary and the report file.
    pub fn report(&self, horizon_baked: u64, panics: u64) -> String {
        format!(
            "gate: {} frames, longest {:.0} ms, {} over {FRAME_LIMIT_MS:.0} ms; held back: {} frames, {} with no edit in reach, up to {} under an edit; horizon maps baked {horizon_baked}; panics {panics}",
            self.frames,
            self.longest_ms,
            self.slow.len(),
            self.held_frames,
            self.held_free.len(),
            self.held_under_edit_max
        )
    }
}

/// A camera path file replayed in real time on the window's clock (`PLANET_GATE_PATH`): the pose logged at or before the time since the start.
#[derive(Clone, Debug)]
pub struct Replay {
    samples: Vec<planet_frame::pathlog::PathSample>,
    start_ms: Option<u64>,
}

impl Replay {
    /// Reads the file of `PathLog::to_text`.
    pub fn from_text(text: &str) -> Result<Replay, String> {
        let samples: Vec<_> = planet_frame::pathlog::PathLog::from_text(text)?.samples().copied().collect();
        if samples.is_empty() {
            return Err("the camera path holds no pose".to_string());
        }
        Ok(Replay { samples, start_ms: None })
    }

    /// The first pose, held until [`start`](Replay::start) (the edit is placed from it).
    pub fn first(&self) -> &planet_frame::pathlog::PathSample {
        &self.samples[0]
    }

    /// The replay begins at `now_ms`.
    pub fn start(&mut self, now_ms: u64) {
        self.start_ms.get_or_insert(now_ms);
    }

    pub fn started(&self) -> bool {
        self.start_ms.is_some()
    }

    /// The pose for `now_ms` (the last one once the path has ended) and whether the path has ended.
    pub fn pose(&self, now_ms: u64) -> (&planet_frame::pathlog::PathSample, bool) {
        let Some(start) = self.start_ms else { return (self.first(), false) };
        let t0 = self.samples[0].t_ms;
        let t = t0 + now_ms.saturating_sub(start);
        let i = self.samples.partition_point(|s| s.t_ms <= t).saturating_sub(1);
        (&self.samples[i], t >= self.samples[self.samples.len() - 1].t_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(n: u8) -> TileId {
        TileId::new(planet_core::Face(0), n, 0, 0).unwrap()
    }

    // spec: TEST-061
    #[test]
    fn a_held_back_tile_away_from_an_edit_fails_and_names_the_tiles_and_the_frame() {
        let mut g = Gate::default();
        g.held(7, &[tile(3), tile(3), tile(2)], &[]);
        let f = g.failures(5, true, 0, true);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("frame 7") && f[0].contains("up to 3 at once"), "{f:?}");
    }

    // spec: TEST-061
    #[test]
    fn held_back_tiles_under_an_edit_are_reported_and_do_not_fail() {
        let mut g = Gate::default();
        g.held(2, &[tile(1), tile(2)], &[tile(1), tile(2)]);
        assert!(g.failures(1, true, 0, true).is_empty());
        assert_eq!(g.held_under_edit_max, 2);
        assert!(g.report(1, 0).contains("up to 2 under an edit"));
    }

    // spec: TEST-061
    #[test]
    fn a_slow_frame_fails_naming_its_longest_phase() {
        let mut g = Gate::default();
        g.frame(30, &[("draw", 12.0)]);
        g.frame(3, &[("compile", 900.0)]);
        g.frame(39, &[("input", 2.0), ("chunk frame", 120.0), ("wait for the GPU (x)", 900.0), ("present (swapchain)", 50.0)]);
        let f = g.failures(1, true, 0, true);
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("frame 39") && f[0].contains("122 ms") && f[0].contains("chunk frame"), "{f:?}");
        assert!(g.report(1, 0).contains("1 over 100 ms"));
    }

    // spec: TEST-061
    #[test]
    fn no_horizon_map_over_a_lit_run_with_a_stage_and_a_panic_fail() {
        let g = Gate::default();
        assert!(g.failures(0, false, 0, true).is_empty(), "no stage, no horizon expected");
        let f = g.failures(0, true, 2, true);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].contains("horizon") && f[1].contains("panics caught: 2"));
    }

    // spec: TEST-061
    #[test]
    fn on_the_software_adapter_frame_time_and_horizon_maps_are_reported_and_held_tiles_and_panics_still_fail() {
        let mut g = Gate::default();
        g.frame(30, &[("chunk frame", 900.0)]);
        g.held(31, &[tile(1)], &[]);
        let f = g.failures(0, true, 1, false);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].contains("held-back") && f[1].contains("panics"));
    }

    // spec: TEST-062
    #[test]
    fn a_replay_holds_the_first_pose_until_it_starts_then_follows_the_clock_and_ends_on_the_last_pose() {
        let text = "planet-path 1\nwindow_ms 60000\nperiod_ms 100\nsamples 3\n100 1 0 0 0 1 0 0 0 1 5 16\n200 2 0 0 0 1 0 0 0 1 5 16\n300 3 0 0 0 1 0 0 0 1 5 16\n";
        let mut r = Replay::from_text(text).unwrap();
        assert_eq!(r.pose(9_999), (r.first(), false), "held before the start");
        r.start(1_000);
        assert!(r.started());
        assert_eq!(r.pose(1_050).0.position[0], 1.0);
        assert_eq!(r.pose(1_150).0.position[0], 2.0);
        let (last, done) = r.pose(1_200);
        assert!(done && last.position[0] == 3.0);
        assert!(Replay::from_text("planet-path 1\nwindow_ms 60000\nperiod_ms 100\nsamples 0\n").is_err());
    }
}
