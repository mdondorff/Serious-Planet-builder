//! The whole-planet preview (GEN-103): equirectangular images of the default class source and of the elevation it makes, from the generator's own code, so
//! the look of the world can be iterated with images before any bake. CPU only; reads no store.
//!
//! [`compute`] evaluates every pixel's direction once ([`ClassSource::eval`], the class lattice, today's [`crate::global::sample`]); [`render`] turns the fields
//! into PNGs, each with a header (seed and versions) and a legend. The numbers in the images are a pure function of the seed and the size: the same bytes for any thread count.

use crate::classes::*;
use crate::dump::Rgba;
use crate::global::{self, GLOBAL_VERSION};
use crate::mesh::MESH_VERSION;
use planet_core::par::{self, Par};
use planet_core::{smoothstep, Vec3};

/// Bump whenever an image's content or layout changes.
pub const PREVIEW_VERSION: u32 = 1;

/// One pixel of the preview.
#[derive(Clone, Copy)]
pub struct Pixel {
    pub land_depth: f32,
    pub today_m: f32,
    pub new_m: f32,
    pub k: f32,
    pub terms: [f32; 6],
    pub margin_km: f32,
    pub weights: [f32; CLASS_COUNT],
}

pub struct Fields {
    pub seed: u64,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Pixel>,
    pub largest_k_step: f64,
    /// The share of neighbouring lattice point pairs whose `k` differs by more than GEN-097's limit of 2.0.
    pub k_edges_over: f64,
}

/// The pixel centre's latitude in radians.
fn latitude(y: usize, height: usize) -> f64 {
    core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI
}

/// Evaluates the fields at `width x height` pixels on `threads` threads, with a class lattice of `lattice_cells` cells per face edge.
pub fn compute(seed: u64, width: usize, height: usize, lattice_cells: usize, threads: usize) -> Fields {
    let source = ClassSource::new(seed);
    let lattice = ClassLattice::build_with(&source, lattice_cells, threads);
    let par: &dyn Par = &threads;
    let rows = par::map(par, height, &|y| {
        let lat = latitude(y, height);
        (0..width)
            .map(|x| {
                let lon = -core::f64::consts::PI + (x as f64 + 0.5) / width as f64 * 2.0 * core::f64::consts::PI;
                let d = Vec3::from_lat_lon(lat, lon);
                let p = source.eval(d);
                let w = lattice.weights_at(d);
                let k = blended_k(&w);
                let mut weights = [0f32; CLASS_COUNT];
                for (o, v) in weights.iter_mut().zip(w) {
                    *o = v as f32;
                }
                let t = p.terms;
                Pixel {
                    land_depth: p.land_depth as f32,
                    today_m: global::sample(seed, d).elevation_m,
                    new_m: source.elevation_m(d, &p, k) as f32,
                    k: k as f32,
                    terms: [
                        t.core as f32,
                        t.flank as f32,
                        t.old_margin as f32,
                        t.back_arc as f32,
                        t.plateau_field as f32,
                        t.convergence as f32,
                    ],
                    margin_km: t.margin_km as f32,
                    weights,
                }
            })
            .collect::<Vec<_>>()
    });
    Fields {
        seed,
        width,
        height,
        pixels: rows.into_iter().flatten().collect(),
        largest_k_step: lattice.largest_k_step(),
        k_edges_over: lattice.k_steps_over(2.0),
    }
}

/// The land mask of the global stage alone (the continent stage, GEN-104) on a `width x height` equirectangular grid, row 0 north: what TEST-087 and TEST-088 score.
pub fn land_mask(seed: u64, width: usize, height: usize, threads: usize) -> Vec<bool> {
    let par: &dyn Par = &threads;
    let rows = par::map(par, height, &|y| {
        let lat = latitude(y, height);
        (0..width)
            .map(|x| {
                let lon = -core::f64::consts::PI + (x as f64 + 0.5) / width as f64 * 2.0 * core::f64::consts::PI;
                global::continent_land_depth(seed, Vec3::from_lat_lon(lat, lon)) > 0.0
            })
            .collect::<Vec<_>>()
    });
    rows.into_iter().flatten().collect()
}

// ---- text: a 5 x 7 pixel font for the headers and legends ----

fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        _ => [0; 7],
    }
}

const SCALE: usize = 2;
const CELL_W: usize = 6 * SCALE;
const ROW_H: usize = 10 * SCALE;

fn put_text(img: &mut Rgba, x: usize, y: usize, text: &str, color: [u8; 3]) {
    for (n, c) in text.chars().enumerate() {
        let g = glyph(c);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits >> (4 - col) & 1 == 1 {
                    for sy in 0..SCALE {
                        for sx in 0..SCALE {
                            let (px, py) = (x + n * CELL_W + col * SCALE + sx, y + row * SCALE + sy);
                            if px < img.width as usize && py < img.height as usize {
                                let i = (py * img.width as usize + px) * 4;
                                img.data[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn fill(img: &mut Rgba, x: usize, y: usize, w: usize, h: usize, color: [u8; 3]) {
    for py in y..(y + h).min(img.height as usize) {
        for px in x..(x + w).min(img.width as usize) {
            let i = (py * img.width as usize + px) * 4;
            img.data[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
        }
    }
}

/// The legend of an image: a swatch and a label.
pub type Legend = Vec<([u8; 3], String)>;

/// Frames `body` (RGB, `w x h`) with a header of the title, the seed and the versions, and a legend underneath.
fn frame(body: &[[u8; 3]], w: usize, h: usize, seed: u64, title: &str, legend: &Legend) -> Rgba {
    let header = 2 * ROW_H + 8;
    let per_row = (w / (CELL_W * 26)).max(1);
    let legend_rows = legend.len().div_ceil(per_row).max(1);
    let footer = legend_rows * ROW_H + 8;
    let total_h = header + h + footer;
    let mut img = Rgba { width: w as u32, height: total_h as u32, data: vec![255; w * total_h * 4] };
    fill(&mut img, 0, 0, w, total_h, [24, 24, 28]);
    put_text(&mut img, 6, 4, &format!("WORLD PREVIEW: {title}"), [240, 240, 240]);
    let versions = format!(
        "SEED {seed}  GLOBAL V{GLOBAL_VERSION}  MESH V{MESH_VERSION}  CLASS LIST V{CLASS_LIST_VERSION}  CLASS SOURCE V{CLASS_SOURCE_VERSION}  PREVIEW V{PREVIEW_VERSION}"
    );
    put_text(&mut img, 6, 4 + ROW_H, &versions, [170, 190, 220]);
    for y in 0..h {
        for x in 0..w {
            let c = body[y * w + x];
            let i = ((header + y) * w + x) * 4;
            img.data[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    for (n, (color, label)) in legend.iter().enumerate() {
        let (col, row) = (n % per_row, n / per_row);
        let (x, y) = (6 + col * CELL_W * 26, header + h + 4 + row * ROW_H);
        fill(&mut img, x, y, 14, 14, *color);
        put_text(&mut img, x + 20, y, label, [220, 220, 220]);
    }
    img
}

// ---- colours ----

const SEA: [u8; 3] = [18, 36, 70];
pub const CLASS_COLORS: [[u8; 3]; CLASS_COUNT] =
    [[120, 170, 90], [175, 195, 100], [205, 175, 100], [140, 100, 70], [235, 235, 245], [225, 140, 60], [190, 60, 50], [240, 220, 120]];

fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn to_u8(c: [f64; 3]) -> [u8; 3] {
    [(c[0].clamp(0.0, 255.0) + 0.5) as u8, (c[1].clamp(0.0, 255.0) + 0.5) as u8, (c[2].clamp(0.0, 255.0) + 0.5) as u8]
}

/// Hypsometric tint: sea by depth, land by height (green, yellow, brown, white).
fn tint(e: f64) -> [f64; 3] {
    if e < 0.0 {
        return mix([18.0, 36.0, 70.0], [50.0, 90.0, 150.0], smoothstep(-6000.0, 0.0, e));
    }
    let stops: [(f64, [f64; 3]); 5] = [
        (0.0, [70.0, 130.0, 70.0]),
        (500.0, [150.0, 170.0, 80.0]),
        (1500.0, [190.0, 150.0, 90.0]),
        (3000.0, [140.0, 100.0, 80.0]),
        (5500.0, [250.0, 250.0, 255.0]),
    ];
    for w in stops.windows(2) {
        if e <= w[1].0 {
            return mix(w[0].1, w[1].1, (e - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops[4].1
}

/// Hillshade of an equirectangular elevation grid (light from the north-west at 45 degrees, relief exaggerated 8 times); 1 is lit, 0 is dark.
fn hillshade(f: &Fields, elevation: impl Fn(&Pixel) -> f64) -> Vec<f64> {
    let (w, h) = (f.width, f.height);
    let z = |x: isize, y: isize| -> f64 {
        let xi = x.rem_euclid(w as isize) as usize;
        let yi = y.clamp(0, h as isize - 1) as usize;
        elevation(&f.pixels[yi * w + xi]).max(0.0)
    };
    let (az, alt) = (315f64.to_radians(), 45f64.to_radians());
    let mut out = vec![0.5; w * h];
    for y in 0..h {
        let lat = latitude(y, h);
        let dx = EARTH_RADIUS_M * libm::cos(lat).max(0.02) * 2.0 * core::f64::consts::PI / w as f64;
        let dy = EARTH_RADIUS_M * core::f64::consts::PI / h as f64;
        for x in 0..w {
            let (xi, yi) = (x as isize, y as isize);
            let dzdx = 8.0 * (z(xi + 1, yi) - z(xi - 1, yi)) / (2.0 * dx);
            let dzdy = 8.0 * (z(xi, yi - 1) - z(xi, yi + 1)) / (2.0 * dy);
            let slope = libm::atan(libm::sqrt(dzdx * dzdx + dzdy * dzdy));
            let aspect = libm::atan2(dzdy, -dzdx);
            let shade = libm::cos(core::f64::consts::FRAC_PI_2 - alt) * libm::cos(slope)
                + libm::sin(core::f64::consts::FRAC_PI_2 - alt) * libm::sin(slope) * libm::cos(az - aspect);
            out[y * w + x] = shade.clamp(0.0, 1.0);
        }
    }
    out
}

fn elevation_image(f: &Fields, title: &str, elevation: impl Fn(&Pixel) -> f64) -> Rgba {
    let shade = hillshade(f, &elevation);
    let body: Vec<[u8; 3]> = f
        .pixels
        .iter()
        .zip(&shade)
        .map(|(p, s)| {
            let e = elevation(p);
            let c = tint(e);
            let k = if e < 0.0 { 1.0 } else { 0.55 + 0.9 * s };
            to_u8([c[0] * k, c[1] * k, c[2] * k])
        })
        .collect();
    let legend: Legend =
        [(-4000.0, "-4000 M"), (0.0, "0 M"), (500.0, "500 M"), (1500.0, "1500 M"), (3000.0, "3000 M"), (5500.0, "5500 M AND UP")]
            .iter()
            .map(|&(e, l)| (to_u8(tint(e)), l.to_string()))
            .collect();
    frame(&body, f.width, f.height, f.seed, title, &legend)
}

fn argmax(w: &[f32; CLASS_COUNT]) -> usize {
    let mut best = 0;
    for c in 1..CLASS_COUNT {
        if w[c] > w[best] {
            best = c;
        }
    }
    best
}

fn class_legend() -> Legend {
    CLASS_COLORS.iter().zip(CLASS_NAMES).map(|(c, n)| (*c, n.to_string())).collect()
}

/// All images of the preview, by file name.
pub fn render(f: &Fields) -> Vec<(String, Rgba)> {
    let (w, h) = (f.width, f.height);
    let mut out: Vec<(String, Rgba)> = Vec::new();

    let land: Vec<[u8; 3]> = f.pixels.iter().map(|p| if p.land_depth > 0.0 { [150, 180, 110] } else { SEA }).collect();
    out.push((
        "land-sea.png".into(),
        frame(&land, w, h, f.seed, "LAND AND SEA", &vec![([150, 180, 110], "LAND".into()), (SEA, "SEA".into())]),
    ));

    out.push(("elevation.png".into(), elevation_image(f, "ELEVATION (CLASS SOURCE, HILLSHADE)", |p| f64::from(p.new_m))));
    out.push(("elevation-today.png".into(), elevation_image(f, "ELEVATION TODAY (GLOBAL STAGE, HILLSHADE)", |p| f64::from(p.today_m))));

    // Today's elevation and the new one side by side.
    let (a, b) = (&out[2].1, &out[1].1);
    let (aw, ah) = (a.width as usize, a.height as usize);
    let mut side = Rgba { width: (2 * aw) as u32, height: ah as u32, data: vec![0; 2 * aw * ah * 4] };
    for y in 0..ah {
        for x in 0..aw {
            let (i, j) = ((y * aw + x) * 4, (y * 2 * aw + x) * 4);
            side.data[j..j + 4].copy_from_slice(&a.data[i..i + 4]);
            side.data[j + aw * 4..j + aw * 4 + 4].copy_from_slice(&b.data[i..i + 4]);
        }
    }
    out.push(("elevation-compare.png".into(), side));

    let mask: Vec<[u8; 3]> = f
        .pixels
        .iter()
        .map(|p| {
            if p.margin_km < 20.0 {
                return [235, 235, 90];
            }
            let base = if p.land_depth > 0.0 { 40.0 } else { 18.0 };
            let c = |v: f32| (base + 215.0 * f64::from(v)).clamp(0.0, 255.0);
            to_u8([c(p.terms[0]), c(p.terms[1]), c(p.terms[2])])
        })
        .collect();
    out.push((
        "plate-mask.png".into(),
        frame(
            &mask,
            w,
            h,
            f.seed,
            "PLATE MASK (RED CORE, GREEN FLANK, BLUE OLD MARGIN, YELLOW PLATE MARGIN LINE)",
            &vec![
                ([255, 40, 40], "CORE".into()),
                ([40, 255, 40], "FLANK".into()),
                ([40, 40, 255], "OLD MARGIN".into()),
                ([235, 235, 90], "MARGIN WITHIN 20 KM".into()),
            ],
        ),
    ));

    let plateau: Vec<[u8; 3]> = f
        .pixels
        .iter()
        .map(|p| {
            let base = if p.land_depth > 0.0 { 40.0 } else { 18.0 };
            let c = |v: f32| (base + 215.0 * f64::from(v)).clamp(0.0, 255.0);
            to_u8([c(p.terms[3]), c(p.terms[4]), c(p.terms[0])])
        })
        .collect();
    out.push((
        "plateau-backarc.png".into(),
        frame(
            &plateau,
            w,
            h,
            f.seed,
            "PLATEAU AND BACK-ARC (RED BACK-ARC, GREEN PLATEAU FIELD, BLUE CORE)",
            &vec![([255, 40, 40], "BACK-ARC".into()), ([40, 255, 40], "PLATEAU FIELD".into()), ([40, 40, 255], "CORE".into())],
        ),
    ));

    let argmax_img: Vec<[u8; 3]> =
        f.pixels.iter().map(|p| if p.land_depth > 0.0 { CLASS_COLORS[argmax(&p.weights)] } else { SEA }).collect();
    out.push(("classes-argmax.png".into(), frame(&argmax_img, w, h, f.seed, "RELIEF CLASS (LARGEST WEIGHT)", &class_legend())));

    let blend: Vec<[u8; 3]> = f
        .pixels
        .iter()
        .map(|p| {
            if p.land_depth <= 0.0 {
                return SEA;
            }
            let mut c = [0.0; 3];
            for (k, col) in CLASS_COLORS.iter().enumerate() {
                for i in 0..3 {
                    c[i] += f64::from(p.weights[k]) * f64::from(col[i]);
                }
            }
            to_u8(c)
        })
        .collect();
    out.push(("classes-blend.png".into(), frame(&blend, w, h, f.seed, "RELIEF CLASS (WEIGHTS BLENDED)", &class_legend())));

    let k_img: Vec<[u8; 3]> = f
        .pixels
        .iter()
        .map(|p| {
            if p.land_depth <= 0.0 {
                return SEA;
            }
            let t = ((f64::from(p.k) - 0.35) / (7.5 - 0.35)).clamp(0.0, 1.0);
            to_u8(mix(mix([30.0, 60.0, 120.0], [240.0, 230.0, 80.0], (t * 2.0).min(1.0)), [220.0, 40.0, 40.0], (t * 2.0 - 1.0).max(0.0)))
        })
        .collect();
    let k_legend: Legend = [1.0, 3.0, 5.0, 7.5]
        .iter()
        .map(|&k| {
            let t: f64 = (k - 0.35) / (7.5 - 0.35);
            (
                to_u8(mix(
                    mix([30.0, 60.0, 120.0], [240.0, 230.0, 80.0], (t * 2.0f64).min(1.0)),
                    [220.0, 40.0, 40.0],
                    (t * 2.0 - 1.0).max(0.0),
                )),
                format!("K {k}"),
            )
        })
        .collect();
    out.push(("k.png".into(), frame(&k_img, w, h, f.seed, "RELIEF DRIVER K (UNTAPERED, BLENDED BY THE WEIGHTS)", &k_legend)));
    out
}

/// The numbers of the preview for the report (TEST-086): class shares of land under 60 degrees latitude, area weighted, against TEST-082's bands.
pub struct Shares {
    pub land_fraction: f64,
    pub plains: f64,
    pub hills: f64,
    pub mountains: f64,
    pub plateau: f64,
    pub per_class: [f64; CLASS_COUNT],
    pub above_2km: f64,
    pub above_4km: f64,
    pub highest_m: f64,
    pub largest_k_step: f64,
}

pub fn shares(f: &Fields) -> Shares {
    let (mut land_all, mut all) = (0.0, 0.0);
    let (mut land, mut above2, mut above4) = (0.0, 0.0, 0.0);
    let mut per_class = [0.0; CLASS_COUNT];
    let mut highest = f64::MIN;
    for y in 0..f.height {
        let lat = latitude(y, f.height);
        let area = libm::cos(lat);
        for x in 0..f.width {
            let p = &f.pixels[y * f.width + x];
            all += area;
            if p.land_depth <= 0.0 {
                continue;
            }
            land_all += area;
            if libm::fabs(lat) >= 60f64.to_radians() {
                continue;
            }
            land += area;
            for (acc, w) in per_class.iter_mut().zip(p.weights) {
                *acc += area * f64::from(w);
            }
            let e = f64::from(p.new_m);
            highest = highest.max(e);
            if e >= 2000.0 {
                above2 += area;
            }
            if e >= 4000.0 {
                above4 += area;
            }
        }
    }
    // A body with no land under 60 degrees has no shares; they stay 0 instead of NaN.
    let norm = if land > 0.0 { land } else { 1.0 };
    for v in &mut per_class {
        *v /= norm;
    }
    Shares {
        land_fraction: land_all / all,
        plains: per_class[PLAIN] + per_class[ROLLING],
        hills: per_class[HILLS],
        mountains: per_class[MOUNTAINS] + per_class[HIGH_MOUNTAINS],
        plateau: per_class[PLATEAU],
        per_class,
        above_2km: above2 / norm,
        above_4km: above4 / norm,
        highest_m: if highest == f64::MIN { 0.0 } else { highest },
        largest_k_step: f.largest_k_step,
    }
}

/// The relief measures of SPIKE-93 (Earth's hypsometry, coastal land, belt land and the k-step distribution), land under 60 degrees latitude, area weighted by cos(latitude).
pub struct Relief {
    pub under_500m: f64,
    pub median_m: f64,
    pub coastal_median_m: f64,
    pub above_1km: f64,
    pub above_3km: f64,
    pub belt_land: f64,
    /// `|dk|` between 4-neighbouring 2 x 2 pixel cells (about 0.35 degrees, 40 km) of at least half land: the share over 2.0, the 99th and the 90th percentile.
    pub k_over_2: f64,
    pub k_p99: f64,
    pub k_p90: f64,
}

/// The weighted quantile `q` of `(value, weight)` pairs (sorted by value with a total order).
fn weighted_quantile(v: &mut [(f64, f64)], q: f64) -> f64 {
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = v.iter().map(|x| x.1).sum();
    let mut acc = 0.0;
    for (x, w) in v.iter() {
        acc += w;
        if acc >= q * total {
            return *x;
        }
    }
    v.last().map_or(f64::NAN, |x| x.0)
}

pub fn relief(f: &Fields) -> Relief {
    let (w, h) = (f.width, f.height);
    let px = |x: usize, y: usize| &f.pixels[y * w + x];
    let cell_deg = 180.0 / h as f64;
    // About 55 km (the coast band) and 50 km (the belt box) in pixels.
    let coast_r = (55.0 / (111.2 * cell_deg)).round().max(1.0) as isize;
    let box_r = (50.0 / (111.2 * cell_deg)).round().max(1.0) as isize;
    let (mut land, mut under, mut a1, mut a3, mut belt) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut elev: Vec<(f64, f64)> = Vec::new();
    let mut coastal: Vec<(f64, f64)> = Vec::new();
    for y in 0..h {
        let lat = latitude(y, h);
        if libm::fabs(lat) >= 60f64.to_radians() {
            continue;
        }
        let area = libm::cos(lat);
        let cols = |r: isize| ((r as f64) / libm::cos(lat).max(0.2)).round() as isize;
        for x in 0..w {
            let p = px(x, y);
            if p.land_depth <= 0.0 {
                continue;
            }
            let e = f64::from(p.new_m).max(0.0);
            land += area;
            under += area * f64::from(u8::from(e < 500.0));
            a1 += area * f64::from(u8::from(e >= 1000.0));
            a3 += area * f64::from(u8::from(e >= 3000.0));
            elev.push((e, area));
            let near_sea = |r: isize| {
                let c = cols(r);
                (-r..=r).any(|dy| {
                    let yy = y as isize + dy;
                    yy >= 0
                        && yy < h as isize
                        && (-c..=c).any(|dx| px((x as isize + dx).rem_euclid(w as isize) as usize, yy as usize).land_depth <= 0.0)
                })
            };
            if near_sea(coast_r) {
                coastal.push((e, area));
            }
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            let c = cols(box_r);
            for dy in -box_r..=box_r {
                let yy = (y as isize + dy).clamp(0, h as isize - 1) as usize;
                for dx in -c..=c {
                    let q = f64::from(px((x as isize + dx).rem_euclid(w as isize) as usize, yy).new_m).max(0.0);
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
            }
            belt += area * f64::from(u8::from(hi - lo >= 1000.0));
        }
    }
    let norm = if land > 0.0 { land } else { 1.0 };
    // The k of 2 x 2 pixel cells with at least two land pixels (the mean over the land pixels, as the SPIKE-93 reference), under 60 degrees latitude, and the steps between 4-neighbours.
    let (cw, ch) = (w / 2, h / 2);
    let cell: Vec<Option<f64>> = (0..cw * ch)
        .map(|i| {
            let (cx, cy) = (i % cw, i / cw);
            if libm::fabs(latitude(2 * cy + 1, h)) >= 60f64.to_radians() {
                return None;
            }
            let ps = [px(2 * cx, 2 * cy), px(2 * cx + 1, 2 * cy), px(2 * cx, 2 * cy + 1), px(2 * cx + 1, 2 * cy + 1)];
            let on: Vec<f64> = ps.iter().filter(|p| p.land_depth > 0.0).map(|p| f64::from(p.k)).collect();
            (on.len() >= 2).then(|| on.iter().sum::<f64>() / on.len() as f64)
        })
        .collect();
    let mut steps: Vec<(f64, f64)> = Vec::new();
    for cy in 0..ch {
        for cx in 0..cw {
            let Some(a) = cell[cy * cw + cx] else { continue };
            if let Some(b) = cell[cy * cw + (cx + 1) % cw] {
                steps.push((libm::fabs(a - b), 1.0));
            }
            if cy + 1 < ch {
                if let Some(b) = cell[(cy + 1) * cw + cx] {
                    steps.push((libm::fabs(a - b), 1.0));
                }
            }
        }
    }
    let over = steps.iter().filter(|s| s.0 > 2.0).count() as f64 / steps.len().max(1) as f64;
    Relief {
        under_500m: under / norm,
        median_m: weighted_quantile(&mut elev, 0.5),
        coastal_median_m: weighted_quantile(&mut coastal, 0.5),
        above_1km: a1 / norm,
        above_3km: a3 / norm,
        belt_land: belt / norm,
        k_over_2: over,
        k_p99: weighted_quantile(&mut steps.clone(), 0.99),
        k_p90: weighted_quantile(&mut steps, 0.90),
    }
}

/// The report lines of the relief measures beside SPIKE-93's bands (proposed, the owner's yes pending; Earth from ETOPO 2022, land under 60 degrees).
fn relief_report(f: &Fields) -> String {
    let r = relief(f);
    let band = |v: f64, lo: f64, hi: f64| if v >= lo && v <= hi { "in band" } else { "OUT OF BAND" };
    let mut t = String::from("relief against Earth (SPIKE-93, bands proposed and the owner's yes pending; land under 60 degrees; on the preview grid, before any bake):
");
    let pct = |name: &str, v: f64, earth: &str, lo: f64, hi: f64| {
        format!(
            "  {name} {:.1} % (Earth {earth}, band {lo}-{hi} %, {})
",
            100.0 * v,
            band(100.0 * v, lo, hi)
        )
    };
    t += &pct("land under 500 m", r.under_500m, "58.4 %", 50.0, 65.0);
    t += &format!(
        "  median land elevation {:.0} m (Earth about 410 m, band 300-550 m, {})
",
        r.median_m,
        band(r.median_m, 300.0, 550.0)
    );
    t += &format!(
        "  coastal land (within about 55 km of the sea) median {:.0} m (Earth 82 m, band under 200 m, {})
",
        r.coastal_median_m,
        if r.coastal_median_m < 200.0 { "in band" } else { "OUT OF BAND" }
    );
    t += &pct("land above 1 km", r.above_1km, "21.6 %", 17.0, 27.0);
    t += &pct("land above 3 km", r.above_3km, "3.5 %", 2.0, 5.0);
    t += &pct("belt land (R50 at least 1 km)", r.belt_land, "17.6 %", 13.0, 23.0);
    t += &format!(
        "  k steps between 40 km cells: over 2.0 on {:.1} % of pairs (Earth 6.7 %), p99 {:.2} (Earth 3.31, realism band at most 3.5), p90 {:.2} (Earth 1.63)
",
        100.0 * r.k_over_2, r.k_p99, r.k_p90
    );
    t
}

/// The report text: each measure beside TEST-082's band; reported, not judged (the judgement belongs to TEST-082 with the bake).
pub fn report(f: &Fields) -> String {
    let s = shares(f);
    let band = |v: f64, lo: f64, hi: f64| if v >= lo && v <= hi { "in band" } else { "OUT OF BAND" };
    let mut r = format!(
        "world preview seed {} {}x{}: global v{GLOBAL_VERSION}, mesh v{MESH_VERSION}, class list v{CLASS_LIST_VERSION}, class source v{CLASS_SOURCE_VERSION}, preview v{PREVIEW_VERSION}\n",
        f.seed, f.width, f.height
    );
    r += &format!("land fraction {:.1} %\nland under 60 degrees latitude, by the class weights (TEST-082's bands, reported not judged; its classes come from k R14 once the bake exists):\n", 100.0 * s.land_fraction);
    r += &format!("  plains (plain + rolling) {:.1} % (band 45-62 %, {})\n", 100.0 * s.plains, band(100.0 * s.plains, 45.0, 62.0));
    r += &format!("  hills {:.1} % (band 15-30 %, {})\n", 100.0 * s.hills, band(100.0 * s.hills, 15.0, 30.0));
    r +=
        &format!("  mountains (mountains + high) {:.1} % (band 20-32 %, {})\n", 100.0 * s.mountains, band(100.0 * s.mountains, 20.0, 32.0));
    r += &format!("  plateau {:.1} % (no band; counted with the plains by TEST-082)\n", 100.0 * s.plateau);
    r += &format!("  land above 2 km {:.1} % (band 5-10 %, {})\n", 100.0 * s.above_2km, band(100.0 * s.above_2km, 5.0, 10.0));
    r += &format!("  land above 4 km {:.1} % (band 1-3 %, {})\n", 100.0 * s.above_4km, band(100.0 * s.above_4km, 1.0, 3.0));
    r += &format!("  highest land {:.2} km (band 7-9 km, {})\n", s.highest_m / 1000.0, band(s.highest_m / 1000.0, 7.0, 9.0));
    r += &format!(
        "  largest step of the untapered k between lattice points {:.2} (limit 2.0, {})\n",
        s.largest_k_step,
        if s.largest_k_step <= 2.0 { "within" } else { "OVER" }
    );
    r += &format!("  share of lattice steps of k over 2.0: {:.2} %\n", 100.0 * f.k_edges_over);
    r += "  per class:";
    for (name, share) in CLASS_NAMES.iter().zip(s.per_class) {
        r += &format!(" {name} {:.1} %", 100.0 * share);
    }
    r.push('\n');
    let land: Vec<bool> = f.pixels.iter().map(|p| p.land_depth > 0.0).collect();
    let high: Vec<bool> = f.pixels.iter().zip(&land).map(|(p, &l)| l && p.new_m >= 2000.0).collect();
    let m = crate::shape::shape_metrics(&crate::shape::Grid { width: f.width, height: f.height, land: &land, high: &high });
    let list = |v: &[f64], unit: &str| v.iter().map(|x| format!("{x:.1}{unit}")).collect::<Vec<_>>().join(" / ");
    r += "shape metrics (SPIKE-91; Earth beside; reported, not judged):\n";
    r += &format!("  S1 land masses of at least 1 Mkm2: {} ({}); Earth 5\n", m.land_masses.len(), list(&m.land_masses, ""));
    r += &format!("  S2 largest land mass's share of land: {:.0} %; Earth about 57 %\n", 100.0 * m.largest_share);
    r += &format!(
        "  S3 coastline dimension (boxes of 2 to 32 cells): {:.2}; Earth 1.1-1.3 (measure Earth at the same band before judging)\n",
        m.coast_dimension
    );
    r += &format!("  S4 high-ground width, area-weighted median {:.0} km, 10th percentile {:.0} km; Earth about 100-500 km, target median about 310 km\n", m.width_median_km, m.width_p10_km);
    r += &format!("  S5 spans of the longest high-ground regions: {}; target the longest at most 7500 km (the greatest distance between two cells, not a skeleton length)\n", list(&m.longest_spans_km, " km"));
    r += &format!("  S6 enclosed area (holes of at least 0.5 Mkm2): {:.1} Mkm2; Earth about 0-1\n", m.enclosed_mkm2);
    r +=
        &format!("  S7 high ground's share of land under 60 degrees: {:.1} %; Earth about 7 % (without Antarctica)\n", 100.0 * s.above_2km);
    if f.width >= 360 && f.height >= 180 {
        let c = crate::shape::continent_metrics(&land, f.width, f.height);
        r += "continent stage (SPIKE-92, GEN-104; TEST-087 and TEST-088 score these on a 1440 x 720 land grid; reported here on the preview grid):
";
        r += &format!(
            "  S9 compactness (area-weighted median of 4 pi A / P^2): {:.2}; Earth 0.10, gate at most 0.20
",
            c.compactness
        );
        r += &format!(
            "  S10 elongation (median sqrt(l1 / l2)): {:.2}; Earth 1.77, band 1.5-2.5
",
            c.elongation
        );
        r += &format!(
            "  S11 islands (masses of 10^4 to 10^6 km2): {}, {:.1} % of land; Earth 50, 3.9 %, band 35-70 and 1.9-6 %
",
            c.islands,
            100.0 * c.island_share
        );
        r += &format!("  masses of at least 1 Mkm2 centred poleward of 70 degrees: {}; limit 1\n", c.polar_masses);
    } else {
        r += "continent stage metrics S9 to S11: not measured (they need a grid of at least 360 x 180)\n";
    }
    r += "  S8 sinuosity: not measured (SPIKE-91 has no comparable measure yet)\n";
    r += &relief_report(f);
    r
}
