//! The look sheet (BUILD-067, `cargo xtask look-sheet RUN [--against DEFAULTS] [--az 315,135] [--out DIR]`): a switch row's crops at full resolution under a fixed low sun, beside
//! the defaults row and their difference. It extends the chain sheet (BUILD-060) and reads only its layout: a crop is a small chain directory, so the chain sheet's shade and colours
//! are the look sheet's, and `cargo xtask stages --chain` renders a crop directory as well.
//!
//! A crop store (the chain hook `--probe-crops`) is a directory holding:
//! - `crops.txt`: `look-crops 1`, then one `crop NAME X Y SIZE BX BY` a crop, in the sheet's row order: the crop's `SIZE x SIZE` L cells from window cell `(X, Y)`, cut into a box
//!   from window cell `(BX, BY)` aligned to the H1 grid and one H1 cell larger on each side (held inside the window), so the shade reads the crop's border cells as the full window does;
//! - one chain directory a crop, `NAME/`, with the chain's `chain.txt` (its window line the box's) and the box's cut of every field the chain wrote (L fields, H1 fields and `stages/`).
//!
//! Sheets go to `target/review/look/<run>/` and are committed only at a gate, under `docs/stages/sheets/<gate>/` (BUILD-048's caps).

use std::path::{Path, PathBuf};

use planet_generators::dump::Rgba;
use planet_generators::preview::{put_text, TEXT_CELL_W, TEXT_ROW_H};

use crate::stage_sheet::{base_colour, read_f32, shade_at, tint_change, Manifest, View, SUN_ALT_DEG};

/// A crop of a window: `size x size` L cells from window cell `(x, y)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crop {
    pub name: String,
    pub x: usize,
    pub y: usize,
    pub size: usize,
}

/// The largest crop side (L cells).
pub const MAX_CROP: usize = 128;
/// The per-sheet cap (bytes), BUILD-048's.
pub const SHEET_LIMIT: u64 = 1_000_000;
/// The difference row's scale (m): the tint saturates there.
pub const DIFF_M: f64 = 10.0;
/// The default azimuths (degrees clockwise from north).
pub const AZIMUTHS: [f64; 2] = [315.0, 135.0];
/// The sheet's columns: the chain's surfaces in the chain's order (SPIKE-143's layout). A stage the run did not write stays grey, labelled `not dumped`, so every sheet has one layout.
pub const STAGES: [(&str, &str); 8] = [
    ("base", "B"),
    ("s0", "S0"),
    ("kh", "S_KH"),
    ("ks0", "KS0"),
    ("ks1c", "KS1+C"),
    ("band6c", "BAND6+C"),
    ("kv", "KV"),
    ("final", "FINAL"),
];
/// The row label column, in glyphs.
pub const LABEL_CHARS: usize = 8;
const GAP: usize = 8;

/// Reads a crops file of SPIKE-143's form, `{"size": N, "<name>": [x, y], ...}`: keys whose value is not a pair of whole numbers (such as `hill_x8`) are skipped; the crops keep the
/// file's order.
pub fn parse_crops_json(text: &str) -> Result<Vec<Crop>, String> {
    let body = text.trim().strip_prefix('{').and_then(|t| t.strip_suffix('}')).ok_or("crops: not a JSON object")?;
    let mut size = None;
    let mut pairs: Vec<(String, usize, usize)> = Vec::new();
    let mut rest = body;
    while let Some(q) = rest.find('"') {
        let after = &rest[q + 1..];
        let e = after.find('"').ok_or("crops: an unclosed key")?;
        let key = after[..e].to_string();
        let v = after[e + 1..].trim_start().strip_prefix(':').ok_or(format!("crops: no ':' after \"{key}\""))?.trim_start();
        let (value, tail) = match v.strip_prefix('[') {
            Some(arr) => {
                let c = arr.find(']').ok_or(format!("crops: \"{key}\": an unclosed list"))?;
                (&arr[..c], &arr[c + 1..])
            }
            None => {
                let c = v.find(',').unwrap_or(v.len());
                (&v[..c], &v[c..])
            }
        };
        if key == "size" {
            size = Some(value.trim().parse::<usize>().map_err(|_| format!("crops: size `{}` is not a whole number", value.trim()))?);
        } else if v.starts_with('[') {
            let n: Vec<&str> = value.split(',').map(str::trim).collect();
            if let [a, b] = n[..] {
                if let (Ok(x), Ok(y)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    pairs.push((key, x, y));
                }
            }
        }
        rest = tail;
    }
    let size = size.ok_or("crops: no size")?;
    if size == 0 || size > MAX_CROP {
        return Err(format!("crops: size {size} is outside 1 to {MAX_CROP} L cells"));
    }
    if pairs.is_empty() {
        return Err("crops: no crop (a key with a pair of whole numbers)".into());
    }
    Ok(pairs.into_iter().map(|(name, x, y)| Crop { name, x, y, size }).collect())
}

/// The box a crop is cut into, `(bx, by, bw, bh)` in window cells: aligned to the H1 grid of `coarse` cells, one H1 cell larger than the crop on each side, held inside the window.
pub fn crop_box(w: usize, h: usize, coarse: usize, c: &Crop) -> Result<(usize, usize, usize, usize), String> {
    if c.size == 0 || c.x + c.size > w || c.y + c.size > h {
        return Err(format!("crop {} ({}, {}) of {} cells is not inside the window {w} x {h}", c.name, c.x, c.y, c.size));
    }
    let bx = (c.x / coarse).saturating_sub(1) * coarse;
    let by = (c.y / coarse).saturating_sub(1) * coarse;
    let ex = ((c.x + c.size).div_ceil(coarse) + 1).min(w / coarse) * coarse;
    let ey = ((c.y + c.size).div_ceil(coarse) + 1).min(h / coarse) * coarse;
    Ok((bx, by, ex - bx, ey - by))
}

/// Writes a chain's fields as crops while the chain runs (the chain hook `--probe-crops`), so the crops outlive a deletion of the full dumps.
pub struct CropWriter {
    root: PathBuf,
    w: usize,
    h: usize,
    coarse: usize,
    crops: Vec<(Crop, (usize, usize, usize, usize))>,
}

impl CropWriter {
    /// A store at `root` for a window of `w x h` L cells and `coarse` L cells an H1 cell; writes `crops.txt` and makes the crop directories.
    pub fn new(root: &Path, w: usize, h: usize, coarse: usize, crops: &[Crop]) -> Result<CropWriter, String> {
        if coarse == 0 || w % coarse != 0 || h % coarse != 0 {
            return Err(format!("the window {w} x {h} is not a multiple of coarse {coarse}"));
        }
        let mut boxed = Vec::new();
        let mut text = String::from("look-crops 1\n");
        for c in crops {
            if c.name.is_empty() || !c.name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
                return Err(format!("crop name `{}`: letters, digits, '-' and '_' only", c.name));
            }
            if boxed.iter().any(|(b, _): &(Crop, _)| b.name == c.name) {
                return Err(format!("crop name `{}` given twice", c.name));
            }
            let b = crop_box(w, h, coarse, c)?;
            text.push_str(&format!("crop {} {} {} {} {} {}\n", c.name, c.x, c.y, c.size, b.0, b.1));
            std::fs::create_dir_all(root.join(&c.name).join("stages")).map_err(|e| format!("{}: {e}", root.display()))?;
            boxed.push((c.clone(), b));
        }
        std::fs::write(root.join("crops.txt"), text).map_err(|e| format!("{}: {e}", root.display()))?;
        Ok(CropWriter { root: root.to_path_buf(), w, h, coarse, crops: boxed })
    }

    /// Cuts one field of the chain (`name` as in the chain layout, such as `s0.f32` or `stages/kv-lower.u16`) into every crop: an L field of 1, 2, 4 or 8 bytes a cell, or an H1 field
    /// of as many bytes an H1 cell; any other length is refused, naming the field.
    pub fn field(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let (nl, k) = (self.w * self.h, self.coarse);
        let nh = (self.w / k) * (self.h / k);
        let (fw, scale, elem) = if bytes.len() % nl == 0 && [1, 2, 4, 8].contains(&(bytes.len() / nl)) {
            (self.w, 1, bytes.len() / nl)
        } else if bytes.len() % nh == 0 && [1, 2, 4, 8].contains(&(bytes.len() / nh)) {
            (self.w / k, k, bytes.len() / nh)
        } else {
            return Err(format!("{name}: {} bytes are neither an L field of {nl} cells nor an H1 field of {nh} cells", bytes.len()));
        };
        for (c, (bx, by, bw, bh)) in &self.crops {
            let (x0, y0, cw, ch) = (bx / scale, by / scale, bw / scale, bh / scale);
            let mut out = Vec::with_capacity(cw * ch * elem);
            for y in y0..y0 + ch {
                out.extend_from_slice(&bytes[(y * fw + x0) * elem..(y * fw + x0 + cw) * elem]);
            }
            let p = self.root.join(&c.name).join(name);
            std::fs::write(&p, out).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }

    /// Writes each crop's `chain.txt`: the chain's manifest with the box's window line (the box's first cell's global L coordinates and its size) and the crop's name after the label.
    pub fn manifest(&self, chain_text: &str) -> Result<(), String> {
        let m = Manifest::parse(chain_text)?;
        if (m.w, m.h, m.coarse) != (self.w, self.h, self.coarse) {
            return Err(format!(
                "the chain's window {} x {} (coarse {}) is not the store's {} x {} (coarse {})",
                m.w, m.h, m.coarse, self.w, self.h, self.coarse
            ));
        }
        for (c, (bx, by, bw, bh)) in &self.crops {
            let cm = Manifest {
                x0: m.x0 + *bx as u64,
                y0: m.y0 + *by as u64,
                w: *bw,
                h: *bh,
                label: format!("{} {}", m.label, c.name),
                ..m.clone()
            };
            let p = self.root.join(&c.name).join("chain.txt");
            std::fs::write(&p, cm.text()).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }
}

/// A crop of a store, read back: the crop, its directory, its manifest and its view (the crop inside the box, one L cell a pixel).
pub struct StoredCrop {
    pub crop: Crop,
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub view: View,
}

/// Reads a crop store's `crops.txt` and its crops' manifests.
pub fn read_store(root: &Path) -> Result<Vec<StoredCrop>, String> {
    let p = root.join("crops.txt");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e} (a crop store of `--probe-crops`)", p.display()))?;
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("look-crops 1") {
        return Err(format!("{}: the first line is not `look-crops 1`", p.display()));
    }
    let mut out = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split_whitespace().collect();
        let n =
            |i: usize| f.get(i).and_then(|s| s.parse::<usize>().ok()).ok_or(format!("{}: `{l}`: crop NAME X Y SIZE BX BY", p.display()));
        if f.first() != Some(&"crop") {
            return Err(format!("{}: unknown line `{l}`", p.display()));
        }
        let crop = Crop { name: f.get(1).ok_or(format!("{}: `{l}`: no name", p.display()))?.to_string(), x: n(2)?, y: n(3)?, size: n(4)? };
        let (bx, by) = (n(5)?, n(6)?);
        let dir = root.join(&crop.name);
        let mp = dir.join("chain.txt");
        let manifest = Manifest::parse(&std::fs::read_to_string(&mp).map_err(|e| format!("{}: {e}", mp.display()))?)?;
        if crop.x < bx || crop.y < by || crop.x - bx + crop.size > manifest.w || crop.y - by + crop.size > manifest.h {
            return Err(format!("{}: crop {} does not lie inside its box of {} x {}", p.display(), crop.name, manifest.w, manifest.h));
        }
        let view = View { x: crop.x - bx, y: crop.y - by, size_x: crop.size, size_y: crop.size, step: 1 };
        out.push(StoredCrop { crop, dir, manifest, view });
    }
    if out.is_empty() {
        return Err(format!("{}: no crop", p.display()));
    }
    Ok(out)
}

/// One row of panels: the row's label and, a stage, its pixels (RGB 0..1), or none when the stage was not dumped.
struct Row {
    label: String,
    panels: Vec<Option<Vec<[f64; 3]>>>,
}

/// The sheet's size in pixels for `rows` rows of `size`-cell crops: two title rows, the stage labels, then the rows, each with its label in a column of [`LABEL_CHARS`] glyphs.
pub fn sheet_px(size: usize, rows: usize) -> (usize, usize) {
    (GAP + LABEL_CHARS * TEXT_CELL_W + GAP + STAGES.len() * (size + GAP), 2 * TEXT_ROW_H + GAP + TEXT_ROW_H + 4 + rows * (size + GAP))
}

/// The sheet's pixels, RGB.
fn compose(title: &[String], size: usize, rows: &[Row]) -> Vec<u8> {
    let (w, h) = sheet_px(size, rows.len());
    let mut img = Rgba { width: w as u32, height: h as u32, data: [24u8, 24, 28, 255].repeat(w * h) };
    for (k, t) in title.iter().enumerate().take(2) {
        put_text(&mut img, GAP, 4 + k * TEXT_ROW_H, t, if k == 0 { [240, 240, 240] } else { [170, 190, 220] });
    }
    let top = 2 * TEXT_ROW_H + GAP;
    let x0 = GAP + LABEL_CHARS * TEXT_CELL_W + GAP;
    for (s, (_, label)) in STAGES.iter().enumerate() {
        put_text(&mut img, x0 + s * (size + GAP), top, label, [230, 230, 230]);
    }
    let body = top + TEXT_ROW_H + 4;
    for (r, row) in rows.iter().enumerate() {
        let oy = body + r * (size + GAP);
        let fit: String = row.label.chars().take(LABEL_CHARS).collect();
        put_text(&mut img, GAP, oy + (size / 2).saturating_sub(TEXT_ROW_H / 2), &fit, [230, 230, 230]);
        for (s, p) in row.panels.iter().enumerate() {
            let ox = x0 + s * (size + GAP);
            for y in 0..size {
                for x in 0..size {
                    let c = p.as_ref().map_or([0.35; 3], |rgb| rgb[y * size + x]);
                    let i = ((oy + y) * w + ox + x) * 4;
                    for (ch, v) in c.iter().enumerate() {
                        img.data[i + ch] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            }
            if p.is_none() {
                put_text(&mut img, ox + 2, oy + 2, "NONE", [200, 200, 200]);
            }
        }
    }
    img.data.chunks_exact(4).flat_map(|c| [c[0], c[1], c[2]]).collect()
}

/// An 8-bit RGB PNG of `rgb`.
fn png_rgb(rgb: &[u8], w: usize, h: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        enc.write_header().map_err(|e| e.to_string())?.write_image_data(rgb).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// A stage's surface of a crop's box, or none when the run did not write it.
fn surface(c: &StoredCrop, stage: &str) -> Result<Option<Vec<f32>>, String> {
    read_f32(&c.dir, &format!("{stage}.f32"), c.manifest.w * c.manifest.h)
}

/// The crop's pixels of a surface: the chain sheet's shade at azimuth `az` and its colours (BUILD-060).
fn shaded(z: &[f32], c: &StoredCrop, az: f64) -> Vec<[f64; 3]> {
    let (g, h) = shade_at(z, &c.manifest, &c.view, az);
    g.iter().zip(&h).map(|(&g, &h)| base_colour(g, h, c.manifest.sea_m)).collect()
}

/// The crop's cells of a surface, row-major.
fn cells(z: &[f32], c: &StoredCrop) -> Vec<f64> {
    let v = &c.view;
    let w = c.manifest.w;
    (0..v.size_y).flat_map(|y| (0..v.size_x).map(move |x| f64::from(z[(v.y + y) * w + v.x + x]))).collect()
}

/// What was written: each sheet's path, its size in pixels and its bytes, and the numbers file.
#[derive(Debug, Default)]
pub struct Written {
    pub sheets: Vec<(PathBuf, (usize, usize), u64)>,
    pub numbers: Option<PathBuf>,
}

/// The look sheets of a run's crop store into `out`. Without `against`: one sheet an azimuth (`azNNN.png`), a row a crop. With `against` (the defaults run's store, the same crops):
/// one sheet a crop and azimuth (`<crop>-azNNN.png`) with the rows defaults, switch, and the switch's shade tinted by its difference to the defaults (red lower, blue higher,
/// saturating at [`DIFF_M`]), and `look.txt`: per crop and stage the cells, the RMS and the largest absolute difference (m). A sheet over [`SHEET_LIMIT`] bytes is refused, naming
/// it, its size and the limit, and is not written.
pub fn write_sheets(run: &Path, against: Option<&Path>, azimuths: &[f64], out: &Path) -> Result<Written, String> {
    let crops = read_store(run)?;
    let base = against.map(read_store).transpose()?;
    if let (Some(b), Some(a)) = (&base, against) {
        let names = |v: &[StoredCrop]| v.iter().map(|c| c.crop.clone()).collect::<Vec<_>>();
        if names(b) != names(&crops) {
            return Err(format!("{} and {} hold other crops: a pair needs the same crops", run.display(), a.display()));
        }
    }
    let size = crops[0].crop.size;
    if crops.iter().any(|c| c.crop.size != size) {
        return Err(format!("{}: the crops are not all {size} cells", run.display()));
    }
    if azimuths.is_empty() {
        return Err("no azimuth".into());
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let label = |m: &Manifest| m.label.rsplit_once(' ').map_or(m.label.clone(), |(l, _)| l.to_string());
    let run_label = label(&crops[0].manifest);
    let mut written = Written::default();
    let mut emit = |name: String, title: Vec<String>, rows: Vec<Row>| -> Result<(), String> {
        let (w, h) = sheet_px(size, rows.len());
        let png = png_rgb(&compose(&title, size, &rows), w, h)?;
        let p = out.join(name);
        if png.len() as u64 > SHEET_LIMIT {
            return Err(format!(
                "{}: {} bytes, over the per-sheet limit of {SHEET_LIMIT} (BUILD-048): fewer or smaller crops",
                p.display(),
                png.len()
            ));
        }
        std::fs::write(&p, &png).map_err(|e| format!("{}: {e}", p.display()))?;
        written.sheets.push((p, (w, h), png.len() as u64));
        Ok(())
    };
    let sun = |az: f64| format!("SUN AZ {az:.0} ALT {SUN_ALT_DEG:.0}  1 PX = 1 L CELL  NORTH UP");
    match &base {
        None => {
            for &az in azimuths {
                let mut rows = Vec::new();
                for c in &crops {
                    let panels =
                        STAGES.iter().map(|(s, _)| surface(c, s).map(|z| z.map(|z| shaded(&z, c, az)))).collect::<Result<Vec<_>, _>>()?;
                    rows.push(Row { label: c.crop.name.to_uppercase(), panels });
                }
                let where_ = crops.iter().map(|c| format!("{} {} {}", c.crop.name, c.crop.x, c.crop.y)).collect::<Vec<_>>().join("  ");
                emit(
                    format!("az{az:03.0}.png"),
                    vec![format!("LOOK: {run_label}  {}", sun(az)), format!("CROPS OF {size} CELLS: {where_}")],
                    rows,
                )?;
            }
        }
        Some(b) => {
            let def_label = label(&b[0].manifest);
            let mut numbers = format!("look-numbers 1\nrun {run_label}\nagainst {def_label}\n");
            for (c, d) in crops.iter().zip(b) {
                let zr: Vec<Option<Vec<f32>>> = STAGES.iter().map(|(s, _)| surface(c, s)).collect::<Result<_, _>>()?;
                let zd: Vec<Option<Vec<f32>>> = STAGES.iter().map(|(s, _)| surface(d, s)).collect::<Result<_, _>>()?;
                let diffs: Vec<Option<Vec<f64>>> = zr
                    .iter()
                    .zip(&zd)
                    .map(|(r, q)| match (r, q) {
                        (Some(r), Some(q)) => Some(cells(r, c).iter().zip(cells(q, d)).map(|(a, b)| a - b).collect()),
                        _ => None,
                    })
                    .collect();
                for ((s, _), diff) in STAGES.iter().zip(&diffs) {
                    if let Some(diff) = diff {
                        let rms = (diff.iter().map(|v| v * v).sum::<f64>() / diff.len() as f64).sqrt();
                        let max = diff.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                        numbers.push_str(&format!("{} {s} cells {} rms_m {rms:.3} max_abs_m {max:.3}\n", c.crop.name, diff.len()));
                    }
                }
                for &az in azimuths {
                    let def_row = zd.iter().map(|z| z.as_ref().map(|z| shaded(z, d, az))).collect();
                    let run_row = zr.iter().map(|z| z.as_ref().map(|z| shaded(z, c, az))).collect();
                    let diff_row = zr
                        .iter()
                        .zip(&diffs)
                        .map(|(r, diff)| match (r, diff) {
                            (Some(r), Some(diff)) => {
                                let mut rgb = shaded(r, c, az);
                                tint_change(&mut rgb, diff, DIFF_M);
                                Some(rgb)
                            }
                            _ => None,
                        })
                        .collect();
                    let rows = vec![
                        Row { label: "DEFAULTS".into(), panels: def_row },
                        Row { label: "SWITCH".into(), panels: run_row },
                        Row { label: "DIFF".into(), panels: diff_row },
                    ];
                    let title = vec![
                        format!("LOOK: {run_label} AGAINST {def_label}  {}", sun(az)),
                        format!(
                            "CROP {} {} {} OF {size} CELLS  DIFF RED LOWER BLUE HIGHER, FULL AT {DIFF_M:.0} M",
                            c.crop.name, c.crop.x, c.crop.y
                        ),
                    ];
                    emit(format!("{}-az{az:03.0}.png", c.crop.name), title, rows)?;
                }
            }
            let p = out.join("look.txt");
            std::fs::write(&p, numbers).map_err(|e| format!("{}: {e}", p.display()))?;
            written.numbers = Some(p);
        }
    }
    Ok(written)
}

/// The default output: `target/review/look/<the run directory's name>/`.
pub fn default_out(root: &Path, run: &Path) -> PathBuf {
    let name = run.file_name().map_or("run".to_string(), |n| n.to_string_lossy().to_string());
    root.join("target/review/look").join(name)
}
