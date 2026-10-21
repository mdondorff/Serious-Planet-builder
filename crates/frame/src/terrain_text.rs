//! Text forms of the terrain plan and of terrain repro bundles (CON-19, ADR 0009): every field of the plan, floats as
//! hexadecimal bit patterns, so a plan can be committed as a snapshot, diffed and replayed exactly.

use crate::camera::Camera;
use crate::lod::LodParams;
use crate::terrain::{TerrainDraw, TerrainPlan, TerrainRequest, TerrainView};
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};

pub const TERRAIN_PLAN_FORMAT_VERSION: u32 = 1;
/// Version 2 adds the optional world section ([`BundleWorld`]); version 1 bundles (the seed-only reference field) still read.
pub const TERRAIN_BUNDLE_FORMAT_VERSION: u32 = 2;

fn h64(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn h32(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn p64(s: &str) -> Result<f64, String> {
    u64::from_str_radix(s, 16).map(f64::from_bits).map_err(|e| format!("bad f64 bits '{s}': {e}"))
}

fn p32(s: &str) -> Result<f32, String> {
    u32::from_str_radix(s, 16).map(f32::from_bits).map_err(|e| format!("bad f32 bits '{s}': {e}"))
}

fn vec3(v: Vec3) -> String {
    format!("{} {} {}", h64(v.x), h64(v.y), h64(v.z))
}

fn parse_vec3(t: &[&str]) -> Result<Vec3, String> {
    match t {
        [x, y, z] => Ok(Vec3::new(p64(x)?, p64(y)?, p64(z)?)),
        _ => Err("expected three f64 bit patterns".into()),
    }
}

impl TerrainPlan {
    /// Stable text form with every field of the plan: size, view, the f64 camera, the f32 view-projection, clear
    /// colour, height range, cells and one line per node (tile, level, origin, camera-relative origin, morph values,
    /// colour and the expected mesh key).
    pub fn to_text(&self) -> String {
        // Exhaustiveness guards: adding a field to the plan, the camera or a draw is a compile error here until the text
        // form below carries it (and `from_text` reads it).
        let TerrainPlan {
            size: _,
            view: _,
            camera: _,
            view_projection: _,
            clear_color: _,
            height_range: _,
            cells: _,
            shore_ramp_per_m: _,
            nodes: _,
            lit: _,
            water: _,
        } = self;
        let Camera { position: _, forward: _, up: _, fov_y_rad: _, aspect: _, near_m: _ } = self.camera;
        let c = &self.camera;
        let mut s = format!("planet-terrainplan {TERRAIN_PLAN_FORMAT_VERSION}\n");
        s += &format!("size {} {}\nview {}\ncells {}\n", self.size.0, self.size.1, self.view.name(), self.cells);
        s += &format!(
            "camera {} forward {} up {} fov {} aspect {} near {}\n",
            vec3(c.position.0),
            vec3(c.forward),
            vec3(c.up),
            h64(c.fov_y_rad),
            h64(c.aspect),
            h64(c.near_m)
        );
        s += &format!("view_projection {}\n", self.view_projection.iter().map(|f| h32(*f)).collect::<Vec<_>>().join(" "));
        let k = self.clear_color;
        s += &format!(
            "clear {} {} {} {}\nheight_range {} {}\n",
            k[0],
            k[1],
            k[2],
            k[3],
            h32(self.height_range[0]),
            h32(self.height_range[1])
        );
        // The ramp's growth with distance (REND-030); a plan without one (zero) has no line and reads as zero.
        if self.shore_ramp_per_m != 0.0 {
            s += &format!("shore_ramp_per_m {}\n", h32(self.shore_ramp_per_m));
        }
        // The lit path's line exists only for a lit plan.
        if let Some(lit) = &self.lit {
            s += &lit.to_text();
            s.push('\n');
        }
        // Likewise the water lines exist only for a plan with water.
        if let Some(water) = &self.water {
            s += &water.to_text();
            s.push('\n');
        }
        for n in &self.nodes {
            let TerrainDraw {
                tile: _,
                level: _,
                origin: _,
                camera_relative_origin: _,
                morph: _,
                morph_start_m: _,
                morph_end_m: _,
                color: _,
                mesh_key: _,
                // A runtime flag of the draw plan (REND-033), not part of a recorded plan.
                preview: _,
                transient_only: _,
                // A runtime flag of the draw plan (ADR 0074), not part of a recorded plan.
                capped: _,
                // Written only when it is not the whole tile (below), so the text of a plan without masks is what it was.
                quadrants: _,
                // A runtime state of the draw plan (VIEW-044), not part of a recorded plan.
                state: _,
            } = n;
            let r = n.camera_relative_origin;
            s += &format!(
                "node {:016x} level {} origin {} rel {} {} {} morph {} {} {} color {} {} {} {} mesh_key {:016x}\n",
                n.tile.raw(),
                n.level,
                vec3(n.origin.0),
                h32(r[0]),
                h32(r[1]),
                h32(r[2]),
                h32(n.morph),
                h32(n.morph_start_m),
                h32(n.morph_end_m),
                n.color[0],
                n.color[1],
                n.color[2],
                n.color[3],
                n.mesh_key
            );
            if n.quadrants != crate::lod::ALL_QUADRANTS {
                s.pop();
                s += &format!(" quadrants {}\n", n.quadrants);
            }
        }
        s
    }

    pub fn from_text(text: &str) -> Result<TerrainPlan, String> {
        let mut lines = text.lines();
        let head = lines.next().ok_or("empty terrain plan")?;
        if head != format!("planet-terrainplan {TERRAIN_PLAN_FORMAT_VERSION}") {
            return Err(format!(
                "unsupported terrain plan header '{head}' (this build reads planet-terrainplan {TERRAIN_PLAN_FORMAT_VERSION})"
            ));
        }
        let (mut size, mut view, mut cells, mut camera, mut vp, mut clear, mut range) = (None, None, None, None, None, None, None);
        let mut nodes = Vec::new();
        let mut lit = None;
        let mut shore_ramp_per_m = 0.0f32;
        let mut water_lines: Vec<Vec<&str>> = Vec::new();
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let t: Vec<&str> = line.split_whitespace().collect();
            let int = |s: &str| s.parse::<u32>().map_err(|e| format!("bad number '{s}' in '{line}': {e}"));
            let byte = |s: &str| s.parse::<u8>().map_err(|e| format!("bad colour value '{s}' in '{line}': {e}"));
            match t.as_slice() {
                ["size", w, h] => size = Some((int(w)?, int(h)?)),
                ["view", v] => view = Some(TerrainView::parse(v).ok_or(format!("unknown view '{v}'"))?),
                ["cells", c] => cells = Some(int(c)?),
                ["camera", px, py, pz, "forward", fx, fy, fz, "up", ux, uy, uz, "fov", fov, "aspect", asp, "near", near] => {
                    camera = Some(Camera {
                        position: PlanetFixed(parse_vec3(&[px, py, pz])?),
                        forward: parse_vec3(&[fx, fy, fz])?,
                        up: parse_vec3(&[ux, uy, uz])?,
                        fov_y_rad: p64(fov)?,
                        aspect: p64(asp)?,
                        near_m: p64(near)?,
                    })
                }
                ["view_projection", rest @ ..] => {
                    let v = rest.iter().map(|s| p32(s)).collect::<Result<Vec<_>, _>>()?;
                    vp = Some(<[f32; 16]>::try_from(v).map_err(|v| format!("view_projection needs 16 values, got {}", v.len()))?);
                }
                ["clear", r, g, b, a] => clear = Some([byte(r)?, byte(g)?, byte(b)?, byte(a)?]),
                ["height_range", a, b] => range = Some([p32(a)?, p32(b)?]),
                ["shore_ramp_per_m", v] => shore_ramp_per_m = p32(v)?,
                ["lit", rest @ ..] => lit = Some(crate::look::LitPlan::from_tokens(rest)?),
                ["water", ..] | ["liquid", ..] | ["wnode", ..] => water_lines.push(t.clone()),
                ["node", id, "level", level, "origin", ox, oy, oz, "rel", rx, ry, rz, "morph", m, ms, me, "color", r, g, b, a, "mesh_key", key, tail @ ..] =>
                {
                    // `shore_ramp <h32>` ended a node line from the first REND-030 until the ramp followed the fragment's distance: read
                    // and ignored, so a plan recorded then still loads (and ramps by the look's blend alone).
                    let quadrants = match tail {
                        [] | ["shore_ramp", _] => crate::lod::ALL_QUADRANTS,
                        ["quadrants", q] | ["shore_ramp", _, "quadrants", q] => match q.parse::<u8>() {
                            Ok(m) if (1..crate::lod::ALL_QUADRANTS).contains(&m) => m,
                            _ => return Err(format!("bad quadrants '{q}' in '{line}' (1 to 14)")),
                        },
                        _ => return Err(format!("cannot parse terrain plan line '{line}'")),
                    };
                    let raw = u64::from_str_radix(id, 16).map_err(|e| format!("bad tile id '{id}': {e}"))?;
                    nodes.push(TerrainDraw {
                        tile: TileId::from_raw(raw).ok_or(format!("invalid tile id {raw:#x}"))?,
                        level: level.parse().map_err(|e| format!("bad level '{level}': {e}"))?,
                        origin: PlanetFixed(parse_vec3(&[ox, oy, oz])?),
                        camera_relative_origin: [p32(rx)?, p32(ry)?, p32(rz)?],
                        morph: p32(m)?,
                        morph_start_m: p32(ms)?,
                        morph_end_m: p32(me)?,
                        color: [byte(r)?, byte(g)?, byte(b)?, byte(a)?],
                        mesh_key: u64::from_str_radix(key, 16).map_err(|e| format!("bad mesh key '{key}': {e}"))?,
                        preview: false,
                        transient_only: false,
                        capped: false,
                        quadrants,
                        state: crate::terrain::TileState::Exact,
                    });
                }
                _ => return Err(format!("cannot parse terrain plan line '{line}'")),
            }
        }
        Ok(TerrainPlan {
            size: size.ok_or("plan has no size")?,
            view: view.ok_or("plan has no view")?,
            camera: camera.ok_or("plan has no camera")?,
            view_projection: vp.ok_or("plan has no view_projection")?,
            clear_color: clear.ok_or("plan has no clear colour")?,
            height_range: range.ok_or("plan has no height range")?,
            cells: cells.ok_or("plan has no cells")?,
            shore_ramp_per_m,
            nodes,
            lit,
            water: if water_lines.is_empty() { None } else { Some(crate::water::WaterPlan::from_lines(&water_lines)?) },
        })
    }

    /// Hash of the full text form: any field change changes it.
    pub fn plan_hash(&self) -> u64 {
        let bytes: Vec<u64> = self.to_text().bytes().map(u64::from).collect();
        hash_u64s(0x7465_7261, &bytes)
    }

    /// Human-readable differences to `other` (the first lines that differ, with both values); empty when equal.
    pub fn diff(&self, other: &TerrainPlan) -> Vec<String> {
        let (a, b) = (self.to_text(), other.to_text());
        let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let mut d: Vec<String> = a
            .iter()
            .zip(&b)
            .enumerate()
            .filter(|(_, (x, y))| x != y)
            .take(8)
            .map(|(i, (x, y))| format!("line {}: replayed `{x}` vs recorded `{y}`", i + 1))
            .collect();
        if a.len() != b.len() {
            d.push(format!("{} lines (replayed) vs {} lines (recorded): node counts differ", a.len(), b.len()));
        }
        d
    }
}

/// The world a bundle was recorded against when the run named one (`--world`): the definition as TOML text (canonical, so its
/// hash is the bundle's `definition_hash`), the implementation id of its height field and the in-memory bake the field read
/// (cells per face edge). A bundle without it is the seed-only world: the reference field of `seed` (GEN-018).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleWorld {
    pub height_field: String,
    pub global_res: u32,
    pub definition_toml: String,
    /// The authored items of the world, one item file's TOML each (AUTH-001): the definition text carries none (VIEW-024).
    pub items: Vec<String>,
}

/// One line: backslash, newline and carriage return escaped.
fn escape_line(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r")
}

fn unescape_line(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            other => return Err(format!("bad escape '\\{}' in the recorded world", other.map_or(String::new(), |c| c.to_string()))),
        }
    }
    Ok(out)
}

/// A terrain repro bundle: the request that produced a plan, the plan itself and the versions it needs to replay.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainBundle {
    pub generator_version: u32,
    pub implementation_id: String,
    pub mesh_version: u32,
    /// Hash of the WorldDefinition read set; the world's `WorldDefinition::hash` (WDEF-003), or the single planet of the seed and radius for a seed-only bundle.
    pub definition_hash: u64,
    /// Adapter the bundle was recorded on (informational: views compare per adapter).
    pub adapter: String,
    /// The world the field came from; `None`: the seed-only reference field of `request.seed`.
    pub world: Option<BundleWorld>,
    pub request: TerrainRequest,
    pub plan: TerrainPlan,
}

impl TerrainBundle {
    pub fn to_text(&self) -> String {
        let r = &self.request;
        let l = &r.lod;
        let mut s = format!("planet-terrain-repro {TERRAIN_BUNDLE_FORMAT_VERSION}\n");
        s += &format!(
            "generator_version {}\nimplementation_id {}\nmesh_version {}\nseed {}\n",
            self.generator_version, self.implementation_id, self.mesh_version, r.seed
        );
        s += &format!("definition_hash {:016x}\nadapter {}\n", self.definition_hash, self.adapter);
        if let Some(w) = &self.world {
            s += &format!("height_field {}\nglobal_res {}\nworld_toml {}\n", w.height_field, w.global_res, escape_line(&w.definition_toml));
            for item in &w.items {
                s += &format!("world_item {}\n", escape_line(item));
            }
        }
        s += &format!("face_mapping {}\nview {}\nsize {} {}\n", r.face_mapping, r.view.name(), r.size.0, r.size.1);
        let c = &r.camera;
        s += &format!(
            "camera {} forward {} up {} fov {} aspect {} near {}\n",
            vec3(c.position.0),
            vec3(c.forward),
            vec3(c.up),
            h64(c.fov_y_rad),
            h64(c.aspect),
            h64(c.near_m)
        );
        s += &format!(
            "lod radius {} max_height {} fov {} viewport_h {} tau {} cells {} max_level {} min_level {} morph_band {} cull_horizon {}\n",
            h64(l.radius_m),
            h64(l.max_height_m),
            h64(l.fov_y_rad),
            h64(l.viewport_h_px),
            h64(l.tau_px),
            l.cells,
            l.max_level,
            l.min_level,
            h64(l.morph_band),
            l.cull_horizon
        );
        s += &format!("plan_hash {:016x}\n---\n", self.plan.plan_hash());
        s += &self.plan.to_text();
        s
    }

    pub fn from_text(text: &str) -> Result<TerrainBundle, String> {
        let (head, plan_text) = text.split_once("\n---\n").ok_or("bundle has no '---' separator before the plan")?;
        let mut lines = head.lines();
        let first = lines.next().ok_or("empty bundle")?;
        let version = match first {
            "planet-terrain-repro 1" => 1,
            "planet-terrain-repro 2" => 2,
            _ => {
                return Err(format!(
                    "unsupported bundle header '{first}' (this build reads planet-terrain-repro 1 and {TERRAIN_BUNDLE_FORMAT_VERSION})"
                ))
            }
        };
        let (mut field_id, mut global_res, mut world_toml) = (None, None, None);
        let mut world_items = Vec::new();
        let plan = TerrainPlan::from_text(plan_text)?;
        let (mut gv, mut imp, mut mv, mut seed, mut dh, mut adapter) = (None, None, None, None, 0u64, String::new());
        let (mut mapping, mut view, mut size, mut camera, mut lod, mut recorded) = (None, None, None, None, None, None);
        for line in lines {
            let t: Vec<&str> = line.split_whitespace().collect();
            let bad = |e: std::num::ParseIntError| format!("bad value in '{line}': {e}");
            match t.as_slice() {
                ["generator_version", v] => gv = Some(v.parse().map_err(bad)?),
                ["implementation_id", v] => imp = Some(v.to_string()),
                ["mesh_version", v] => mv = Some(v.parse().map_err(bad)?),
                ["seed", v] => seed = Some(v.parse::<u64>().map_err(bad)?),
                ["definition_hash", v] => dh = u64::from_str_radix(v, 16).map_err(bad)?,
                ["adapter", v] => adapter = v.to_string(),
                ["height_field", v] => field_id = Some(v.to_string()),
                ["global_res", v] => global_res = Some(v.parse::<u32>().map_err(bad)?),
                ["world_toml", ..] => world_toml = Some(unescape_line(line.strip_prefix("world_toml ").unwrap_or(""))?),
                ["world_item", ..] => world_items.push(unescape_line(line.strip_prefix("world_item ").unwrap_or(""))?),
                ["face_mapping", v] => mapping = Some(v.to_string()),
                ["view", v] => view = Some(TerrainView::parse(v).ok_or(format!("unknown view '{v}'"))?),
                ["size", w, h] => size = Some((w.parse::<u32>().map_err(bad)?, h.parse::<u32>().map_err(bad)?)),
                ["camera", px, py, pz, "forward", fx, fy, fz, "up", ux, uy, uz, "fov", fov, "aspect", asp, "near", near] => {
                    camera = Some(Camera {
                        position: PlanetFixed(parse_vec3(&[px, py, pz])?),
                        forward: parse_vec3(&[fx, fy, fz])?,
                        up: parse_vec3(&[ux, uy, uz])?,
                        fov_y_rad: p64(fov)?,
                        aspect: p64(asp)?,
                        near_m: p64(near)?,
                    })
                }
                // A bundle written before the render floor has no `min_level`: it replays as floor 0, the plan it recorded (TEST-008).
                ["lod", "radius", r, "max_height", mh, "fov", fov, "viewport_h", vh, "tau", tau, "cells", cells, "max_level", ml, "min_level", _, "morph_band", mb, "cull_horizon", ch]
                | ["lod", "radius", r, "max_height", mh, "fov", fov, "viewport_h", vh, "tau", tau, "cells", cells, "max_level", ml, "morph_band", mb, "cull_horizon", ch] =>
                {
                    let min_level = match t.as_slice() {
                        [.., "min_level", _, "morph_band", _, "cull_horizon", _] => t[t.len() - 5].parse().map_err(bad)?,
                        _ => 0,
                    };
                    lod = Some(LodParams {
                        radius_m: p64(r)?,
                        max_height_m: p64(mh)?,
                        fov_y_rad: p64(fov)?,
                        viewport_h_px: p64(vh)?,
                        tau_px: p64(tau)?,
                        cells: cells.parse().map_err(bad)?,
                        max_level: ml.parse().map_err(bad)?,
                        min_level,
                        morph_band: p64(mb)?,
                        cull_horizon: ch.parse().map_err(|e| format!("bad bool '{ch}': {e}"))?,
                    })
                }
                ["plan_hash", v] => recorded = Some(u64::from_str_radix(v, 16).map_err(bad)?),
                _ => return Err(format!("cannot parse bundle line '{line}'")),
            }
        }
        let world = match (field_id, global_res, world_toml) {
            (None, None, None) => None,
            (Some(height_field), Some(global_res), Some(definition_toml)) if version >= 2 => {
                Some(BundleWorld { height_field, global_res, definition_toml, items: world_items })
            }
            (Some(_), Some(_), Some(_)) => {
                return Err("a version 1 bundle cannot record a world (version 1 is the seed-only reference field)".into())
            }
            _ => return Err("bundle records a half of its world: height_field, global_res and world_toml come together".into()),
        };
        let recorded = recorded.ok_or("bundle has no plan_hash")?;
        if recorded != plan.plan_hash() {
            return Err(format!(
                "bundle is corrupt: recorded plan hash {recorded:016x}, embedded plan hashes to {:016x}",
                plan.plan_hash()
            ));
        }
        Ok(TerrainBundle {
            generator_version: gv.ok_or("bundle has no generator_version")?,
            implementation_id: imp.ok_or("bundle has no implementation_id")?,
            mesh_version: mv.ok_or("bundle has no mesh_version")?,
            definition_hash: dh,
            adapter,
            world,
            request: TerrainRequest {
                camera: camera.ok_or("bundle has no camera")?,
                size: size.ok_or("bundle has no size")?,
                view: view.ok_or("bundle has no view")?,
                lod: lod.ok_or("bundle has no lod line")?,
                face_mapping: mapping.ok_or("bundle has no face_mapping")?,
                seed: seed.ok_or("bundle has no seed")?,
            },
            plan,
        })
    }
}
