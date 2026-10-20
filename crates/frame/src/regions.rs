//! The region overlay's polygons (VIEW-030, WDEF-012): the region items nearest the camera as short polygon lists the lit fragment stage tests
//! analytically, decided on the CPU in f64 (CON-03, CON-11).
//!
//! A polygon is held in the orthographic frame of its anchor: the anchor's position relative to the camera, the tangent plane's basis, and
//! the points as metres in that plane. The fragment stage takes the fragment's position relative to the camera, subtracts the anchor and
//! projects onto the same basis, so no planet-radius value reaches the shader. Where it differs from the gnomonic test that decides the
//! stamp mask (`planet_world_def::region::polygon_contains`) is the second order of the angle (about 0.1 m at 20 km) and, above all, the
//! terrain height: a fragment at height `h` is projected scaled by `1 + h/R`, so an edge 20 km from the anchor sits about `h * 20000 / R` m
//! (16 m at 5 km) inward of the mask's on high ground and outward below the datum. A debug tint; a gnomonic test in the shader would need
//! `1/R` there (CON-11) for an overlay that is not the mask.

use planet_core::Vec3;
use planet_world_def::footprint::FootprintItem;
use planet_world_def::region::{basis, SURFACE_TYPES};

/// The most polygons a frame carries (the globals table of the renderer has this many entries); the farthest are left out and counted.
pub const MAX_REGIONS: usize = 8;

/// The most points of one polygon (WDEF-012).
pub const MAX_REGION_POINTS: usize = planet_world_def::stamp::MAX_POINTS;

/// One region polygon, ready for the fragment stage.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionPoly {
    /// The surface type's tint, linear.
    pub tint: [f32; 3],
    /// The anchor (the first point) relative to the camera, metres.
    pub anchor: [f32; 3],
    /// The tangent plane's basis at the anchor, and its unit up.
    pub e1: [f32; 3],
    pub e2: [f32; 3],
    pub up: [f32; 3],
    /// The square of the largest distance of a point from the anchor in the plane, plus a margin (a fragment farther away is outside).
    pub reach_sq: f32,
    /// How far from the anchor's tangent plane a fragment of the surface can be, metres: the curvature over the reach plus the relief. A
    /// fragment farther off (the other side of the planet) is outside.
    pub band_m: f32,
    /// The points in the plane, metres, in the order of the region.
    pub points: Vec<[f32; 2]>,
}

/// The polygons of a frame and how many the cap left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionList {
    /// Nearest the camera first.
    pub polys: Vec<RegionPoly>,
    /// Regions beyond [`MAX_REGIONS`] that were left out (the farthest go first).
    pub dropped: usize,
}

/// The sRGB byte as linear light.
fn linear(c: u8) -> f32 {
    let c = f64::from(c) / 255.0;
    (if c <= 0.04045 { c / 12.92 } else { libm::pow((c + 0.055) / 1.055, 2.4) }) as f32
}

fn f3(v: Vec3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

/// The polygon of one region item, `None` for an item that is not a well-formed region. `radius_m` is the body's, `camera` the planet-fixed
/// camera position in metres.
fn poly_of(item: &FootprintItem, camera: Vec3, radius_m: f64) -> Option<RegionPoly> {
    let stamp = item.stamp.as_ref().filter(|s| s.kind.is_region())?;
    let points = stamp.points();
    let surface = stamp.params()[0];
    if points.len() < 3
        || points.len() > MAX_REGION_POINTS
        || surface < 0.0
        || surface.fract() != 0.0
        || surface as usize >= SURFACE_TYPES.len()
    {
        return None;
    }
    let up = Vec3::new(f64::from_bits(item.dir[0]), f64::from_bits(item.dir[1]), f64::from_bits(item.dir[2]));
    let (e1, e2) = basis(up);
    let flat: Vec<[f64; 2]> = points.iter().map(|q| [radius_m * q.dot(e1), radius_m * q.dot(e2)]).collect();
    let far = flat.iter().map(|p| p[0] * p[0] + p[1] * p[1]).fold(0.0, f64::max);
    let reach = libm::sqrt(far) * 1.01 + 1.0;
    let tint = SURFACE_TYPES[surface as usize].tint;
    Some(RegionPoly {
        tint: tint.map(linear),
        anchor: f3(up * radius_m - camera),
        e1: f3(e1),
        e2: f3(e2),
        up: f3(up),
        reach_sq: (reach * reach) as f32,
        band_m: (reach * reach / radius_m + 20_000.0) as f32,
        points: flat.iter().map(|p| [p[0] as f32, p[1] as f32]).collect(),
    })
}

/// The region polygons of `items` (the footprint items of the body, every layer) seen from `camera`: the [`MAX_REGIONS`] nearest by the
/// distance of their anchor, ties by item id; the rest are counted. Items that are not regions are ignored.
pub fn region_list(items: &[FootprintItem], camera: Vec3, radius_m: f64) -> RegionList {
    let mut found: Vec<(f64, planet_world_def::item_id::ItemId, RegionPoly)> = Vec::new();
    for item in items {
        if let Some(p) = poly_of(item, camera, radius_m) {
            let d = Vec3::new(p.anchor[0].into(), p.anchor[1].into(), p.anchor[2].into()).length();
            found.push((d, item.id, p));
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let dropped = found.len().saturating_sub(MAX_REGIONS);
    found.truncate(MAX_REGIONS);
    RegionList { polys: found.into_iter().map(|(_, _, p)| p).collect(), dropped }
}

impl RegionList {
    /// Stable text: a header line and one line per polygon, floats as bit patterns (the plan text convention). A list with no polygon and
    /// none dropped is the empty string, so a plan without regions has no region text.
    pub fn to_text(&self) -> String {
        if self.polys.is_empty() && self.dropped == 0 {
            return String::new();
        }
        let h = |v: f32| format!("{:08x}", v.to_bits());
        let mut out = format!("regions {} dropped {}\n", self.polys.len(), self.dropped);
        for p in &self.polys {
            let v3 = |v: &[f32; 3]| format!("{} {} {}", h(v[0]), h(v[1]), h(v[2]));
            out.push_str(&format!(
                "region tint {} anchor {} e1 {} e2 {} up {} reach {} band {} points {}",
                v3(&p.tint),
                v3(&p.anchor),
                v3(&p.e1),
                v3(&p.e2),
                v3(&p.up),
                h(p.reach_sq),
                h(p.band_m),
                p.points.len()
            ));
            for q in &p.points {
                out.push_str(&format!(" {} {}", h(q[0]), h(q[1])));
            }
            out.push('\n');
        }
        out
    }

    /// Parse [`to_text`](RegionList::to_text) (the empty string is the empty list).
    pub fn from_text(text: &str) -> Result<RegionList, String> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let Some(head) = lines.next() else { return Ok(RegionList::default()) };
        let usage = "regions text is 'regions <n> dropped <d>' and one 'region ...' line per polygon";
        let t: Vec<&str> = head.split_whitespace().collect();
        let ["regions", n, "dropped", d] = t[..] else { return Err(usage.into()) };
        let n: usize = n.parse().map_err(|e| format!("bad polygon count '{n}': {e}"))?;
        let dropped: usize = d.parse().map_err(|e| format!("bad dropped count '{d}': {e}"))?;
        if n > MAX_REGIONS {
            return Err(format!("{n} region polygons, at most {MAX_REGIONS}"));
        }
        let mut polys = Vec::with_capacity(n);
        for line in lines {
            let mut c = Cursor { it: line.split_whitespace() };
            c.word("region")?;
            let tint = c.triple("tint")?;
            let anchor = c.triple("anchor")?;
            let e1 = c.triple("e1")?;
            let e2 = c.triple("e2")?;
            let up = c.triple("up")?;
            c.word("reach")?;
            let reach_sq = c.bits()?;
            c.word("band")?;
            let band_m = c.bits()?;
            c.word("points")?;
            let count: usize = c.next()?.parse().map_err(|e| format!("bad point count: {e}"))?;
            if !(3..=MAX_REGION_POINTS).contains(&count) {
                return Err(format!("{count} points, 3 to {MAX_REGION_POINTS} expected"));
            }
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                points.push([c.bits()?, c.bits()?]);
            }
            if c.it.next().is_some() {
                return Err("region line: trailing text".into());
            }
            polys.push(RegionPoly { tint, anchor, e1, e2, up, reach_sq, band_m, points });
        }
        if polys.len() != n {
            return Err(format!("{n} polygons announced, {} lines", polys.len()));
        }
        Ok(RegionList { polys, dropped })
    }
}

/// The words of one region line.
struct Cursor<'a> {
    it: std::str::SplitWhitespace<'a>,
}

impl<'a> Cursor<'a> {
    fn next(&mut self) -> Result<&'a str, String> {
        self.it.next().ok_or_else(|| "region line ends early".to_string())
    }

    fn word(&mut self, key: &str) -> Result<(), String> {
        let got = self.next()?;
        if got == key {
            Ok(())
        } else {
            Err(format!("region line: '{key}' expected, '{got}' found"))
        }
    }

    fn bits(&mut self) -> Result<f32, String> {
        let s = self.next()?;
        u32::from_str_radix(s, 16).map(f32::from_bits).map_err(|e| format!("bad f32 bits '{s}': {e}"))
    }

    fn triple(&mut self, key: &str) -> Result<[f32; 3], String> {
        self.word(key)?;
        Ok([self.bits()?, self.bits()?, self.bits()?])
    }
}
