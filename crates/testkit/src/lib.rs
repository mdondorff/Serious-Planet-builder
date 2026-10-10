//! Test support: RGBA8 images, PNG I/O, golden comparison and image metrics.
//!
//! Goldens live in `tests/goldens/<adapter>/<name>.png` and are promoted only by the owner
//! (`/bless`). A test that finds no golden writes a *candidate* to
//! `target/review/candidates/<adapter>/<name>.png` and reports `Pending`; it never creates a golden.

use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// An 8-bit RGBA image, row-major, no padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(rgba.len(), (width * height * 4) as usize, "RGBA buffer size does not match {width}x{height}");
        Self { width, height, rgba }
    }

    pub fn filled(width: u32, height: u32, px: [u8; 4]) -> Self {
        Self::new(width, height, px.iter().copied().cycle().take((width * height * 4) as usize).collect())
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, px: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&px);
    }

    pub fn pixels(&self) -> impl Iterator<Item = [u8; 4]> + '_ {
        self.rgba.as_chunks::<4>().0.iter().copied()
    }

    /// Number of pixels exactly equal to `px`.
    pub fn count(&self, px: [u8; 4]) -> usize {
        self.pixels().filter(|p| *p == px).count()
    }
}

pub fn write_png(path: &Path, img: &Image) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut enc = png::Encoder::new(BufWriter::new(File::create(path)?), img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    w.write_image_data(&img.rgba).map_err(std::io::Error::other)
}

pub fn read_png(path: &Path) -> std::io::Result<Image> {
    let mut dec = png::Decoder::new(BufReader::new(File::open(path)?));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(std::io::Error::other)?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| std::io::Error::other("PNG too large"))?];
    let info = reader.next_frame(&mut buf).map_err(std::io::Error::other)?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.as_chunks::<3>().0.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        other => return Err(std::io::Error::other(format!("unsupported PNG colour type {other:?} in {}", path.display()))),
    };
    Ok(Image::new(info.width, info.height, rgba))
}

/// Walk up from the current directory to the repository root (the folder holding `docs/constitution.md`).
pub fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("current dir");
    loop {
        if dir.join("docs").join("constitution.md").is_file() {
            return dir;
        }
        assert!(dir.pop(), "repository root (docs/constitution.md) not found above the current directory");
    }
}

/// Turn an adapter description into a directory-safe key, e.g. `dx12-microsoft-basic-render-driver`.
pub fn adapter_key(backend: &str, name: &str) -> String {
    let raw = format!("{backend}-{name}").to_lowercase();
    let mut key = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c);
        } else if !key.ends_with('-') {
            key.push('-');
        }
    }
    key.trim_matches('-').to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tolerance {
    /// Debug views: every channel of every pixel must match.
    Exact,
    /// Per-channel absolute difference allowed (lit views; reviewed by the owner before use).
    MaxChannelDelta(u8),
}

impl Tolerance {
    fn allowed(self) -> u8 {
        match self {
            Tolerance::Exact => 0,
            Tolerance::MaxChannelDelta(d) => d,
        }
    }
}

#[derive(Debug)]
pub struct Diff {
    pub differing_pixels: usize,
    pub max_channel_delta: u8,
    pub first_differing: Option<(u32, u32)>,
}

fn channel_delta(a: [u8; 4], b: [u8; 4]) -> u8 {
    a.iter().zip(b.iter()).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

pub fn diff(a: &Image, b: &Image, tol: Tolerance) -> Option<Diff> {
    assert_eq!((a.width, a.height), (b.width, b.height), "image sizes differ");
    let (mut n, mut max, mut first) = (0usize, 0u8, None);
    for (i, (pa, pb)) in a.pixels().zip(b.pixels()).enumerate() {
        let d = channel_delta(pa, pb);
        if d > tol.allowed() {
            n += 1;
            max = max.max(d);
            first.get_or_insert(((i as u32) % a.width, (i as u32) / a.width));
        }
    }
    (n > 0).then_some(Diff { differing_pixels: n, max_channel_delta: max, first_differing: first })
}

/// Diff visualisation: differing pixels red, matching pixels dimmed.
pub fn diff_image(a: &Image, b: &Image, tol: Tolerance) -> Image {
    let mut out = Image::filled(a.width, a.height, [0, 0, 0, 255]);
    for (i, (pa, pb)) in a.pixels().zip(b.pixels()).enumerate() {
        let px = if channel_delta(pa, pb) > tol.allowed() { [255, 0, 0, 255] } else { [pa[0] / 4, pa[1] / 4, pa[2] / 4, 255] };
        out.set_pixel((i as u32) % a.width, (i as u32) / a.width, px);
    }
    out
}

#[derive(Debug)]
pub enum GoldenOutcome {
    Match,
    /// No golden exists. A candidate was written; the owner must bless it.
    Pending {
        candidate: PathBuf,
    },
    Mismatch {
        diff: Diff,
        golden: PathBuf,
        candidate: PathBuf,
        diff_image: Option<PathBuf>,
    },
}

impl fmt::Display for GoldenOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoldenOutcome::Match => write!(f, "golden match"),
            GoldenOutcome::Pending { candidate } => {
                write!(f, "PENDING BLESS: no golden yet; candidate written to {}", candidate.display())
            }
            GoldenOutcome::Mismatch { diff, golden, candidate, diff_image } => {
                write!(
                    f,
                    "golden MISMATCH: {} pixels differ (max channel delta {}, first at {:?}); golden {}, candidate {}",
                    diff.differing_pixels,
                    diff.max_channel_delta,
                    diff.first_differing,
                    golden.display(),
                    candidate.display()
                )?;
                if let Some(d) = diff_image {
                    write!(f, ", diff {}", d.display())?;
                }
                Ok(())
            }
        }
    }
}

/// Compare `img` with `tests/goldens/<adapter>/<name>.png` under `root`, writing review artefacts to
/// `root/target/review/`. Pure file logic, so it is testable without a GPU.
pub fn compare_golden_in(root: &Path, adapter: &str, name: &str, img: &Image, tol: Tolerance) -> std::io::Result<GoldenOutcome> {
    let golden = root.join("tests").join("goldens").join(adapter).join(format!("{name}.png"));
    let review = root.join("target").join("review");
    let candidate = review.join("candidates").join(adapter).join(format!("{name}.png"));
    if !golden.is_file() {
        write_png(&candidate, img)?;
        return Ok(GoldenOutcome::Pending { candidate });
    }
    let expected = read_png(&golden)?;
    if (expected.width, expected.height) != (img.width, img.height) {
        write_png(&candidate, img)?;
        let all = (img.width * img.height) as usize;
        return Ok(GoldenOutcome::Mismatch {
            diff: Diff { differing_pixels: all, max_channel_delta: 255, first_differing: Some((0, 0)) },
            golden,
            candidate,
            diff_image: None,
        });
    }
    match diff(&expected, img, tol) {
        None => Ok(GoldenOutcome::Match),
        Some(d) => {
            write_png(&candidate, img)?;
            let diff_path = review.join("diffs").join(adapter).join(format!("{name}.png"));
            write_png(&diff_path, &diff_image(&expected, img, tol))?;
            Ok(GoldenOutcome::Mismatch { diff: d, golden, candidate, diff_image: Some(diff_path) })
        }
    }
}

/// True when the owner has blessed at least one golden for this adapter.
pub fn adapter_has_goldens(root: &Path, adapter: &str) -> bool {
    std::fs::read_dir(root.join("tests").join("goldens").join(adapter))
        .map(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x == "png")))
        .unwrap_or(false)
}

/// Test-facing wrapper: panics with an actionable message on mismatch. A missing golden is reported loudly and
/// fails when `PLANET_GOLDEN_STRICT=1` or when the adapter already has blessed goldens (so a deleted or
/// forgotten golden of a blessed adapter cannot pass silently); an adapter with none yet only reports pending.
pub fn assert_golden(adapter: &str, name: &str, img: &Image, tol: Tolerance) {
    let outcome = compare_golden_in(&repo_root(), adapter, name, img, tol).expect("golden I/O");
    match &outcome {
        GoldenOutcome::Match => {}
        GoldenOutcome::Pending { .. } => {
            eprintln!("{name} [{adapter}]: {outcome}");
            let root = repo_root();
            if std::env::var("PLANET_GOLDEN_STRICT").as_deref() == Ok("1") || adapter_has_goldens(&root, adapter) {
                panic!("{name} [{adapter}]: {outcome}");
            }
        }
        GoldenOutcome::Mismatch { .. } => panic!("{name} [{adapter}]: {outcome}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("planet-testkit-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    // spec: TEST-002
    #[test]
    fn missing_golden_is_pending_and_never_creates_a_golden() {
        let root = tmp("pending");
        let img = Image::filled(4, 4, [1, 2, 3, 255]);
        let out = compare_golden_in(&root, "a", "t", &img, Tolerance::Exact).unwrap();
        let GoldenOutcome::Pending { candidate } = out else { panic!("expected Pending, got {out:?}") };
        assert!(candidate.is_file());
        assert!(!root.join("tests/goldens/a/t.png").exists(), "a golden must never be created by a test");
    }

    // spec: TEST-002, TEST-007
    #[test]
    fn mismatch_reports_numbers_and_writes_diff_image() {
        let root = tmp("match");
        let img = Image::filled(4, 4, [10, 20, 30, 255]);
        write_png(&root.join("tests/goldens/a/t.png"), &img).unwrap();
        assert!(matches!(compare_golden_in(&root, "a", "t", &img, Tolerance::Exact).unwrap(), GoldenOutcome::Match));

        let mut bad = img.clone();
        bad.set_pixel(2, 1, [10, 20, 35, 255]);
        let GoldenOutcome::Mismatch { diff, diff_image, candidate, .. } =
            compare_golden_in(&root, "a", "t", &bad, Tolerance::Exact).unwrap()
        else {
            panic!("expected Mismatch")
        };
        assert_eq!(diff.differing_pixels, 1);
        assert_eq!(diff.max_channel_delta, 5);
        assert_eq!(diff.first_differing, Some((2, 1)));
        assert!(diff_image.unwrap().is_file() && candidate.is_file());
        // The same change is inside a delta-5 tolerance.
        assert!(matches!(compare_golden_in(&root, "a", "t", &bad, Tolerance::MaxChannelDelta(5)).unwrap(), GoldenOutcome::Match));
    }

    // spec: TEST-002
    #[test]
    fn adapters_with_blessed_goldens_are_detected() {
        let root = tmp("blessed");
        assert!(!adapter_has_goldens(&root, "a"));
        write_png(&root.join("tests/goldens/a/t.png"), &Image::filled(1, 1, [0, 0, 0, 255])).unwrap();
        assert!(adapter_has_goldens(&root, "a"));
        assert!(!adapter_has_goldens(&root, "b"));
    }

    // spec: TEST-003
    #[test]
    fn counts_hole_pixels() {
        let mut img = Image::filled(8, 8, [0, 0, 0, 255]);
        img.set_pixel(3, 3, [255, 0, 255, 255]);
        img.set_pixel(4, 3, [255, 0, 255, 255]);
        assert_eq!(img.count([255, 0, 255, 255]), 2);
    }

    // spec: TEST-002
    #[test]
    fn png_round_trip_is_lossless() {
        let root = tmp("png");
        let mut img = Image::filled(3, 2, [9, 8, 7, 255]);
        img.set_pixel(1, 1, [200, 100, 50, 128]);
        write_png(&root.join("x.png"), &img).unwrap();
        assert_eq!(read_png(&root.join("x.png")).unwrap(), img);
    }

    // spec: TEST-002
    #[test]
    fn adapter_keys_are_directory_safe() {
        assert_eq!(adapter_key("Dx12", "Microsoft Basic Render Driver"), "dx12-microsoft-basic-render-driver");
        assert_eq!(adapter_key("Vulkan", "llvmpipe (LLVM 15.0.7, 256 bits)"), "vulkan-llvmpipe-llvm-15-0-7-256-bits");
    }
}
