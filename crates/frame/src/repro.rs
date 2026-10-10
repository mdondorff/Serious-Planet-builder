//! Repro bundles (CON-19): everything needed to replay a visual result in any run mode.

use crate::plan::{DebugView, FramePlan, PlanRequest};
use planet_core::{PlanetFixed, TileId, Vec3};

pub const BUNDLE_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct ReproBundle {
    pub generator_version: u32,
    pub implementation_id: String,
    pub seed: u64,
    pub res: u32,
    /// Hash of the WorldDefinition read set; 0 until `world-def` exists (M3).
    pub definition_hash: u64,
    /// Adapter the bundle was recorded on (informational: debug views compare per adapter).
    pub adapter: String,
    pub request: PlanRequest,
    pub plan: FramePlan,
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

impl ReproBundle {
    pub fn to_text(&self) -> String {
        let r = &self.request;
        let mut s = format!("planet-repro {BUNDLE_FORMAT_VERSION}\n");
        s += &format!(
            "generator_version {}\nimplementation_id {}\nseed {}\nres {}\n",
            self.generator_version, self.implementation_id, self.seed, self.res
        );
        s += &format!("definition_hash {:016x}\nadapter {}\n", self.definition_hash, self.adapter);
        s += &format!(
            "face_mapping {}\nradius {}\nview {}\nsize {} {}\n",
            r.face_mapping,
            hex(r.radius_m),
            r.view.name(),
            r.size.0,
            r.size.1
        );
        s += &format!("camera {} {} {}\n", hex(r.camera.0.x), hex(r.camera.0.y), hex(r.camera.0.z));
        for t in &r.tiles {
            s += &format!("tile {:016x}\n", t.raw());
        }
        s += &format!("plan_hash {:016x}\n---\n", self.plan.plan_hash());
        s += &self.plan.to_text();
        s
    }

    pub fn from_text(text: &str) -> Result<ReproBundle, String> {
        let (head, plan_text) = text.split_once("\n---\n").ok_or("bundle has no '---' separator before the plan")?;
        let mut lines = head.lines();
        let first = lines.next().ok_or("empty bundle")?;
        if first != format!("planet-repro {BUNDLE_FORMAT_VERSION}") {
            return Err(format!("unsupported bundle header '{first}' (this build reads planet-repro {BUNDLE_FORMAT_VERSION})"));
        }
        let mut b = ReproBundle {
            generator_version: 0,
            implementation_id: String::new(),
            seed: 0,
            res: 0,
            definition_hash: 0,
            adapter: String::new(),
            request: PlanRequest {
                tiles: vec![],
                camera: PlanetFixed(Vec3::ZERO),
                size: (0, 0),
                view: DebugView::Face,
                radius_m: 0.0,
                face_mapping: String::new(),
            },
            plan: FramePlan::from_text(plan_text)?,
        };
        let mut recorded_hash = None;
        let f64s = |s: &str| u64::from_str_radix(s, 16).map(f64::from_bits).map_err(|e| format!("bad f64 bits '{s}': {e}"));
        for line in lines {
            let t: Vec<&str> = line.split_whitespace().collect();
            let bad = |e: std::num::ParseIntError| format!("bad value in '{line}': {e}");
            match t.as_slice() {
                ["generator_version", v] => b.generator_version = v.parse().map_err(bad)?,
                ["implementation_id", v] => b.implementation_id = v.to_string(),
                ["seed", v] => b.seed = v.parse().map_err(bad)?,
                ["res", v] => b.res = v.parse().map_err(bad)?,
                ["definition_hash", v] => b.definition_hash = u64::from_str_radix(v, 16).map_err(bad)?,
                ["adapter", v] => b.adapter = v.to_string(),
                ["face_mapping", v] => b.request.face_mapping = v.to_string(),
                ["radius", v] => b.request.radius_m = f64s(v)?,
                ["view", v] => b.request.view = DebugView::parse(v).ok_or(format!("unknown view '{v}'"))?,
                ["size", w, h] => b.request.size = (w.parse().map_err(bad)?, h.parse().map_err(bad)?),
                ["camera", x, y, z] => b.request.camera = PlanetFixed(Vec3::new(f64s(x)?, f64s(y)?, f64s(z)?)),
                ["tile", id] => {
                    let raw = u64::from_str_radix(id, 16).map_err(bad)?;
                    b.request.tiles.push(TileId::from_raw(raw).ok_or(format!("invalid tile id {raw:#x}"))?);
                }
                ["plan_hash", v] => recorded_hash = Some(u64::from_str_radix(v, 16).map_err(bad)?),
                _ => return Err(format!("cannot parse bundle line '{line}'")),
            }
        }
        let recorded = recorded_hash.ok_or("bundle has no plan_hash")?;
        if recorded != b.plan.plan_hash() {
            return Err(format!(
                "bundle is corrupt: recorded plan hash {recorded:016x}, embedded plan hashes to {:016x}",
                b.plan.plan_hash()
            ));
        }
        Ok(b)
    }
}
