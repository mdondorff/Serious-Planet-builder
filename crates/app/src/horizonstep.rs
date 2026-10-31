//! The per-frame horizon step of the viewer and the scenes (STRM-048, TEST-058): the horizon jobs of the chunks the plan reads, the counters of what
//! the step did, and the announcement of an item change to the plan cache.

use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Wait for the coupled run of the world (made and stored if the store holds none), for a headless scene whose anchor reads it.
pub fn wait_coupled(world: &Arc<WorldTerrain>, out: &mut dyn Write) -> Result<Arc<planet_generators::coupled::CoupledMaps>, String> {
    use crate::coupledworld::Standing;
    world.request_coupled();
    let started = Instant::now();
    let maps = loop {
        match world.coupled_standing() {
            Standing::Ready(m, _) => break m,
            Standing::Failed(e) => return Err(format!("the coupled run failed: {e}")),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    writeln!(out, "coupled run ready after {:.1} s", started.elapsed().as_secs_f64()).map_err(err)?;
    Ok(maps)
}

/// Wait for the stored far-field horizon the horizon start requested; a world without one goes on with the chunk horizons alone and says so.
pub fn wait_coarse_horizon(world: &WorldTerrain, out: &mut dyn Write) -> Result<(), String> {
    use crate::horizonworld::CoarseStanding;
    let line = loop {
        match world.coarse_standing() {
            CoarseStanding::Idle | CoarseStanding::Working => std::thread::sleep(Duration::from_millis(50)),
            CoarseStanding::Ready(h) => break format!("coarse horizon: {} observer cells per face edge, {} azimuths", h.m, h.azimuths),
            CoarseStanding::Absent(t) | CoarseStanding::Failed(t) => break format!("horizon: {t}"),
        }
    };
    writeln!(out, "{line}").map_err(err)
}

/// The generation workers of the options, which the horizon jobs take their pool size from.
pub fn stage_gen(o: &Options) -> planet_streaming::GenWorkers {
    o.gen_workers()
}

/// The features of the per-frame step a caller may turn off on purpose (TEST-058): all on by default, and a scene that turns one off says so in its log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameFeatures {
    /// The horizon jobs of the chunks the plan reads run (STRM-048).
    pub horizon: bool,
}

impl Default for FrameFeatures {
    fn default() -> FrameFeatures {
        FrameFeatures { horizon: true }
    }
}

/// What the step did over a run, one counter per feature (TEST-058).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureCounters {
    /// Steps run.
    pub frames: u64,
    /// Steps that ran the horizon jobs, and the maps that landed.
    pub horizon_frames: u64,
    pub horizon_landed: u64,
    /// Item changes announced to the plan cache (STRM-034).
    pub plan_cache_announcements: u64,
}

/// Which features a scene expects to have acted over its run (TEST-058); a feature the scene turned off is never checked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expect {
    /// The horizon step ran.
    pub horizon: bool,
    /// An item change was announced to the plan cache.
    pub announcements: bool,
}

impl FeatureCounters {
    /// The counters as one log line; a feature the scene turned off is named `off` in place of its count (TEST-058).
    pub fn line(&self, features: &FrameFeatures) -> String {
        let on = |enabled: bool, text: String| if enabled { text } else { "off".to_string() };
        format!(
            "frames {}, horizon {}, plan-cache announcements {}",
            self.frames,
            on(features.horizon, format!("{} frames, {} maps landed", self.horizon_frames, self.horizon_landed)),
            self.plan_cache_announcements
        )
    }

    /// The counters as a JSON object; a feature turned off is `"off"` (TEST-058).
    pub fn json(&self, features: &FrameFeatures) -> String {
        let on = |enabled: bool, value: String| if enabled { value } else { "\"off\"".to_string() };
        format!(
            "{{\"frames\": {}, \"horizon_frames\": {}, \"horizon_landed\": {}, \"plan_cache_announcements\": {}}}",
            self.frames,
            on(features.horizon, self.horizon_frames.to_string()),
            on(features.horizon, self.horizon_landed.to_string()),
            self.plan_cache_announcements
        )
    }

    /// A scene fails when a feature it enabled and expects to act counted 0 (TEST-058): the message names the feature and the counter table.
    pub fn check(&self, features: &FrameFeatures, expect: &Expect) -> Result<(), String> {
        let missing: Vec<&str> = [
            ("horizon", features.horizon && expect.horizon && self.horizon_frames == 0),
            ("plan-cache announcements", expect.announcements && self.plan_cache_announcements == 0),
        ]
        .into_iter()
        .filter_map(|(name, bad)| bad.then_some(name))
        .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("an enabled feature counted 0: {} (TEST-058); counters: {}", missing.join(", "), self.line(features)))
        }
    }
}

/// What one step reported: the horizon update.
#[derive(Clone, Debug, Default)]
pub struct StepReport {
    pub horizon: planet_streaming::horizonjobs::HorizonUpdate,
}

/// Tell the plan cache where the items changed from `old` to the world's definition now, as the viewer does after a swap (STRM-034), so the plan after
/// the edit keeps what the change cannot reach; `old_revision` is the item revision of the field before the swap.
pub fn announce_item_change(
    managed: &mut planet_frame::managed::ManagedLoop,
    world: &WorldTerrain,
    old: &planet_world_def::WorldDefinition,
    old_revision: u64,
    counters: &mut FeatureCounters,
) {
    if let Ok(field) = world.field() {
        let changes = world
            .definition()
            .item_changes_from(old)
            .into_iter()
            .map(|(dir, reach_m)| planet_frame::lod::ItemChange { dir, reach_m })
            .collect();
        managed.note_item_change(old_revision, field.item_revision(), changes);
        counters.plan_cache_announcements += 1;
    }
}

/// The per-frame step of the viewer and of every scene that drives the horizon jobs (STRM-048, TEST-058): the horizon jobs of the chunks the plan's
/// `tiles` read. Called after the managed frame of `camera`; the caller does what is its own (the viewer uploads the horizon maps of the report, a
/// scene counts).
pub fn frame_step(
    world: &WorldTerrain,
    tiles: &[planet_core::TileId],
    camera: planet_core::PlanetFixed,
    features: &FrameFeatures,
    counters: &mut FeatureCounters,
) -> StepReport {
    let horizon = if features.horizon { world.horizon_frame(tiles, camera) } else { Default::default() };
    counters.frames += 1;
    counters.horizon_frames += u64::from(features.horizon);
    counters.horizon_landed += horizon.landed.len() as u64;
    StepReport { horizon }
}

/// Whether a frame still waits for the horizon jobs: a horizon job is in flight. A scene that settles before a flight waits until this is false.
pub fn horizon_busy(horizon_jobs: usize) -> bool {
    horizon_jobs > 0
}

/// The features of a settling loop that is not a frame: the horizon jobs when `horizon`.
pub fn settling_features(horizon: bool) -> FrameFeatures {
    FrameFeatures { horizon }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-055
    // spec: TEST-058
    #[test]
    fn a_scene_fails_naming_the_feature_when_an_enabled_feature_it_expects_counted_zero_and_a_feature_turned_off_is_named_off() {
        let all = FrameFeatures::default();
        let expect = Expect { horizon: true, announcements: true };
        let acted = FeatureCounters { frames: 10, horizon_frames: 10, plan_cache_announcements: 1, ..Default::default() };
        assert_eq!(acted.check(&all, &expect), Ok(()));
        let err = FeatureCounters { horizon_frames: 0, plan_cache_announcements: 0, ..acted }.check(&all, &expect).unwrap_err();
        assert!(err.contains("horizon, plan-cache announcements") && err.contains("frames 10"), "{err}");
        // Turned off: not checked, and the log says so.
        let off = FrameFeatures { horizon: false };
        let zero = FeatureCounters { horizon_frames: 0, ..acted };
        assert_eq!(zero.check(&off, &expect), Ok(()));
        assert!(zero.line(&off).contains("horizon off") && zero.json(&off).contains("\"horizon_frames\": \"off\""));
        assert!(acted.line(&all).contains("horizon 10 frames, 0 maps landed"));
    }

    // spec: TEST-054
    #[test]
    fn a_frame_waits_for_the_horizon_jobs_while_a_node_stands_in_or_a_job_runs() {
        assert!(!horizon_busy(0));
        assert!(horizon_busy(1));
    }
}
