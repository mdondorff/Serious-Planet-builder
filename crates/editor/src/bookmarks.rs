//! Saved camera bookmarks, live coordinate readout, and `LAT,LON[,ALT]` jump parsing (VIEW-073).
//! GPU-free, float64 throughout, `libm` for transcendentals (CON-02, CON-03, CON-14, COORD-009).

use crate::params::{format_metres, parse_value, Unit};
use crate::shape::{lat_lon_of, tangent_frame};
use planet_core::{PlanetFixed, Vec3};
use planet_frame::freecam::{FreePose, FREE_EYE_CLEARANCE_M};
use planet_frame::Camera;
use std::path::Path;

/// Format version written into `bookmarks.toml`.
pub const FORMAT_VERSION: u32 = 1;
/// Longest allowed bookmark label in Unicode scalar values.
pub const MAX_NAME_CHARS: usize = 80;
/// Least camera clearance above terrain (`FREE_EYE_CLEARANCE_M`, VIEW-004).
pub const MIN_ALTITUDE_M: f64 = FREE_EYE_CLEARANCE_M;
/// Greatest camera altitude accepted by bookmarks and coordinate jumps.
pub const MAX_ALTITUDE_M: f64 = 1.0e8;
/// Largest pitch magnitude above or below the horizon, degrees (VIEW-004).
pub const MAX_PITCH_DEG: f64 = 89.0;
/// Default altitude above ground when jumping to `LAT,LON` without an explicit altitude.
pub const DEFAULT_JUMP_ALTITUDE_M: f64 = 1_500.0;
/// Default downward pitch in degrees when jumping to `LAT,LON` (negative pitches down).
pub const DEFAULT_JUMP_PITCH_DEG: f64 = -35.0;

fn clean_zero(v: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else {
        v
    }
}

fn normalize_lon_deg(mut lon_deg: f64) -> f64 {
    while lon_deg <= -180.0 {
        lon_deg += 360.0;
    }
    while lon_deg > 180.0 {
        lon_deg -= 360.0;
    }
    clean_zero(lon_deg)
}

fn normalize_lat_lon(lat_deg: f64, lon_deg: f64) -> (f64, f64) {
    let lat = lat_deg.clamp(-90.0, 90.0);
    if lat.abs() >= 90.0 - 1e-9 {
        return (if lat > 0.0 { 90.0 } else { -90.0 }, 0.0);
    }
    (clean_zero(lat), normalize_lon_deg(lon_deg))
}

fn normalize_yaw_deg(mut yaw_deg: f64) -> f64 {
    yaw_deg = yaw_deg.rem_euclid(360.0);
    if yaw_deg >= 360.0 || yaw_deg == 0.0 {
        0.0
    } else {
        yaw_deg
    }
}

/// Formats an unsigned geographic coordinate component in degrees with a cardinal suffix (`N`/`S` or `E`/`W`).
pub fn format_deg(v: f64, pos: char, neg: char) -> String {
    let text = format!("{:.5}", v.abs());
    let side = if v < 0.0 && text.chars().any(|c| matches!(c, '1'..='9')) { neg } else { pos };
    format!("{text}° {side}")
}

fn pose_from_geo(lat_deg: f64, lon_deg: f64, altitude_m: f64, yaw_deg: f64, pitch_deg: f64, radius_m: f64, ground_m: f64) -> FreePose {
    let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
    let (north, east) = tangent_frame(dir);
    let yaw = yaw_deg.to_radians();
    let heading = (north * libm::cos(yaw) + east * libm::sin(yaw)).normalized();
    let g = if ground_m.is_finite() { ground_m } else { 0.0 };
    let alt = if altitude_m.is_finite() { altitude_m.clamp(MIN_ALTITUDE_M, MAX_ALTITUDE_M) } else { MIN_ALTITUDE_M };
    let r = (radius_m + g + alt).max(1.0);
    let pitch = pitch_deg.clamp(-MAX_PITCH_DEG, MAX_PITCH_DEG).to_radians();
    FreePose::new(PlanetFixed(dir * r), heading, pitch)
}

/// Live camera position and orientation in geographic coordinates and local compass bearing (VIEW-073).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraReadout {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub altitude_m: f64,
    /// Compass bearing in `[0.0, 360.0)` clockwise from local north.
    pub yaw_deg: f64,
    /// Pitch in degrees in `[-89.0, 89.0]`, positive above the horizon.
    pub pitch_deg: f64,
}

impl CameraReadout {
    /// Derives a geographic readout from a roll-free [`FreePose`] and ground clearance `altitude_m`.
    pub fn from_pose(pose: &FreePose, altitude_m: f64) -> CameraReadout {
        let up = pose.up();
        let (lat_deg, lon_deg) = lat_lon_of(up);
        let (north, east) = tangent_frame(up);
        let yaw_deg = normalize_yaw_deg(libm::atan2(pose.heading.dot(east), pose.heading.dot(north)).to_degrees());
        let pitch_deg = clean_zero(pose.pitch.to_degrees().clamp(-MAX_PITCH_DEG, MAX_PITCH_DEG));
        let alt = if altitude_m.is_finite() { altitude_m.clamp(MIN_ALTITUDE_M, MAX_ALTITUDE_M) } else { MIN_ALTITUDE_M };
        CameraReadout { lat_deg: clean_zero(lat_deg), lon_deg: clean_zero(lon_deg), altitude_m: alt, yaw_deg, pitch_deg }
    }

    /// Derives a geographic readout from a [`Camera`] and ground clearance `altitude_m`.
    pub fn from_camera(camera: &Camera, altitude_m: f64) -> CameraReadout {
        Self::from_pose(&FreePose::from_camera(camera), altitude_m)
    }

    /// Coordinates formatted for CLI `--site LAT,LON`.
    pub fn site_arg(&self) -> String {
        format!("{:.5},{:.5}", clean_zero(self.lat_deg), clean_zero(self.lon_deg))
    }

    /// Ready-to-copy CLI flag `--site LAT,LON`.
    pub fn site_flag(&self) -> String {
        format!("--site {}", self.site_arg())
    }

    /// One-line summary suitable for status labels and panels.
    pub fn summary(&self) -> String {
        format!(
            "{}, {} · alt {} · hdg {:05.1}° · pitch {:+.1}°",
            format_deg(self.lat_deg, 'N', 'S'),
            format_deg(self.lon_deg, 'E', 'W'),
            format_metres(self.altitude_m),
            self.yaw_deg,
            self.pitch_deg,
        )
    }
}

/// A named site bookmark stored independently of any [`planet_world_def::WorldDefinition`] (VIEW-073).
#[derive(Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub id: u64,
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub altitude_m: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub notes: Option<String>,
}

impl Bookmark {
    /// Checks all fields against valid geographic, altitude, orientation, and name bounds.
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0 {
            return Err("bookmark id must be >= 1".to_string());
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err("bookmark name cannot be empty".to_string());
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(format!("bookmark name exceeds {MAX_NAME_CHARS} characters"));
        }
        if !self.lat_deg.is_finite() || !(-90.0..=90.0).contains(&self.lat_deg) {
            return Err(format!("latitude {} is outside -90..90 degrees", self.lat_deg));
        }
        if !self.lon_deg.is_finite() || !(-180.0..=180.0).contains(&self.lon_deg) {
            return Err(format!("longitude {} is outside -180..180 degrees", self.lon_deg));
        }
        if !self.altitude_m.is_finite() || !(MIN_ALTITUDE_M..=MAX_ALTITUDE_M).contains(&self.altitude_m) {
            return Err(format!("altitude {} m is outside {MIN_ALTITUDE_M}..{MAX_ALTITUDE_M} m", self.altitude_m));
        }
        if !self.yaw_deg.is_finite() || !(0.0..=360.0).contains(&self.yaw_deg) {
            return Err(format!("yaw {} is outside 0..360 degrees", self.yaw_deg));
        }
        if !self.pitch_deg.is_finite() || !(-MAX_PITCH_DEG..=MAX_PITCH_DEG).contains(&self.pitch_deg) {
            return Err(format!("pitch {} is outside -{MAX_PITCH_DEG}..{MAX_PITCH_DEG} degrees", self.pitch_deg));
        }
        Ok(())
    }

    /// Canonical form with normalized longitude, poles (`lon = 0`), yaw in `[0, 360)`, trimmed name, and `-0.0 -> 0.0`.
    pub fn canonical(mut self) -> Bookmark {
        let (lat, lon) = normalize_lat_lon(self.lat_deg, self.lon_deg);
        self.lat_deg = lat;
        self.lon_deg = lon;
        self.yaw_deg = normalize_yaw_deg(self.yaw_deg);
        self.pitch_deg = clean_zero(self.pitch_deg.clamp(-MAX_PITCH_DEG, MAX_PITCH_DEG));
        self.altitude_m = clean_zero(self.altitude_m);
        self.name = self.name.trim().to_string();
        self.notes = self.notes.and_then(|n| {
            let t = n.trim();
            (!t.is_empty()).then(|| t.to_string())
        });
        self
    }

    /// Unit sphere direction of the bookmark in `PlanetFixed`.
    pub fn direction(&self) -> Vec3 {
        Vec3::from_lat_lon(self.lat_deg.to_radians(), self.lon_deg.to_radians())
    }

    /// Builds a free-camera pose at this bookmark's altitude above terrain height `ground_m`.
    pub fn pose(&self, radius_m: f64, ground_m: f64) -> FreePose {
        pose_from_geo(self.lat_deg, self.lon_deg, self.altitude_m, self.yaw_deg, self.pitch_deg, radius_m, ground_m)
    }

    /// Chord distance in metres from `camera_pos` (`PlanetFixed`) to the bookmark's surface point.
    pub fn distance_m(&self, camera_pos: Vec3, radius_m: f64) -> f64 {
        (self.direction() * radius_m - camera_pos).length()
    }

    /// Coordinates formatted for `--site LAT,LON`.
    pub fn site_arg(&self) -> String {
        format!("{:.5},{:.5}", clean_zero(self.lat_deg), clean_zero(self.lon_deg))
    }

    /// One-line readout summary of coordinates, altitude, bearing, and pitch.
    pub fn summary(&self) -> String {
        CameraReadout {
            lat_deg: self.lat_deg,
            lon_deg: self.lon_deg,
            altitude_m: self.altitude_m,
            yaw_deg: normalize_yaw_deg(self.yaw_deg),
            pitch_deg: self.pitch_deg,
        }
        .summary()
    }
}

/// Parsed target of a `LAT,LON[,ALT]` jump request (VIEW-073).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpTarget {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub altitude_m: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
}

impl JumpTarget {
    /// Builds a free-camera pose above `ground_m` at the target's coordinates and altitude.
    pub fn pose(&self, radius_m: f64, ground_m: f64) -> FreePose {
        pose_from_geo(self.lat_deg, self.lon_deg, self.altitude_m, self.yaw_deg, self.pitch_deg, radius_m, ground_m)
    }

    /// `--site LAT,LON` spelling of this target.
    pub fn site_arg(&self) -> String {
        format!("{:.5},{:.5}", clean_zero(self.lat_deg), clean_zero(self.lon_deg))
    }
}

fn parse_coord(raw: &str, axis_name: &str, pos_hemi: char, neg_hemi: char, min_deg: f64, max_deg: f64) -> Result<f64, String> {
    let mut s = raw.trim();
    if s.is_empty() {
        return Err(format!("{axis_name} is empty"));
    }
    let mut sign = 1.0;
    let mut has_hemi = false;
    if let Some(last) = s.chars().last().filter(|c| c.is_ascii_alphabetic()) {
        let up = last.to_ascii_uppercase();
        if up == pos_hemi {
            sign = 1.0;
        } else if up == neg_hemi {
            sign = -1.0;
        } else {
            return Err(format!("invalid hemisphere '{last}' for {axis_name} '{raw}' (expected {pos_hemi} or {neg_hemi})"));
        }
        has_hemi = true;
        s = s[..s.len() - last.len_utf8()].trim();
    }
    if let Some(stripped) = s.strip_suffix('°') {
        s = stripped.trim();
    }
    if s.is_empty() {
        return Err(format!("{axis_name} '{raw}' has no number"));
    }
    if has_hemi && (s.starts_with('+') || s.starts_with('-')) {
        return Err(format!("{axis_name} '{raw}' mixes a sign with a hemisphere letter"));
    }
    let num: f64 = s.parse().map_err(|_| format!("'{raw}' is not a valid {axis_name} number"))?;
    if !num.is_finite() {
        return Err(format!("{axis_name} '{raw}' is not finite"));
    }
    let val = num * sign;
    if !(min_deg..=max_deg).contains(&val) {
        return Err(format!("{axis_name} {val} is outside {min_deg}..{max_deg} degrees"));
    }
    Ok(clean_zero(val))
}

fn strip_cli_prefix(input: &str) -> &str {
    let t = input.trim();
    for flag in ["--site", "--marker"] {
        if let Some(rest) = t.strip_prefix(flag) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return rest.trim();
            }
        }
    }
    t
}

/// Parses `LAT,LON` or `LAT,LON,ALT` (with optional `--site` / `--marker` prefix, `°`, and `N/S/E/W` suffixes) into a [`JumpTarget`] (VIEW-073).
pub fn parse_jump_target(input: &str) -> Result<JumpTarget, String> {
    let body = strip_cli_prefix(input);
    if body.is_empty() {
        return Err("enter coordinates as LAT, LON[, ALT]".to_string());
    }
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    if parts.len() != 2 && parts.len() != 3 {
        return Err("expected 'LAT, LON' or 'LAT, LON, ALT'".to_string());
    }
    let raw_lat = parse_coord(parts[0], "latitude", 'N', 'S', -90.0, 90.0)?;
    let raw_lon = parse_coord(parts[1], "longitude", 'E', 'W', -360.0, 360.0)?;
    let (lat_deg, lon_deg) = normalize_lat_lon(raw_lat, raw_lon);
    let altitude_m = match parts.get(2) {
        None => DEFAULT_JUMP_ALTITUDE_M,
        Some(alt) => {
            let m = parse_value(alt, Unit::Metres)?;
            if !(MIN_ALTITUDE_M..=MAX_ALTITUDE_M).contains(&m) {
                return Err(format!("altitude {m} m is outside {MIN_ALTITUDE_M}..{MAX_ALTITUDE_M} m"));
            }
            clean_zero(m)
        }
    };
    Ok(JumpTarget { lat_deg, lon_deg, altitude_m, yaw_deg: 0.0, pitch_deg: DEFAULT_JUMP_PITCH_DEG })
}

const SHELF_TOP_KEYS: [&str; 2] = ["version", "bookmark"];
const BOOKMARK_KEYS: [&str; 8] = ["id", "name", "lat_deg", "lon_deg", "altitude_m", "yaw_deg", "pitch_deg", "notes"];

/// Collection of saved site bookmarks with deterministic TOML serialization and atomic file saves (VIEW-073).
#[derive(Clone, Debug, PartialEq)]
pub struct BookmarkShelf {
    entries: Vec<Bookmark>,
    next_id: u64,
}

impl Default for BookmarkShelf {
    fn default() -> Self {
        Self::new()
    }
}

impl BookmarkShelf {
    /// Creates an empty bookmark shelf (`next_id = 1`).
    pub fn new() -> BookmarkShelf {
        BookmarkShelf { entries: Vec::new(), next_id: 1 }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn list(&self) -> &[Bookmark] {
        &self.entries
    }

    pub fn get(&self, id: u64) -> Option<&Bookmark> {
        self.entries.iter().find(|b| b.id == id)
    }

    fn alloc_id(&mut self) -> u64 {
        let mut id = self.next_id.max(1);
        while self.entries.iter().any(|b| b.id == id) {
            id += 1;
        }
        self.next_id = id + 1;
        id
    }

    /// Saves a camera pose and ground clearance as a new bookmark; blank names become `"Site <id>"`.
    pub fn add_from_pose(&mut self, name: &str, pose: &FreePose, altitude_m: f64, notes: Option<String>) -> Result<u64, String> {
        let id = self.alloc_id();
        let label = match name.trim() {
            "" => format!("Site {id}"),
            n => n.to_string(),
        };
        let r = CameraReadout::from_pose(pose, altitude_m);
        let b = Bookmark {
            id,
            name: label,
            lat_deg: r.lat_deg,
            lon_deg: r.lon_deg,
            altitude_m: r.altitude_m,
            yaw_deg: r.yaw_deg,
            pitch_deg: r.pitch_deg,
            notes,
        }
        .canonical();
        b.validate()?;
        self.entries.push(b);
        Ok(id)
    }

    /// Inserts a bookmark, allocating a fresh id when `bookmark.id == 0` or already taken.
    pub fn insert(&mut self, mut bookmark: Bookmark) -> Result<u64, String> {
        if bookmark.id == 0 || self.entries.iter().any(|b| b.id == bookmark.id) {
            bookmark.id = self.alloc_id();
        } else {
            self.next_id = self.next_id.max(bookmark.id + 1);
        }
        let b = bookmark.canonical();
        b.validate()?;
        let id = b.id;
        self.entries.push(b);
        Ok(id)
    }

    /// Renames bookmark `id`, refusing blank or over-long names.
    pub fn rename(&mut self, id: u64, name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("bookmark name cannot be empty".to_string());
        }
        if trimmed.chars().count() > MAX_NAME_CHARS {
            return Err(format!("bookmark name exceeds {MAX_NAME_CHARS} characters"));
        }
        let Some(entry) = self.entries.iter_mut().find(|b| b.id == id) else {
            return Err(format!("bookmark {id} was not found"));
        };
        entry.name = trimmed.to_string();
        Ok(())
    }

    /// Deletes bookmark `id`, returning true when an entry was removed.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|b| b.id != id);
        self.entries.len() != before
    }

    /// Deterministic canonical TOML representation (`version = 1` + `[[bookmark]]` tables).
    pub fn to_toml(&self) -> String {
        let mut s = format!("version = {FORMAT_VERSION}\n");
        let fmt_float = |v: f64| {
            let v = clean_zero(v);
            let t = format!("{v:?}");
            if t.contains('.') || t.contains('e') || t.contains('E') {
                t
            } else {
                format!("{t}.0")
            }
        };
        let toml_quote = |raw: &str| {
            let mut out = String::with_capacity(raw.len() + 2);
            out.push('"');
            for c in raw.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        };
        for b in &self.entries {
            let b = b.clone().canonical();
            s.push_str("\n[[bookmark]]\n");
            s.push_str(&format!("id = {}\n", b.id));
            s.push_str(&format!("name = {}\n", toml_quote(&b.name)));
            s.push_str(&format!("lat_deg = {}\n", fmt_float(b.lat_deg)));
            s.push_str(&format!("lon_deg = {}\n", fmt_float(b.lon_deg)));
            s.push_str(&format!("altitude_m = {}\n", fmt_float(b.altitude_m)));
            s.push_str(&format!("yaw_deg = {}\n", fmt_float(b.yaw_deg)));
            s.push_str(&format!("pitch_deg = {}\n", fmt_float(b.pitch_deg)));
            if let Some(notes) = &b.notes {
                s.push_str(&format!("notes = {}\n", toml_quote(notes)));
            }
        }
        s
    }

    /// Parses a `bookmarks.toml` text, refusing unknown keys, bad versions, duplicate IDs, and out-of-range fields.
    pub fn from_toml(text: &str) -> Result<BookmarkShelf, String> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| format!("bookmarks TOML: {e}"))?;
        if let Some(k) = table.keys().find(|k| !SHELF_TOP_KEYS.contains(&k.as_str())) {
            return Err(format!("bookmarks: unknown key '{k}'"));
        }
        let version =
            table.get("version").and_then(|v| v.as_integer()).ok_or_else(|| "bookmarks: missing integer `version`".to_string())?;
        if version != i64::from(FORMAT_VERSION) {
            return Err(format!("bookmarks: unsupported version {version} (expected {FORMAT_VERSION})"));
        }
        let mut shelf = BookmarkShelf::new();
        let Some(items_val) = table.get("bookmark") else {
            return Ok(shelf);
        };
        let items = items_val.as_array().ok_or_else(|| "bookmarks: `bookmark` must be an array of tables".to_string())?;
        for (idx, val) in items.iter().enumerate() {
            let row = val.as_table().ok_or_else(|| format!("bookmark[{idx}]: expected a table"))?;
            if let Some(k) = row.keys().find(|k| !BOOKMARK_KEYS.contains(&k.as_str())) {
                return Err(format!("bookmark[{idx}]: unknown key '{k}'"));
            }
            let req_num = |key: &str| -> Result<f64, String> {
                let v = row.get(key).ok_or_else(|| format!("bookmark[{idx}]: missing `{key}`"))?;
                v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).ok_or_else(|| format!("bookmark[{idx}].{key}: expected a number"))
            };
            let id_raw = row.get("id").and_then(|v| v.as_integer()).ok_or_else(|| format!("bookmark[{idx}]: missing integer `id`"))?;
            let id = u64::try_from(id_raw).map_err(|_| format!("bookmark[{idx}].id: {id_raw} is negative"))?;
            if shelf.entries.iter().any(|e| e.id == id) {
                return Err(format!("bookmark[{idx}].id: duplicate bookmark id {id}"));
            }
            let name =
                row.get("name").and_then(|v| v.as_str()).ok_or_else(|| format!("bookmark[{idx}]: missing string `name`"))?.to_string();
            let notes = match row.get("notes") {
                None => None,
                Some(v) => Some(v.as_str().ok_or_else(|| format!("bookmark[{idx}].notes: expected a string"))?.to_string()),
            };
            let b = Bookmark {
                id,
                name,
                lat_deg: req_num("lat_deg")?,
                lon_deg: req_num("lon_deg")?,
                altitude_m: req_num("altitude_m")?,
                yaw_deg: req_num("yaw_deg")?,
                pitch_deg: req_num("pitch_deg")?,
                notes,
            };
            b.validate().map_err(|e| format!("bookmark[{idx}]: {e}"))?;
            let canon = b.canonical();
            shelf.next_id = shelf.next_id.max(canon.id + 1);
            shelf.entries.push(canon);
        }
        Ok(shelf)
    }

    /// Loads bookmarks from `path`; returns an empty shelf when `path` does not exist yet.
    pub fn load_file(path: &Path) -> Result<BookmarkShelf, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BookmarkShelf::new()),
            Err(e) => return Err(format!("reading bookmarks {}: {e}", path.display())),
        };
        Self::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Atomically saves bookmarks to `path` via a dot-prefixed temporary file and rename (`CON-05`).
    pub fn save_file(&self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("bookmarks.toml");
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = parent.join(format!(".{fname}.{}.{n}.tmp", std::process::id()));
        let text = self.to_toml();
        let write = || -> Result<(), String> {
            let mut file = std::fs::File::create(&temp).map_err(|e| format!("creating {}: {e}", temp.display()))?;
            file.write_all(text.as_bytes()).map_err(|e| format!("writing {}: {e}", temp.display()))?;
            file.sync_all().map_err(|e| format!("flushing {}: {e}", temp.display()))?;
            std::fs::rename(&temp, path).map_err(|e| format!("moving {} into {}: {e}", temp.display(), path.display()))
        };
        let res = write();
        if res.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        res
    }
}
