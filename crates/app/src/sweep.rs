//! The item sweep of the editor window (`PLANET_SWEEP=<report file>`, an automated stand-in for the owner's retest): the real window
//! loop visits every item of the `--items` directory at 2 km, 200 m and 30 m above its anchor and records, per stop, the frame time
//! (p50, p95, max), the plan jobs the worker delivered and the jobs for R (count and p50, p95 of their worker time: the cost of the
//! plan, for comparing builds), the plans the worker refused and the first refusal's text. The window ends when the last stop is done and the run
//! fails when a plan was refused. A pure state machine over frames (no window): `camera` and `frame` are what the loop calls.
//!
//! Environment: `PLANET_SWEEP` (the report path, switches the sweep on), `PLANET_SWEEP_ALTS` (comma-separated metres, default
//! `2000,200,30`), `PLANET_SWEEP_HOLD` (frames per stop, default 90), `PLANET_SWEEP_DRIFT_M` (metres east per frame, default 0: with a drift each stop is a slow flight past the item, so its plans are warm ones), `PLANET_SWEEP_ONLY` (comma-separated item names: only the items
//! with exactly these names are visited, for example `Platonmax,Hill 7,Ridge 3`).

use planet_core::{PlanetFixed, Vec3};
use planet_frame::Camera;
use std::path::Path;

/// Altitudes above the ground at the camera, metres.
pub const DEFAULT_ALTITUDES_M: [f64; 3] = [2000.0, 200.0, 30.0];
/// Frames a stop lasts (the arrival frames, where the tiles stream in, included).
pub const DEFAULT_HOLD_FRAMES: u64 = 90;
/// The camera stands this many altitudes south of the anchor and looks at it.
const BACK_OFF: f64 = 1.5;
const FOV_Y_DEG: f64 = 60.0;
const NEAR_M: f64 = 0.1;

/// One item of the directory: the name, the kind and the anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepItem {
    pub name: String,
    pub kind: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
}

/// What a stop recorded.
#[derive(Clone, Debug, Default)]
pub struct StopRecord {
    pub frame_ms: Vec<f64>,
    /// The worker time of each plan delivered during the stop, and of each residency set (R) delivered, milliseconds.
    pub plan_ms: Vec<f64>,
    pub residency_ms: Vec<f64>,
    pub refused: u64,
    pub first_refusal: Option<String>,
}

/// The `p` quantile (0 to 1) of `values`, nearest rank; 0 for none.
fn quantile_of(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).expect("times are numbers"));
    v[((v.len() as f64 * p).ceil() as usize).clamp(1, v.len()) - 1]
}

impl StopRecord {
    /// The `p` quantile (0 to 1) of the frame times, nearest rank; 0 for a stop with no frames.
    pub fn quantile(&self, p: f64) -> f64 {
        quantile_of(&self.frame_ms, p)
    }
}

pub struct Sweep {
    items: Vec<SweepItem>,
    altitudes: Vec<f64>,
    hold: u64,
    /// The stop the loop is at (item index x altitudes + altitude index) and the frames spent there.
    stop: usize,
    in_stop: u64,
    records: Vec<StopRecord>,
    refused_seen: u64,
    report_path: String,
    /// Metres the camera moves east per frame of a stop (`PLANET_SWEEP_DRIFT_M`, default 0: a still camera, one cold plan per stop). With a drift the
    /// stop is a slow flight past the item: many plans, the caches warm after the first, which is what a flight in the editor costs.
    drift_m: f64,
}

/// The items of `dir` (`dir/<body>/<id>.toml`, one file each), in file-name order; a file without a name, kind or anchor is an error.
pub fn read_items(dir: &Path) -> Result<Vec<SweepItem>, String> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for body in std::fs::read_dir(dir).map_err(|e| format!("sweep: reading {}: {e}", dir.display()))? {
        let body = body.map_err(|e| e.to_string())?.path();
        if body.is_dir() {
            for f in std::fs::read_dir(&body).map_err(|e| e.to_string())? {
                let f = f.map_err(|e| e.to_string())?.path();
                if f.extension().is_some_and(|x| x == "toml") {
                    files.push(f);
                }
            }
        }
    }
    files.sort();
    files.iter().map(|f| parse_item(&std::fs::read_to_string(f).map_err(|e| format!("sweep: {}: {e}", f.display()))?, f)).collect()
}

/// The name, kind and `[anchor]` latitude and longitude of one item file.
pub fn parse_item(text: &str, file: &Path) -> Result<SweepItem, String> {
    let (mut name, mut kind, mut lat, mut lon) = (None, None, None, None);
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim().trim_matches('"'));
        match (section.as_str(), k) {
            ("", "name") => name = Some(v.to_string()),
            ("", "kind") => kind = Some(v.to_string()),
            ("[anchor]", "latitude") => lat = v.parse::<f64>().ok(),
            ("[anchor]", "longitude") => lon = v.parse::<f64>().ok(),
            _ => {}
        }
    }
    let bad = |what: &str| format!("sweep: {} has no {what}", file.display());
    Ok(SweepItem {
        name: name.ok_or_else(|| bad("name"))?,
        kind: kind.ok_or_else(|| bad("kind"))?,
        lat_deg: lat.ok_or_else(|| bad("anchor latitude"))?,
        lon_deg: lon.ok_or_else(|| bad("anchor longitude"))?,
    })
}

/// The items of `items` whose names are in the comma-separated list `only` (exact names, items of one name all kept); an error when none is.
pub fn keep_only(mut items: Vec<SweepItem>, only: &str) -> Result<Vec<SweepItem>, String> {
    let names: Vec<&str> = only.split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
    items.retain(|i| names.contains(&i.name.as_str()));
    if items.is_empty() {
        return Err(format!("PLANET_SWEEP_ONLY '{only}' names no item"));
    }
    Ok(items)
}

impl Sweep {
    /// The sweep the environment asks for over the items of `dir` (`None` without `PLANET_SWEEP`).
    pub fn from_env(dir: Option<&Path>) -> Result<Option<Sweep>, String> {
        let Ok(report_path) = std::env::var("PLANET_SWEEP") else { return Ok(None) };
        let dir = dir.ok_or("PLANET_SWEEP needs --items DIR (the items to visit)")?;
        let altitudes = match std::env::var("PLANET_SWEEP_ALTS") {
            Ok(v) => v
                .split(',')
                .map(|s| s.trim().parse::<f64>().map_err(|e| format!("PLANET_SWEEP_ALTS '{s}': {e}")))
                .collect::<Result<_, _>>()?,
            Err(_) => DEFAULT_ALTITUDES_M.to_vec(),
        };
        let hold = std::env::var("PLANET_SWEEP_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_HOLD_FRAMES);
        let mut items = read_items(dir)?;
        if let Ok(only) = std::env::var("PLANET_SWEEP_ONLY") {
            items = keep_only(items, &only).map_err(|e| format!("{e} in {}", dir.display()))?;
        }
        let mut sweep = Sweep::new(items, altitudes, hold, report_path)?;
        sweep.drift_m = std::env::var("PLANET_SWEEP_DRIFT_M").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        Ok(Some(sweep))
    }

    pub fn new(items: Vec<SweepItem>, altitudes: Vec<f64>, hold: u64, report_path: String) -> Result<Sweep, String> {
        if items.is_empty() || altitudes.is_empty() || hold == 0 {
            return Err("sweep: no items, no altitudes or an empty hold".into());
        }
        let stops = items.len() * altitudes.len();
        Ok(Sweep {
            items,
            altitudes,
            hold,
            stop: 0,
            in_stop: 0,
            records: vec![StopRecord::default(); stops],
            refused_seen: 0,
            report_path,
            drift_m: 0.0,
        })
    }

    pub fn stops(&self) -> usize {
        self.records.len()
    }

    pub fn done(&self) -> bool {
        self.stop >= self.records.len()
    }

    fn item_of(&self, stop: usize) -> (&SweepItem, f64) {
        (&self.items[stop / self.altitudes.len()], self.altitudes[stop % self.altitudes.len()])
    }

    /// The camera of the current stop: `alt` above the ground `BACK_OFF` altitudes south of the item's anchor, looking at the anchor.
    /// `height` is the terrain height above the reference sphere at a direction (the field), `radius_m` the sphere's radius.
    pub fn camera(&self, height: &dyn Fn(Vec3) -> f64, radius_m: f64, aspect: f64) -> Camera {
        let (item, alt) = self.item_of(self.stop.min(self.records.len() - 1));
        let (la, lo) = (item.lat_deg.to_radians(), item.lon_deg.to_radians());
        let dir = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
        let north = {
            let n = Vec3::new(0.0, 0.0, 1.0);
            (n - dir * n.dot(dir)).normalized()
        };
        let h_anchor = height(dir);
        let back = BACK_OFF * alt;
        let mut behind = (dir * (radius_m + h_anchor) - north * back).normalized();
        if self.drift_m != 0.0 {
            // East of the first pose by the frames spent at this stop: a slow flight past the item.
            let east = Vec3::new(0.0, 0.0, 1.0).cross(behind).normalized();
            behind = (behind + east * (self.drift_m * self.in_stop as f64 / radius_m)).normalized();
        }
        let h_here = height(behind);
        let position = PlanetFixed(behind * (radius_m + h_here + alt));
        let pitch = libm::atan2(alt + h_here - h_anchor, back);
        Camera::at_surface_point(position, north, pitch, FOV_Y_DEG.to_radians(), aspect, NEAR_M)
    }

    /// One frame at the current stop: its time in milliseconds, the worker time of the plan and of R delivered this frame (if any), the
    /// refused plans so far and the refusal text taken this frame.
    pub fn frame(&mut self, frame_ms: f64, plan_ms: Option<f64>, residency_ms: Option<f64>, refused_total: u64, refusal: Option<String>) {
        if self.done() {
            return;
        }
        let rec = &mut self.records[self.stop];
        rec.frame_ms.push(frame_ms);
        rec.plan_ms.extend(plan_ms);
        rec.residency_ms.extend(residency_ms);
        rec.refused += refused_total.saturating_sub(self.refused_seen);
        self.refused_seen = refused_total;
        if rec.first_refusal.is_none() {
            rec.first_refusal = refusal;
        }
        self.in_stop += 1;
        if self.in_stop >= self.hold {
            self.in_stop = 0;
            self.stop += 1;
        }
    }

    /// The report: a line per stop and the verdict line (`[sweep ok]` or `[sweep FAIL]`).
    pub fn report(&self) -> (String, bool) {
        let mut out = String::new();
        let mut refused_stops = 0;
        let mut worst_p95 = 0.0f64;
        out +=
            "stop item kind altitude_m frames p50_ms p95_ms max_ms plans plan_p50_ms plan_p95_ms r_p50_ms r_p95_ms refused first_refusal\n";
        for (i, rec) in self.records.iter().enumerate() {
            let (item, alt) = self.item_of(i);
            let p95 = rec.quantile(0.95);
            worst_p95 = worst_p95.max(p95);
            refused_stops += u64::from(rec.refused > 0);
            out += &format!(
                "{i} {} {} {alt} {} {:.2} {:.2} {:.2} {} {:.2} {:.2} {:.2} {:.2} {} {}\n",
                item.name.replace(' ', "_"),
                item.kind,
                rec.frame_ms.len(),
                rec.quantile(0.5),
                p95,
                rec.frame_ms.iter().copied().fold(0.0, f64::max),
                rec.plan_ms.len(),
                quantile_of(&rec.plan_ms, 0.5),
                quantile_of(&rec.plan_ms, 0.95),
                quantile_of(&rec.residency_ms, 0.5),
                quantile_of(&rec.residency_ms, 0.95),
                rec.refused,
                rec.first_refusal.as_deref().unwrap_or("-").replace('\n', " | ")
            );
        }
        let ok = refused_stops == 0;
        out += &format!(
            "[sweep {}]: {} stops, {} items x {} altitudes, {refused_stops} stops with a refused plan, worst p95 {worst_p95:.2} ms\n",
            if ok { "ok" } else { "FAIL" },
            self.stops(),
            self.items.len(),
            self.altitudes.len()
        );
        (out, ok)
    }

    /// Write the report to the file named by `PLANET_SWEEP` and return it with the verdict.
    pub fn finish(&self) -> Result<(String, bool), String> {
        let (text, ok) = self.report();
        std::fs::write(&self.report_path, &text).map_err(|e| format!("sweep: writing {}: {e}", self.report_path))?;
        Ok((text, ok))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, lat: f64) -> SweepItem {
        SweepItem { name: name.into(), kind: "plateau".into(), lat_deg: lat, lon_deg: 30.0 }
    }

    // spec: VIEW-022
    #[test]
    fn an_item_file_gives_its_name_kind_and_anchor_and_a_file_without_one_is_an_error() {
        let text = "schema_version = 1\nname = \"Plateau 7\"\nkind = \"plateau\"\n[anchor]\nlatitude = 27.5\nlongitude = -50.25\n[params]\ntarget_m = 300.0\n";
        let it = parse_item(text, Path::new("a.toml")).unwrap();
        assert_eq!((it.name.as_str(), it.kind.as_str(), it.lat_deg, it.lon_deg), ("Plateau 7", "plateau", 27.5, -50.25));
        let err = parse_item("kind = \"hill\"\n", Path::new("b.toml")).unwrap_err();
        assert!(err.contains("b.toml") && err.contains("name"), "{err}");
    }

    // spec: VIEW-022
    #[test]
    fn the_stops_visit_every_item_at_every_altitude_for_the_hold_and_count_refusals_where_they_happened() {
        let mut s = Sweep::new(vec![item("a", 10.0), item("b", 20.0)], vec![2000.0, 30.0], 3, "x".into()).unwrap();
        assert_eq!(s.stops(), 4);
        for k in 0..12u64 {
            // The second stop (item a, 30 m) is the one where the worker refuses a plan.
            let refused = if k >= 4 { 1 } else { 0 };
            s.frame(
                10.0 + k as f64,
                (k % 3 == 0).then_some(2.0 + k as f64),
                None,
                refused,
                (k == 4).then(|| "plan for frame 5: refused".to_string()),
            );
        }
        assert!(s.done());
        let (text, ok) = s.report();
        assert!(!ok, "{text}");
        assert!(text.contains("1 stops with a refused plan"), "{text}");
        let second = text.lines().nth(2).unwrap();
        assert!(second.starts_with("1 a plateau 30 3 ") && second.ends_with("1 plan for frame 5: refused"), "{second}");
        // The plan jobs delivered during the stop: frames 3, 4, 5 of the run, k = 3 is the only one in it (k % 3 == 0): 5 ms, a lone sample.
        assert!(second.contains(" 1 5.00 5.00 0.00 0.00 "), "{second}");
        assert!(text.lines().nth(1).unwrap().ends_with(" 0 -"), "{text}");
    }

    // spec: VIEW-022
    // spec: VIEW-022
    #[test]
    fn a_drift_moves_the_camera_east_by_that_many_metres_a_frame_and_none_leaves_it_where_it_was() {
        let flat = |_: Vec3| 0.0;
        let mut s = Sweep::new(vec![item("a", 20.0)], vec![200.0], 100, "x".into()).unwrap();
        let first = s.camera(&flat, 6_371_000.0, 16.0 / 9.0).position;
        for _ in 0..10 {
            s.frame(1.0, None, None, 0, None);
        }
        assert_eq!(s.camera(&flat, 6_371_000.0, 16.0 / 9.0).position, first, "without a drift the camera stays");
        s.drift_m = 5.0;
        let moved = s.camera(&flat, 6_371_000.0, 16.0 / 9.0).position;
        let d = (moved.0 - first.0).length();
        assert!((d - 50.0).abs() < 0.5, "10 frames at 5 m a frame: {d} m");
    }

    // spec: VIEW-022
    #[test]
    fn the_camera_stands_back_from_the_anchor_at_the_altitude_and_looks_down_at_it() {
        let s = Sweep::new(vec![item("a", 20.0)], vec![200.0], 1, "x".into()).unwrap();
        let flat = |_: Vec3| 0.0;
        let cam = s.camera(&flat, 6_371_000.0, 16.0 / 9.0);
        let alt = cam.position.0.length() - 6_371_000.0;
        assert!((alt - 200.0).abs() < 0.5, "{alt}");
        let (la, lo) = (20f64.to_radians(), 30f64.to_radians());
        let anchor = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la)) * 6_371_000.0;
        let to = (anchor - cam.position.0).normalized();
        assert!(to.dot(cam.forward.normalized()) > 0.999, "the anchor is straight ahead");
        assert!((anchor - cam.position.0).length() > 300.0 && (anchor - cam.position.0).length() < 400.0);
    }

    // spec: VIEW-022
    #[test]
    fn only_the_items_of_the_named_list_are_kept_every_item_of_a_name_and_a_list_naming_none_is_an_error() {
        let items = vec![item("Platonmax", 1.0), item("Hill 7", 2.0), item("Hill 7", 3.0), item("Plateau 7", 4.0)];
        let kept = keep_only(items.clone(), "Platonmax, Hill 7").unwrap();
        assert_eq!(
            kept.iter().map(|i| (i.name.as_str(), i.lat_deg)).collect::<Vec<_>>(),
            [("Platonmax", 1.0), ("Hill 7", 2.0), ("Hill 7", 3.0)]
        );
        assert!(keep_only(items, "Ridge 3").unwrap_err().contains("Ridge 3"));
    }

    // spec: VIEW-022
    #[test]
    fn the_quantiles_are_nearest_rank() {
        let r = StopRecord { frame_ms: (1..=100).map(f64::from).collect(), ..Default::default() };
        assert_eq!((r.quantile(0.5), r.quantile(0.95), r.quantile(1.0)), (50.0, 95.0, 100.0));
        assert_eq!(StopRecord::default().quantile(0.5), 0.0);
    }
}
