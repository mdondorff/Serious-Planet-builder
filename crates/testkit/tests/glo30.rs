//! The GLO-30 reader (TEST-078). Tier A builds TIFFs with a writer that shares no code with the reader;
//! tier B (ignored) decodes a real tile from `PLANET_GLO30_DIR`.

use planet_testkit::glo30::{self, Glo30Error, Tile};

const NODATA: f32 = -32767.0;

struct Opts {
    tiled: bool,
    compression: u16,
    predictor: u16,
    sample_format: u16,
}

impl Opts {
    fn good(tiled: bool) -> Self {
        Self { tiled, compression: 8, predictor: 3, sample_format: 3 }
    }
}

/// Forward floating-point predictor (TIFF Technical Note 3), written from the definition:
/// split the row into byte planes (most significant byte first), then difference each byte from its left neighbour.
fn forward_predict(samples: &[f32]) -> Vec<u8> {
    let w = samples.len();
    let mut planes = vec![0u8; w * 4];
    for (x, s) in samples.iter().enumerate() {
        let b = s.to_bits();
        for p in 0..4 {
            planes[p * w + x] = (b >> (24 - 8 * p)) as u8;
        }
    }
    for i in (1..planes.len()).rev() {
        planes[i] = planes[i].wrapping_sub(planes[i - 1]);
    }
    planes
}

fn encode_block(rows: &[Vec<f32>], o: &Opts) -> Vec<u8> {
    let mut raw = Vec::new();
    for r in rows {
        if o.predictor == 3 {
            raw.extend(forward_predict(r));
        } else {
            raw.extend(r.iter().flat_map(|v| v.to_be_bytes()));
        }
    }
    miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6)
}

struct Tag {
    id: u16,
    ty: u16,
    count: u32,
    data: Vec<u8>,
}

fn shorts(id: u16, v: &[u16]) -> Tag {
    Tag { id, ty: 3, count: v.len() as u32, data: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
}
fn longs(id: u16, v: &[u32]) -> Tag {
    Tag { id, ty: 4, count: v.len() as u32, data: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
}
fn doubles(id: u16, v: &[f64]) -> Tag {
    Tag { id, ty: 12, count: v.len() as u32, data: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
}
fn ascii(id: u16, s: &str) -> Tag {
    let mut d = s.as_bytes().to_vec();
    d.push(0);
    Tag { id, ty: 2, count: d.len() as u32, data: d }
}

/// Build a little-endian classic TIFF of `img` (rows of f32). Tiles are 16x16 and padded with a marker value;
/// strips hold 7 rows (the last strip is short).
fn write_tiff(img: &[Vec<f32>], o: &Opts, geo: (f64, f64, f64, f64)) -> Vec<u8> {
    let (h, w) = (img.len(), img[0].len());
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let (bw, bh);
    if o.tiled {
        (bw, bh) = (16usize, 16usize);
        for ty in 0..h.div_ceil(bh) {
            for tx in 0..w.div_ceil(bw) {
                let rows: Vec<Vec<f32>> = (0..bh)
                    .map(|r| {
                        (0..bw).map(|c| img.get(ty * bh + r).and_then(|row| row.get(tx * bw + c)).copied().unwrap_or(12345.0)).collect()
                    })
                    .collect();
                blocks.push(encode_block(&rows, o));
            }
        }
    } else {
        (bw, bh) = (w, 7usize);
        for y0 in (0..h).step_by(bh) {
            let rows: Vec<Vec<f32>> = img[y0..(y0 + bh).min(h)].to_vec();
            blocks.push(encode_block(&rows, o));
        }
    }
    let mut out = vec![b'I', b'I', 42, 0, 0, 0, 0, 0];
    let mut offsets = Vec::new();
    let counts: Vec<u32> = blocks.iter().map(|b| b.len() as u32).collect();
    for b in &blocks {
        offsets.push(out.len() as u32);
        out.extend(b);
    }
    let (off_id, cnt_id) = if o.tiled { (324, 325) } else { (273, 279) };
    let mut tags = vec![
        longs(256, &[w as u32]),
        longs(257, &[h as u32]),
        shorts(258, &[32]),
        shorts(259, &[o.compression]),
        shorts(277, &[1]),
        shorts(284, &[1]),
        shorts(317, &[o.predictor]),
        shorts(339, &[o.sample_format]),
        longs(off_id, &offsets),
        longs(cnt_id, &counts),
        doubles(33550, &[geo.2, geo.3, 0.0]),
        doubles(33922, &[0.0, 0.0, 0.0, geo.0, geo.1, 0.0]),
        ascii(42113, "-32767"),
    ];
    if o.tiled {
        tags.push(longs(322, &[bw as u32]));
        tags.push(longs(323, &[bh as u32]));
    } else {
        tags.push(longs(278, &[bh as u32]));
    }
    tags.sort_by_key(|t| t.id);
    // Out-of-line values first, then the IFD.
    let mut entries = Vec::new();
    for t in &tags {
        let mut value = [0u8; 4];
        if t.data.len() <= 4 {
            value[..t.data.len()].copy_from_slice(&t.data);
        } else {
            if out.len() % 2 == 1 {
                out.push(0);
            }
            value = (out.len() as u32).to_le_bytes();
            out.extend(&t.data);
        }
        entries.push((t.id, t.ty, t.count, value));
    }
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let ifd_at = out.len() as u32;
    out.extend((entries.len() as u16).to_le_bytes());
    for (id, ty, count, value) in entries {
        out.extend(id.to_le_bytes());
        out.extend(ty.to_le_bytes());
        out.extend(count.to_le_bytes());
        out.extend(value);
    }
    out.extend(0u32.to_le_bytes());
    out[4..8].copy_from_slice(&ifd_at.to_le_bytes());
    out
}

fn test_image(w: usize, h: usize) -> Vec<Vec<f32>> {
    let mut s = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| {
                    let v = next();
                    if (x, y) == (3, 2) || (x, y) == (39, 29) || (x, y) == (17, 16) {
                        NODATA
                    } else if v % 7 == 0 {
                        // Wide dynamic range, negative values and tiny fractions.
                        ((v >> 8) as f32 / 1000.0) * if v & 1 == 0 { 1.0 } else { -1.0 }
                    } else {
                        1000.0 + (x as f32) * 0.37 - (y as f32) * 1.9 + (v % 1000) as f32 / 1000.0
                    }
                })
                .collect()
        })
        .collect()
}

fn assert_round_trip(tile: &Tile, img: &[Vec<f32>], geo: (f64, f64, f64, f64)) {
    assert_eq!(tile.size(), (img[0].len(), img.len()));
    assert_eq!(tile.nodata, Some(NODATA));
    let mut nodata_cells = 0;
    for (y, row) in img.iter().enumerate() {
        for (x, &v) in row.iter().enumerate() {
            if v == NODATA {
                nodata_cells += 1;
                assert!(tile.is_nodata(y, x), "cell ({y},{x}) should be no-data");
            } else {
                assert_eq!(tile.sample(y, x).to_bits(), v.to_bits(), "cell ({y},{x})");
            }
        }
    }
    assert_eq!(nodata_cells, 3);
    assert_eq!(tile.nodata_count(), 3);
    assert_eq!((tile.geo.origin_lon, tile.geo.origin_lat, tile.geo.scale_lon, tile.geo.scale_lat), geo);
    let (lon, lat) = tile.pixel_centre(0, 0);
    assert_eq!((lon, lat), (geo.0 + 0.5 * geo.2, geo.1 - 0.5 * geo.3));
}

// spec: TEST-078
#[test]
fn round_trip_tiled_and_stripped() {
    let geo = (11.0, 48.0, 1.0 / 40.0, 1.0 / 30.0);
    let img = test_image(40, 30);
    for tiled in [true, false] {
        let bytes = write_tiff(&img, &Opts::good(tiled), geo);
        let tile = glo30::parse(&bytes).unwrap_or_else(|e| panic!("tiled={tiled}: {e}"));
        assert_round_trip(&tile, &img, geo);
    }
}

// spec: TEST-078
#[test]
fn other_encodings_are_refused_naming_the_tag() {
    let geo = (0.0, 1.0, 0.1, 0.1);
    let img = test_image(40, 30);
    let cases: [(Opts, &str, &str); 3] = [
        (Opts { compression: 5, ..Opts::good(true) }, "Compression", "259"),
        (Opts { predictor: 2, ..Opts::good(true) }, "Predictor", "317"),
        (Opts { sample_format: 1, ..Opts::good(false) }, "SampleFormat", "339"),
    ];
    for (o, name, code) in cases {
        let err = glo30::parse(&write_tiff(&img, &o, geo)).expect_err("must be refused");
        assert!(matches!(err, Glo30Error::Unsupported { .. }), "{err}");
        let text = err.to_string();
        assert!(text.contains(name) && text.contains(code), "error should name {name} ({code}): {text}");
    }
}

fn dir() -> std::path::PathBuf {
    match std::env::var("PLANET_GLO30_DIR") {
        Ok(d) => d.into(),
        Err(_) => {
            println!("GLO-30 real tile: NOT RUN (PLANET_GLO30_DIR not set)");
            panic!("GLO-30 real tile: NOT RUN (PLANET_GLO30_DIR not set)");
        }
    }
}

/// p50 of the slope in degrees by central differences at the true metric spacing, over rows and columns `[r0, r1)` x `[c0, c1)`.
#[allow(clippy::disallowed_methods)] // reason: a tolerance check of half a degree; platform maths cannot move it
fn slope_p50(t: &Tile, r0: usize, r1: usize, c0: usize, c1: usize) -> f64 {
    let mut slopes = Vec::new();
    let dy = 2.0 * t.geo.scale_lat * 3600.0 * glo30::METRES_PER_ARCSEC;
    for r in r0.max(1)..r1.min(t.height - 1) {
        let lat = t.pixel_centre(r, 0).1;
        let dx = 2.0 * t.geo.scale_lon * 3600.0 * glo30::METRES_PER_ARCSEC * lat.to_radians().cos();
        for c in c0.max(1)..c1.min(t.width - 1) {
            let (e, w, n, s) = (t.sample(r, c + 1), t.sample(r, c - 1), t.sample(r - 1, c), t.sample(r + 1, c));
            if e.is_nan() || w.is_nan() || n.is_nan() || s.is_nan() {
                continue;
            }
            let (gx, gy) = ((e - w) as f64 / dx, (n - s) as f64 / dy);
            slopes.push((gx * gx + gy * gy).sqrt().atan().to_degrees());
        }
    }
    slopes.sort_by(f64::total_cmp);
    slopes[(slopes.len() - 1) / 2]
}

// spec: TEST-078
#[test]
#[ignore = "reads the sample tiles from PLANET_GLO30_DIR; NOT RUN when it is not set"]
fn real_tile_n47e011_slope_p50() {
    let path = dir().join("Copernicus_DSM_COG_10_N47_00_E011_00_DEM.tif");
    let t = glo30::read(&path).unwrap();
    let (w, h) = t.size();
    // The central 0.5-degree square: the middle half of the tile in both axes.
    let central = slope_p50(&t, h / 4, 3 * h / 4, w / 4, 3 * w / 4);
    let whole = slope_p50(&t, 0, h, 0, w);
    println!(
        "GLO-30 N47E011 {w}x{h}, no-data cells {}: central-square p50 slope {central:.2} deg, whole-tile {whole:.2} deg (SPIKE-55: 19.3)",
        t.nodata_count()
    );
    // SPIKE-55's 19.3 is a whole-tile number; the central square is steeper (the scenario's premise does not hold),
    // so the check that the reader and the slope definition reproduce SPIKE-55 uses the whole tile.
    assert!((whole - 19.3).abs() <= 0.5, "whole-tile p50 slope {whole:.2} deg is not within 0.5 of 19.3 (central square {central:.2})");
}
