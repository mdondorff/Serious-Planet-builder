//! The terrain scorecard (change `terrain-scorecard`, TEST-079, TEST-080): the pure parts of `cargo xtask terrain-score` and its example
//! `examples/terrain_score.rs`: the sites file, the 30 m grids, the statistics of a raster, the distances between a generator site and its tile, and the
//! report writers. No GPU, no store and no GLO-30 reader here (the reader is a dev-dependency of the example); the notice texts come in as arguments.
//!
//! Results never depend on the thread count (CON-14): each raster is analysed on its own with the kernels of `planet_generators::descriptors`, every
//! percentile and sum runs on one thread in index order, and the example writes each result into its own slot.

use planet_core::Vec3;
use planet_generators::descriptors::{self as desc, Raster};

/// Cell size of every raster in metres.
pub const CELL_M: f64 = 30.0;
/// Cells along each side of a raster: a 0.5 degree square at 30 m.
pub const GRID: usize = 1853;
/// Half the side of the window in degrees.
pub const HALF_DEG: f64 = 0.25;
/// Metres per degree of latitude at GLO-30's 30.87 m per arc-second.
pub const METRES_PER_DEG: f64 = 30.87 * 3600.0;
/// Percentiles kept per descriptor: p1 to p99.
pub const PERCENTILES: usize = 99;
/// Side of the maps in pixels (`--maps`).
pub const MAP_PX: usize = 512;
/// Geomorphon lookup distance and flatness threshold (GEN-091).
pub const GEOMORPHON_LOOKUP_M: f64 = 1000.0;
pub const GEOMORPHON_FLAT_DEG: f64 = 1.0;

/// A descriptor reported as a distribution: percentiles p1 to p99.
#[derive(Clone, Copy, Debug)]
pub struct Continuous {
    pub id: &'static str,
    /// Name with the unit, for a table row.
    pub label: &'static str,
}

/// The continuous descriptors in report order.
pub const CONTINUOUS: [Continuous; 8] = [
    Continuous { id: "slope", label: "Slope (deg)" },
    Continuous { id: "profile_curvature", label: "Profile curvature (1/m)" },
    Continuous { id: "contour_curvature", label: "Contour curvature (1/m)" },
    Continuous { id: "relief_1km", label: "Relief 1 km (m)" },
    Continuous { id: "relief_5km", label: "Relief 5 km (m)" },
    Continuous { id: "tpi_300m", label: "Height above 300 m mean (m)" },
    Continuous { id: "tpi_2km", label: "Height above 2 km mean (m)" },
    Continuous { id: "drainage_area", label: "Drainage area (m2)" },
];
/// Geomorphon class names by class value (`descriptors::form`).
pub const FORM_NAMES: [&str; 10] = ["flat", "peak", "ridge", "shoulder", "spur", "slope", "hollow", "footslope", "valley", "pit"];

/// One landform of the sites file: a real tile and the generator's matched sites.
#[derive(Clone, Debug, PartialEq)]
pub struct LandformSpec {
    pub name: String,
    /// Tile name such as `N52E012`.
    pub tile: String,
    /// `(latitude, longitude)` in degrees of each generator site.
    pub sites: Vec<(f64, f64)>,
}

/// Parses the sites file (TOML): `[[landform]]` tables with `name`, `tile` and `sites = [[lat, lon], ...]`.
pub fn parse_sites(text: &str) -> Result<Vec<LandformSpec>, String> {
    let doc: toml::Table = text.parse().map_err(|e| format!("sites file: {e}"))?;
    let list = doc.get("landform").and_then(|v| v.as_array()).ok_or("sites file: no [[landform]] table")?;
    let mut out = Vec::new();
    for (k, item) in list.iter().enumerate() {
        let t = item.as_table().ok_or_else(|| format!("sites file: landform {k} is not a table"))?;
        let text_of = |key: &str| {
            t.get(key).and_then(|v| v.as_str()).map(str::to_string).ok_or_else(|| format!("sites file: landform {k} has no `{key}`"))
        };
        let (name, tile) = (text_of("name")?, text_of("tile")?);
        tile_file_name(&tile).ok_or_else(|| format!("sites file: tile '{tile}' of '{name}' is not a name like N52E012"))?;
        let sites_v = t.get("sites").and_then(|v| v.as_array()).ok_or_else(|| format!("sites file: '{name}' has no `sites`"))?;
        let mut sites = Vec::new();
        for s in sites_v {
            let pair: Vec<f64> = s
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).collect())
                .unwrap_or_default();
            if pair.len() != 2 || pair[0].abs() > 90.0 || pair[1].abs() > 180.0 {
                return Err(format!("sites file: a site of '{name}' is not [latitude, longitude] in degrees"));
            }
            sites.push((pair[0], pair[1]));
        }
        if sites.is_empty() {
            return Err(format!("sites file: '{name}' lists no site"));
        }
        out.push(LandformSpec { name, tile, sites });
    }
    if out.is_empty() {
        return Err("sites file: no landform".into());
    }
    Ok(out)
}

/// File name of a Copernicus GLO-30 tile from its short name (`N52E012` is `Copernicus_DSM_COG_10_N52_00_E012_00_DEM.tif`); `None` for another shape.
pub fn tile_file_name(tile: &str) -> Option<String> {
    let b = tile.as_bytes();
    let ok = b.len() == 7
        && matches!(b[0], b'N' | b'S')
        && b[1..3].iter().all(u8::is_ascii_digit)
        && matches!(b[3], b'E' | b'W')
        && b[4..7].iter().all(u8::is_ascii_digit);
    ok.then(|| format!("Copernicus_DSM_COG_10_{}_00_{}_00_DEM.tif", &tile[..3], &tile[3..]))
}

/// East and north unit vectors at a site (latitude, longitude in degrees) and its direction, in f64 with `libm`.
fn site_frame(lat: f64, lon: f64) -> (Vec3, Vec3, Vec3) {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
    (Vec3::new(cla * clo, cla * slo, sla), Vec3::new(-slo, clo, 0.0), Vec3::new(-sla * clo, -sla * slo, cla))
}

/// Metres east (`x`) or north (`y`) of the centre of cell `(row, col)` of a [`GRID`] square whose half side is `half_m`; row 0 is the north edge.
fn cell_offset(half_m: f64, row: usize, col: usize) -> (f64, f64) {
    (-half_m + (col as f64 + 0.5) * CELL_M, half_m - (row as f64 + 0.5) * CELL_M)
}

/// Unit directions of the generator window around a site: [`GRID`] x [`GRID`] cells of [`CELL_M`] on an east-north tangent plane through the site at the
/// planet's radius, row-major with row 0 at the north edge (SPIKE-55's probe), built in f64.
pub fn generator_directions(lat: f64, lon: f64, radius_m: f64) -> Vec<Vec3> {
    let (c, east, north) = site_frame(lat, lon);
    let half_m = HALF_DEG.to_radians() * radius_m;
    let mut dirs = Vec::with_capacity(GRID * GRID);
    for row in 0..GRID {
        for col in 0..GRID {
            let (x, y) = cell_offset(half_m, row, col);
            let p = c * radius_m + east * x + north * y;
            dirs.push(p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z)));
        }
    }
    dirs
}

/// A GLO-30 style tile borrowed for resampling: row-major `f32` (NaN is no-data), row 0 north, with its georeference in degrees.
#[derive(Clone, Copy)]
pub struct TileView<'a> {
    pub data: &'a [f32],
    pub width: usize,
    pub height: usize,
    /// Longitude and latitude of the top-left corner of the top-left pixel.
    pub origin_lon: f64,
    pub origin_lat: f64,
    /// Degrees per pixel (positive; rows run south).
    pub scale_lon: f64,
    pub scale_lat: f64,
}

impl TileView<'_> {
    /// Latitude and longitude of the tile's centre.
    pub fn centre(&self) -> (f64, f64) {
        (self.origin_lat - self.height as f64 * self.scale_lat / 2.0, self.origin_lon + self.width as f64 * self.scale_lon / 2.0)
    }
}

/// The real raster: the central 0.5 degree square of a tile resampled bilinearly onto the east-north 30 m grid of [`GRID`] x [`GRID`] cells centred on the
/// tile (true metric spacing: [`METRES_PER_DEG`] per degree of latitude, longitude scaled by the cosine of the centre latitude). NaN where a source pixel
/// of the bilinear stencil is no-data or the cell lies off the tile.
pub fn resample_real(t: &TileView) -> Vec<f64> {
    let (clat, clon) = t.centre();
    let m_per_deg_lon = METRES_PER_DEG * libm::cos(clat.to_radians());
    let half_m = HALF_DEG * METRES_PER_DEG;
    // Fractional source column of every grid column and row of every grid row.
    let fx: Vec<f64> = (0..GRID).map(|c| (clon + cell_offset(half_m, 0, c).0 / m_per_deg_lon - t.origin_lon) / t.scale_lon - 0.5).collect();
    let fy: Vec<f64> =
        (0..GRID).map(|r| (t.origin_lat - (clat + cell_offset(half_m, r, 0).1 / METRES_PER_DEG)) / t.scale_lat - 0.5).collect();
    let mut out = vec![f64::NAN; GRID * GRID];
    for (r, row) in out.chunks_mut(GRID).enumerate() {
        let y = fy[r];
        if y < 0.0 || y >= (t.height - 1) as f64 {
            continue;
        }
        let (y0, wy) = (y as usize, y - (y as usize) as f64);
        for (c, o) in row.iter_mut().enumerate() {
            let x = fx[c];
            if x < 0.0 || x >= (t.width - 1) as f64 {
                continue;
            }
            let (x0, wx) = (x as usize, x - (x as usize) as f64);
            let s = |dy: usize, dx: usize| f64::from(t.data[(y0 + dy) * t.width + x0 + dx]);
            let v = (s(0, 0) * (1.0 - wx) + s(0, 1) * wx) * (1.0 - wy) + (s(1, 0) * (1.0 - wx) + s(1, 1) * wx) * wy;
            // A no-data pixel anywhere in the stencil makes the cell NaN (0 * NaN is NaN, so even a zero weight does not hide it).
            *o = v;
        }
    }
    out
}

/// Percentiles p1 to p99 of `values` (linear between ranks, as numpy's default). Sorts in place; `None` for no values.
pub fn percentiles(values: &mut [f64]) -> Option<Vec<f64>> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable_by(f64::total_cmp);
    let n = values.len();
    Some(
        (1..=PERCENTILES)
            .map(|q| {
                let pos = q as f64 / 100.0 * (n - 1) as f64;
                let lo = pos as usize;
                let hi = (lo + 1).min(n - 1);
                values[lo] + (values[hi] - values[lo]) * (pos - lo as f64)
            })
            .collect(),
    )
}

/// The distribution of one continuous descriptor on a raster.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    /// Cells (or blocks) counted.
    pub counted: usize,
    /// p1 to p99; empty when nothing was counted.
    pub pct: Vec<f64>,
}

impl Series {
    fn of(values: impl Iterator<Item = f64>) -> Series {
        let mut v: Vec<f64> = values.filter(|x| x.is_finite()).collect();
        let counted = v.len();
        Series { counted, pct: percentiles(&mut v).unwrap_or_default() }
    }

    /// Percentile `q` (1 to 99), NaN when empty.
    pub fn p(&self, q: usize) -> f64 {
        self.pct.get(q - 1).copied().unwrap_or(f64::NAN)
    }
}

/// Everything the scorecard keeps of one raster.
#[derive(Clone, Debug, PartialEq)]
pub struct RasterStats {
    pub cells: usize,
    /// One per [`CONTINUOUS`] entry.
    pub series: Vec<Series>,
    /// Share of the counted cells per geomorphon class ([`FORM_NAMES`]).
    pub forms: [f64; 10],
    pub forms_counted: usize,
    /// Drainage density in km per km2 (GEN-092).
    pub density: f64,
}

/// A map of one descriptor on a raster (`--maps`): RGBA, [`MAP_PX`] square.
pub struct MapImage {
    pub descriptor: &'static str,
    pub rgba: Vec<u8>,
}

fn cells_for(metres: f64) -> usize {
    ((metres / CELL_M).round() as usize).max(1)
}

/// A square map by nearest sampling of a `w x h` field: grey between `lo` and `hi`, magenta for NaN.
fn grey_map(values: &[f64], w: usize, h: usize, lo: f64, hi: f64) -> Vec<u8> {
    map_of(w, h, |k| {
        let v = values[k];
        if v.is_nan() {
            [255, 0, 255, 255]
        } else {
            let g = if hi > lo { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.5 };
            let g = (g * 255.0 + 0.5) as u8;
            [g, g, g, 255]
        }
    })
}

fn map_of(w: usize, h: usize, px: impl Fn(usize) -> [u8; 4]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(MAP_PX * MAP_PX * 4);
    for y in 0..MAP_PX {
        let j = (y * h / MAP_PX).min(h - 1);
        for x in 0..MAP_PX {
            rgba.extend_from_slice(&px(j * w + (x * w / MAP_PX).min(w - 1)));
        }
    }
    rgba
}

/// Colour of a geomorphon class for the maps.
const FORM_COLOURS: [[u8; 4]; 10] = [
    [220, 220, 220, 255],
    [180, 0, 0, 255],
    [240, 100, 40, 255],
    [250, 170, 60, 255],
    [250, 220, 90, 255],
    [190, 190, 120, 255],
    [140, 200, 140, 255],
    [60, 160, 120, 255],
    [40, 90, 200, 255],
    [10, 20, 100, 255],
];

/// Runs the seven descriptors on a `w x h` raster of [`CELL_M`] cells and reduces them to [`RasterStats`]; with `maps` also one image per descriptor.
/// `threads` splits the kernels over rows only (CON-14).
pub fn analyse(z: &[f64], w: usize, h: usize, threads: usize, maps: bool) -> (RasterStats, Vec<MapImage>) {
    let r = Raster::new(z, w, h, CELL_M);
    let mut series = Vec::with_capacity(CONTINUOUS.len());
    let mut images = Vec::new();
    // `field` is the descriptor's values on a `fw x fh` grid.
    let mut push = |id: &'static str, field: &[f64], fw: usize, fh: usize| {
        let s = Series::of(field.iter().copied());
        if maps {
            let (lo, hi) = if s.pct.is_empty() { (0.0, 1.0) } else { (s.p(2), s.p(98)) };
            images.push(MapImage { descriptor: id, rgba: grey_map(field, fw, fh, lo, hi) });
        }
        series.push(s);
    };
    push("slope", &desc::slope_degrees(&r, threads), w, h);
    let c = desc::curvature(&r, threads);
    push("profile_curvature", &c.profile, w, h);
    push("contour_curvature", &c.contour, w, h);
    drop(c);
    for (id, side_m) in [("relief_1km", 1000.0), ("relief_5km", 5000.0)] {
        let b = desc::local_relief(&r, cells_for(side_m), threads);
        push(id, &b.values, b.bw.max(1), b.bh.max(1));
    }
    for (id, radius_m) in [("tpi_300m", 300.0), ("tpi_2km", 2000.0)] {
        push(id, &desc::tpi(&r, cells_for(radius_m), threads), w, h);
    }
    let forms = desc::geomorphons(&r, cells_for(GEOMORPHON_LOOKUP_M), GEOMORPHON_FLAT_DEG, threads);
    let mut counts = [0usize; 10];
    for &f in &forms {
        if f != desc::UNCOUNTED {
            counts[usize::from(f)] += 1;
        }
    }
    let forms_counted: usize = counts.iter().sum();
    let mut shares = [0.0; 10];
    if forms_counted > 0 {
        for (s, &n) in shares.iter_mut().zip(&counts) {
            *s = n as f64 / forms_counted as f64;
        }
    }
    if maps {
        images.push(MapImage {
            descriptor: "geomorphons",
            rgba: map_of(w, h, |k| if forms[k] == desc::UNCOUNTED { [255, 0, 255, 255] } else { FORM_COLOURS[usize::from(forms[k])] }),
        });
    }
    drop(forms);
    let d = desc::drainage(&r);
    let density = desc::drainage_density(&r, &d, desc::CHANNEL_AREA_M2);
    // Drainage area is shown on a log scale: the distribution spans decades.
    if maps {
        let logs: Vec<f64> = d.area.iter().map(|a| if *a > 0.0 { libm::log10(*a) } else { f64::NAN }).collect();
        let s = Series::of(logs.iter().copied());
        let (lo, hi) = if s.pct.is_empty() { (0.0, 1.0) } else { (s.p(2), s.p(98)) };
        images.push(MapImage { descriptor: "drainage_area", rgba: grey_map(&logs, w, h, lo, hi) });
    }
    series.push(Series::of(d.area.iter().copied()));
    (RasterStats { cells: w * h, series, forms: shares, forms_counted, density }, images)
}

/// Wasserstein-1 distance between two sets of percentiles p1 to p99 divided by the real set's p5-to-p95 range; `None` when either is empty or the range is not
/// positive (TEST-079).
pub fn wasserstein_over_range(real: &[f64], generator: &[f64]) -> Option<f64> {
    if real.len() != PERCENTILES || generator.len() != PERCENTILES {
        return None;
    }
    let range = real[94] - real[4];
    if !range.is_finite() || range <= 0.0 {
        return None;
    }
    Some(real.iter().zip(generator).map(|(a, b)| (a - b).abs()).sum::<f64>() / PERCENTILES as f64 / range)
}

/// L1 distance between two class histograms of shares (0 to 2).
pub fn histogram_l1(a: &[f64; 10], b: &[f64; 10]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum()
}

/// What one generator site gets measured against its tile (TEST-079).
#[derive(Clone, Debug, PartialEq)]
pub struct SiteDistances {
    /// One per [`CONTINUOUS`] entry; `None` when undefined.
    pub continuous: Vec<Option<f64>>,
    pub forms_l1: f64,
    /// The three classes with the largest gap: `(class, generator share minus real share)`.
    pub forms_top: Vec<(usize, f64)>,
    /// Generator density over real density; `None` when the real density is 0.
    pub density_ratio: Option<f64>,
}

/// Distances of a generator raster to the real one.
pub fn distances(real: &RasterStats, generator: &RasterStats) -> SiteDistances {
    let continuous = real.series.iter().zip(&generator.series).map(|(r, g)| wasserstein_over_range(&r.pct, &g.pct)).collect();
    let mut gaps: Vec<(usize, f64)> = (0..10).map(|k| (k, generator.forms[k] - real.forms[k])).collect();
    gaps.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
    gaps.truncate(3);
    SiteDistances {
        continuous,
        forms_l1: histogram_l1(&real.forms, &generator.forms),
        forms_top: gaps,
        density_ratio: (real.density > 0.0 && real.density.is_finite()).then(|| generator.density / real.density),
    }
}

/// One landform's measured rasters.
#[derive(Clone, Debug)]
pub struct LandformResult {
    pub name: String,
    pub tile: String,
    pub sites: Vec<(f64, f64)>,
    pub real: RasterStats,
    /// One per site.
    pub generator: Vec<RasterStats>,
}

/// The names of the gaps ranked: the continuous descriptors by their distance, "landform classes" by the L1 and "drainage density" by `|ln ratio|`, each the
/// mean over the sites that define it; no aggregate (SPIKE-57).
pub fn ranked_gaps(l: &LandformResult) -> Vec<(&'static str, f64)> {
    let per_site: Vec<SiteDistances> = l.generator.iter().map(|g| distances(&l.real, g)).collect();
    ranked(&per_site)
}

/// [`ranked_gaps`] over the given distances (one site's, for the table of that site).
pub fn ranked(per_site: &[SiteDistances]) -> Vec<(&'static str, f64)> {
    let mean = |vals: Vec<f64>| (!vals.is_empty()).then(|| vals.iter().sum::<f64>() / vals.len() as f64);
    let mut out: Vec<(&'static str, f64)> = Vec::new();
    for (k, c) in CONTINUOUS.iter().enumerate() {
        if let Some(m) = mean(per_site.iter().filter_map(|d| d.continuous[k]).collect()) {
            out.push((c.label, m));
        }
    }
    out.push(("Landform classes", per_site.iter().map(|d| d.forms_l1).sum::<f64>() / per_site.len().max(1) as f64));
    if let Some(m) = mean(per_site.iter().filter_map(|d| d.density_ratio).map(|r| libm::fabs(libm::log(r.max(1e-3)))).collect()) {
        out.push(("Drainage density", m));
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// A number in three significant digits, scientific for very large or small magnitudes, `n/a` for NaN.
pub fn num(x: f64) -> String {
    if !x.is_finite() {
        return "n/a".into();
    }
    let a = x.abs();
    if a == 0.0 {
        "0".into()
    } else if !(1e-3..1e5).contains(&a) {
        format!("{x:.1e}")
    } else if a >= 100.0 {
        format!("{x:.0}")
    } else if a >= 10.0 {
        format!("{x:.1}")
    } else if a >= 1.0 {
        format!("{x:.2}")
    } else {
        format!("{x:.3}")
    }
}

/// The licence texts that open every report (the testkit's constants; `LICENCE-NOTES.md` of the tiles, Art. 6).
#[derive(Clone, Copy)]
pub struct Notice<'a> {
    pub notice: &'a str,
    pub no_liability: &'a str,
    pub doi: &'a str,
}

/// The run's inputs, written to both reports.
#[derive(Clone, Debug, Default)]
pub struct RunInfo {
    pub date: String,
    pub commit: String,
    pub store: String,
    pub definition_hash: String,
    pub dem_dir: String,
    pub sites_file: String,
    pub threads: usize,
    /// Wall seconds per phase in order.
    pub phases: Vec<(String, f64)>,
}

fn dominant_form(s: &RasterStats) -> String {
    let (k, share) = s.forms.iter().enumerate().fold((0, -1.0), |best, (k, &v)| if v > best.1 { (k, v) } else { best });
    format!("{} {:.0}%", FORM_NAMES[k], share * 100.0)
}

/// `report.md`: the notice straight after the title, then one section per landform with a table of four columns and the three largest gaps (TEST-080).
pub fn render_markdown(info: &RunInfo, notice: &Notice, landforms: &[LandformResult]) -> String {
    let mut m = format!("# Terrain scorecard {}\n\n", info.date);
    m += &format!(
        "Statistics {}. {}. Citation: {}\n\n",
        notice.notice.trim_end_matches('.'),
        notice.no_liability.trim_end_matches('.'),
        notice.doi
    );
    m += &format!(
        "Date {}. Commit {}. Store {} (definition {}). Tiles {}. Sites file {}. Each landform: the central 0.5 degree square of a real GLO-30 tile against the generator at its matched sites, one table per site, all at 30 m. \
         Distance: 0 means equal; for the curves it is the mean gap between the percentiles p1 to p99 in units of the real p5-to-p95 range; for the classes the sum of \
         share gaps (0 to 2); for drainage density the ratio generator / real.\n\n",
        info.date, info.commit, info.store, info.definition_hash, info.dem_dir, info.sites_file
    );
    for l in landforms {
        m += &format!("## {} ({})\n\n", l.name, l.tile);
        for (k, (g, site)) in l.generator.iter().zip(&l.sites).enumerate() {
            let d = distances(&l.real, g);
            m += &format!("### Site {} of {}: ({}, {})\n\n", k + 1, l.name, site.0, site.1);
            m += "| Descriptor | Real p50 | Generator p50 | Distance |\n|---|---|---|---|\n";
            for (i, c) in CONTINUOUS.iter().enumerate() {
                m += &format!(
                    "| {} | {} | {} | {} |\n",
                    c.label,
                    num(l.real.series[i].p(50)),
                    num(g.series[i].p(50)),
                    d.continuous[i].map_or("n/a".to_string(), num)
                );
            }
            m += &format!("| Landform classes (most common) | {} | {} | {} |\n", dominant_form(&l.real), dominant_form(g), num(d.forms_l1));
            m += &format!(
                "| Drainage density (km/km2) | {} | {} | ratio {} |\n\n",
                num(l.real.density),
                num(g.density),
                d.density_ratio.map_or("n/a".to_string(), num)
            );
            let top: Vec<String> = ranked(std::slice::from_ref(&d)).iter().take(3).map(|(n, v)| format!("{n} ({})", num(*v))).collect();
            m += &format!("Largest gaps: {}.\n\n", top.join(", "));
        }
    }
    m += "## Notes\n\n";
    m += "- GLO-30 is a surface model: canopy, buildings and 1 to 2 m of vertical noise raise the real slopes, so the slope gaps on plains are upper bounds.\n";
    m += "- One real tile per landform and one 0.5 degree window per site; the real side is a single sample.\n";
    m += "- The largest gaps of a site rank its own distances (report.json ranks the mean over the sites per landform); drainage density is ranked by the size of the log of its ratio. There is no aggregate score.\n\n";
    m += &format!(
        "Run: store {} (definition {}), tiles {}, sites {}, {} threads. Phases (s): {}.\n",
        info.store,
        info.definition_hash,
        info.dem_dir,
        info.sites_file,
        info.threads,
        info.phases.iter().map(|(n, s)| format!("{n} {s:.1}")).collect::<Vec<_>>().join(", ")
    );
    m
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            '\n' => o += "\\n",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_num(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".into()
    }
}

fn json_opt(x: Option<f64>) -> String {
    x.map_or("null".into(), json_num)
}

fn json_list(v: &[f64]) -> String {
    format!("[{}]", v.iter().map(|x| json_num(*x)).collect::<Vec<_>>().join(","))
}

fn json_stats(s: &RasterStats) -> String {
    let series: Vec<String> = CONTINUOUS
        .iter()
        .zip(&s.series)
        .map(|(c, v)| format!("{}:{{\"counted\":{},\"percentiles_p1_to_p99\":{}}}", json_str(c.id), v.counted, json_list(&v.pct)))
        .collect();
    format!(
        "{{\"cells\":{},\"continuous\":{{{}}},\"geomorphon_shares\":{{{}}},\"geomorphon_counted\":{},\"drainage_density_km_per_km2\":{}}}",
        s.cells,
        series.join(","),
        FORM_NAMES.iter().zip(&s.forms).map(|(n, v)| format!("{}:{}", json_str(n), json_num(*v))).collect::<Vec<_>>().join(","),
        s.forms_counted,
        json_num(s.density)
    )
}

fn json_distances(d: &SiteDistances) -> String {
    format!(
        "{{\"continuous\":{{{}}},\"geomorphon_l1\":{},\"geomorphon_largest_gaps\":[{}],\"drainage_density_ratio\":{}}}",
        CONTINUOUS.iter().zip(&d.continuous).map(|(c, v)| format!("{}:{}", json_str(c.id), json_opt(*v))).collect::<Vec<_>>().join(","),
        json_num(d.forms_l1),
        d.forms_top
            .iter()
            .map(|(k, g)| format!("{{\"class\":{},\"generator_minus_real\":{}}}", json_str(FORM_NAMES[*k]), json_num(*g)))
            .collect::<Vec<_>>()
            .join(","),
        json_opt(d.density_ratio)
    )
}

/// `report.json`: every percentile, histogram and distance with the run's inputs and the wall time per phase (TEST-080).
pub fn render_json(info: &RunInfo, notice: &Notice, landforms: &[LandformResult]) -> String {
    let lands: Vec<String> = landforms
        .iter()
        .map(|l| {
            let gens: Vec<String> = l
                .generator
                .iter()
                .zip(&l.sites)
                .map(|(g, s)| {
                    format!(
                        "{{\"site\":[{},{}],\"stats\":{},\"distance\":{}}}",
                        json_num(s.0),
                        json_num(s.1),
                        json_stats(g),
                        json_distances(&distances(&l.real, g))
                    )
                })
                .collect();
            format!(
                "{{\"name\":{},\"tile\":{},\"real\":{},\"generator\":[{}],\"largest_gaps\":[{}]}}",
                json_str(&l.name),
                json_str(&l.tile),
                json_stats(&l.real),
                gens.join(","),
                ranked_gaps(l)
                    .iter()
                    .take(3)
                    .map(|(n, v)| format!("{{\"descriptor\":{},\"gap\":{}}}", json_str(n), json_num(*v)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect();
    format!(
        "{{\"notice\":{},\"no_liability\":{},\"doi\":{},\"date\":{},\"commit\":{},\"store\":{},\"definition_hash\":{},\"tiles\":{},\"sites_file\":{},\"threads\":{},\
         \"phase_seconds\":{{{}}},\"landforms\":[{}]}}\n",
        json_str(notice.notice),
        json_str(notice.no_liability),
        json_str(notice.doi),
        json_str(&info.date),
        json_str(&info.commit),
        json_str(&info.store),
        json_str(&info.definition_hash),
        json_str(&info.dem_dir),
        json_str(&info.sites_file),
        info.threads,
        info.phases.iter().map(|(n, s)| format!("{}:{}", json_str(n), json_num(*s))).collect::<Vec<_>>().join(","),
        lands.join(",")
    )
}

/// `YYYY-MM-DD` (UTC) of a time in seconds since the Unix epoch.
pub fn utc_date(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTICE: Notice<'static> = Notice {
        notice: planet_testkit::glo30::COPERNICUS_NOTICE,
        no_liability: planet_testkit::glo30::COPERNICUS_NO_LIABILITY,
        doi: planet_testkit::glo30::COPERNICUS_DOI,
    };

    /// Stats whose slope percentiles are `base + k * step` (so p5 to p95 is 90 steps) and which are otherwise `fill`.
    fn stats(base: f64, step: f64, forms: [f64; 10], density: f64) -> RasterStats {
        let pct: Vec<f64> = (1..=PERCENTILES).map(|q| base + q as f64 * step).collect();
        RasterStats {
            cells: 100,
            series: CONTINUOUS.iter().map(|_| Series { counted: 99, pct: pct.clone() }).collect(),
            forms,
            forms_counted: 100,
            density,
        }
    }

    fn one_hot(k: usize) -> [f64; 10] {
        let mut f = [0.0; 10];
        f[k] = 1.0;
        f
    }

    fn landform(g: &[RasterStats]) -> LandformResult {
        LandformResult {
            name: "Test hills".into(),
            tile: "N47E011".into(),
            sites: vec![(1.5, 2.5), (-3.5, 4.5)],
            real: stats(0.0, 1.0, one_hot(5), 2.0),
            generator: g.to_vec(),
        }
    }

    // spec: TEST-079
    #[test]
    fn percentiles_of_a_ramp_are_the_ramp() {
        let mut v: Vec<f64> = (0..=100).map(f64::from).rev().collect();
        let p = percentiles(&mut v).unwrap();
        assert_eq!((p[0], p[49], p[98]), (1.0, 50.0, 99.0));
        assert_eq!(percentiles(&mut []), None);
        let mut two = [0.0, 10.0];
        assert!((percentiles(&mut two).unwrap()[49] - 5.0).abs() < 1e-12, "linear between ranks");
    }

    // spec: TEST-079
    #[test]
    fn distances_are_zero_for_equal_sets_a_quarter_for_a_quarter_shift_and_two_for_disjoint_classes() {
        let real = stats(0.0, 1.0, one_hot(0), 1.0);
        let same = distances(&real, &real);
        assert!(same.continuous.iter().all(|d| *d == Some(0.0)));
        assert_eq!((same.forms_l1, same.density_ratio), (0.0, Some(1.0)));
        // p5 to p95 of the real set is 90 steps: a shift of 22.5 is a quarter of it.
        let shifted = stats(22.5, 1.0, one_hot(9), 3.0);
        let d = distances(&real, &shifted);
        assert!(d.continuous.iter().all(|v| (v.unwrap() - 0.25).abs() < 1e-12), "{:?}", d.continuous);
        assert_eq!(d.forms_l1, 2.0);
        assert_eq!(d.forms_top.iter().map(|t| t.0).collect::<Vec<_>>(), vec![0, 9, 1]);
        assert_eq!(d.density_ratio, Some(3.0));
        // A real set with no spread has no distance, never a division by zero.
        assert_eq!(wasserstein_over_range(&vec![5.0; 99], &vec![6.0; 99]), None);
    }

    // spec: TEST-080
    #[test]
    fn the_report_opens_with_the_notice_keeps_tables_to_four_columns_and_names_three_gaps() {
        let a = stats(5.0, 1.0, one_hot(5), 1.0);
        let b = stats(0.0, 2.0, one_hot(2), 4.0);
        let l = landform(&[a, b]);
        let info =
            RunInfo { date: "2026-10-31".into(), commit: "abc1234".into(), phases: vec![("read tiles".into(), 1.5)], ..RunInfo::default() };
        let md = render_markdown(&info, &NOTICE, &[l.clone(), l]);
        let lines: Vec<&str> = md.lines().collect();
        assert_eq!(lines[0], "# Terrain scorecard 2026-10-31");
        assert!(lines[1].is_empty());
        assert!(
            lines[2].contains("Copernicus WorldDEM-30") && lines[2].contains("do not incur any liability") && lines[2].contains(NOTICE.doi),
            "{}",
            lines[2]
        );
        for t in lines.iter().filter(|l| l.starts_with('|')) {
            assert!(t.trim_matches('|').split('|').count() <= 4, "a table row over four columns: {t}");
        }
        assert_eq!(md.matches("\n## Test hills").count(), 2);
        assert_eq!(md.matches("\n### Site ").count(), 4, "a table per site");
        assert_eq!(lines.iter().filter(|l| l.starts_with("| Descriptor")).count(), 4);
        assert!(md.contains("Commit abc1234") && md.contains("Date 2026-10-31"), "inputs at the top");
        let gap_lines: Vec<&&str> = lines.iter().filter(|l| l.starts_with("Largest gaps:")).collect();
        assert_eq!(gap_lines.len(), 4, "one line of gaps per site");
        assert!(gap_lines.iter().all(|l| l.trim_end_matches(").").split("), ").count() == 3), "three named gaps: {gap_lines:?}");
        assert!(md.contains("Phases (s): read tiles 1.5"));
    }

    // spec: TEST-080
    #[test]
    fn the_json_report_holds_every_percentile_and_the_inputs_and_is_well_formed() {
        let l = landform(&[stats(5.0, 1.0, one_hot(5), f64::NAN), stats(0.0, 2.0, one_hot(2), 4.0)]);
        let info = RunInfo { store: "C:/store \"x\"".into(), threads: 8, phases: vec![("descriptors".into(), 2.0)], ..RunInfo::default() };
        let js = render_json(&info, &NOTICE, &[l]);
        assert!(js.contains("\"threads\":8") && js.contains("\"descriptors\":2") && js.contains("C:/store \\\"x\\\""));
        assert!(js.contains("\"drainage_density_km_per_km2\":null"), "NaN is null");
        assert_eq!(js.matches("\"percentiles_p1_to_p99\"").count(), 3 * CONTINUOUS.len());
        // Braces and brackets balance outside strings.
        let (mut depth, mut in_str, mut esc) = (0i32, false, false);
        for c in js.chars() {
            if in_str {
                match (esc, c) {
                    (true, _) => esc = false,
                    (false, '\\') => esc = true,
                    (false, '"') => in_str = false,
                    _ => {}
                }
            } else {
                match c {
                    '"' => in_str = true,
                    '{' | '[' => depth += 1,
                    '}' | ']' => depth -= 1,
                    _ => {}
                }
            }
            assert!(depth >= 0);
        }
        assert_eq!(depth, 0);
    }

    // spec: TEST-079
    #[test]
    fn the_sites_file_has_nine_landforms_with_two_sites_and_tile_names_map_to_files() {
        let sites = parse_sites(include_str!("../data/terrain-score-sites.toml")).unwrap();
        assert_eq!(sites.len(), 9);
        assert!(sites.iter().all(|s| s.sites.len() == 2));
        assert_eq!(
            sites[0],
            LandformSpec { name: "Brandenburg plain".into(), tile: "N52E012".into(), sites: vec![(-38.5, 30.5), (41.5, 149.5)] }
        );
        assert_eq!(tile_file_name("N43W102").unwrap(), "Copernicus_DSM_COG_10_N43_00_W102_00_DEM.tif");
        assert_eq!(tile_file_name("43N102W"), None);
        assert!(parse_sites("[[landform]]\nname = \"x\"\ntile = \"N52E012\"\nsites = []\n").is_err());
        assert!(parse_sites("").is_err());
    }

    // spec: TEST-079
    #[test]
    fn the_generator_window_is_a_30_m_east_north_grid_round_the_site() {
        let r = 6_371_000.0;
        let dirs = generator_directions(47.5, 11.5, r);
        assert_eq!(dirs.len(), GRID * GRID);
        let at = |row: usize, col: usize| dirs[row * GRID + col] * r;
        let dist = |a: Vec3, b: Vec3| libm::sqrt((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y) + (a.z - b.z) * (a.z - b.z));
        let mid = GRID / 2;
        assert!((dist(at(mid, mid), at(mid, mid + 1)) - 30.0).abs() < 0.01);
        assert!((dist(at(mid, mid), at(mid + 1, mid)) - 30.0).abs() < 0.01);
        // Row 0 is north: its latitude is higher than the middle row's.
        assert!(dirs[mid].z > dirs[mid * GRID + mid].z);
        // East is increasing longitude.
        let lon = |d: Vec3| libm::atan2(d.y, d.x).to_degrees();
        assert!(lon(dirs[mid * GRID + mid + 1]) > lon(dirs[mid * GRID + mid]));
        let (la, lo) = (libm::asin(dirs[mid * GRID + mid].z).to_degrees(), lon(dirs[mid * GRID + mid]));
        assert!((la - 47.5).abs() < 1e-3 && (lo - 11.5).abs() < 1e-3, "{la} {lo}");
    }

    // spec: TEST-079
    #[test]
    fn the_real_raster_reproduces_a_linear_surface_exactly() {
        // A 1 degree tile of 3600 x 3600 pixels at 47 degrees north whose height is 1 m per 0.001 degree of longitude plus 2 m per 0.001 degree of latitude.
        let (w, h) = (3600usize, 3600usize);
        let (scale, lon0, lat0) = (1.0 / 3600.0, 11.0, 48.0);
        let data: Vec<f32> = (0..w * h)
            .map(|k| {
                let (lon, lat) = (lon0 + ((k % w) as f64 + 0.5) * scale, lat0 - ((k / w) as f64 + 0.5) * scale);
                ((lon - 11.0) * 1000.0 + (lat - 47.0) * 2000.0) as f32
            })
            .collect();
        let t = TileView { data: &data, width: w, height: h, origin_lon: lon0, origin_lat: lat0, scale_lon: scale, scale_lat: scale };
        assert_eq!(t.centre(), (47.5, 11.5));
        let z = resample_real(&t);
        let mid = GRID / 2;
        // The central cell sits half a cell off the centre; its height is the plane's at that offset.
        let (x, y) = cell_offset(HALF_DEG * METRES_PER_DEG, mid, mid);
        let (lon, lat) = (11.5 + x / (METRES_PER_DEG * libm::cos(47.5f64.to_radians())), 47.5 + y / METRES_PER_DEG);
        let want = (lon - 11.0) * 1000.0 + (lat - 47.0) * 2000.0;
        assert!((z[mid * GRID + mid] - want).abs() < 1e-3, "{} vs {want}", z[mid * GRID + mid]);
        assert!(z.iter().all(|v| v.is_finite()));
        // No-data in the stencil gives NaN.
        let mut holey = data.clone();
        holey[1800 * w + 1800] = f32::NAN;
        let t2 = TileView { data: &holey, ..t };
        assert!(resample_real(&t2).iter().any(|v| v.is_nan()));
    }

    // spec: TEST-079
    #[test]
    fn a_tilted_plane_has_its_slope_and_the_result_does_not_depend_on_the_thread_count() {
        // 10 degrees rising east on a 200 x 200 raster.
        let (w, h) = (200usize, 200usize);
        let rise = libm::tan(10f64.to_radians()) * CELL_M;
        let z: Vec<f64> = (0..w * h).map(|k| 500.0 + (k % w) as f64 * rise).collect();
        let (one, maps) = analyse(&z, w, h, 1, true);
        let (four, _) = analyse(&z, w, h, 4, false);
        assert_eq!(one, four, "CON-14: one thread or four give the same statistics");
        assert!((one.series[0].p(50) - 10.0).abs() < 1e-9 && (one.series[0].p(99) - 10.0).abs() < 1e-9);
        assert_eq!(one.series.len(), CONTINUOUS.len());
        // A curved surface (a bowl rising east) through every kernel, again at one thread and at four.
        let bowl: Vec<f64> =
            (0..w * h).map(|k| 500.0 + ((k % w) as f64 * rise) + 0.009 * ((k / w) as f64 - 100.0) * ((k / w) as f64 - 100.0)).collect();
        assert_eq!(analyse(&bowl, w, h, 1, false).0, analyse(&bowl, w, h, 4, false).0);
        assert_eq!(maps.len(), 9);
        assert!(maps.iter().all(|m| m.rgba.len() == MAP_PX * MAP_PX * 4));
    }

    // spec: TEST-080
    #[test]
    fn numbers_print_in_three_digits_and_dates_in_utc() {
        assert_eq!(num(23.456), "23.5");
        assert_eq!(num(0.01234), "0.012");
        assert_eq!(num(1234.6), "1235");
        assert_eq!(num(1.2e-4), "1.2e-4");
        assert_eq!(num(f64::NAN), "n/a");
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400), "2000-02-29");
    }
}
