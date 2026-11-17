//! The chain sheet (BUILD-060, `cargo xtask stages --chain DIR`): one PNG per window showing every stage of the river-graph chain side by side, at one fixed scale and a low sun.
//! It reads only the chain layout below, so any run that writes the layout can be rendered; it knows nothing of the code that wrote it.
//!
//! The layout of a chain directory:
//! - `chain.txt`: `stage-chain 1`, then `window X0 Y0 W H` (the window's L grid: its first cell's global face-local L coordinates and its size), `coarse C` (L cells per H1 cell),
//!   `cell_m M` (an L cell's side), `sea_m S` (sea level) and `label TEXT`, one per line;
//! - L grid surfaces, `W x H` little-endian f32, row-major: `base.f32` (the H1 base read at L), `s0.f32` (`S0`), `kh.f32` (KH, the surface KS starts from), `ks.f32` (after KS),
//!   `kv.f32` (after KV), `final.f32` (after KC, the chain's output);
//! - H1 grid fields, `W/C x H/C`, row-major: `zs.f32` (`z_s`, f32) and `h1-rivers.u8` (1 on an H1 river cell, else 0);
//! - `graph.txt`: the canonical river graph (GEN-189);
//! - `stages/`: BUILD-060's per-stage fields (`ks|kv|kc-lower.u16`, `kv|kc-fill.u16`).
//!
//! Every file but `chain.txt` is optional: a missing one leaves its panel grey and says `not dumped`. A file of the wrong size is refused, naming it and both sizes.

use std::path::{Path, PathBuf};

use planet_generators::bake::river_graph::{Graph, Level, Represented};
use planet_generators::dump::Rgba;
use planet_generators::preview::{put_text, TEXT_ROW_H};

use crate::river_gate_graph::read_graph;

/// The sun of every panel: azimuth (degrees clockwise from north, the window's -y) and altitude (owner, 2026-11-17: 10 degrees).
pub const SUN_AZ_DEG: f64 = 315.0;
pub const SUN_ALT_DEG: f64 = 10.0;
/// The shade's fixed contrast: grey = 0.5 + GAIN * (shade - sin(altitude)), so a flat cell is mid-grey in every panel and run.
pub const GAIN: f64 = 2.0;
/// The change and lowering scales (m): the colour saturates there.
pub const CHANGE_M: f64 = 50.0;
pub const LOWER_M: f64 = 20.0;

/// The layout's manifest (`chain.txt`).
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub x0: u64,
    pub y0: u64,
    pub w: usize,
    pub h: usize,
    pub coarse: usize,
    pub cell_m: f64,
    pub sea_m: f64,
    pub label: String,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines.next() != Some("stage-chain 1") {
            return Err("chain.txt: the first line is not `stage-chain 1`".into());
        }
        let (mut win, mut coarse, mut cell_m, mut sea_m, mut label) = (None, None, None, None, None);
        for l in lines {
            let (k, v) = l.split_once(' ').unwrap_or((l, ""));
            let num = |s: &str| s.parse::<f64>().map_err(|_| format!("chain.txt: `{l}`: not a number"));
            match k {
                "window" => {
                    let p: Vec<&str> = v.split_whitespace().collect();
                    let u = |i: usize| p.get(i).and_then(|s| s.parse::<u64>().ok()).ok_or(format!("chain.txt: `{l}`: window X0 Y0 W H"));
                    win = Some((u(0)?, u(1)?, u(2)? as usize, u(3)? as usize));
                }
                "coarse" => coarse = Some(v.parse::<usize>().map_err(|_| format!("chain.txt: `{l}`: coarse C"))?),
                "cell_m" => cell_m = Some(num(v)?),
                "sea_m" => sea_m = Some(num(v)?),
                "label" => label = Some(v.to_string()),
                _ => return Err(format!("chain.txt: unknown line `{l}`")),
            }
        }
        let (x0, y0, w, h) = win.ok_or("chain.txt: no window line")?;
        let coarse = coarse.ok_or("chain.txt: no coarse line")?;
        if coarse == 0 || w == 0 || h == 0 || w % coarse != 0 || h % coarse != 0 {
            return Err(format!("chain.txt: the window {w} x {h} is not a positive multiple of coarse {coarse}"));
        }
        Ok(Manifest {
            x0,
            y0,
            w,
            h,
            coarse,
            cell_m: cell_m.ok_or("chain.txt: no cell_m line")?,
            sea_m: sea_m.unwrap_or(0.0),
            label: label.unwrap_or_default(),
        })
    }

    pub fn text(&self) -> String {
        format!(
            "stage-chain 1\nwindow {} {} {} {}\ncoarse {}\ncell_m {}\nsea_m {}\nlabel {}\n",
            self.x0, self.y0, self.w, self.h, self.coarse, self.cell_m, self.sea_m, self.label
        )
    }
}

/// The part of the window a sheet shows and its scale: `size x size` L cells from `(x, y)`, `step` L cells per pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: usize,
    pub y: usize,
    pub size_x: usize,
    pub size_y: usize,
    pub step: usize,
}

impl View {
    /// The whole window at `step`.
    pub fn whole(m: &Manifest, step: usize) -> View {
        View { x: 0, y: 0, size_x: m.w, size_y: m.h, step: step.max(1) }
    }
    pub fn px(&self) -> (usize, usize) {
        (self.size_x / self.step, self.size_y / self.step)
    }
    /// The pixel of a window L cell, if the view shows it.
    pub fn pixel(&self, cx: usize, cy: usize) -> Option<(usize, usize)> {
        let (pw, ph) = self.px();
        let (px, py) = (cx.checked_sub(self.x)? / self.step, cy.checked_sub(self.y)? / self.step);
        (px < pw && py < ph).then_some((px, py))
    }
}

fn read_bytes(dir: &Path, name: &str, len: usize) -> Result<Option<Vec<u8>>, String> {
    let p = dir.join(name);
    if !p.exists() {
        return Ok(None);
    }
    let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    if b.len() != len {
        return Err(format!("{}: {} bytes, the layout's are {len}", p.display(), b.len()));
    }
    Ok(Some(b))
}

/// An f32 field of `n` cells, or none when the file is missing.
pub fn read_f32(dir: &Path, name: &str, n: usize) -> Result<Option<Vec<f32>>, String> {
    Ok(read_bytes(dir, name, 4 * n)?.map(|b| b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()))
}

fn read_u16(dir: &Path, name: &str, n: usize) -> Result<Option<Vec<u16>>, String> {
    Ok(read_bytes(dir, name, 2 * n)?.map(|b| b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect()))
}

/// The shade of a surface at every pixel of `v`: the mean over the pixel's L cells of the cell's hillshade (central differences, the sun of `SUN_*`), as grey 0..1 at the fixed `GAIN`,
/// and the pixel's mean height.
pub fn shade(z: &[f32], m: &Manifest, v: &View) -> (Vec<f64>, Vec<f64>) {
    let (pw, ph) = v.px();
    let (az, alt) = (SUN_AZ_DEG.to_radians(), SUN_ALT_DEG.to_radians());
    // the light's direction: x east (the window's +x), y north (the window's -y), z up
    let (lx, ly, lz) = (libm::cos(alt) * libm::sin(az), libm::cos(alt) * libm::cos(az), libm::sin(alt));
    let at = |x: usize, y: usize| f64::from(z[y.min(m.h - 1) * m.w + x.min(m.w - 1)]);
    let mut grey = vec![0.0; pw * ph];
    let mut mean = vec![0.0; pw * ph];
    for py in 0..ph {
        for px in 0..pw {
            let (mut s, mut hsum) = (0.0, 0.0);
            for dy in 0..v.step {
                for dx in 0..v.step {
                    let (x, y) = (v.x + px * v.step + dx, v.y + py * v.step + dy);
                    let dzdx = (at(x + 1, y) - at(x.saturating_sub(1), y)) / (2.0 * m.cell_m);
                    // north is -y
                    let dzdn = (at(x, y.saturating_sub(1)) - at(x, y + 1)) / (2.0 * m.cell_m);
                    let norm = (dzdx * dzdx + dzdn * dzdn + 1.0).sqrt();
                    s += ((-dzdx * lx - dzdn * ly + lz) / norm).max(0.0);
                    hsum += at(x, y);
                }
            }
            let k = (v.step * v.step) as f64;
            grey[py * pw + px] = (0.5 + GAIN * (s / k - lz)).clamp(0.0, 1.0);
            mean[py * pw + px] = hsum / k;
        }
    }
    (grey, mean)
}

/// The colour of a shaded pixel: grey land, blue-tinted sea at or below `sea_m`.
fn base_colour(g: f64, h: f64, sea_m: f64) -> [f64; 3] {
    if h <= sea_m {
        [0.15 + 0.3 * g, 0.25 + 0.35 * g, 0.45 + 0.4 * g]
    } else {
        [0.25 + 0.7 * g, 0.24 + 0.68 * g, 0.22 + 0.64 * g]
    }
}

fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Lowering colour (red) and raising colour (blue) of the change and stage overlays.
pub const LOWER: [f64; 3] = [0.85, 0.1, 0.1];
pub const RAISE: [f64; 3] = [0.1, 0.35, 0.95];

/// A panel: its pixels (RGB 0..1) and its label.
pub struct Panel {
    pub label: String,
    pub rgb: Option<Vec<[f64; 3]>>,
}

fn shaded(z: &[f32], m: &Manifest, v: &View) -> Vec<[f64; 3]> {
    let (g, h) = shade(z, m, v);
    g.iter().zip(&h).map(|(&g, &h)| base_colour(g, h, m.sea_m)).collect()
}

/// The pixel mean of `f(cell)` over each pixel of `v`.
fn pixel_mean(m: &Manifest, v: &View, f: &dyn Fn(usize) -> f64) -> Vec<f64> {
    let (pw, ph) = v.px();
    let mut out = vec![0.0; pw * ph];
    for py in 0..ph {
        for px in 0..pw {
            let mut s = 0.0;
            for dy in 0..v.step {
                for dx in 0..v.step {
                    s += f((v.y + py * v.step + dy) * m.w + v.x + px * v.step + dx);
                }
            }
            out[py * pw + px] = s / (v.step * v.step) as f64;
        }
    }
    out
}

/// Tints `rgb` by a signed change per pixel (negative = lowered, red; positive = raised, blue), saturating at `scale_m`.
fn tint_change(rgb: &mut [[f64; 3]], change: &[f64], scale_m: f64) {
    for (c, &d) in rgb.iter_mut().zip(change) {
        if d < 0.0 {
            *c = mix(*c, LOWER, -d / scale_m);
        } else if d > 0.0 {
            *c = mix(*c, RAISE, d / scale_m);
        }
    }
}

/// The coarse grid's value at a pixel (nearest H1 cell of the pixel's centre).
fn coarse_at<T: Copy>(f: &[T], m: &Manifest, v: &View, px: usize, py: usize) -> T {
    let (cw, ch) = (m.w / m.coarse, m.h / m.coarse);
    let (x, y) = (v.x + px * v.step + v.step / 2, v.y + py * v.step + v.step / 2);
    f[(y / m.coarse).min(ch - 1) * cw + (x / m.coarse).min(cw - 1)]
}

/// The window L cell of a graph vertex: an L id is the cell itself; an H1 id is its H1 cell's centre.
fn vertex_cell(m: &Manifest, level: Level, id: u64) -> Option<(usize, usize)> {
    let (gx, gy) = (id & 0xffff_ffff, id >> 32);
    let (gx, gy) = match level {
        Level::L => (gx, gy),
        Level::H1 => (gx * m.coarse as u64 + m.coarse as u64 / 2, gy * m.coarse as u64 + m.coarse as u64 / 2),
    };
    let (x, y) = (gx.checked_sub(m.x0)? as usize, gy.checked_sub(m.y0)? as usize);
    (x < m.w && y < m.h).then_some((x, y))
}

/// Graph overlay colours.
pub const TRUNK: [f64; 3] = [0.1, 0.45, 1.0];
pub const TRIB: [f64; 3] = [0.2, 0.85, 0.95];
pub const BASIN_KEPT: [f64; 3] = [0.95, 0.2, 0.85];
pub const BASIN_OTHER: [f64; 3] = [0.95, 0.85, 0.2];

fn put(rgb: &mut [[f64; 3]], v: &View, cell: (usize, usize), r: usize, colour: [f64; 3]) {
    let (pw, ph) = v.px();
    if let Some((px, py)) = v.pixel(cell.0, cell.1) {
        for y in py.saturating_sub(r)..=(py + r).min(ph - 1) {
            for x in px.saturating_sub(r)..=(px + r).min(pw - 1) {
                rgb[y * pw + x] = colour;
            }
        }
    }
}

/// A tributary is drawn when its drained area (its last vertex's `a_geo`) is at least this many pixels' area, so the overlay keeps the terrain readable at every scale.
pub const TRIB_PIXELS: f64 = 25.0;

/// The smallest drained area of a drawn tributary at view `v` (m2).
pub fn trib_min_m2(m: &Manifest, v: &View) -> f64 {
    let side = v.step as f64 * m.cell_m;
    TRIB_PIXELS * side * side
}

/// Which reaches an overlay draws.
#[derive(Clone, Copy, PartialEq)]
enum Reaches {
    Trunks,
    Tributaries,
    All,
}

fn draw_reaches(rgb: &mut [[f64; 3]], m: &Manifest, v: &View, g: &Graph, which: Reaches) {
    for r in &g.reaches {
        let trunk = r.level == Level::H1 || r.parent.is_some();
        let big = r.attrs.a_geo.last().is_some_and(|&a| a >= trib_min_m2(m, v));
        let draw = match which {
            Reaches::Trunks => trunk,
            Reaches::Tributaries => !trunk && big,
            Reaches::All => trunk || big,
        };
        if draw {
            for &id in &r.vertices {
                if let Some(c) = vertex_cell(m, r.level, id) {
                    put(rgb, v, c, 0, if trunk { TRUNK } else { TRIB });
                }
            }
        }
    }
}

fn draw_basins(rgb: &mut [[f64; 3]], m: &Manifest, v: &View, g: &Graph) {
    for b in &g.basins {
        if let Some(c) = vertex_cell(m, b.level, b.floor_cell) {
            put(rgb, v, c, usize::from(v.step < 4), if b.represented == Represented::None { BASIN_OTHER } else { BASIN_KEPT });
        }
    }
}

/// The twelve panels of a chain directory, in the owner's order (lead #167a): the base, `z_s`, H1 rivers, `S0`, KH, KS, the trunk graph, tributaries and basins, KV, KC,
/// the final surface (graph on top) and its change against `S0`.
pub fn panels(dir: &Path, m: &Manifest, v: &View) -> Result<Vec<Panel>, String> {
    let n = m.w * m.h;
    let nc = n / (m.coarse * m.coarse);
    let surf = |name: &str| read_f32(dir, &format!("{name}.f32"), n);
    let stage = |name: &str| read_u16(&dir.join("stages"), &format!("{name}.u16"), n);
    let graph = {
        let p = dir.join("graph.txt");
        if p.exists() {
            Some(read_graph(&p.to_string_lossy())?)
        } else {
            None
        }
    };
    let (pw, ph) = v.px();
    let mut out = Vec::new();
    let plain = |z: &Option<Vec<f32>>| z.as_ref().map(|z| shaded(z, m, v));

    let base = surf("base")?;
    out.push(Panel { label: "1 BASE (H1 BASE AT L)".into(), rgb: plain(&base) });
    drop(base);

    let zs = read_f32(dir, "zs.f32", nc)?;
    let rivers = read_bytes(dir, "h1-rivers.u8", nc)?;
    let zs_rgb = zs.as_ref().map(|zs| {
        // z_s held on the L grid by its H1 cell, shaded at L (its 814 m steps show as such)
        let held: Vec<f32> = (0..n).map(|p| zs[(p / m.w / m.coarse) * (m.w / m.coarse) + (p % m.w) / m.coarse]).collect();
        shaded(&held, m, v)
    });
    out.push(Panel { label: format!("2 Z_S ({:.0} M)", m.cell_m * m.coarse as f64), rgb: zs_rgb.clone() });
    out.push(Panel {
        label: "3 H1 RIVERS ON Z_S".into(),
        rgb: match (&zs_rgb, &rivers) {
            (Some(rgb), Some(riv)) => {
                let mut rgb = rgb.clone();
                for py in 0..ph {
                    for px in 0..pw {
                        if coarse_at(riv, m, v, px, py) != 0 {
                            rgb[py * pw + px] = TRUNK;
                        }
                    }
                }
                Some(rgb)
            }
            _ => None,
        },
    });
    drop((zs, rivers, zs_rgb));

    let s0 = surf("s0")?;
    out.push(Panel { label: "4 S0".into(), rgb: plain(&s0) });
    let kh = surf("kh")?;
    out.push(Panel { label: "5 KH".into(), rgb: plain(&kh) });
    let ks = surf("ks")?;
    let ks_rgb = plain(&ks);
    out.push(Panel {
        label: format!("6 KS (RED: KS LOWERING, {LOWER_M:.0} M FULL)"),
        rgb: match (&ks_rgb, &kh, &ks) {
            (Some(rgb), Some(kh), Some(ks)) => {
                let mut rgb = rgb.clone();
                tint_change(&mut rgb, &pixel_mean(m, v, &|p| f64::from(ks[p]) - f64::from(kh[p])), LOWER_M);
                Some(rgb)
            }
            (rgb, ..) => rgb.clone(),
        },
    });
    drop(kh);
    let with_graph = |rgb: &Option<Vec<[f64; 3]>>, f: &dyn Fn(&mut Vec<[f64; 3]>, &Graph)| match (rgb, &graph) {
        (Some(rgb), Some(g)) => {
            let mut rgb = rgb.clone();
            f(&mut rgb, g);
            Some(rgb)
        }
        _ => None,
    };
    out.push(Panel {
        label: "7 TRUNK GRAPH (KG-TRUNK) ON KS".into(),
        rgb: with_graph(&ks_rgb, &|rgb, g| draw_reaches(rgb, m, v, g, Reaches::Trunks)),
    });
    out.push(Panel {
        label: format!("8 TRIBUTARIES (A >= {:.1} KM2), BASINS (MAGENTA KEPT)", trib_min_m2(m, v) / 1.0e6),
        rgb: with_graph(&ks_rgb, &|rgb, g| {
            draw_reaches(rgb, m, v, g, Reaches::Tributaries);
            draw_basins(rgb, m, v, g);
        }),
    });
    drop((ks, ks_rgb));

    let stage_panel = |label: String, z: &Option<Vec<f32>>, st: &str| -> Result<Panel, String> {
        let (lower, fill) = (stage(&format!("{st}-lower"))?, stage(&format!("{st}-fill"))?);
        let rgb = z.as_ref().map(|z| {
            let mut rgb = shaded(z, m, v);
            let d =
                pixel_mean(m, v, &|p| 0.1 * (f64::from(fill.as_ref().map_or(0, |f| f[p])) - f64::from(lower.as_ref().map_or(0, |l| l[p]))));
            tint_change(&mut rgb, &d, LOWER_M);
            rgb
        });
        Ok(Panel { label, rgb })
    };
    let kv = surf("kv")?;
    out.push(stage_panel(format!("9 KV (RED LOWER, BLUE FILL, {LOWER_M:.0} M FULL)"), &kv, "kv")?);
    drop(kv);
    let fin = surf("final")?;
    out.push(stage_panel(format!("10 KC (RED LOWER, BLUE FILL, {LOWER_M:.0} M FULL)"), &fin, "kc")?);
    let fin_rgb = plain(&fin);
    out.push(Panel {
        label: format!("11 FINAL, TRUNKS AND TRIBUTARIES (A >= {:.1} KM2)", trib_min_m2(m, v) / 1.0e6),
        rgb: with_graph(&fin_rgb, &|rgb, g| draw_reaches(rgb, m, v, g, Reaches::All)).or(fin_rgb),
    });
    out.push(Panel {
        label: format!("12 FINAL - S0 (RED LOWER, BLUE RAISE, {CHANGE_M:.0} M FULL)"),
        rgb: match (&fin, &s0) {
            (Some(f), Some(s)) => {
                let (g, h) = shade(f, m, v);
                let mut rgb: Vec<[f64; 3]> =
                    g.iter().zip(&h).map(|(&g, &h)| if h <= m.sea_m { base_colour(g, h, m.sea_m) } else { [0.3 + 0.5 * g; 3] }).collect();
                tint_change(&mut rgb, &pixel_mean(m, v, &|p| f64::from(f[p]) - f64::from(s[p])), CHANGE_M);
                Some(rgb)
            }
            _ => None,
        },
    });
    Ok(out)
}

/// Columns of the sheet; the gap between panels and the label band (pixels).
pub const COLUMNS: usize = 4;
const GAP: usize = 8;

/// The sheet: a title band (the label, the sun, the scale and a 50 km bar), then the panels in rows of `COLUMNS`, each under its label band.
pub fn compose(m: &Manifest, v: &View, panels: &[Panel]) -> Rgba {
    let (pw, ph) = v.px();
    let rows = panels.len().div_ceil(COLUMNS);
    let title = 2 * TEXT_ROW_H + GAP;
    let band = TEXT_ROW_H + 4;
    let (w, h) = (GAP + COLUMNS * (pw + GAP), title + rows * (band + ph + GAP));
    let mut img = Rgba { width: w as u32, height: h as u32, data: [24u8, 24, 28, 255].repeat(w * h) };
    let km_px = m.cell_m * v.step as f64 / 1000.0;
    put_text(
        &mut img,
        GAP,
        4,
        &format!("STAGE CHAIN: {}  SUN AZ {SUN_AZ_DEG:.0} ALT {SUN_ALT_DEG:.0}  1 PX = {km_px:.2} KM  BAR 50 KM", m.label),
        [240, 240, 240],
    );
    put_text(
        &mut img,
        GAP,
        4 + TEXT_ROW_H,
        &format!(
            "WINDOW L {} {} SIZE {} X {}  VIEW {} {} SIZE {} X {} STEP {}",
            m.x0, m.y0, m.w, m.h, v.x, v.y, v.size_x, v.size_y, v.step
        ),
        [170, 190, 220],
    );
    let bar = (50.0 / km_px).round() as usize;
    let bx = w.saturating_sub(GAP + bar);
    for y in 6..12 {
        for x in bx..(bx + bar).min(w) {
            img.data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[240, 240, 240]);
        }
    }
    for (k, p) in panels.iter().enumerate() {
        let (ox, oy) = (GAP + (k % COLUMNS) * (pw + GAP), title + (k / COLUMNS) * (band + ph + GAP));
        let missing = p.rgb.is_none();
        put_text(&mut img, ox, oy, &if missing { format!("{} - NOT DUMPED", p.label) } else { p.label.clone() }, [230, 230, 230]);
        for y in 0..ph {
            for x in 0..pw {
                let c = p.rgb.as_ref().map_or([0.35; 3], |rgb| rgb[y * pw + x]);
                let i = ((oy + band + y) * w + ox + x) * 4;
                for (ch, v) in c.iter().enumerate() {
                    img.data[i + ch] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
    }
    img
}

/// Reads a chain directory and writes its sheet to `out`; returns the sheet's size in pixels.
pub fn render(dir: &Path, view: Option<View>, step: usize, out: &Path) -> Result<(u32, u32), String> {
    let mp = dir.join("chain.txt");
    let m = Manifest::parse(&std::fs::read_to_string(&mp).map_err(|e| format!("{}: {e}", mp.display()))?)?;
    let v = view.unwrap_or_else(|| View::whole(&m, step));
    if v.x + v.size_x > m.w || v.y + v.size_y > m.h || v.size_x < v.step || v.size_y < v.step {
        return Err(format!("the view {v:?} is not inside the window {} x {} or is smaller than a pixel", m.w, m.h));
    }
    let img = compose(&m, &v, &panels(dir, &m, &v)?);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    img.write_png(out).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok((img.width, img.height))
}

/// The default output: `target/review/stages/chain/<label>.png` (the label's spaces as `-`), or `chain.png` without a label.
pub fn default_out(root: &Path, m: &Manifest) -> PathBuf {
    let name: String = m.label.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    root.join("target/review/stages/chain").join(format!("{}.png", if name.is_empty() { "chain".to_string() } else { name }))
}
