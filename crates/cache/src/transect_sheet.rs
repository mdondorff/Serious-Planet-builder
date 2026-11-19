//! The cross-section sheet (BUILD-061, `cargo xtask transects --chain DIR`): per river transect of SPIKE-119 inside a chain directory's window, the start surface `S0` and the chain's
//! output (`final.f32`) across the river, and the GLO-30 valley matched to it, on one vertical scale per panel; per profile the valley depth and its width at 10 m and 50 m above the
//! river level, and marks a transect stale when `S0`'s depth on it fell under 10 % of its depth at the pick. It reads BUILD-060's chain layout ([`crate::stage_sheet`]) and two data files: the transects (face L cells) and the matched GLO-30 profiles (derived data, with the
//! Copernicus notice).
//!
//! Definitions (one per measure):
//! - a profile: the surface sampled bilinearly every [`STEP_CELLS`] L cells along the transect's line, at the distance `d` from the line's middle (negative towards its first end);
//! - the river level: the lowest sample within [`RIVER_SEARCH_M`] of `d = 0`; its sample is the profile's river point;
//! - the valley depth: the lower of the two sides' highest samples, less the river level;
//! - the width at `h`: walking out from the river point on each side to the first sample above the river level + `h`, the distance between the two crossings (linear between
//!   samples); a side that never rises above it is open, and the width is then at least the distance to that line end (`>=` on the sheet, `open` in the table).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use planet_generators::dump::Rgba;
use planet_generators::preview::{put_text, TEXT_CELL_W, TEXT_ROW_H};

use crate::stage_sheet::{read_f32, Manifest};

/// The sampling step along a line, in L cells (SPIKE-119: every 50.9 m at 101.8 m cells).
pub const STEP_CELLS: f64 = 0.5;
/// How far from the line's middle the river level is sought, in metres.
pub const RIVER_SEARCH_M: f64 = 250.0;
/// The two widths' heights above the river level, in metres.
pub const WIDTH_HEIGHTS_M: [f64; 2] = [10.0, 50.0];
/// A transect is stale when `S0`'s valley depth on it is under this share of its depth at the pick (lead #150a: the start surface changed under a fixed transect).
pub const STALE_SHARE: f64 = 0.1;

/// The transects (SPIKE-119's rule, re-picked on g1f8's prototype `S0`, lead #150a), face L cells.
pub const TRANSECTS_CSV: &str = include_str!("../data/transects.csv");
/// The GLO-30 valley matched to each transect (SPIKE-119's rule), heights relative to its river point.
pub const GLO30_CSV: &str = include_str!("../data/transects-glo30.csv");

/// One transect: its window name and id, its face, the line's ends and the river point in face L cells, the drained area and relief SPIKE-119 measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Transect {
    pub window: String,
    pub id: String,
    pub face: u8,
    pub ends: [(f64, f64); 2],
    pub river: (f64, f64),
    pub area_km2: f64,
    pub relief_m: f64,
    /// `S0`'s valley depth on the transect when it was picked (the optional `s0_depth_m` column), by this module's measure.
    pub s0_depth_m: Option<f64>,
}

/// The matched GLO-30 profile of a transect: the tile, its relief and drained area, and the samples `(d, z - z_river)` in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct DemProfile {
    pub tile: String,
    pub relief_m: f64,
    pub area_km2: f64,
    pub samples: Vec<(f64, f64)>,
}

/// A width: closed (both sides cross) or open (at least the value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
    Closed(f64),
    Open(f64),
}

impl Width {
    fn text(self) -> String {
        match self {
            Width::Closed(w) => format!("{:.2}", w / 1000.0),
            Width::Open(w) => format!(">{:.1}", w / 1000.0),
        }
    }
    fn table(self) -> String {
        match self {
            Width::Closed(w) => format!("{w:.0}"),
            Width::Open(w) => format!("{w:.0} open"),
        }
    }
}

/// A profile's valley measures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Valley {
    /// The river level (m) and the river point's distance `d` (m).
    pub river_m: f64,
    pub river_d: f64,
    pub depth_m: f64,
    /// The widths at [`WIDTH_HEIGHTS_M`].
    pub widths: [Width; 2],
}

fn columns(header: &str, names: &[&str]) -> Result<Vec<usize>, String> {
    let cols: Vec<&str> = header.split(',').map(str::trim).collect();
    names.iter().map(|n| cols.iter().position(|c| c == n).ok_or(format!("the header has no column '{n}'"))).collect()
}

fn data_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'))
}

/// Parses the transects' csv (`window,id,face,seed,gx0,gy0,gx1,gy1,river_gx,river_gy,area_km2,area_scope,relief_m,half_km`, optionally `s0_depth_m`).
pub fn parse_transects(text: &str) -> Result<Vec<Transect>, String> {
    let mut lines = data_lines(text);
    let (_, header) = lines.next().ok_or("the transects file is empty")?;
    let c = columns(header, &["window", "id", "face", "gx0", "gy0", "gx1", "gy1", "river_gx", "river_gy", "area_km2", "relief_m"])?;
    let depth_col = columns(header, &["s0_depth_m"]).ok().map(|v| v[0]);
    lines
        .map(|(n, l)| {
            let f: Vec<&str> = l.split(',').map(str::trim).collect();
            if f.len() <= c.iter().copied().max().unwrap_or(0) {
                return Err(format!("transects line {}: too few fields", n + 1));
            }
            let num = |k: usize| -> Result<f64, String> {
                f[c[k]].parse().map_err(|_| format!("transects line {}: '{}' is not a number", n + 1, f[c[k]]))
            };
            Ok(Transect {
                window: f[c[0]].to_string(),
                id: f[c[1]].to_string(),
                face: num(2)? as u8,
                ends: [(num(3)?, num(4)?), (num(5)?, num(6)?)],
                river: (num(7)?, num(8)?),
                area_km2: num(9)?,
                relief_m: num(10)?,
                s0_depth_m: match depth_col {
                    Some(k) => Some(
                        f.get(k)
                            .ok_or(format!("transects line {}: too few fields", n + 1))?
                            .parse()
                            .map_err(|_| format!("transects line {}: '{}' is not a number", n + 1, f[k]))?,
                    ),
                    None => None,
                },
            })
        })
        .collect()
}

/// Parses the GLO-30 profiles' csv (`window,id,tile,relief_m,area_km2,k,d_m,z_rel_m`, the samples of a profile in order of `k`), keyed by `(window, id)`.
pub fn parse_dem(text: &str) -> Result<BTreeMap<(String, String), DemProfile>, String> {
    let mut lines = data_lines(text);
    let (_, header) = lines.next().ok_or("the GLO-30 profiles file is empty")?;
    let c = columns(header, &["window", "id", "tile", "relief_m", "area_km2", "d_m", "z_rel_m"])?;
    let mut out: BTreeMap<(String, String), DemProfile> = BTreeMap::new();
    for (n, l) in lines {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() <= c.iter().copied().max().unwrap_or(0) {
            return Err(format!("GLO-30 line {}: too few fields", n + 1));
        }
        let num = |k: usize| -> Result<f64, String> {
            f[c[k]].parse().map_err(|_| format!("GLO-30 line {}: '{}' is not a number", n + 1, f[c[k]]))
        };
        let p = out.entry((f[c[0]].to_string(), f[c[1]].to_string())).or_insert_with(|| DemProfile {
            tile: f[c[2]].to_string(),
            relief_m: f[c[3]].parse().unwrap_or(f64::NAN),
            area_km2: f[c[4]].parse().unwrap_or(f64::NAN),
            samples: Vec::new(),
        });
        p.samples.push((num(5)?, num(6)?));
    }
    Ok(out)
}

/// Whether a transect is stale on this run: `S0`'s valley depth under [`STALE_SHARE`] of its depth at the pick (`s0_depth_m`; the stored relief when the column is absent).
/// `None` without `S0`, and for a transect picked with no valley (a pick depth of 0: nothing to lose).
pub fn stale(t: &Transect, s0: Option<&Valley>) -> Option<bool> {
    let reference = t.s0_depth_m.unwrap_or(t.relief_m);
    if reference <= 0.0 {
        return None;
    }
    s0.map(|v| v.depth_m < STALE_SHARE * reference)
}

/// The chain's face: the first field of its `switch window F,...` line.
pub fn chain_face(m: &Manifest) -> Option<u8> {
    m.run.iter().find_map(|l| l.strip_prefix("switch window ")).and_then(|v| v.split(',').next()).and_then(|f| f.trim().parse().ok())
}

/// The transects of `face` whose whole line lies inside the window (with one L cell to spare for the bilinear read).
pub fn in_window<'a>(all: &'a [Transect], m: &Manifest, face: u8) -> Vec<&'a Transect> {
    let inside = |(x, y): (f64, f64)| {
        let (lx, ly) = (x - m.x0 as f64, y - m.y0 as f64);
        lx >= 0.0 && ly >= 0.0 && lx <= (m.w - 2) as f64 && ly <= (m.h - 2) as f64
    };
    all.iter().filter(|t| t.face == face && inside(t.ends[0]) && inside(t.ends[1])).collect()
}

/// A surface's profile along a transect: `(d, z)` every [`STEP_CELLS`], bilinear on the window's grid `z` (`m.w x m.h`); `n = floor(length / step) + 1` samples.
pub fn profile(z: &[f32], m: &Manifest, t: &Transect) -> Vec<(f64, f64)> {
    let ((x0, y0), (x1, y1)) =
        ((t.ends[0].0 - m.x0 as f64, t.ends[0].1 - m.y0 as f64), (t.ends[1].0 - m.x0 as f64, t.ends[1].1 - m.y0 as f64));
    let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
    let n = (len / STEP_CELLS).floor() as usize + 1;
    (0..n)
        .map(|k| {
            let s = if n > 1 { k as f64 / (n - 1) as f64 } else { 0.5 };
            let (x, y) = (x0 + (x1 - x0) * s, y0 + (y1 - y0) * s);
            let (xi, yi) = ((x.floor() as usize).min(m.w - 2), (y.floor() as usize).min(m.h - 2));
            let (fx, fy) = (x - xi as f64, y - yi as f64);
            let at = |i: usize, j: usize| f64::from(z[j * m.w + i]);
            let v = at(xi, yi) * (1.0 - fx) * (1.0 - fy)
                + at(xi + 1, yi) * fx * (1.0 - fy)
                + at(xi, yi + 1) * (1.0 - fx) * fy
                + at(xi + 1, yi + 1) * fx * fy;
            ((s - 0.5) * len * m.cell_m, v)
        })
        .collect()
}

/// The valley measures of a profile (the module's definitions); `None` for fewer than 3 samples.
pub fn valley(p: &[(f64, f64)]) -> Option<Valley> {
    if p.len() < 3 {
        return None;
    }
    let near: Vec<usize> = (0..p.len()).filter(|&k| p[k].0.abs() <= RIVER_SEARCH_M).collect();
    let centre = near.iter().copied().min_by(|&a, &b| p[a].1.total_cmp(&p[b].1).then(p[a].0.abs().total_cmp(&p[b].0.abs())))?;
    let river = p[centre].1;
    let left = p[..centre].iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);
    let right = p[centre + 1..].iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);
    let depth = (left.min(right) - river).max(0.0);
    let cross = |h: f64, dir: isize| -> (f64, bool) {
        let level = river + h;
        let mut k = centre as isize;
        loop {
            let next = k + dir;
            if next < 0 || next >= p.len() as isize {
                return (p[k as usize].0, false);
            }
            let (a, b) = (p[k as usize], p[next as usize]);
            if b.1 > level {
                let t = if b.1 > a.1 { (level - a.1) / (b.1 - a.1) } else { 0.0 };
                return (a.0 + (b.0 - a.0) * t.clamp(0.0, 1.0), true);
            }
            k = next;
        }
    };
    let widths = WIDTH_HEIGHTS_M.map(|h| {
        let ((dl, cl), (dr, cr)) = (cross(h, -1), cross(h, 1));
        if cl && cr {
            Width::Closed(dr - dl)
        } else {
            Width::Open(dr - dl)
        }
    });
    Some(Valley { river_m: river, river_d: p[centre].0, depth_m: depth, widths })
}

const PLOT_W: usize = 480;
const PLOT_H: usize = 240;
const GAP: usize = 16;
const TEXT_ROWS: usize = 5;
const S0_RGB: [u8; 3] = [170, 170, 170];
const FINAL_RGB: [u8; 3] = [255, 170, 60];
const DEM_RGB: [u8; 3] = [90, 200, 255];

fn set(img: &mut Rgba, x: i64, y: i64, c: [u8; 3]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height {
        let i = ((y as usize) * img.width as usize + x as usize) * 4;
        img.data[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
    }
}

fn line(img: &mut Rgba, (x0, y0): (i64, i64), (x1, y1): (i64, i64), c: [u8; 3], dashed: bool) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for s in 0..=n {
        if dashed && (s / 4) % 2 == 1 {
            continue;
        }
        let (x, y) = (x0 + (x1 - x0) * s / n, y0 + (y1 - y0) * s / n);
        set(img, x, y, c);
        set(img, x, y + 1, c);
    }
}

/// One panel's inputs: the transect, the surfaces' profiles (`S0`, final; heights as read) and the matched GLO-30 profile.
pub struct Panel<'a> {
    pub t: &'a Transect,
    pub s0: Option<Vec<(f64, f64)>>,
    pub fin: Option<Vec<(f64, f64)>>,
    pub dem: Option<&'a DemProfile>,
}

fn row_text(name: &str, v: Option<Valley>) -> String {
    match v {
        Some(v) => format!("{name} D {:.0} M W10 {} W50 {}", v.depth_m, v.widths[0].text(), v.widths[1].text()),
        None => format!("{name}: NOT DUMPED"),
    }
}

/// The sheet: a title band, then one panel per transect in a row; the chain's surfaces relative to `S0`'s river level, the GLO-30 valley relative to its own river point, all on the
/// panel's one vertical scale (stated as its exaggeration).
pub fn compose(m: &Manifest, panels: &[Panel]) -> Rgba {
    let n = panels.len().max(1);
    let band = TEXT_ROW_H + 4;
    let title = 3 * TEXT_ROW_H + GAP;
    let (w, h) = (GAP + n * (PLOT_W + GAP), title + band + PLOT_H + TEXT_ROWS * TEXT_ROW_H + GAP);
    let mut img = Rgba { width: w as u32, height: h as u32, data: [24u8, 24, 28, 255].repeat(w * h) };
    put_text(
        &mut img,
        GAP,
        4,
        &format!(
            "CROSS-SECTIONS: {}  S0 GREY, FINAL ORANGE, GLO-30 BLUE DASHED  D DEPTH, W10 W50 WIDTH IN KM AT 10 50 M (> OPEN)",
            m.label
        ),
        [240, 240, 240],
    );
    put_text(
        &mut img,
        GAP,
        4 + TEXT_ROW_H,
        &format!(
            "WINDOW L {} {} SIZE {} X {}{}  HEIGHTS: CHAIN TO S0'S RIVER, GLO-30 TO ITS OWN",
            m.x0,
            m.y0,
            m.w,
            m.h,
            m.commit().map_or(String::new(), |c| format!("  COMMIT {c}"))
        ),
        [170, 190, 220],
    );
    put_text(
        &mut img,
        GAP,
        4 + 2 * TEXT_ROW_H,
        "GLO-30: COPERNICUS WORLDDEM-30 (C) DLR E.V. 2010-2014, AIRBUS DS 2014-2018, PROVIDED UNDER COPERNICUS BY THE EU AND ESA",
        [140, 140, 140],
    );
    for (k, p) in panels.iter().enumerate() {
        let ox = GAP + k * (PLOT_W + GAP);
        let oy = title;
        let (v0, vf) = (p.s0.as_deref().and_then(valley), p.fin.as_deref().and_then(valley));
        let vd = p.dem.and_then(|d| valley(&d.samples));
        let base = v0.or(vf).map_or(0.0, |v| v.river_m);
        let curves: Vec<(Vec<(f64, f64)>, [u8; 3], bool)> = [
            p.s0.as_ref().map(|s| (s.iter().map(|&(d, z)| (d, z - base)).collect(), S0_RGB, false)),
            p.fin.as_ref().map(|s| (s.iter().map(|&(d, z)| (d, z - base)).collect(), FINAL_RGB, false)),
            p.dem.map(|d| (d.samples.clone(), DEM_RGB, true)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let dmax = curves.iter().flat_map(|c| c.0.iter().map(|s| s.0.abs())).fold(1.0, f64::max);
        let (lo, hi) = curves.iter().flat_map(|c| c.0.iter().map(|s| s.1)).fold((0.0f64, 1.0f64), |(a, b), z| (a.min(z), b.max(z)));
        let span = (hi - lo).max(1.0) * 1.1;
        let top = hi + 0.05 * span;
        let px = |d: f64| ox as i64 + ((d + dmax) / (2.0 * dmax) * (PLOT_W - 1) as f64).round() as i64;
        let py = |z: f64| (oy + band) as i64 + ((top - z) / span * (PLOT_H - 1) as f64).round() as i64;
        let exag = (2.0 * dmax / PLOT_W as f64) / (span / PLOT_H as f64);
        let is_stale = stale(p.t, v0.as_ref()) == Some(true);
        let head = format!(
            "{} {} {:.0} KM2 RELIEF {:.0} M VX {:.1}{}",
            p.t.window.to_uppercase(),
            p.t.id,
            p.t.area_km2,
            p.t.relief_m,
            exag,
            if is_stale { " STALE" } else { "" }
        );
        put_text(
            &mut img,
            ox,
            oy,
            &head.chars().take(PLOT_W / TEXT_CELL_W).collect::<String>(),
            if is_stale { [255, 90, 90] } else { [230, 230, 230] },
        );
        for y in 0..PLOT_H {
            for x in 0..PLOT_W {
                set(&mut img, (ox + x) as i64, (oy + band + y) as i64, [36, 36, 42]);
            }
        }
        line(&mut img, (px(-dmax), py(0.0)), (px(dmax), py(0.0)), [70, 70, 80], false);
        line(&mut img, (px(0.0), py(top)), (px(0.0), py(top - span)), [70, 70, 80], true);
        for (pts, c, dashed) in &curves {
            for s in pts.windows(2) {
                line(&mut img, (px(s[0].0), py(s[0].1)), (px(s[1].0), py(s[1].1)), *c, *dashed);
            }
        }
        let ty = oy + band + PLOT_H + 4;
        let dem_name = p.dem.map_or("GLO".to_string(), |d| d.tile.clone());
        for (r, (text, c)) in [
            (row_text("S0", v0), S0_RGB),
            (row_text("FINAL", vf), FINAL_RGB),
            (if p.dem.is_some() { row_text(&dem_name, vd) } else { "GLO-30 NO MATCH".into() }, DEM_RGB),
        ]
        .iter()
        .enumerate()
        {
            put_text(&mut img, ox, ty + r * TEXT_ROW_H, &text.chars().take(PLOT_W / TEXT_CELL_W).collect::<String>(), *c);
        }
    }
    img
}

/// The table of a sheet: one line per transect and profile, `TRANSECT label window id profile river_m depth_m width10_m width50_m` (a width `N open`: at least N m), the `s0`
/// line ending in `stale` when the transect is stale ([`stale`]).
pub fn table(m: &Manifest, panels: &[Panel]) -> String {
    let mut t = String::from(
        "# label window id profile river_m depth_m width10_m width50_m (BUILD-061; a width 'N open' is at least N m: one side never rises that high; 'stale': S0's depth is under 10 % of its depth at the pick)\n",
    );
    // the label as one token: its spaces as '-', '-' when there is none
    let label = if m.label.is_empty() { "-".to_string() } else { m.label.replace(char::is_whitespace, "-") };
    for p in panels {
        let rows: [(&str, Option<Valley>); 3] = [
            ("s0", p.s0.as_deref().and_then(valley)),
            ("final", p.fin.as_deref().and_then(valley)),
            ("glo30", p.dem.and_then(|d| valley(&d.samples))),
        ];
        for (name, v) in rows {
            let name = if name == "glo30" { p.dem.map_or("glo30".to_string(), |d| format!("glo30:{}", d.tile)) } else { name.to_string() };
            match v {
                Some(v) => {
                    t += &format!(
                        "TRANSECT {} {} {} {name} {:.2} {:.2} {} {}{}\n",
                        &label,
                        p.t.window,
                        p.t.id,
                        v.river_m,
                        v.depth_m,
                        v.widths[0].table(),
                        v.widths[1].table(),
                        if name == "s0" && stale(p.t, Some(&v)) == Some(true) { " stale" } else { "" }
                    )
                }
                None => t += &format!("TRANSECT {} {} {} {name} missing\n", &label, p.t.window, p.t.id),
            }
        }
    }
    t
}

/// Reads a chain directory, writes the sheet to `out` and its table beside it (`.txt`); returns the transects drawn and the sheet's size. A window that holds no transect is refused,
/// naming its face and box.
pub fn render(
    dir: &Path,
    face: Option<u8>,
    out: &Path,
    transects: &[Transect],
    dem: &BTreeMap<(String, String), DemProfile>,
) -> Result<(usize, u32, u32), String> {
    let mp = dir.join("chain.txt");
    let m = Manifest::parse(&std::fs::read_to_string(&mp).map_err(|e| format!("{}: {e}", mp.display()))?)?;
    let face = face.or_else(|| chain_face(&m)).ok_or("chain.txt has no 'switch window F,...' line: give the face with --face F")?;
    let chosen = in_window(transects, &m, face);
    if chosen.is_empty() {
        return Err(format!(
            "no transect lies inside face {face}, L cells {}..{} x {}..{}",
            m.x0,
            m.x0 + m.w as u64,
            m.y0,
            m.y0 + m.h as u64
        ));
    }
    let n = m.w * m.h;
    let (s0, fin) = (read_f32(dir, "s0.f32", n)?, read_f32(dir, "final.f32", n)?);
    let panels: Vec<Panel> = chosen
        .iter()
        .map(|t| Panel {
            t,
            s0: s0.as_deref().map(|z| profile(z, &m, t)),
            fin: fin.as_deref().map(|z| profile(z, &m, t)),
            dem: dem.get(&(t.window.clone(), t.id.clone())),
        })
        .collect();
    let img = compose(&m, &panels);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    img.write_png(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let tp = out.with_extension("txt");
    std::fs::write(&tp, table(&m, &panels)).map_err(|e| format!("{}: {e}", tp.display()))?;
    Ok((panels.len(), img.width, img.height))
}

/// The default output: `target/review/stages/transects/<label>.png` (the label's other characters than letters, digits and `-` as `-`), or `transects.png` without a label.
pub fn default_out(root: &Path, m: &Manifest) -> PathBuf {
    let name: String = m.label.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    root.join("target/review/stages/transects").join(format!("{}.png", if name.is_empty() { "transects".to_string() } else { name }))
}
