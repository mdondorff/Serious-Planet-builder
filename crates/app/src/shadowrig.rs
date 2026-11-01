//! The clipmap planner off the frame thread (REND-046, REND-050, ADR 0085; opus-planner, 2026-10-31): the first plan of a view takes about
//! 400 ms and a re-anchor as long, so the frame thread never runs [`ClipmapPlanner::plan`]. The planner lives in a job on the terrain share
//! (the injected spawner, CON-04) with the frame's inputs; the frame thread hands in the next inputs and takes the plan that landed. While
//! a job runs the shadow pass is not asked to run: the last resolved shadow stays where it is.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId};
use planet_frame::clipmap::{ClipmapParams, ClipmapPlan, ClipmapPlanner};
use planet_frame::preview::PreviewStamp;
use planet_frame::DEFAULT_RENDER_FLOOR;
use planet_streaming::{Job, JobClass, Spawner};

/// What the frame thread hands in each frame.
pub struct ShadowInputs {
    pub camera: PlanetFixed,
    /// The resident tiles of R, ascending tile ids.
    pub resident: Vec<TileId>,
    /// The resident tiles placed or replaced since the last call (an edit's refresh, a tile that landed).
    pub placed: Vec<TileId>,
    pub stamps: Vec<PreviewStamp>,
}

/// A plan that landed.
pub struct ShadowLanded {
    pub plan: ClipmapPlan,
    /// The floor tiles (levels `DEFAULT_RENDER_FLOOR` and the next) when they changed since the previous plan, else empty.
    pub floor_tiles: Vec<TileId>,
}

struct Result {
    planner: ClipmapPlanner,
    landed: ShadowLanded,
}

pub struct ShadowRig {
    spawner: Arc<dyn Spawner>,
    mailbox: Arc<Mutex<Option<Result>>>,
    /// `None` while a job holds it.
    planner: Option<ClipmapPlanner>,
    /// Tiles placed while a job was running: the next job refreshes them.
    carried: BTreeSet<TileId>,
    floor_seen: Vec<TileId>,
    /// The last plan that landed: the frames between landings reuse it with nothing dirty.
    last: Option<ClipmapPlan>,
    /// The thread each job ran on, newest last (tests).
    job_threads: Arc<Mutex<Vec<std::thread::ThreadId>>>,
}

/// The resident floor tiles whose footprints are disjoint and cover every face (the coarse cube's input): a floor-level tile, replaced by its
/// four children only where all four are resident.
fn floor_leaves(resident: &[TileId]) -> Vec<TileId> {
    let has = |t: &TileId| resident.binary_search(t).is_ok();
    let mut out = Vec::new();
    for t in resident.iter().filter(|t| t.level() == DEFAULT_RENDER_FLOOR) {
        match t.children() {
            Some(cs) if cs.iter().all(has) => out.extend(cs),
            _ => out.push(*t),
        }
    }
    out.sort();
    out
}

impl ShadowRig {
    pub fn new(spawner: Arc<dyn Spawner>, params: ClipmapParams) -> ShadowRig {
        ShadowRig {
            spawner,
            mailbox: Arc::new(Mutex::new(None)),
            planner: Some(ClipmapPlanner::new(params)),
            carried: BTreeSet::new(),
            floor_seen: Vec::new(),
            last: None,
            job_threads: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// One frame, on the frame thread: take the plan that landed (if any) and, when the planner is free, start the next job from `inputs`.
    /// Never plans.
    pub fn step(&mut self, inputs: ShadowInputs) -> Option<ShadowLanded> {
        self.carried.extend(inputs.placed.iter().copied());
        let landed = self.mailbox.lock().expect("shadow mailbox").take().map(|r| {
            self.planner = Some(r.planner);
            r.landed
        });
        if let Some(mut planner) = self.planner.take() {
            let refreshed: Vec<TileId> =
                std::mem::take(&mut self.carried).into_iter().filter(|t| inputs.resident.binary_search(t).is_ok()).collect();
            let floor = floor_leaves(&inputs.resident);
            let floor_changed = floor != self.floor_seen;
            self.floor_seen = floor.clone();
            let mailbox = Arc::clone(&self.mailbox);
            let threads = Arc::clone(&self.job_threads);
            let job: Job = Box::new(move || {
                threads.lock().expect("job threads").push(std::thread::current().id());
                let plan = planner.plan(inputs.camera, &inputs.resident, &refreshed, &inputs.stamps, &TangentWarp);
                let landed = ShadowLanded { plan, floor_tiles: if floor_changed { floor } else { Vec::new() } };
                *mailbox.lock().expect("shadow mailbox") = Some(Result { planner, landed });
            });
            self.spawner.spawn_in(JobClass::Prefetch, job);
        }
        landed
    }

    /// The shadow frame of this frame: the plan that landed, or the last one with nothing dirty (the camera, the sun and the preview are this
    /// frame's); `None` until the first plan has landed.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        landed: Option<ShadowLanded>,
        camera: &planet_frame::Camera,
        sun: planet_core::Vec3,
        radius_m: f64,
        stamps: &[PreviewStamp],
        delta_len: usize,
        nodes: &[planet_frame::TerrainDraw],
    ) -> Option<planet_render::shadow_path::ShadowFrame> {
        let (plan, floor_tiles) = match landed {
            Some(l) => {
                self.last = Some(l.plan.clone());
                (l.plan, l.floor_tiles)
            }
            None => {
                let mut p = self.last.clone()?;
                p.levels.iter_mut().for_each(|l| l.dirty.clear());
                p.reanchored = false;
                (p, Vec::new())
            }
        };
        Some(planet_render::shadow_path::ShadowFrame {
            view: planet_frame::shadowview::ShadowView::new(plan.anchor, camera, sun, radius_m),
            preview: planet_render::shadow::ClipmapPreview::of_draw(stamps, delta_len, nodes, plan.anchor, radius_m),
            plan,
            floor_tiles,
            cut: false,
        })
    }

    /// Whether a job holds the planner.
    pub fn planning(&self) -> bool {
        self.planner.is_none()
    }

    /// The threads the jobs ran on (tests).
    pub fn job_threads(&self) -> Vec<std::thread::ThreadId> {
        self.job_threads.lock().expect("job threads").clone()
    }
}
