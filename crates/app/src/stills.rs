//! The pose stills (TEST-048): `test-render --scene capture-stills --poses FILE --out-dir DIR`, the lit path drawn at fixed poses over a
//! world with its A-net chunk stage, for the owner's look reviews (M8-4, M8-8) and later regression checks.
//!
//! A still waits for its chunks: the loop plans the pose, asks the chunk manager for every chunk the plan's tiles read, and plans again
//! over the field that includes what landed until every tile of the plan reads only ready chunks. So no tile of the image is a
//! stand-in; when the chunks are not all ready within the timeout the run fails and names what was missing.

use crate::litperf::{capture_lod, new_renderer, request, LitSetup, Rig, WATER_T0};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::chunkshare::ChunkShare;
use planet_frame::{Camera, SelectionCache};
use planet_generators::chunkfield::ChunkKey;
use planet_render::GpuContext;
use planet_streaming::chunks::ChunkConfig;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Whether `--scene NAME` names the pose stills.
pub fn is_stills_scene(name: &str) -> bool {
    name == "capture-stills"
}

/// Rounds of settling the chunks and moving the eye to the ground they give that a pose may take.
const MAX_CAMERA_ROUNDS: usize = 4;

/// Seconds a pose may wait for its chunks when the file gives no `chunk_timeout_s`.
pub const DEFAULT_CHUNK_TIMEOUT_S: f64 = 900.0;

/// Where the poses look: the largest river of the world's coupled run, or a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// The land cell of the coupled run at least this many metres high with the largest drainage area that drains on (0: the cell of the
    /// chunk scenes, which can be a river mouth over the sea; a higher bound puts the anchor inland on the river).
    LargestRiver { min_height_m: f64 },
    /// Latitude and longitude in degrees.
    LatLon(f64, f64),
}

/// One still: the eye `back_m` before the anchor against the heading, `altitude_m` above the ground there, looking along the heading
/// pitched down by `pitch_deg`, with the sun at `sun_local_hours` of local solar time at the anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub name: String,
    pub altitude_m: f64,
    pub back_m: f64,
    /// Degrees clockwise from north.
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub sun_local_hours: f64,
    pub sun_declination_deg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StillsConfig {
    pub anchor: Anchor,
    pub chunk_timeout_s: f64,
    /// The cell of the chunks in metres (the viewer's 8 m unless the file says otherwise; a test uses coarse chunks to stay quick).
    pub chunk_cell_m: f64,
    pub poses: Vec<Pose>,
}

/// A value as the file wrote it, for a message.
fn show(v: &toml::Value) -> String {
    v.as_str().map_or_else(|| format!("{v:?}"), |s| format!("{s:?}"))
}

fn number(t: &toml::Table, key: &str, whose: &str) -> Result<Option<f64>, String> {
    match t.get(key) {
        None => Ok(None),
        Some(v) => match v.as_float().or_else(|| v.as_integer().map(|i| i as f64)) {
            Some(x) if x.is_finite() => Ok(Some(x)),
            _ => Err(format!("{whose}: '{key}' must be a finite number, got {}", show(v))),
        },
    }
}

fn required(t: &toml::Table, key: &str, whose: &str) -> Result<f64, String> {
    number(t, key, whose)?.ok_or_else(|| format!("{whose}: '{key}' is missing"))
}

/// Read the poses file: `[place]` (`anchor = "largest-river"`, or `lat` and `lon` in degrees), an optional `chunk_timeout_s`, and one
/// `[[pose]]` per still with `name`, `altitude_m`, `back_m`, `heading_deg`, `pitch_deg` and `sun_local_hours` (`sun_declination_deg`
/// defaults to 0). A bad file is refused naming the key and the value.
pub fn parse_config(text: &str) -> Result<StillsConfig, String> {
    let root: toml::Table = text.parse().map_err(|e| format!("the poses file: {e}"))?;
    let place = root.get("place").and_then(|v| v.as_table()).ok_or("the poses file: [place] is missing")?;
    let anchor = match (place.get("anchor"), place.get("lat"), place.get("lon")) {
        (Some(a), None, None) if a.as_str() == Some("largest-river") => {
            Anchor::LargestRiver { min_height_m: number(place, "min_height_m", "[place]")?.unwrap_or(0.0) }
        }
        (Some(a), None, None) => {
            return Err(format!("[place]: anchor {} is unknown (known: \"largest-river\"; or give lat and lon)", show(a)))
        }
        (None, Some(_), Some(_)) => {
            let (lat, lon) = (required(place, "lat", "[place]")?, required(place, "lon", "[place]")?);
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return Err(format!("[place]: lat {lat} and lon {lon} must be within 90 and 180 degrees"));
            }
            Anchor::LatLon(lat, lon)
        }
        _ => return Err("[place]: give anchor = \"largest-river\", or lat and lon".into()),
    };
    let chunk_timeout_s = number(&root, "chunk_timeout_s", "the poses file")?.unwrap_or(DEFAULT_CHUNK_TIMEOUT_S);
    if chunk_timeout_s <= 0.0 {
        return Err(format!("chunk_timeout_s must be positive, got {chunk_timeout_s}"));
    }
    let chunk_cell_m = number(&root, "chunk_cell_m", "the poses file")?.unwrap_or(crate::anetworld::ANET_CELL_M);
    if chunk_cell_m <= 0.0 {
        return Err(format!("chunk_cell_m must be positive, got {chunk_cell_m}"));
    }
    let list = root.get("pose").and_then(|v| v.as_array()).ok_or("the poses file: no [[pose]]")?;
    let mut poses: Vec<Pose> = Vec::new();
    for (i, v) in list.iter().enumerate() {
        let t = v.as_table().ok_or_else(|| format!("[[pose]] {}: not a table", i + 1))?;
        let name = t.get("name").and_then(|n| n.as_str()).ok_or_else(|| format!("[[pose]] {}: 'name' is missing", i + 1))?.to_string();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
            return Err(format!("[[pose]] {}: name '{name}' must be letters, digits, '-', '_' or '.' (it names the PNG)", i + 1));
        }
        if poses.iter().any(|p| p.name == name) {
            return Err(format!("[[pose]] '{name}' is given twice"));
        }
        let whose = format!("pose '{name}'");
        let pose = Pose {
            altitude_m: required(t, "altitude_m", &whose)?,
            back_m: required(t, "back_m", &whose)?,
            heading_deg: required(t, "heading_deg", &whose)?,
            pitch_deg: required(t, "pitch_deg", &whose)?,
            sun_local_hours: required(t, "sun_local_hours", &whose)?,
            sun_declination_deg: number(t, "sun_declination_deg", &whose)?.unwrap_or(0.0),
            name,
        };
        if pose.altitude_m <= 0.0 || pose.back_m < 0.0 {
            return Err(format!(
                "{whose}: altitude_m must be positive and back_m not negative, got {} and {}",
                pose.altitude_m, pose.back_m
            ));
        }
        if !(-90.0..=90.0).contains(&pose.pitch_deg) {
            return Err(format!("{whose}: pitch_deg {} must be within 90 degrees", pose.pitch_deg));
        }
        poses.push(pose);
    }
    if poses.is_empty() {
        return Err("the poses file: no [[pose]]".into());
    }
    Ok(StillsConfig { anchor, chunk_timeout_s, chunk_cell_m, poses })
}

/// The unit vector towards the sun when it stands at `local_hours` of local solar time (12 is noon) at longitude `lon_deg`, with
/// the declination `declination_deg`: the planet-fixed axes of COORD, through [`planet_frame::look::shading::sun_direction`].
pub fn sun_for(local_hours: f64, lon_deg: f64, declination_deg: f64) -> [f32; 3] {
    // The sub-solar longitude of the shading helper is -15 degrees per hour from noon at longitude 0; local time T at longitude L is the
    // time at which the sun is (T - 12) * 15 degrees west of L.
    planet_frame::look::shading::sun_direction(local_hours - lon_deg / 15.0, declination_deg)
}

/// The unit direction at latitude and longitude in degrees.
fn direction_of(lat_deg: f64, lon_deg: f64) -> Vec3 {
    Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians())
}

/// The unit tangent at `at` pointing `heading_deg` clockwise from north.
fn heading_vector(at: Vec3, heading_deg: f64) -> Vec3 {
    let (lat, lon) = at.lat_lon();
    let north = Vec3::new(-libm::sin(lat) * libm::cos(lon), -libm::sin(lat) * libm::sin(lon), libm::cos(lat));
    let east = Vec3::new(-libm::sin(lon), libm::cos(lon), 0.0);
    let h = heading_deg.to_radians();
    (north * libm::cos(h) + east * libm::sin(h)).normalized()
}

/// The camera of a pose: the eye on the great circle `back_m` behind `anchor` against the heading, `altitude_m` above the ground or the
/// sea there (the higher), looking along the heading pitched down. `ground_m` is the height above the reference sphere.
pub fn pose_camera(
    anchor: Vec3,
    pose: &Pose,
    ground_m: &dyn Fn(Vec3) -> f64,
    radius_m: f64,
    level_m: f64,
    aspect: f64,
    fov_y_rad: f64,
) -> Camera {
    let heading = heading_vector(anchor, pose.heading_deg);
    let theta = pose.back_m / radius_m;
    let eye_dir = (anchor * libm::cos(theta) - heading * libm::sin(theta)).normalized();
    let eye_alt = ground_m(eye_dir).max(level_m) + pose.altitude_m;
    // The heading carried to the eye: the tangent at the eye along the great circle through the anchor.
    let ahead = (anchor - eye_dir * anchor.dot(eye_dir)).normalized();
    let ahead = if pose.back_m > 0.0 { ahead } else { heading };
    let near = (pose.altitude_m * 0.05).clamp(0.05, 0.1);
    Camera::at_surface_point(PlanetFixed(eye_dir * (radius_m + eye_alt)), ahead, pose.pitch_deg.to_radians(), fov_y_rad, aspect, near)
}

/// The chunk keys the tiles of a plan read.
fn needed(world: &WorldTerrain, tiles: &[TileId]) -> Vec<ChunkKey> {
    let Some(m) = world.chunk_manager() else { return Vec::new() };
    let mut seen = HashSet::new();
    tiles.iter().flat_map(|&t| m.reads(t)).filter(|k| seen.insert(*k)).collect()
}

/// A still waited too long for its chunks.
fn not_ready(pose: &str, secs: f64, missing: &[ChunkKey], needed_n: usize, in_flight: usize) -> String {
    let some: Vec<String> = missing.iter().take(4).map(|k| format!("face {} x {} y {}", k.0, k.1, k.2)).collect();
    format!(
        "pose '{pose}': {} of {needed_n} chunks were not ready after {secs:.0} s ({in_flight} jobs in flight; first missing: {}); raise chunk_timeout_s, or check the chunk cache's byte cap",
        missing.len(),
        some.join(", ")
    )
}

/// The result of waiting for one pose: the field is rebuilt by the caller from the world; the number of rounds and the chunks it read.
struct Settled {
    rounds: usize,
    chunks: usize,
    waited_s: f64,
}

/// Plan `camera` until every tile of the plan reads only chunks that were ready before the plan's field was built, driving the chunk
/// jobs nearest first. Fails with [`not_ready`] after `timeout_s`.
fn settle(
    o: &Options,
    ctx: &GpuContext,
    world: &WorldTerrain,
    pose: &Pose,
    camera: Camera,
    size: (u32, u32),
    timeout_s: f64,
) -> Result<Settled, String> {
    let manager = world.chunk_manager().ok_or("the world has no chunk stage")?;
    let started = Instant::now();
    let mut wanted: HashSet<ChunkKey> = HashSet::new();
    let mut rounds = 0;
    loop {
        rounds += 1;
        // Ready is monotonic while the chunks are wanted, so what is ready now is in the snapshot of the field built after this line.
        let ready: HashSet<ChunkKey> = wanted.iter().copied().filter(|k| manager.is_ready(*k)).collect();
        let field = world.field()?;
        let lod = capture_lod(o, size, world.radius_m(), field.as_ref())?;
        let rig = Rig {
            ctx,
            world,
            field: field.as_ref(),
            lod: &lod,
            size,
            smoke: true,
            calibrate: false,
            frames: 0,
            cells: o.cells,
            radius_m: world.radius_m(),
        };
        let plan = planet_frame::plan_terrain_cached(&request(&rig, camera), field.as_ref(), &mut SelectionCache::new())
            .map_err(|e| format!("pose '{}': {e}", pose.name))?;
        let tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
        let reads = needed(world, &tiles);
        let missing: Vec<ChunkKey> = reads.iter().copied().filter(|k| !ready.contains(k)).collect();
        if missing.is_empty() {
            return Ok(Settled { rounds, chunks: reads.len(), waited_s: started.elapsed().as_secs_f64() });
        }
        wanted.extend(reads.iter().copied());
        drop(plan);
        drop(field);
        world.chunk_frame(&tiles, camera.position, &[]);
        let waited = started.elapsed().as_secs_f64();
        if waited > timeout_s {
            let still: Vec<ChunkKey> = missing.into_iter().filter(|k| !manager.is_ready(*k)).collect();
            return Err(not_ready(&pose.name, waited, &still, reads.len(), manager.in_flight()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Wait for the coupled run of the world (made and stored if the store holds none) and give the world its A-net chunk stage.
fn enable_chunks(
    world: &Arc<WorldTerrain>,
    cell_m: f64,
    workers: usize,
    out: &mut dyn Write,
) -> Result<Arc<planet_generators::coupled::CoupledMaps>, String> {
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
    let pool = planet_streaming::GenWorkers::new(workers).pool();
    let cfg = ChunkConfig { workers, evict_ms: 3_600_000, cap_bytes: 4 << 30 };
    world.enable_anet(maps.clone(), cell_m, pool, Arc::new(planet_streaming::SystemClock::new()), cfg)?;
    Ok(maps)
}

/// The direction of the anchor and, for a river cell, the bearing of the river's flow there (degrees clockwise from north), so the poses can
/// be given along the river.
fn anchor_direction(anchor: Anchor, maps: &planet_generators::coupled::CoupledMaps) -> Result<(Vec3, Option<f64>), String> {
    match anchor {
        Anchor::LatLon(lat, lon) => Ok((direction_of(lat, lon), None)),
        Anchor::LargestRiver { min_height_m } => {
            let rec = maps.receivers();
            let mut best: Option<(usize, f32)> = None;
            for (c, &r) in rec.iter().enumerate() {
                let a = maps.network_area_m2(c);
                if f64::from(maps.z(c)) > min_height_m.max(0.0) && r != planet_generators::coupled::NONE && best.is_none_or(|(_, b)| a > b)
                {
                    best = Some((c, a));
                }
            }
            let (cell, _) = best.ok_or_else(|| format!("the coupled run has no land river at least {min_height_m} m high"))?;
            let lat = maps.lattice();
            let here = lat.direction(cell);
            let there = lat.direction(rec[cell] as usize);
            let (north, east) = (heading_vector(here, 0.0), heading_vector(here, 90.0));
            let bearing = libm::atan2(there.dot(east), there.dot(north)).to_degrees().rem_euclid(360.0);
            Ok((here, Some(bearing)))
        }
    }
}

/// `test-render --scene capture-stills --poses FILE --out-dir DIR [--store DIR | --allow-memory-bake --global-res N] [--size WxH]
/// [--views A,B]` (TEST-048): one PNG per pose, `DIR/<pose>.png`, and `DIR/stills.txt` with the camera and sun of each. A tool beside
/// the harness, never a measurement: no gate, no timestamps, no log; not to be run during a perf window.
pub fn capture_stills(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.out_dir.clone().ok_or("capture-stills needs --out-dir")?;
    let file = o.poses.clone().ok_or("capture-stills needs --poses FILE")?;
    let text = std::fs::read_to_string(&file).map_err(|e| format!("--poses {}: {e}", file.display()))?;
    let mut config = parse_config(&text).map_err(|e| format!("--poses {}: {e}", file.display()))?;
    if let Some(names) = &o.views {
        if let Some(bad) = names.iter().find(|n| !config.poses.iter().any(|p| &p.name == *n)) {
            let known: Vec<&str> = config.poses.iter().map(|p| p.name.as_str()).collect();
            return Err(format!("--views: no pose '{bad}' (the poses: {})", known.join(", ")));
        }
        config.poses.retain(|p| names.contains(&p.name));
    }
    let world = Arc::new(crate::streamperf::open_world(o, false)?.bounding_water());
    if !world.has_bake() {
        return Err("the stills need a world with a bake (--store DIR, or --allow-memory-bake with --global-res N): the chunk stage reads the coupled run".into());
    }
    let size = o.size.unwrap_or((2560, 1440));
    let aspect = f64::from(size.0) / f64::from(size.1);
    let radius_m = world.radius_m();
    writeln!(
        out,
        "capture-stills: NOT a measurement (no gate, no timestamps, no log); {}x{}, {} poses, {}{}",
        size.0,
        size.1,
        config.poses.len(),
        world.describe(),
        if o.store.is_some() { "" } else { " (in-memory bake)" }
    )
    .map_err(err)?;
    let workers = o.gen_workers().workers.max(1);
    let maps = enable_chunks(&world, config.chunk_cell_m, workers, out)?;
    let (anchor, flow) = anchor_direction(config.anchor, &maps)?;
    let (lat, lon) = anchor.lat_lon();
    let ground = world.field()?.height(anchor);
    writeln!(
        out,
        "anchor: latitude {:.4} longitude {:.4} degrees, ground {ground:.1} m{}",
        lat.to_degrees(),
        lon.to_degrees(),
        flow.map_or(String::new(), |b| format!(", the river flows towards bearing {b:.0} degrees"))
    )
    .map_err(err)?;
    let level_m = world.definition().sea(planet_world_def::BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
    std::fs::create_dir_all(&dir).map_err(err)?;
    let mut index =
        String::from("# pose width height eye_lat_deg eye_lon_deg eye_height_m heading_deg pitch_deg sun_local_hours chunks waited_s\n");
    for pose in &config.poses {
        let started = Instant::now();
        // The eye is `altitude_m` above the ground the chunks give, so the camera follows the field as chunks land: settle, move the
        // camera to the new ground, and settle again until the camera stays.
        let camera_over = |world: &WorldTerrain| -> Result<Camera, String> {
            let field = world.field()?;
            let fov = planet_frame::litscenes::FOV_DEG.to_radians();
            Ok(pose_camera(anchor, pose, &|d| field.height(d), radius_m, level_m, aspect, fov))
        };
        let mut camera = camera_over(&world)?;
        let mut settled = settle(o, ctx, &world, pose, camera, size, config.chunk_timeout_s)?;
        for attempt in 1.. {
            let next = camera_over(&world)?;
            if next.position == camera.position {
                break;
            }
            if attempt > MAX_CAMERA_ROUNDS {
                return Err(format!(
                    "pose '{}': the eye did not stay over the ground after {MAX_CAMERA_ROUNDS} rounds of chunks",
                    pose.name
                ));
            }
            camera = next;
            let again = settle(o, ctx, &world, pose, camera, size, config.chunk_timeout_s)?;
            settled = Settled { rounds: settled.rounds + again.rounds, chunks: again.chunks, waited_s: settled.waited_s + again.waited_s };
        }
        // The field of the settled round includes every chunk the plan reads.
        let field = world.field()?;
        let lod = capture_lod(o, size, radius_m, field.as_ref())?;
        let mut setup = LitSetup::new(o, &world, &lod)?;
        let rig = Rig {
            ctx,
            world: &world,
            field: field.as_ref(),
            lod: &lod,
            size,
            smoke: true,
            calibrate: false,
            frames: 0,
            cells: o.cells,
            radius_m,
        };
        let plan =
            planet_frame::plan_terrain_cached(&request(&rig, camera), rig.field, &mut SelectionCache::new()).map_err(|e| e.to_string())?;
        let tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
        let missing = needed(&world, &tiles).into_iter().filter(|k| !world.chunk_manager().is_some_and(|m| m.is_ready(*k))).count();
        if missing > 0 {
            return Err(format!(
                "pose '{}': {missing} chunks left the cache between the plan and the draw (the byte cap is too small)",
                pose.name
            ));
        }
        let share = world.chunk_view().map(|v| ChunkShare::of(tiles.iter().copied(), v.as_ref()));
        if let Some(s) = share.filter(|s| s.chunk_stand_in > 0) {
            return Err(format!("pose '{}': {} of {} chunk nodes are stand-in", pose.name, s.chunk_stand_in, s.chunk_nodes));
        }
        let mut r = new_renderer(&rig, &setup, camera)?;
        crate::skeleton::upload_missing_meshes(ctx, Some(&world), &mut r, &plan, rig.field, radius_m, o.cells)?;
        let (_, eye_lon) = camera.position.0.lat_lon();
        let sun = sun_for(pose.sun_local_hours, eye_lon.to_degrees(), pose.sun_declination_deg);
        // The exposure adapts over a few frames of half a second, the first snapping to its target.
        for k in 0..4 {
            let mut lit = setup.plan(&world, rig.field, &plan, WATER_T0 + k as f64 * 0.5, 0.5, k == 0)?;
            if let Some(l) = lit.lit.as_mut() {
                l.sun_dir = sun;
            }
            r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
        }
        r.read_color(ctx).write_png(&dir.join(format!("{}.png", pose.name))).map_err(|e| format!("writing {}: {e}", pose.name))?;
        let (eye_lat, _) = camera.position.0.lat_lon();
        let height = camera.position.0.length() - radius_m;
        writeln!(
            out,
            "{}: {} chunks after {} rounds and {:.1} s, drawn in {:.1} s -> {}",
            pose.name,
            settled.chunks,
            settled.rounds,
            settled.waited_s,
            started.elapsed().as_secs_f64(),
            dir.join(format!("{}.png", pose.name)).display()
        )
        .map_err(err)?;
        index += &format!(
            "{} {} {} {:.5} {:.5} {:.1} {} {} {} {} {:.1}\n",
            pose.name,
            size.0,
            size.1,
            eye_lat.to_degrees(),
            eye_lon.to_degrees(),
            height,
            pose.heading_deg,
            pose.pitch_deg,
            pose.sun_local_hours,
            settled.chunks,
            settled.waited_s
        );
    }
    std::fs::write(dir.join("stills.txt"), index).map_err(err)?;
    writeln!(out, "wrote {} (one PNG per pose and stills.txt)", dir.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[place]
anchor = "largest-river"

[[pose]]
name = "300m-sunset"
altitude_m = 300
back_m = 1200.0
heading_deg = 90
pitch_deg = 12
sun_local_hours = 17.5

[[pose]]
name = "1.8m-midday"
altitude_m = 1.8
back_m = 0
heading_deg = 0
pitch_deg = 0
sun_local_hours = 12
sun_declination_deg = 10
"#;

    // spec: TEST-050
    #[test]
    fn the_poses_file_is_read_with_its_defaults_and_a_bad_one_names_the_key() {
        let c = parse_config(GOOD).unwrap();
        assert_eq!(c.anchor, Anchor::LargestRiver { min_height_m: 0.0 });
        assert_eq!(c.chunk_timeout_s, DEFAULT_CHUNK_TIMEOUT_S);
        assert_eq!(c.chunk_cell_m, 8.0);
        assert_eq!(c.poses.len(), 2);
        assert_eq!(c.poses[0].sun_declination_deg, 0.0);
        assert_eq!(c.poses[1].sun_declination_deg, 10.0);
        let e = |s: &str| parse_config(s).unwrap_err();
        assert!(e(&GOOD.replace("heading_deg = 90", "heading_deg = \"east\"")).contains("'heading_deg' must be a finite number"));
        assert!(e(&GOOD.replace("pitch_deg = 12", "")).contains("'pitch_deg' is missing"));
        assert!(e(&GOOD.replace("1.8m-midday", "300m-sunset")).contains("given twice"));
        assert!(e(&GOOD.replace("1.8m-midday", "a b")).contains("must be letters"));
        assert!(e(&GOOD.replace("anchor = \"largest-river\"", "anchor = \"lake\"")).contains("anchor \"lake\" is unknown"));
        assert!(e(&GOOD.replace("anchor = \"largest-river\"", "lat = 95\nlon = 0")).contains("within 90 and 180"));
        assert!(e("[place]\nanchor = \"largest-river\"\n").contains("no [[pose]]"));
        assert!(parse_config(&GOOD.replace("anchor = \"largest-river\"", "lat = 12.5\nlon = -40")).is_ok());
    }

    // spec: TEST-048
    #[test]
    fn the_sun_stands_where_local_time_puts_it() {
        for lon in [-120.0_f64, 0.0, 77.0] {
            // Noon: the sun is overhead at the equator, straight along the anchor's direction.
            let noon = sun_for(12.0, lon, 0.0);
            let up = direction_of(0.0, lon);
            let dot = noon[0] as f64 * up.x + noon[1] as f64 * up.y + noon[2] as f64 * up.z;
            assert!(dot > 0.999_999, "noon at {lon}: {dot}");
            // 18:00 is a quarter turn west: the sun is on the horizon of the anchor, and the unit vector stays a unit vector.
            let dusk = sun_for(18.0, lon, 0.0);
            let dot = dusk[0] as f64 * up.x + dusk[1] as f64 * up.y + dusk[2] as f64 * up.z;
            assert!(dot.abs() < 1e-6, "18:00 at {lon}: {dot}");
            let west = Vec3::new(libm::sin(lon.to_radians()), -libm::cos(lon.to_radians()), 0.0);
            assert!(dusk[0] as f64 * west.x + dusk[1] as f64 * west.y > 0.999_999, "the sun sets in the west");
        }
    }

    // spec: TEST-048
    #[test]
    fn a_pose_puts_the_eye_behind_the_anchor_at_the_altitude_looking_along_the_heading() {
        let radius = 6_371_000.0;
        let anchor = direction_of(30.0, 40.0);
        let pose = Pose {
            name: "p".into(),
            altitude_m: 300.0,
            back_m: 2_000.0,
            heading_deg: 90.0,
            pitch_deg: 10.0,
            sun_local_hours: 12.0,
            sun_declination_deg: 0.0,
        };
        let ground = |d: Vec3| 100.0 + 50.0 * d.z;
        let cam = pose_camera(anchor, &pose, &ground, radius, 0.0, 16.0 / 9.0, 1.0);
        let eye = cam.position.0;
        let dir = eye.normalized();
        // 2 km behind along the great circle, 300 m over the ground there.
        let arc = libm::acos(dir.dot(anchor).clamp(-1.0, 1.0)) * radius;
        assert!((arc - 2_000.0).abs() < 0.01, "arc {arc}");
        assert!((eye.length() - radius - (ground(dir) + 300.0)).abs() < 1e-6);
        // Heading east: the anchor is east of the eye, and the forward vector is pitched 10 degrees down.
        let east = heading_vector(dir, 90.0);
        assert!(cam.forward.dot(east) > 0.98 && cam.forward.dot(dir) < -0.17 && cam.forward.dot(dir) > -0.18, "{:?}", cam.forward);
        // The sea is the floor of the ground: over deep water the eye keeps `altitude_m` above the level.
        let sea = pose_camera(anchor, &pose, &|_| -4000.0, radius, 0.0, 1.0, 1.0);
        assert!((sea.position.0.length() - radius - 300.0).abs() < 1e-6);
    }

    // spec: TEST-049
    #[test]
    fn the_wait_names_the_pose_and_the_missing_chunks() {
        let m = not_ready("300m-sunset", 900.4, &[(2, 5, 7), (2, 5, 8)], 12, 3);
        assert!(
            m.contains("pose '300m-sunset'") && m.contains("2 of 12 chunks") && m.contains("900 s") && m.contains("face 2 x 5 y 7"),
            "{m}"
        );
    }
}
