//! `generate --plan-check DIR --global-res N`: plan the standard flight at production parameters (cells 32, viewport / tau 240:
//! ADR 0016) over a stored global bake, with the terrain field and the eroded field, and report every view's node count and
//! deepest level. The M3 acceptance check that realistic terrain is not rejected for steepness at production resolution
//! (ADR 0028 point 6, ADR 0030): the tier A tests plan on a 256-cell bake only.

use std::io::Write;
use std::time::Instant;

use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::height::HeightField;
use planet_generators::terrain::TerrainGenerator;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::{err, Options};

const OTHER_COMMANDS: [&str; 16] = [
    "--tile",
    "--out",
    "--face-net",
    "--manifest",
    "--smoke",
    "--cache-dir",
    "--verify-cache",
    "--region",
    "--res",
    "--sample",
    "--allow-stale",
    "--global-dir",
    "--verify-global",
    "--global-products",
    "--compact-cache",
    "--cap-bytes",
];

pub fn plan_check(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.plan_check.as_ref().expect("checked by the caller");
    let clash: Vec<&str> = OTHER_COMMANDS.into_iter().chain(["--threads"]).filter(|f| o.given.contains(*f)).collect();
    if !clash.is_empty() {
        return Err(format!("--plan-check takes only --global-res and --world or --seed, not {}", clash.join(", ")));
    }
    if !o.given.contains("--global-res") {
        return Err("--plan-check needs --global-res N, the resolution the bake was made at (a store holds one bake per resolution)".into());
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if !dir.is_dir() {
        return Err(format!("{}: no such store directory (check the path; plan-check reads a bake made by --global-dir)", dir.display()));
    }
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
    };
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let store = planet_cache::pack::TileStore::open(dir, world.params()).map_err(|e| e.to_string())?;
    let src = StoreSource::new(&store, &world, o.global_res).map_err(|e| e.to_string())?;
    let bake_key = world.stage_key(o.global_res).0;
    let radius = GenContext::new(&def).body_radius_m(BodyId::PLANET).map_err(|e| format!("{e:?}"))?;
    let terrain = TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).map_err(|e| format!("{e:?}"))?;
    let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, bake_key).map_err(|e| format!("{e:?}"))?;
    writeln!(out, "plan-check: {} cells per face edge, cells 32, viewport / tau 240, standard flight, {}", o.global_res, dir.display())
        .map_err(err)?;
    let mut failures = 0usize;
    let mut check = |label: &str, field: &dyn HeightField, error: &dyn Fn() -> Option<String>, out: &mut dyn Write| -> Result<(), String> {
        let params = LodParams {
            radius_m: radius,
            max_height_m: field.max_abs_height_m(),
            cells: 32,
            viewport_h_px: 1080.0,
            tau_px: 4.5,
            ..LodParams::earth_1080p()
        };
        // The scripted cameras sit above the terrain: a bake that cannot be read gives NaN heights, so say so before building them.
        let _ = field.height(planet_core::Vec3::new(0.3, 0.5, 0.8).normalized());
        if let Some(e) = error() {
            failures += 1;
            return writeln!(out, "  {label}: the bake could not be read: {e}").map_err(err);
        }
        for (name, cam) in scripted_views_field(radius, 16.0 / 9.0, field) {
            let req = TerrainRequest {
                camera: cam,
                size: (192, 108),
                view: TerrainView::Level,
                lod: params.clone(),
                face_mapping: TangentWarp.id().to_string(),
                seed: field.seed(),
            };
            let t = Instant::now();
            match plan_terrain_field(&req, field) {
                Ok(plan) => writeln!(
                    out,
                    "  {label} {name}: {} nodes, deepest level {}, {:.1} s",
                    plan.nodes.len(),
                    plan.nodes.iter().map(|n| n.level).max().unwrap_or(0),
                    t.elapsed().as_secs_f64()
                )
                .map_err(err)?,
                Err(e) => {
                    failures += 1;
                    writeln!(out, "  {label} {name}: REJECTED: {e}").map_err(err)?;
                }
            }
        }
        if let Some(e) = error() {
            failures += 1;
            writeln!(out, "  {label}: the bake could not be read: {e}").map_err(err)?;
        }
        Ok(())
    };
    check("terrain", &terrain, &|| terrain.error(), out)?;
    check("erosion", &eroded, &|| eroded.error(), out)?;
    if failures > 0 {
        return Err(format!("plan-check: {failures} view(s) rejected or unreadable"));
    }
    writeln!(out, "plan-check: both fields plan every view without steepness rejection").map_err(err)
}
