//! `test-render --repro BUNDLE --replay-path PATH --out-dir DIR [--replay-every N]` (VIEW-026): the camera path F9 wrote, driven through the
//! terrain plan of the bundle's world, one plan per logged pose in the logged order, so that what the owner saw (a level that dropped on
//! approach, a hole at a seam) can be looked at frame by frame without the window.

use crate::skeleton::{open_terrain_bundle, upload_missing_meshes};
use crate::{err, Options};
use planet_frame::pathlog::PathLog;
use planet_frame::TerrainRequest;
use planet_render::{GpuContext, TerrainRenderer};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

/// What one pose planned to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseRow {
    pub t_ms: u64,
    pub altitude_m: f64,
    pub nodes: usize,
    pub plan_hash: u64,
    pub finest_level: u8,
}

/// The line of `replay.txt` for one pose (`t_ms altitude_m nodes plan_hash finest_level plan_ms logged_frame_ms`).
pub fn row_line(r: &PoseRow, plan_ms: f64, frame_ms: f64) -> String {
    format!("{} {:.3} {} {:016x} {} {:.3} {:.3}", r.t_ms, r.altitude_m, r.nodes, r.plan_hash, r.finest_level, plan_ms, frame_ms)
}

/// The poses at which the finest level fell while the camera got lower: a level that drops on approach (the owner's tracked bug).
pub fn level_drops(rows: &[PoseRow]) -> Vec<usize> {
    (1..rows.len()).filter(|&i| rows[i].finest_level < rows[i - 1].finest_level && rows[i].altitude_m < rows[i - 1].altitude_m).collect()
}

/// A pose the plan refused: its index, its time and the reason. A replay goes on past it (it is what a diagnosis wants to see).
pub type Refused = (usize, u64, String);

/// The summary lines of a replay: the planned poses, the refused ones with their reasons (at most 5 shown), the level drops.
pub fn summary(rows: &[PoseRow], refused: &[Refused], slowest_plan_ms: f64) -> String {
    let (lo, hi) = rows.iter().fold((usize::MAX, 0), |(lo, hi), r| (lo.min(r.nodes), hi.max(r.nodes)));
    let lo = if rows.is_empty() { 0 } else { lo };
    let finest = rows.iter().map(|r| r.finest_level).max().unwrap_or(0);
    let drops = level_drops(rows);
    let mut s = format!(
        "replay path: {} poses, {lo} to {hi} nodes, finest level {finest}, slowest plan {slowest_plan_ms:.1} ms, {} level drop(s) while descending",
        rows.len() + refused.len(),
        drops.len()
    );
    if !refused.is_empty() {
        s += &format!(
            "
  {} pose(s) refused by the plan",
            refused.len()
        );
        for (i, t_ms, why) in refused.iter().take(5) {
            s += &format!(
                "
    pose {i} (t {t_ms} ms): {why}"
            );
        }
    }
    for &i in drops.iter().take(5) {
        s += &format!(
            "\n  pose {i} (t {} ms, {:.1} m): level {} after {}",
            rows[i].t_ms,
            rows[i].altitude_m,
            rows[i].finest_level,
            rows[i - 1].finest_level
        );
    }
    s
}

pub fn replay_path(ctx: &GpuContext, o: &Options, bundle: &Path, out: &mut dyn Write) -> Result<(), String> {
    let path = o.replay_path.as_ref().ok_or("--replay-path needs a camera path file")?;
    let dir = o.out_dir.as_ref().ok_or("--replay-path needs --out-dir")?;
    let text = std::fs::read_to_string(bundle).map_err(|e| format!("reading {}: {e}", bundle.display()))?;
    if !text.starts_with("planet-terrain-repro") {
        return Err(format!("{}: --replay-path replays a terrain repro bundle (planet-terrain-repro)", bundle.display()));
    }
    let (b, world) = open_terrain_bundle(&text, bundle, o.store.as_deref())?;
    let log = PathLog::from_text(&std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if log.is_empty() {
        return Err(format!("{}: the camera path holds no poses", path.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let field = world.as_ref().map(|w| w.field()).transpose()?;
    let reference = planet_generators::height::Reference(b.request.seed);
    let height: &dyn planet_generators::height::HeightField = match &field {
        Some(f) => f.as_ref(),
        None => &reference,
    };
    let (size, cells, radius) = (b.request.size, b.request.lod.cells, b.request.lod.radius_m);
    let mut renderer = (o.replay_every > 0).then(|| TerrainRenderer::with_frames_in_flight(ctx, size, cells, 1));
    let mut index = String::from("# t_ms altitude_m nodes plan_hash finest_level plan_ms logged_frame_ms\n");
    let mut rows = Vec::new();
    let mut refused: Vec<Refused> = Vec::new();
    let mut slowest = 0.0f64;
    for (i, s) in log.samples().enumerate() {
        let req = TerrainRequest { camera: s.camera(&b.request.camera), ..b.request.clone() };
        let started = Instant::now();
        let planned = match &field {
            Some(f) => planet_frame::plan_terrain_field(&req, f.as_ref()),
            None => planet_frame::plan_terrain(&req),
        };
        let plan = match planned {
            Ok(plan) => plan,
            Err(e) => {
                index += &format!(
                    "{} {:.3} REFUSED {e}
",
                    s.t_ms, s.altitude_m
                );
                refused.push((i, s.t_ms, e.to_string()));
                continue;
            }
        };
        let plan_ms = started.elapsed().as_secs_f64() * 1000.0;
        slowest = slowest.max(plan_ms);
        let row = PoseRow {
            t_ms: s.t_ms,
            altitude_m: s.altitude_m,
            nodes: plan.nodes.len(),
            plan_hash: plan.plan_hash(),
            finest_level: plan.nodes.iter().map(|n| n.level).max().unwrap_or(0),
        };
        index += &row_line(&row, plan_ms, s.frame_ms);
        index.push('\n');
        rows.push(row);
        if let Some(r) = renderer.as_mut().filter(|_| i % o.replay_every == 0) {
            upload_missing_meshes(ctx, world.as_ref(), r, &plan, height, radius, cells)?;
            r.render(ctx, &plan, false).map_err(|e| format!("pose {i}: {e}"))?;
            let png = dir.join(format!("pose_{i:04}.png"));
            r.read_color(ctx).write_png(&png).map_err(|e| format!("writing {}: {e}", png.display()))?;
        }
    }
    let index_path = dir.join("replay.txt");
    std::fs::write(&index_path, &index).map_err(|e| format!("writing {}: {e}", index_path.display()))?;
    writeln!(out, "{}", summary(&rows, &refused, slowest)).map_err(err)?;
    writeln!(out, "wrote {} (every {} pose(s) as pose_NNNN.png)", index_path.display(), o.replay_every).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(t_ms: u64, altitude_m: f64, nodes: usize, finest_level: u8) -> PoseRow {
        PoseRow { t_ms, altitude_m, nodes, plan_hash: 0xabc, finest_level }
    }

    // spec: VIEW-026
    #[test]
    fn a_level_that_falls_while_the_camera_descends_is_found_and_a_climb_or_a_rise_is_not() {
        let rows = [
            row(0, 900.0, 100, 18),
            row(100, 800.0, 110, 19),
            row(200, 700.0, 90, 17), // lower and coarser: a drop
            row(300, 750.0, 90, 16), // coarser but higher: the camera climbed
            row(400, 700.0, 95, 16),
        ];
        assert_eq!(level_drops(&rows), vec![2]);
        let s = summary(&rows, &[], 3.25);
        assert!(s.starts_with("replay path: 5 poses, 90 to 110 nodes, finest level 19, slowest plan 3.2 ms, 1 level drop(s)"), "{s}");
        assert!(s.contains("pose 2 (t 200 ms, 700.0 m): level 17 after 19"), "{s}");
        let refused = vec![(5, 500, "invalid LOD parameters: morph interval is empty".to_string())];
        let s = summary(&rows, &refused, 3.25);
        assert!(s.starts_with("replay path: 6 poses, "), "refused poses count as poses: {s}");
        assert!(
            s.contains(
                "1 pose(s) refused by the plan
    pose 5 (t 500 ms): invalid LOD parameters"
            ),
            "{s}"
        );
        assert_eq!(row_line(&rows[0], 1.5, 16.0), "0 900.000 100 0000000000000abc 18 1.500 16.000");
    }
}
