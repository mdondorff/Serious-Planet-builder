//! The M1 walking skeleton: generator -> FramePlan -> GPU executor -> pixels, driven from all three run modes.

use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, FramePlan, PlanRequest, ReproBundle};
use planet_generators::{generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
use planet_render::{execute, AdapterPolicy, ExecStats, GpuContext, RgbaFrame, TileResource};
use std::io::Write;
use std::path::Path;

const FRAME_SIZE: (u32, u32) = (256, 128);
const RADIUS_M: f64 = 6_371_000.0;

/// `FACE,LEVEL,X,Y`
pub fn parse_tile(s: &str) -> Result<TileId, String> {
    let p: Vec<&str> = s.split(',').collect();
    let [f, l, x, y] = p.as_slice() else {
        return Err(format!("--tile expects FACE,LEVEL,X,Y, got '{s}'"));
    };
    let n = |v: &str| v.trim().parse::<u32>().map_err(|e| format!("--tile '{s}': {e}"));
    TileId::new(Face(n(f)? as u8), n(l)? as u8, n(x)?, n(y)?)
        .ok_or_else(|| format!("--tile '{s}' is not a valid tile (face 0..5, level 0..28, x and y below 2^level)"))
}

fn default_tiles() -> Vec<TileId> {
    let a = TileId::new(Face(3), 3, 5, 2).expect("valid");
    vec![a, a.neighbor(&TangentWarp, Side::East)]
}

fn tiles_of(o: &Options) -> Result<Vec<TileId>, String> {
    if o.tiles.is_empty() {
        Ok(default_tiles())
    } else {
        o.tiles.iter().map(|t| parse_tile(t)).collect()
    }
}

fn request_of(o: &Options) -> Result<PlanRequest, String> {
    let view = DebugView::parse(&o.view).ok_or_else(|| format!("--view must be face, tile-id or height, got '{}'", o.view))?;
    Ok(PlanRequest {
        tiles: tiles_of(o)?,
        // A fixed camera 12,000 km from the centre; the skeleton only needs a defined origin for camera-relative values.
        camera: PlanetFixed(Vec3::new(12.0e6, 1.0e6, -3.0e6)),
        size: FRAME_SIZE,
        view,
        radius_m: RADIUS_M,
        face_mapping: TangentWarp.id().to_string(),
    })
}

fn run_plan(ctx: &GpuContext, plan: &FramePlan, tiles: &[TileData]) -> Result<(RgbaFrame, ExecStats), String> {
    let resources: Vec<TileResource> = tiles.iter().map(|t| TileResource { id: t.id, res: t.res, heights: &t.heights }).collect();
    execute(ctx, plan, &resources).map_err(|e| e.to_string())
}

fn write_frame(frame: &RgbaFrame, path: &Path, out: &mut dyn Write) -> Result<(), String> {
    frame.write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

pub fn render_tiles(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let request = request_of(o)?;
    let plan = plan_tiles(&request).map_err(|e| e.to_string())?;
    let tiles: Vec<TileData> = request.tiles.iter().map(|&id| generate_tile(o.seed, &TangentWarp, id, o.res)).collect();
    let (frame, stats) = run_plan(ctx, &plan, &tiles)?;
    writeln!(
        out,
        "plan {:016x}: {} draws, {} draw calls, {} texture bytes",
        plan.plan_hash(),
        plan.draws.len(),
        stats.draw_calls,
        stats.texture_bytes_uploaded
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        write_frame(&frame, path, out)?;
    }
    if let Some(path) = &o.bundle_out {
        let bundle = ReproBundle {
            generator_version: GENERATOR_VERSION,
            implementation_id: IMPLEMENTATION_ID.to_string(),
            seed: o.seed,
            res: o.res,
            definition_hash: 0,
            adapter: ctx.adapter_key(),
            request,
            plan,
        };
        std::fs::write(path, bundle.to_text()).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote repro bundle {}", path.display()).map_err(err)?;
    }
    Ok(())
}

/// `editor --offscreen`: the same skeleton as the viewer will run, without a window (the window arrives in M2).
pub fn render_offscreen(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let ctx = GpuContext::new(o.adapter.unwrap_or(AdapterPolicy::Software)).map_err(|e| e.to_string())?;
    writeln!(out, "adapter: {:?} {} ({:?}), key {}", ctx.info.backend, ctx.info.name, ctx.info.device_type, ctx.adapter_key())
        .map_err(err)?;
    if let Some(bundle) = &o.repro {
        return replay(&ctx, bundle, o.out.as_deref(), out);
    }
    render_tiles(&ctx, o, out)
}

/// Replay a repro bundle: refuse on version mismatch, rebuild the plan from the recorded request, compare, render.
pub fn replay(ctx: &GpuContext, path: &Path, png: Option<&Path>, out: &mut dyn Write) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let b = ReproBundle::from_text(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if b.generator_version != GENERATOR_VERSION || b.implementation_id != IMPLEMENTATION_ID {
        return Err(format!(
            "bundle was recorded with generator {} ({}), this build has {} ({}); check out the matching commit to replay it",
            b.generator_version, b.implementation_id, GENERATOR_VERSION, IMPLEMENTATION_ID
        ));
    }
    let rebuilt = plan_tiles(&b.request).map_err(|e| e.to_string())?;
    let diff = rebuilt.diff(&b.plan);
    if diff.is_empty() && rebuilt.plan_hash() != b.plan.plan_hash() {
        return Err(format!(
            "replayed plan hash {:016x} differs from the recorded {:016x} although no field differs: the plan text form changed",
            rebuilt.plan_hash(),
            b.plan.plan_hash()
        ));
    }
    if !diff.is_empty() {
        return Err(format!("replayed plan differs from the recorded plan:\n  {}", diff.join("\n  ")));
    }
    writeln!(out, "replay: plan identical ({:016x}), recorded on adapter {}", rebuilt.plan_hash(), b.adapter).map_err(err)?;
    let map = planet_frame::plan::face_mapping_by_id(&b.request.face_mapping)
        .ok_or_else(|| format!("unknown face mapping '{}'", b.request.face_mapping))?;
    let tiles: Vec<TileData> = b.request.tiles.iter().map(|&id| generate_tile(b.seed, map.as_ref(), id, b.res)).collect();
    let (frame, stats) = run_plan(ctx, &rebuilt, &tiles)?;
    writeln!(out, "replay: {} draw calls", stats.draw_calls).map_err(err)?;
    if let Some(p) = png {
        write_frame(&frame, p, out)?;
    }
    Ok(())
}

/// `generate`: headless CPU generation of one tile with its hash, height map and optionally the face net.
pub fn generate(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    if o.repro.is_some() || o.bundle_out.is_some() {
        return Err("generate has no frames: --repro and --bundle belong to test-render and editor --offscreen".into());
    }
    let id = tiles_of(o)?[0];
    let tile = generate_tile(o.seed, &TangentWarp, id, o.res);
    writeln!(
        out,
        "tile {:#018x} face {} level {} ({}, {}): content hash {:016x}, cache key {:016x}",
        id.raw(),
        id.face().0,
        id.level(),
        id.x(),
        id.y(),
        tile.content_hash(),
        planet_generators::cache_key(&TangentWarp, o.seed, id, o.res)
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        planet_generators::dump::tile_height_map(&tile).write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    if let Some(path) = &o.face_net {
        planet_generators::dump::face_net(&TangentWarp, 128, 2).write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    Ok(())
}

const TERRAIN_SIZE: (u32, u32) = (256, 192);
const TERRAIN_CELLS: u32 = 16;

/// `--scene terrain`: node selection -> TerrainPlan -> meshes -> GPU, one of the ten scripted cameras.
pub fn render_terrain(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
    let view = TerrainView::parse(&o.view)
        .ok_or_else(|| format!("--view must be one of face, tile-id, level, morph, height, normals, depth, got '{}'", o.view))?;
    let views = scripted_views(RADIUS_M, f64::from(TERRAIN_SIZE.0) / f64::from(TERRAIN_SIZE.1), o.seed);
    let index = match o.script.parse::<usize>() {
        Ok(i) => i,
        Err(_) => views
            .iter()
            .position(|(n, _)| *n == o.script)
            .ok_or_else(|| format!("--script '{}' is neither an index nor one of the scripted names", o.script))?,
    };
    let (name, camera) = *views.get(index).ok_or_else(|| format!("--script {index} is out of range (0..{})", views.len()))?;
    let lod = LodParams {
        cells: TERRAIN_CELLS,
        viewport_h_px: f64::from(TERRAIN_SIZE.1),
        tau_px: 4.0,
        max_level: 26,
        ..LodParams::earth_1080p()
    };
    let plan = plan_terrain(&TerrainRequest { camera, size: TERRAIN_SIZE, view, lod, face_mapping: TangentWarp.id().to_string() })
        .map_err(|e| e.to_string())?;
    let meshes: Vec<planet_generators::mesh::TileMesh> =
        plan.nodes.iter().map(|n| planet_generators::mesh::tile_mesh(o.seed, &TangentWarp, n.tile, TERRAIN_CELLS, RADIUS_M)).collect();
    let refs: Vec<&planet_generators::mesh::TileMesh> = meshes.iter().collect();
    let (frame, stats) = planet_render::execute_terrain(ctx, &plan, &refs).map_err(|e| e.to_string())?;
    writeln!(out, "terrain {name} ({}): {}; {} draw calls, {} triangles", view.name(), plan.summary(), stats.draw_calls, stats.triangles)
        .map_err(err)?;
    if let Some(path) = &o.out {
        write_frame(&frame, path, out)?;
    }
    Ok(())
}
