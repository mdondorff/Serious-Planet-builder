//! Reader for a Copernicus GLO-30 COG GeoTIFF tile (TEST-078).
//!
//! Hand-written for the one file shape the tiles have: classic little-endian TIFF, one float32
//! band, deflate with the floating-point predictor (TIFF Technical Note 3), tiled or stripped.
//! Anything else is refused with an error naming the tag. Only the first IFD (full resolution) is read.
//!
//! Representation: the raster is row-major f32, row 0 at the north edge. Cells equal to the file's
//! GDAL_NODATA value are stored as `NaN`; `Tile::nodata` keeps the original value.

use std::fmt;
use std::path::Path;

/// Notice for anything adapted or derived from the data (statistics, images, tables); LICENCE-NOTES.md, Art. 6.
pub const COPERNICUS_NOTICE: &str = "produced using Copernicus WorldDEM-30 \u{a9} DLR e.V. 2010-2014 and \u{a9} Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved";
/// Added to any distribution or public communication, modified or not (Art. 6).
pub const COPERNICUS_NO_LIABILITY: &str =
    "The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30";
/// Research citation requested on the COP-DEM collection page.
pub const COPERNICUS_DOI: &str = "https://doi.org/10.5270/ESA-c5d3d65";

/// Metres per arc-second of latitude used for the metric spacing of a pixel.
pub const METRES_PER_ARCSEC: f64 = 30.87;

#[derive(Debug)]
pub enum Glo30Error {
    Io(std::io::Error),
    /// The tag or header field is outside what the reader handles.
    Unsupported {
        tag: &'static str,
        code: u16,
        found: String,
    },
    /// A required tag is absent.
    Missing {
        tag: &'static str,
        code: u16,
    },
    /// The file is cut off or inconsistent.
    Corrupt(String),
}

impl fmt::Display for Glo30Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Glo30Error::Io(e) => write!(f, "GLO-30 read failed: {e}"),
            Glo30Error::Unsupported { tag, code, found } => {
                write!(f, "GLO-30 tile refused: tag {tag} ({code}) is {found}, not supported")
            }
            Glo30Error::Missing { tag, code } => write!(f, "GLO-30 tile refused: tag {tag} ({code}) is missing"),
            Glo30Error::Corrupt(m) => write!(f, "GLO-30 tile is corrupt: {m}"),
        }
    }
}

impl std::error::Error for Glo30Error {}

impl From<std::io::Error> for Glo30Error {
    fn from(e: std::io::Error) -> Self {
        Glo30Error::Io(e)
    }
}

/// Georeference from ModelTiepoint and ModelPixelScale (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoRef {
    /// Longitude of the top-left corner of the top-left pixel.
    pub origin_lon: f64,
    /// Latitude of the top-left corner of the top-left pixel.
    pub origin_lat: f64,
    /// Degrees of longitude per pixel (positive, east).
    pub scale_lon: f64,
    /// Degrees of latitude per pixel (positive; rows run south).
    pub scale_lat: f64,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub width: usize,
    pub height: usize,
    /// Row-major, row 0 north; no-data cells are NaN.
    pub data: Vec<f32>,
    pub geo: GeoRef,
    /// The file's no-data value, if it names one.
    pub nodata: Option<f32>,
}

impl Tile {
    /// `(width, height)` in pixels.
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    pub fn sample(&self, row: usize, col: usize) -> f32 {
        self.data[row * self.width + col]
    }

    pub fn is_nodata(&self, row: usize, col: usize) -> bool {
        self.sample(row, col).is_nan()
    }

    pub fn nodata_count(&self) -> usize {
        self.data.iter().filter(|v| v.is_nan()).count()
    }

    /// `(lon, lat)` in degrees of the centre of pixel `(row, col)` (area pixels: corner plus half a pixel).
    pub fn pixel_centre(&self, row: usize, col: usize) -> (f64, f64) {
        let g = &self.geo;
        (g.origin_lon + (col as f64 + 0.5) * g.scale_lon, g.origin_lat - (row as f64 + 0.5) * g.scale_lat)
    }
}

// TIFF tag numbers.
const T_WIDTH: u16 = 256;
const T_HEIGHT: u16 = 257;
const T_BITS: u16 = 258;
const T_COMPRESSION: u16 = 259;
const T_STRIP_OFFSETS: u16 = 273;
const T_SAMPLES_PER_PIXEL: u16 = 277;
const T_ROWS_PER_STRIP: u16 = 278;
const T_STRIP_COUNTS: u16 = 279;
const T_PLANAR: u16 = 284;
const T_PREDICTOR: u16 = 317;
const T_TILE_WIDTH: u16 = 322;
const T_TILE_LENGTH: u16 = 323;
const T_TILE_OFFSETS: u16 = 324;
const T_TILE_COUNTS: u16 = 325;
const T_SAMPLE_FORMAT: u16 = 339;
const T_PIXEL_SCALE: u16 = 33550;
const T_TIEPOINT: u16 = 33922;
const T_NODATA: u16 = 42113;

struct Entry {
    ty: u16,
    count: usize,
    /// The 4 value bytes of the entry (inline value, or the offset).
    raw: [u8; 4],
}

struct Ifd<'a> {
    file: &'a [u8],
    entries: Vec<(u16, Entry)>,
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, Glo30Error> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| Glo30Error::Corrupt(format!("read past end at byte {at}")))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, Glo30Error> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| Glo30Error::Corrupt(format!("read past end at byte {at}")))
}

impl Ifd<'_> {
    fn entry(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|(t, _)| *t == tag).map(|(_, e)| e)
    }

    /// The bytes of an entry's value: inline when at most 4, else at the offset.
    fn bytes<'a>(&'a self, e: &'a Entry, unit: usize) -> Result<&'a [u8], Glo30Error> {
        let len = e.count * unit;
        if len <= 4 {
            return Ok(&e.raw[..len]);
        }
        let off = u32::from_le_bytes(e.raw) as usize;
        self.file.get(off..off + len).ok_or_else(|| Glo30Error::Corrupt(format!("tag value at {off}..{} is outside the file", off + len)))
    }

    fn uints(&self, tag: u16, name: &'static str) -> Result<Vec<u64>, Glo30Error> {
        let e = self.entry(tag).ok_or(Glo30Error::Missing { tag: name, code: tag })?;
        let (unit, ty) = match e.ty {
            3 => (2, 3),
            4 => (4, 4),
            16 => (8, 16),
            t => return Err(Glo30Error::Unsupported { tag: name, code: tag, found: format!("field type {t}") }),
        };
        let b = self.bytes(e, unit)?;
        Ok(b.chunks_exact(unit)
            .map(|c| match ty {
                3 => u16::from_le_bytes([c[0], c[1]]) as u64,
                4 => u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64,
                _ => u64::from_le_bytes(c.try_into().unwrap()),
            })
            .collect())
    }

    fn uint(&self, tag: u16, name: &'static str) -> Result<u64, Glo30Error> {
        self.uints(tag, name)?.first().copied().ok_or(Glo30Error::Missing { tag: name, code: tag })
    }

    fn uint_or(&self, tag: u16, name: &'static str, default: u64) -> Result<u64, Glo30Error> {
        if self.entry(tag).is_none() {
            Ok(default)
        } else {
            self.uint(tag, name)
        }
    }

    fn doubles(&self, tag: u16, name: &'static str) -> Result<Vec<f64>, Glo30Error> {
        let e = self.entry(tag).ok_or(Glo30Error::Missing { tag: name, code: tag })?;
        if e.ty != 12 {
            return Err(Glo30Error::Unsupported { tag: name, code: tag, found: format!("field type {}", e.ty) });
        }
        Ok(self.bytes(e, 8)?.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect())
    }
}

fn refuse(tag: &'static str, code: u16, found: impl Into<String>) -> Glo30Error {
    Glo30Error::Unsupported { tag, code, found: found.into() }
}

/// Read a tile from a file.
pub fn read(path: &Path) -> Result<Tile, Glo30Error> {
    parse(&std::fs::read(path)?)
}

/// Read a tile from the bytes of a TIFF file.
pub fn parse(file: &[u8]) -> Result<Tile, Glo30Error> {
    if file.len() < 8 {
        return Err(Glo30Error::Corrupt("shorter than a TIFF header".into()));
    }
    match &file[0..2] {
        b"II" => {}
        b"MM" => return Err(refuse("ByteOrder", 0, "big-endian (MM)")),
        _ => return Err(Glo30Error::Corrupt("not a TIFF (no II/MM byte order mark)".into())),
    }
    match u16_at(file, 2)? {
        42 => {}
        43 => return Err(refuse("TIFF version", 43, "BigTIFF")),
        v => return Err(refuse("TIFF version", v, format!("magic {v}"))),
    }
    let ifd_at = u32_at(file, 4)? as usize;
    let n = u16_at(file, ifd_at)? as usize;
    let mut entries = Vec::with_capacity(n);
    for i in 0..n {
        let at = ifd_at + 2 + i * 12;
        let raw = file.get(at + 8..at + 12).ok_or_else(|| Glo30Error::Corrupt("IFD cut off".into()))?;
        entries.push((
            u16_at(file, at)?,
            Entry { ty: u16_at(file, at + 2)?, count: u32_at(file, at + 4)? as usize, raw: [raw[0], raw[1], raw[2], raw[3]] },
        ));
    }
    let ifd = Ifd { file, entries };

    let width = ifd.uint(T_WIDTH, "ImageWidth")? as usize;
    let height = ifd.uint(T_HEIGHT, "ImageLength")? as usize;
    let bits = ifd.uint(T_BITS, "BitsPerSample")?;
    if bits != 32 {
        return Err(refuse("BitsPerSample", T_BITS, format!("{bits}")));
    }
    let spp = ifd.uint_or(T_SAMPLES_PER_PIXEL, "SamplesPerPixel", 1)?;
    if spp != 1 {
        return Err(refuse("SamplesPerPixel", T_SAMPLES_PER_PIXEL, format!("{spp}")));
    }
    let planar = ifd.uint_or(T_PLANAR, "PlanarConfiguration", 1)?;
    if planar != 1 {
        return Err(refuse("PlanarConfiguration", T_PLANAR, format!("{planar}")));
    }
    let comp = ifd.uint_or(T_COMPRESSION, "Compression", 1)?;
    if comp != 8 && comp != 32946 {
        return Err(refuse("Compression", T_COMPRESSION, format!("{comp}")));
    }
    let pred = ifd.uint_or(T_PREDICTOR, "Predictor", 1)?;
    if pred != 3 {
        return Err(refuse("Predictor", T_PREDICTOR, format!("{pred}")));
    }
    let fmt_ = ifd.uint_or(T_SAMPLE_FORMAT, "SampleFormat", 1)?;
    if fmt_ != 3 {
        return Err(refuse("SampleFormat", T_SAMPLE_FORMAT, format!("{fmt_}")));
    }

    let tiled = ifd.entry(T_TILE_OFFSETS).is_some();
    let (block_w, block_h, offsets, counts) = if tiled {
        (
            ifd.uint(T_TILE_WIDTH, "TileWidth")? as usize,
            ifd.uint(T_TILE_LENGTH, "TileLength")? as usize,
            ifd.uints(T_TILE_OFFSETS, "TileOffsets")?,
            ifd.uints(T_TILE_COUNTS, "TileByteCounts")?,
        )
    } else {
        (
            width,
            ifd.uint_or(T_ROWS_PER_STRIP, "RowsPerStrip", height as u64)?.min(height as u64) as usize,
            ifd.uints(T_STRIP_OFFSETS, "StripOffsets")?,
            ifd.uints(T_STRIP_COUNTS, "StripByteCounts")?,
        )
    };
    if width == 0 || height == 0 || block_w == 0 || block_h == 0 {
        return Err(Glo30Error::Corrupt("zero image or block size".into()));
    }
    let across = width.div_ceil(block_w);
    let down = height.div_ceil(block_h);
    if offsets.len() != across * down || counts.len() != offsets.len() {
        return Err(Glo30Error::Corrupt(format!("{} offsets and {} counts for {across}x{down} blocks", offsets.len(), counts.len())));
    }

    let mut data = vec![0f32; width * height];
    for by in 0..down {
        for bx in 0..across {
            let i = by * across + bx;
            let (off, len) = (offsets[i] as usize, counts[i] as usize);
            let comp_bytes = file
                .get(off..off + len)
                .ok_or_else(|| Glo30Error::Corrupt(format!("block {bx},{by} at {off}+{len} is outside the file")))?;
            // Tiles are stored padded to the full tile size; the last strip holds only its own rows.
            let rows = if tiled { block_h } else { block_h.min(height - by * block_h) };
            let mut raw = miniz_oxide::inflate::decompress_to_vec_zlib(comp_bytes)
                .map_err(|e| Glo30Error::Corrupt(format!("block {bx},{by}: inflate failed: {e:?}")))?;
            if raw.len() != rows * block_w * 4 {
                return Err(Glo30Error::Corrupt(format!("block {bx},{by}: {} bytes inflated, {} expected", raw.len(), rows * block_w * 4)));
            }
            for row in raw.chunks_exact_mut(block_w * 4) {
                undo_float_predictor(row, block_w);
            }
            for r in 0..rows {
                let y = by * block_h + r;
                if y >= height {
                    break;
                }
                for c in 0..block_w {
                    let x = bx * block_w + c;
                    if x >= width {
                        break;
                    }
                    let s = (r * block_w + c) * 4;
                    data[y * width + x] = f32::from_ne_bytes([raw[s], raw[s + 1], raw[s + 2], raw[s + 3]]);
                }
            }
        }
    }

    let tie = ifd.doubles(T_TIEPOINT, "ModelTiepoint")?;
    let scale = ifd.doubles(T_PIXEL_SCALE, "ModelPixelScale")?;
    if tie.len() < 6 || scale.len() < 2 {
        return Err(Glo30Error::Corrupt("ModelTiepoint or ModelPixelScale too short".into()));
    }
    // The tiepoint ties raster (i, j) to model (x, y); move to pixel (0, 0).
    let geo =
        GeoRef { origin_lon: tie[3] - tie[0] * scale[0], origin_lat: tie[4] + tie[1] * scale[1], scale_lon: scale[0], scale_lat: scale[1] };

    let nodata = match ifd.entry(T_NODATA) {
        None => None,
        Some(e) => {
            let s = String::from_utf8_lossy(ifd.bytes(e, 1)?).trim_end_matches('\0').trim().to_string();
            Some(s.parse::<f32>().map_err(|_| refuse("GDAL_NODATA", T_NODATA, format!("{s:?}")))?)
        }
    };
    if let Some(nd) = nodata {
        for v in &mut data {
            if v.to_bits() == nd.to_bits() {
                *v = f32::NAN;
            }
        }
    }
    Ok(Tile { width, height, data, geo, nodata })
}

/// TIFF Technical Note 3: undo the horizontal byte differencing over the row, then gather the bytes
/// from the planes (most significant byte plane first) into big-endian samples, stored here native-endian.
fn undo_float_predictor(row: &mut [u8], width: usize) {
    for i in 1..row.len() {
        row[i] = row[i].wrapping_add(row[i - 1]);
    }
    let planes = row.to_vec();
    for x in 0..width {
        let be = [planes[x], planes[width + x], planes[2 * width + x], planes[3 * width + x]];
        row[x * 4..x * 4 + 4].copy_from_slice(&f32::from_be_bytes(be).to_ne_bytes());
    }
}
