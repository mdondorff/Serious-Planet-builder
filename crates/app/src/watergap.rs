//! SPIKE (water-motion-gap, never merged): a scripted camera path at 80 m over a coast for the streaming driver, and a per-frame log
//! of why each node of the drawn plan has or has no water node. Switched on by the environment variable `WATER_GAP_LOG` (a file).
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::litscenes::{lit_coast, Coast};
use planet_frame::scenes::{SceneScript, StreamScene};
use planet_frame::water::NodeDiag;
use planet_frame::{Camera, TerrainPlan};
use planet_generators::height::HeightField;
use std::io::Write;

pub const ALT_M: f64 = 80.0;
const FOV_DEG: f64 = 60.0;

pub fn enabled() -> bool {
    std::env::var("WATER_GAP_LOG").is_ok()
}

fn cam(c: &Coast, radius_m: f64, level_m: f64, back_m: f64, along_m: f64, yaw_deg: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let pos_dir = (c.dir + c.heading * ((along_m - back_m) / radius_m)).normalized();
    let up = pos_dir;
    let east = c.heading.cross(up).normalized();
    let y = yaw_deg.to_radians();
    let heading = c.heading * libm::cos(y) + east * libm::sin(y);
    Camera::at_surface_point(
        PlanetFixed(pos_dir * (radius_m + level_m + ALT_M)),
        heading,
        pitch_deg.to_radians(),
        FOV_DEG.to_radians(),
        aspect,
        0.1,
    )
}

/// A camera `back_m` out to sea, `shore_m` along the shore (east of the coastal point), yawed.
fn cam_shore(c: &Coast, radius_m: f64, level_m: f64, back_m: f64, shore_m: f64, yaw_deg: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let up0 = c.dir;
    let east = c.heading.cross(up0).normalized();
    let pos_dir = (c.dir - c.heading * (back_m / radius_m) + east * (shore_m / radius_m)).normalized();
    let up = pos_dir;
    let east2 = c.heading.cross(up).normalized();
    let y = yaw_deg.to_radians();
    let heading = c.heading * libm::cos(y) + east2 * libm::sin(y);
    Camera::at_surface_point(
        PlanetFixed(pos_dir * (radius_m + level_m + ALT_M)),
        heading,
        pitch_deg.to_radians(),
        FOV_DEG.to_radians(),
        aspect,
        0.1,
    )
}

/// SPIKE-17: the owner's descent: from 100 km over the sea 20 km off the coast down to 60 m, the altitude falling by e^-k (k 0.255 per
/// second: 100 km to 100 m in 27 s, the video's 0:26 to 0:53), the camera moving 5 % of its altitude per second towards the coast,
/// pitched 20 degrees down, then 5 s of hold.
pub fn descent_script(radius_m: f64, aspect: f64, level_m: f64, c: &Coast, scene: StreamScene) -> SceneScript {
    let k: f64 = std::env::var("WATER_GAP_RATE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.255);
    let mut cameras = Vec::new();
    let (mut alt, mut back) = (100_000.0f64, std::env::var("WATER_GAP_BACK").ok().and_then(|v| v.parse().ok()).unwrap_or(6_000.0f64));
    let dt = 1.0 / 60.0;
    for _ in 0..120 {
        cameras.push(cam_at(c, radius_m, level_m, alt, back, 20.0, aspect));
    }
    while alt > 60.0 {
        back -= std::env::var("WATER_GAP_DRIFT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.01f64) * alt * dt;
        alt *= libm::exp(-k * dt);
        cameras.push(cam_at(c, radius_m, level_m, alt, back, 20.0, aspect));
    }
    for _ in 0..300 {
        cameras.push(cam_at(c, radius_m, level_m, alt, back, 20.0, aspect));
    }
    let prelude = Some(cameras[0]);
    SceneScript { scene, prelude, cameras, descent_frames: 0 }
}

fn cam_at(c: &Coast, radius_m: f64, level_m: f64, alt_m: f64, back_m: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let pos_dir = (c.dir - c.heading * (back_m / radius_m)).normalized();
    Camera::at_surface_point(PlanetFixed(pos_dir * (radius_m + level_m + alt_m)), c.heading, pitch_deg.to_radians(), FOV_DEG.to_radians(), aspect, 0.1)
}

/// The path of the viewer's free camera at 80 m (its speed is about 200 m/s there): 120 still frames, a 10 s yaw sweep of +-90 degrees,
/// 20 s along the shore at SPEED m/s (tiles stream behind it), 10 s of sweep while gliding back, 400 m out to sea.
pub fn script(
    radius_m: f64,
    aspect: f64,
    field: &dyn HeightField,
    level_m: f64,
    is_sea: &dyn Fn(Vec3) -> bool,
    scene: StreamScene,
) -> Result<SceneScript, String> {
    let coast = lit_coast(radius_m, field, level_m, is_sea)?;
    if std::env::var("COARSEN_APPROACH").is_ok() {
        return Ok(approach_script(radius_m, aspect, level_m, &coast, scene));
    }
    if std::env::var("WATER_GAP_DESCENT").is_ok() {
        return Ok(descent_script(radius_m, aspect, level_m, &coast, scene));
    }
    let speed: f64 = std::env::var("WATER_GAP_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(150.0);
    let back = 400.0;
    let pitch = 12.0;
    let mut cameras = Vec::new();
    let at = |shore: f64, yaw: f64| cam_shore(&coast, radius_m, level_m, back, shore, yaw, pitch, aspect);
    for _ in 0..120 {
        cameras.push(at(0.0, 0.0));
    }
    for k in 0..600 {
        let t = k as f64 / 60.0;
        cameras.push(at(0.0, 90.0 * libm::sin(std::f64::consts::TAU * t / 10.0)));
    }
    for k in 0..1200 {
        cameras.push(at(speed * k as f64 / 60.0, 0.0));
    }
    let far = speed * 20.0;
    for k in 0..600 {
        let t = k as f64 / 60.0;
        cameras.push(at(far - speed * t, 90.0 * libm::sin(std::f64::consts::TAU * t / 10.0)));
    }
    let prelude = Some(cameras[0]);
    Ok(SceneScript { scene, prelude, cameras, descent_frames: 0 })
}

pub struct GapLog {
    out: std::fs::File,
    pub shots: usize,
    mins: std::collections::HashMap<TileId, f64>,
    pub dir: std::path::PathBuf,
}

impl GapLog {
    pub fn open() -> Option<GapLog> {
        let p = std::env::var("WATER_GAP_LOG").ok()?;
        let dir = std::path::PathBuf::from(&p).with_extension("shots");
        std::fs::create_dir_all(&dir).ok()?;
        Some(GapLog { out: std::fs::File::create(&p).ok()?, shots: 0, mins: Default::default(), dir })
    }

    /// The lowest terrain height over the tile's 9x9 sample directions.
    fn true_min(field: &dyn HeightField, tile: TileId) -> f64 {
        let map = planet_core::cube::TangentWarp;
        let n = 8;
        let mut m = f64::INFINITY;
        for l in 0..=n {
            for k in 0..=n {
                m = m.min(field.height(tile.sample_direction(&map, n, k, l)));
            }
        }
        m
    }

    /// One line per frame; returns the number of gap nodes (a node under the sea level that has no water node).
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        k: u64,
        kind: &str,
        substituted: usize,
        blank: usize,
        base_seq: Option<u64>,
        frame_no: u64,
        req_cam: &Camera,
        plan: &TerrainPlan,
        diag: &[NodeDiag],
        field: &dyn HeightField,
        level_m: f64,
    ) -> usize {
        let water_nodes = plan.water.as_ref().map_or(0, |w| w.nodes.len());
        let alt = req_cam.position.0.length() - 6_371_000.0 - level_m;
        let mut gaps = Vec::new();
        let mut reasons = std::collections::BTreeMap::<&str, usize>::new();
        let cpos = req_cam.position.0;
        for d in diag {
            *reasons.entry(d.reason).or_default() += 1;
            if d.reason == "kept" {
                continue;
            }
            let node = &plan.nodes[d.node as usize];
            if (node.origin.0 - cpos).length() > (4.0 * alt).max(20_000.0) {
                continue;
            }
            let tm = *self.mins.entry(d.tile).or_insert_with(|| Self::true_min(field, d.tile));
            if tm < level_m - 0.1 {
                let node = &plan.nodes[d.node as usize];
                let dist = (node.origin.0 - cpos).length();
                gaps.push((d.tile, d.reason, tm, d.lowest_bound, d.exact_min, dist));
            }
        }
        let cam_same = plan.camera.position.0 == cpos;
        let mut line = format!(
            "frame {k} alt {alt:.0} bevy {frame_no} kind {kind} subst {substituted} blank {blank} base_seq {base_seq:?} cam_same {cam_same} nodes {} water {water_nodes} reasons {reasons:?} gaps {}",
            plan.nodes.len(),
            gaps.len()
        );
        for (t, r, tm, lo, ex, dist) in &gaps {
            line += &format!(" | L{} {:?} {r} truemin {tm:.1} lowest {lo:.1} exact {ex:?} dist {dist:.0}", t.level(), t);
        }
        let _ = writeln!(self.out, "{line}");
        if k % 40 == 1 {
            let mut near: Vec<(f64, String)> = Vec::new();
            for d in diag {
                let node = &plan.nodes[d.node as usize];
                let dist = (node.origin.0 - cpos).length();
                if dist < 1500.0 {
                    let tm = *self.mins.entry(d.tile).or_insert_with(|| Self::true_min(field, d.tile));
                    near.push((dist, format!("   near L{} dist {dist:.0} {} truemin {tm:.1} lowest {:.1} exact {:?}", d.tile.level(), d.reason, d.lowest_bound, d.exact_min)));
                }
            }
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, t) in near.iter().take(40) {
                let _ = writeln!(self.out, "{t}");
            }
        }
        gaps.len()
    }
}

/// SPIKE-18: where the approach looks (read by the coarsening detector to add probes around it).
pub static TARGET: std::sync::Mutex<Option<Vec3>> = std::sync::Mutex::new(None);

/// SPIKE-18: a slow approach from orbit straight down to the coast point: the altitude falls by e^(-k t) (k `COARSEN_RATE`, default
/// 0.08 per second) from `COARSEN_START_KM` (20,000) to `COARSEN_END_M` (3,000), the camera looking `COARSEN_PITCH` degrees down
/// (default -75) along the coast's heading, then 3 s of hold.
pub fn approach_script(radius_m: f64, aspect: f64, level_m: f64, c: &Coast, scene: StreamScene) -> SceneScript {
    let env = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let k = env("COARSEN_RATE", 0.08);
    let (mut alt, end) = (env("COARSEN_START_KM", 20_000.0) * 1000.0, env("COARSEN_END_M", 3_000.0));
    let pitch = env("COARSEN_PITCH", 75.0).abs();
    let yaw_rate = env("COARSEN_YAW", 0.0);
    let mut yaw = 0.0f64;
    *TARGET.lock().unwrap() = Some(c.dir);
    let mut cameras = Vec::new();
    let dt = 1.0 / 60.0;
    for _ in 0..60 {
        cameras.push(cam_at(c, radius_m, level_m, alt, 0.0, pitch, aspect));
    }
    while alt > end {
        alt *= libm::exp(-k * dt);
        cameras.push(cam_yawed(c, radius_m, level_m, alt, { yaw += yaw_rate * dt; yaw }, pitch, aspect));
    }
    for _ in 0..180 {
        cameras.push(cam_at(c, radius_m, level_m, alt, 0.0, pitch, aspect));
    }
    let prelude = Some(cameras[0]);
    SceneScript { scene, prelude, cameras, descent_frames: 0 }
}

fn cam_yawed(c: &Coast, radius_m: f64, level_m: f64, alt_m: f64, yaw_deg: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let up = c.dir;
    let east = c.heading.cross(up).normalized();
    let y = yaw_deg.to_radians();
    let heading = c.heading * libm::cos(y) + east * libm::sin(y);
    Camera::at_surface_point(PlanetFixed(up * (radius_m + level_m + alt_m)), heading, pitch_deg.to_radians(), FOV_DEG.to_radians(), aspect, 0.1)
}
