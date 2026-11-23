//! Scanner and inspector for F9 repro bundles and rolling camera paths in `PLANET_REPRO_DIR` (VIEW-074).
//! GPU-free, read-only over bundle files, with isolate-on-error semantics for corrupt bundles.

use crate::bookmarks::{Bookmark, CameraReadout, MAX_ALTITUDE_M, MIN_ALTITUDE_M};
use planet_core::Vec3;
use planet_frame::freecam::FreePose;
use planet_frame::{Camera, ReproBundle, TerrainBundle};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Most `.bundle` files scanned in a single pass, newest first.
pub const MAX_REPRO_ENTRIES: usize = 200;

/// Bundle header flavour recognised by [`ReproShelf`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleKind {
    TerrainV1,
    TerrainV2,
    FrameV1,
}

/// How a captured bundle relates to the currently loaded world definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReproCompatibility {
    MatchesWorld,
    DifferentDefinition,
    DifferentSeed,
}

/// A `.bundle` file whose contents failed to parse or validate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokenBundle {
    pub path: PathBuf,
    pub error: String,
}

/// Metadata and camera pose of a captured `.bundle` file and its sibling F9 artifacts (`-path.txt`, `.png`) (VIEW-074).
#[derive(Clone, Debug, PartialEq)]
pub struct ReproEntry {
    pub stem: String,
    pub frame_number: Option<u64>,
    pub kind: BundleKind,
    pub bundle_path: PathBuf,
    pub path_file: Option<PathBuf>,
    pub path_samples: Option<usize>,
    pub png_path: Option<PathBuf>,
    pub seed: u64,
    pub definition_hash: u64,
    pub commit: Option<String>,
    pub open_commit: Option<String>,
    pub open_site: Option<String>,
    pub view: String,
    pub camera: Camera,
    pub radius_m: f64,
    pub readout: CameraReadout,
}

fn quote_cli_path(path: &Path) -> String {
    let clean = path.to_string_lossy().replace('\\', "/");
    if clean.chars().any(char::is_whitespace) {
        format!("\"{clean}\"")
    } else {
        clean
    }
}

impl ReproEntry {
    /// Exact roll-free `FreePose` matching the bundle's recorded `PlanetFixed` camera.
    pub fn free_pose(&self) -> FreePose {
        FreePose::from_camera(&self.camera)
    }

    /// Converts this capture into a [`Bookmark`] at altitude above terrain height `ground_m`.
    pub fn to_bookmark(&self, id: u64, ground_m: f64) -> Bookmark {
        let pose = self.free_pose();
        let g = if ground_m.is_finite() { ground_m } else { 0.0 };
        let alt = (pose.position.0.length() - (self.radius_m + g)).clamp(MIN_ALTITUDE_M, MAX_ALTITUDE_M);
        let readout = CameraReadout::from_pose(&pose, alt);
        Bookmark {
            id,
            name: self.stem.clone(),
            lat_deg: readout.lat_deg,
            lon_deg: readout.lon_deg,
            altitude_m: readout.altitude_m,
            yaw_deg: readout.yaw_deg,
            pitch_deg: readout.pitch_deg,
            notes: Some(format!("From {}.bundle (seed {})", self.stem, self.seed)),
        }
        .canonical()
    }

    /// Compares this capture's seed and `definition_hash` against an active world.
    pub fn compatibility(&self, world_seed: u64, world_def_hash: u64) -> ReproCompatibility {
        if self.seed != world_seed {
            ReproCompatibility::DifferentSeed
        } else if self.definition_hash != world_def_hash {
            ReproCompatibility::DifferentDefinition
        } else {
            ReproCompatibility::MatchesWorld
        }
    }

    /// Ready-to-run `cargo xtask repro ...` command matching `xtask::testcmd` / `BUILD-054`.
    pub fn replay_command(&self) -> String {
        if let Some(pf) = &self.path_file {
            if self.open_commit.is_some() {
                format!("cargo xtask repro {} --window", quote_cli_path(pf))
            } else {
                format!("cargo xtask repro {}", quote_cli_path(pf))
            }
        } else {
            format!("cargo xtask repro {}", quote_cli_path(&self.bundle_path))
        }
    }

    /// Concise summary line for display in editor lists.
    pub fn summary(&self) -> String {
        format!("seed {} · {} · {}", self.seed, self.view, self.readout.summary())
    }
}

/// Directory snapshot of captured F9 bundles (`entries` newest first, corrupt files isolated in `broken`) (VIEW-074).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReproShelf {
    pub dir: PathBuf,
    pub entries: Vec<ReproEntry>,
    pub broken: Vec<BrokenBundle>,
    pub truncated: usize,
}

fn read_path_samples(path: &Path) -> Option<usize> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().take(8).map_while(Result::ok) {
        if let Some(rest) = line.strip_prefix("samples ") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn parse_entry(dir: &Path, stem: String, frame_number: Option<u64>, bundle_path: &Path) -> Result<ReproEntry, String> {
    let text = std::fs::read_to_string(bundle_path).map_err(|e| format!("reading {}: {e}", bundle_path.display()))?;
    let first_line = text.lines().next().unwrap_or("");
    let (kind, seed, definition_hash, commit, open_commit, open_site, view, camera, radius_m) =
        if first_line.starts_with("planet-terrain-repro ") {
            let b = TerrainBundle::from_text(&text)?;
            let kind = if first_line == "planet-terrain-repro 1" { BundleKind::TerrainV1 } else { BundleKind::TerrainV2 };
            let cam = b.request.camera;
            if !cam.position.0.length().is_finite()
                || cam.position.0.length() <= 0.0
                || !cam.forward.length().is_finite()
                || cam.forward.length() <= 0.0
            {
                return Err("bundle camera vector is zero or non-finite".to_string());
            }
            let (open_commit, open_site) = b.world.map_or((None, None), |w| (w.open_commit, w.open_site));
            (
                kind,
                b.request.seed,
                b.definition_hash,
                b.commit,
                open_commit,
                open_site,
                b.request.view.name().to_string(),
                cam,
                b.request.lod.radius_m,
            )
        } else if first_line == "planet-repro 1" {
            let b = ReproBundle::from_text(&text)?;
            if !b.request.camera.0.length().is_finite() || b.request.camera.0.length() <= 0.0 {
                return Err("bundle camera position is zero or non-finite".to_string());
            }
            let cam = Camera::looking_at_centre(b.request.camera, Vec3::new(0.0, 0.0, 1.0), 1.0, 1.0, 0.1);
            (BundleKind::FrameV1, b.seed, b.definition_hash, None, None, None, b.request.view.name().to_string(), cam, b.request.radius_m)
        } else {
            return Err(format!("unrecognised bundle header '{first_line}'"));
        };

    let alt = (camera.position.0.length() - radius_m).clamp(MIN_ALTITUDE_M, MAX_ALTITUDE_M);
    let readout = CameraReadout::from_camera(&camera, alt);
    let path_candidate = dir.join(format!("{stem}-path.txt"));
    let (path_file, path_samples) = if path_candidate.is_file() {
        let samples = read_path_samples(&path_candidate);
        (Some(path_candidate), samples)
    } else {
        (None, None)
    };
    let png_candidate = dir.join(format!("{stem}.png"));
    let png_path = png_candidate.is_file().then_some(png_candidate);

    Ok(ReproEntry {
        stem,
        frame_number,
        kind,
        bundle_path: bundle_path.to_path_buf(),
        path_file,
        path_samples,
        png_path,
        seed,
        definition_hash,
        commit,
        open_commit,
        open_site,
        view,
        camera,
        radius_m,
        readout,
    })
}

impl ReproShelf {
    /// Scans `dir` for `.bundle` captures (up to [`MAX_REPRO_ENTRIES`], newest `frame-<n>` first).
    /// Returns an empty shelf when `dir` does not exist and isolates invalid files in `broken`.
    pub fn scan_dir(dir: &Path) -> Result<ReproShelf, String> {
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ReproShelf { dir: dir.to_path_buf(), entries: Vec::new(), broken: Vec::new(), truncated: 0 });
            }
            Err(e) => return Err(format!("listing repro directory {}: {e}", dir.display())),
        };

        let mut candidates: Vec<(Option<u64>, String, PathBuf)> = Vec::new();
        for item in read_dir {
            let entry = item.map_err(|e| format!("reading entry in {}: {e}", dir.display()))?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("bundle") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            let frame_number = stem.strip_prefix("frame-").and_then(|s| s.parse::<u64>().ok());
            candidates.push((frame_number, stem, path));
        }

        candidates.sort_by(|(an, a_stem, _), (bn, b_stem, _)| match (an, bn) {
            (Some(a), Some(b)) => b.cmp(a).then_with(|| b_stem.cmp(a_stem)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b_stem.cmp(a_stem),
        });

        let truncated = candidates.len().saturating_sub(MAX_REPRO_ENTRIES);
        candidates.truncate(MAX_REPRO_ENTRIES);

        let mut shelf = ReproShelf { dir: dir.to_path_buf(), entries: Vec::new(), broken: Vec::new(), truncated };
        for (frame_number, stem, path) in candidates {
            match parse_entry(dir, stem, frame_number, &path) {
                Ok(entry) => shelf.entries.push(entry),
                Err(error) => shelf.broken.push(BrokenBundle { path, error }),
            }
        }
        Ok(shelf)
    }
}
