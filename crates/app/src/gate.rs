//! The no-degradation gate (TEST-061): what a window run counts and the limits it fails on.
//!
//! The window records, with `PLANET_GATE` set, every frame's time with its phases, the tiles the plan holds back (ADR 0074) with whether an edit
//! reaches them, the panics caught and the horizon maps baked. [`Gate::failures`] judges them against [`FRAME_LIMIT_MS`] and the rule of the
//! owner's decision of 2026-10-28; GPU-free, so tier A tests it.

use planet_core::TileId;
use std::sync::atomic::{AtomicU64, Ordering};

/// A frame over this many milliseconds fails the gate.
pub const FRAME_LIMIT_MS: f64 = 100.0;

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
    /// Bake blocks the main thread decoded in this frame (VIEW-061).
    pub decodes: u64,
}

/// A frame whose plan held back tiles with no edit in reach.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldFrame {
    pub frame: u64,
    pub tiles: Vec<TileId>,
}

/// A frame in which the main thread decoded bake blocks itself (VIEW-061).
#[derive(Clone, Debug, PartialEq)]
pub struct DecodeFrame {
    pub frame: u64,
    pub phase: String,
    pub blocks: u64,
}

/// What a run counted.
#[derive(Clone, Debug, Default)]
pub struct Gate {
    pub frames: u64,
    pub longest_ms: f64,
    pub slow: Vec<SlowFrame>,
    /// The frames in which the main thread decoded a bake block, the first phase of each (a decode waits for the disk and the codec: a stall the warm-up thread should take).
    pub decoded: Vec<DecodeFrame>,
    pub main_decodes: u64,
    pub held_free: Vec<HeldFrame>,
    /// Most tiles held back under an edit at once (reported, not failed).
    pub held_under_edit_max: usize,
    pub held_frames: u64,
    /// The clock time the scripted edit was published at, and the wanted horizon maps it touched (their edit word changed).
    pub edit_at: Option<u64>,
    pub touched: Vec<planet_generators::chunkfield::ChunkKey>,
    /// The touched maps that landed after the edit.
    pub rebaked: Vec<planet_generators::chunkfield::ChunkKey>,
    /// Milliseconds from the edit to the first touched map that landed after it.
    pub rebake_ms: Option<u64>,
    /// Milliseconds from the edit to the first upload of a re-baked map, and the frame it was drawn from: the map on screen.
    pub drawn_ms: Option<(u64, u64)>,
}

impl Gate {
    /// The scripted edit was published at `now_ms` and touched the wanted maps `touched` (once; later calls do nothing).
    pub fn edit_published(&mut self, now_ms: u64, touched: Vec<planet_generators::chunkfield::ChunkKey>) {
        if self.edit_at.is_none() {
            self.edit_at = Some(now_ms);
            self.touched = touched;
        }
    }

    /// The map of `key` was put on the GPU at `now_ms` in `frame`: the first re-baked one after the edit is its shadow on screen.
    pub fn map_uploaded(&mut self, key: planet_generators::chunkfield::ChunkKey, now_ms: u64, frame: u64) {
        if let (Some(at), None) = (self.edit_at, self.drawn_ms) {
            if self.rebaked.contains(&key) {
                self.drawn_ms = Some((now_ms.saturating_sub(at), frame));
            }
        }
    }

    /// The maps that landed at `now_ms`: the first touched one after the edit is the re-bake (an unrelated map landing does not count).
    pub fn horizon_landed(&mut self, now_ms: u64, landed: &[planet_generators::chunkfield::ChunkKey]) {
        let Some(at) = self.edit_at else { return };
        for k in landed.iter().filter(|k| self.touched.contains(k)) {
            if !self.rebaked.contains(k) {
                self.rebaked.push(*k);
            }
            self.rebake_ms.get_or_insert(now_ms.saturating_sub(at));
        }
    }

    /// One finished frame: its phases (name, milliseconds). The frame's time is the sum of the phases that are not a wait for the GPU, so a slow
    /// adapter (the software one in CI) does not fail the gate for drawing slowly; a stall on the main thread does.
    ///
    /// `decodes` are the bake blocks the main thread decoded itself in each phase of the frame (the stall probe's count, phases with none left out): a block is
    /// decoded off the main thread by the warm-up and the hover threads (VIEW-059, VIEW-061), so any count here is a read that waited for the disk.
    pub fn frame(&mut self, frame: u64, phases: &[(&str, f64)], decodes: &[(&str, u64)]) {
        if frame < WARMUP_FRAMES {
            return;
        }
        let blocks: u64 = decodes.iter().map(|d| d.1).sum();
        if let Some((phase, _)) = decodes.iter().max_by_key(|d| d.1) {
            self.main_decodes += blocks;
            self.decoded.push(DecodeFrame { frame, phase: (*phase).to_string(), blocks });
        }
        let phases: Vec<(&str, f64)> = phases.iter().copied().filter(|(name, _)| !GPU_PHASES.iter().any(|g| name.starts_with(g))).collect();
        let ms: f64 = phases.iter().map(|p| p.1).sum();
        self.frames += 1;
        self.longest_ms = self.longest_ms.max(ms);
        if ms > FRAME_LIMIT_MS {
            let (phase, phase_ms) = phases.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or(("(no phase)", 0.0));
            self.slow.push(SlowFrame { frame, ms, phase: phase.to_string(), phase_ms, decodes: blocks });
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
    ///
    /// The reasons the run fails, each naming the counter, the frame and the phase or the tiles; empty when it passes. `horizon_baked` is the
    /// maps baked over the run and `lit_with_horizon` whether the run drew lit with the horizon jobs on (where none baked is a failure).
    pub fn failures(&self, horizon_baked: u64, lit_with_horizon: bool, panics: u64, judge_perf: bool) -> Vec<String> {
        let mut f = Vec::new();
        if let Some(s) = self.slow.iter().filter(|_| judge_perf).max_by(|a, b| a.ms.total_cmp(&b.ms)) {
            let decoded =
                if s.decodes > 0 { format!(", {} bake blocks decoded on the main thread in that frame", s.decodes) } else { String::new() };
            f.push(format!(
                "frames over {FRAME_LIMIT_MS:.0} ms: {} (longest frame {}: {:.0} ms, most of it in \"{}\" at {:.0} ms{decoded}; {} of them decoded a block)",
                self.slow.len(),
                s.frame,
                s.ms,
                s.phase,
                s.phase_ms,
                self.slow.iter().filter(|s| s.decodes > 0).count()
            ));
        }
        if let (true, Some(first)) = (judge_perf, self.decoded.first()) {
            f.push(format!(
                "bake blocks decoded on the main thread (VIEW-061): {} in {} frames; first in frame {} in \"{}\" ({} blocks)",
                self.main_decodes,
                self.decoded.len(),
                first.frame,
                first.phase,
                first.blocks
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
        if judge_perf && lit_with_horizon && horizon_baked == 0 {
            f.push("horizon maps baked: 0 over a lit run with the horizon jobs on".to_string());
        }
        if panics > 0 {
            f.push(format!("panics caught: {panics}"));
        }
        f
    }

    /// The counters, a few lines, for the smoke summary and the report file.
    pub fn report(&self, horizon_baked: u64, panics: u64) -> String {
        let edit = match (self.edit_at, self.rebake_ms, self.drawn_ms) {
            (None, _, _) => String::new(),
            (Some(_), None, _) if self.touched.is_empty() => "; edit: touched no wanted map".to_string(),
            (Some(_), None, _) => format!("; edit: touched {} maps, none re-baked", self.touched.len()),
            (Some(_), Some(b), None) => format!("; edit: re-bake landed {b} ms after it, not drawn yet"),
            (Some(_), Some(b), Some((d, f))) => format!("; edit: re-bake landed {b} ms after it, first drawn {d} ms after it (frame {f})"),
        };
        format!(
            "gate: {} frames, longest {:.0} ms, {} over {FRAME_LIMIT_MS:.0} ms; main-thread block decodes {} in {} frames; held back: {} frames, {} with no edit in reach, up to {} under an edit; horizon maps baked {horizon_baked}; panics {panics}{edit}",
            self.frames,
            self.longest_ms,
            self.slow.len(),
            self.main_decodes,
            self.decoded.len(),
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
    fn a_block_decoded_by_the_main_thread_fails_and_names_the_frame_the_phase_and_the_count() {
        let mut g = Gate::default();
        g.frame(30, &[("draw", 12.0)], &[]);
        g.frame(31, &[("plan and residency (begin_frame)", 20.0)], &[("plan and residency (begin_frame)", 1)]);
        g.frame(3, &[("compile", 900.0)], &[("compile", 5)]);
        let f = g.failures(5, true, 0, true);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].contains("VIEW-061") && f[0].contains("1 in 1 frames") && f[0].contains("frame 31") && f[0].contains("begin_frame"),
            "{f:?} (frame 3 is a warm-up frame and is not counted)"
        );
        assert!(g.report(5, 0).contains("main-thread block decodes 1 in 1 frames"), "{}", g.report(5, 0));
        assert!(g.failures(5, true, 0, false).is_empty(), "not judged where the frame time is not (the software adapter)");
    }

    // spec: TEST-061
    #[test]
    fn a_slow_frame_that_decoded_a_block_says_so() {
        let mut g = Gate::default();
        g.frame(40, &[("editor update (picks, outlines, lists)", 140.0)], &[("editor update (picks, outlines, lists)", 2)]);
        let f = g.failures(5, true, 0, true);
        assert!(
            f.iter().any(|m| m.contains("2 bake blocks decoded on the main thread in that frame") && m.contains("1 of them decoded")),
            "{f:?}"
        );
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
        g.frame(30, &[("draw", 12.0)], &[]);
        g.frame(3, &[("compile", 900.0)], &[]);
        g.frame(39, &[("input", 2.0), ("horizon frame", 120.0), ("wait for the GPU (x)", 900.0), ("present (swapchain)", 50.0)], &[]);
        let f = g.failures(1, true, 0, true);
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("frame 39") && f[0].contains("122 ms") && f[0].contains("horizon frame"), "{f:?}");
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

    // spec: TEST-065
    #[test]
    fn the_rebake_after_the_edit_is_reported_not_judged_and_an_unrelated_landing_does_not_count() {
        let (a, b, other) = ((2, 10, 11), (2, 11, 11), (2, 50, 50));
        let mut g = Gate::default();
        assert!(g.failures(3, true, 0, true).is_empty(), "no edit, nothing to re-bake");
        g.edit_published(1_000, vec![a, b]);
        g.horizon_landed(1_300, &[other]);
        assert_eq!(g.rebake_ms, None, "an initial bake of another chunk is not the re-bake");
        assert!(g.failures(3, true, 0, true).is_empty(), "no landing is reported, not judged");
        assert!(g.report(3, 0).contains("none re-baked"));
        assert_eq!(g.failures(0, true, 0, true).len(), 1, "no map baked at all still fails");
        g.horizon_landed(12_000, &[a]);
        assert_eq!(g.rebake_ms, Some(11_000));
        assert!(g.failures(4, true, 0, true).is_empty(), "11 s is reported, not judged");
        assert!(g.report(4, 0).contains("re-bake landed 11000 ms after it"));
        let mut ok = Gate::default();
        ok.edit_published(1_000, vec![a, b]);
        ok.horizon_landed(4_000, &[b]);
        ok.horizon_landed(20_000, &[a]);
        assert!(ok.failures(4, true, 0, true).is_empty() && ok.rebake_ms == Some(3_000), "the first touched landing counts");
        let mut none = Gate::default();
        none.edit_published(1_000, Vec::new());
        assert!(none.failures(4, true, 0, true).is_empty(), "an edit that touched no wanted map has no re-bake to wait for");
        assert!(none.report(4, 0).contains("touched no wanted map"));
    }

    // spec: TEST-065
    #[test]
    fn the_report_says_when_the_rebake_landed_and_when_it_was_first_drawn() {
        let (a, other) = ((2, 10, 11), (2, 50, 50));
        let mut g = Gate::default();
        assert!(!g.report(3, 0).contains("edit:"), "no edit, nothing to say");
        g.edit_published(1_000, vec![a]);
        g.map_uploaded(a, 1_500, 9);
        assert_eq!(g.drawn_ms, None, "an upload before the re-bake landed is not the edit's map");
        assert!(g.report(3, 0).contains("touched 1 maps, none re-baked"));
        g.horizon_landed(3_000, &[a]);
        assert!(g.report(4, 0).contains("re-bake landed 2000 ms after it, not drawn yet"));
        g.map_uploaded(other, 3_010, 119);
        assert_eq!(g.drawn_ms, None, "another map's upload is not the re-baked one");
        g.map_uploaded(a, 3_017, 120);
        g.map_uploaded(a, 3_034, 121);
        assert_eq!(g.drawn_ms, Some((2_017, 120)), "the first upload of the re-baked map");
        assert!(g.report(4, 0).contains("first drawn 2017 ms after it (frame 120)"));
    }
    // spec: TEST-061
    #[test]
    fn on_the_software_adapter_frame_time_and_horizon_maps_are_reported_and_held_tiles_and_panics_still_fail() {
        let mut g = Gate::default();
        g.frame(30, &[("horizon frame", 900.0)], &[]);
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
