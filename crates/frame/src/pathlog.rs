//! The rolling camera-path log of the window (VIEW-024): the last [`WINDOW_MS`] of camera poses, one about every [`PERIOD_MS`], so that
//! a bug an owner saw can be replayed from where the camera had been. GPU-free; the window records into it and F9 writes it out.

use crate::camera::Camera;

/// First line of a path file; the number is the format version.
pub const PATH_HEADER: &str = "planet-path 1";
/// How much history is kept, in milliseconds of the injected clock.
pub const WINDOW_MS: u64 = 60_000;
/// The least time between two samples (10 Hz).
pub const PERIOD_MS: u64 = 100;

/// One pose of the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathSample {
    /// Time on the window's clock, milliseconds.
    pub t_ms: u64,
    /// Planet-fixed position in metres, f64.
    pub position: [f64; 3],
    /// Unit view direction.
    pub forward: [f64; 3],
    /// Up vector as the camera holds it.
    pub up: [f64; 3],
    /// Height above the terrain in metres.
    pub altitude_m: f64,
    /// Duration of the frame the pose belongs to, milliseconds.
    pub frame_ms: f64,
}

impl PathSample {
    /// The camera of this pose, with the lens (field of view, aspect, near plane) of `lens`: what a replay plans the frame with.
    pub fn camera(&self, lens: &Camera) -> Camera {
        let v = |a: [f64; 3]| planet_core::Vec3::new(a[0], a[1], a[2]);
        Camera { position: planet_core::PlanetFixed(v(self.position)), forward: v(self.forward), up: v(self.up), ..*lens }
    }
}

/// The ring of the last minute of poses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathLog {
    samples: std::collections::VecDeque<PathSample>,
}

impl PathLog {
    pub fn new() -> PathLog {
        PathLog::default()
    }

    /// Records the pose of the frame at `t_ms` when [`PERIOD_MS`] have passed since the last sample (the first call always records), and
    /// forgets what is older than [`WINDOW_MS`] before it. Returns true when it recorded. A clock that went back is not recorded.
    pub fn record(&mut self, t_ms: u64, camera: &Camera, altitude_m: f64, frame_ms: f64) -> bool {
        if self.samples.back().is_some_and(|s| t_ms < s.t_ms.saturating_add(PERIOD_MS)) {
            return false;
        }
        let v = |p: planet_core::Vec3| [p.x, p.y, p.z];
        self.samples.push_back(PathSample {
            t_ms,
            position: v(camera.position.0),
            forward: v(camera.forward),
            up: v(camera.up),
            altitude_m,
            frame_ms,
        });
        while self.samples.front().is_some_and(|s| s.t_ms.saturating_add(WINDOW_MS) < t_ms) {
            self.samples.pop_front();
        }
        true
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn samples(&self) -> impl Iterator<Item = &PathSample> {
        self.samples.iter()
    }

    /// The text of a path file: the header, the window and the period, and one line per sample, oldest first
    /// (`t_ms x y z fx fy fz ux uy uz altitude_m frame_ms`). Floats are written in Rust's shortest round-trip form, so reading restores them
    /// bit for bit.
    pub fn to_text(&self) -> String {
        let mut s = format!("{PATH_HEADER}\nwindow_ms {WINDOW_MS}\nperiod_ms {PERIOD_MS}\nsamples {}\n", self.samples.len());
        for p in &self.samples {
            s += &format!(
                "{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}\n",
                p.t_ms,
                p.position[0],
                p.position[1],
                p.position[2],
                p.forward[0],
                p.forward[1],
                p.forward[2],
                p.up[0],
                p.up[1],
                p.up[2],
                p.altitude_m,
                p.frame_ms
            );
        }
        s
    }

    /// Reads a path file back; an unknown header, a wrong sample count, a malformed line or time going backwards is an error that names the line.
    pub fn from_text(text: &str) -> Result<PathLog, String> {
        let mut lines = text.lines();
        let first = lines.next().ok_or("empty path file")?;
        if first != PATH_HEADER {
            return Err(format!("unsupported path header '{first}' (this build reads '{PATH_HEADER}')"));
        }
        let mut keyed = |key: &str| -> Result<u64, String> {
            let line = lines.next().ok_or(format!("the path file ends before '{key}'"))?;
            line.strip_prefix(key).and_then(|r| r.trim().parse::<u64>().ok()).ok_or(format!("expected '{key} <number>', found '{line}'"))
        };
        keyed("window_ms")?;
        keyed("period_ms")?;
        let count = keyed("samples")? as usize;
        let mut log = PathLog::new();
        for (k, line) in lines.enumerate() {
            let n = k + 5;
            let mut f = line.split_whitespace();
            let t_ms: u64 = f.next().and_then(|t| t.parse().ok()).ok_or(format!("line {n}: no time in '{line}'"))?;
            let mut v = [0.0f64; 11];
            for slot in v.iter_mut() {
                *slot = f.next().and_then(|x| x.parse().ok()).ok_or(format!("line {n}: expected 11 numbers after the time in '{line}'"))?;
            }
            if f.next().is_some() {
                return Err(format!("line {n}: more than 11 numbers after the time in '{line}'"));
            }
            if log.samples.back().is_some_and(|s| t_ms < s.t_ms) {
                return Err(format!("line {n}: time {t_ms} ms goes back"));
            }
            log.samples.push_back(PathSample {
                t_ms,
                position: [v[0], v[1], v[2]],
                forward: [v[3], v[4], v[5]],
                up: [v[6], v[7], v[8]],
                altitude_m: v[9],
                frame_ms: v[10],
            });
        }
        if log.len() != count {
            return Err(format!("the header says {count} samples, the file has {}", log.len()));
        }
        Ok(log)
    }

    /// A few lines for a person: how many poses over how long, where the camera started and ended, the lowest altitude, the slowest frame.
    /// The one-line notice the window shows in its title after F9 wrote `frame-<frame>.bundle` and `frame-<frame>-path.txt` to `dir`, so the
    /// owner sees the capture happened without a console.
    pub fn saved_notice(&self, frame: u64, dir: &str) -> String {
        let (Some(a), Some(b)) = (self.samples.front(), self.samples.back()) else {
            return format!("F9: wrote frame-{frame}.bundle and an empty path to {dir}");
        };
        format!(
            "F9: wrote frame-{frame}.bundle and frame-{frame}-path.txt ({} poses, {:.0} s) to {dir}",
            self.samples.len(),
            (b.t_ms - a.t_ms) as f64 / 1000.0
        )
    }

    pub fn summary(&self) -> String {
        let (Some(a), Some(b)) = (self.samples.front(), self.samples.back()) else { return "camera path: no samples".to_string() };
        let low = self.samples.iter().map(|s| s.altitude_m).fold(f64::INFINITY, f64::min);
        let slow = self.samples.iter().map(|s| s.frame_ms).fold(0.0, f64::max);
        let at = |p: &PathSample| {
            format!("({:.1}, {:.1}, {:.1}) m at {:.1} m altitude", p.position[0], p.position[1], p.position[2], p.altitude_m)
        };
        format!(
            "camera path: {} samples over {:.1} s (t {} to {} ms)\n  from {}\n  to   {}\n  lowest altitude {:.1} m, slowest frame {:.1} ms",
            self.samples.len(),
            (b.t_ms - a.t_ms) as f64 / 1000.0,
            a.t_ms,
            b.t_ms,
            at(a),
            at(b),
            low,
            slow
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::{PlanetFixed, Vec3};

    fn camera(x: f64) -> Camera {
        Camera::at_surface_point(PlanetFixed(Vec3::new(x, 0.5, -3.25)), Vec3::new(0.0, 0.0, 1.0), 1.0, 0.9, 1.5, 0.5)
    }

    // spec: VIEW-024
    #[test]
    fn the_ring_takes_ten_poses_a_second_and_forgets_what_is_older_than_a_minute() {
        let mut log = PathLog::new();
        assert!(log.record(1_000, &camera(1.0), 5.0, 16.0), "the first call records");
        assert!(!log.record(1_050, &camera(2.0), 5.0, 16.0), "50 ms later is too soon");
        assert!(!log.record(1_099, &camera(2.0), 5.0, 16.0));
        assert!(log.record(1_100, &camera(3.0), 5.0, 16.0), "exactly 100 ms later records");
        assert!(!log.record(900, &camera(4.0), 5.0, 16.0), "a clock that went back is not recorded");
        assert_eq!(log.len(), 2);
        // A frame every 7 ms for 3 minutes keeps the last 60 s at 10 Hz.
        let mut log = PathLog::new();
        for k in 0..(180_000 / 7) {
            log.record(k * 7, &camera(k as f64), 1.0, 7.0);
        }
        let times: Vec<u64> = log.samples().map(|s| s.t_ms).collect();
        let (first, last) = (times[0], *times.last().unwrap());
        assert!(last - first <= WINDOW_MS && last - first > WINDOW_MS - 2 * PERIOD_MS, "{first}..{last}");
        assert!(log.len() <= (WINDOW_MS / PERIOD_MS) as usize + 1, "bounded: {}", log.len());
        assert!(times.windows(2).all(|w| w[1] - w[0] >= PERIOD_MS && w[1] - w[0] < PERIOD_MS + 7), "about 10 Hz: {times:?}");
    }

    // spec: VIEW-024
    #[test]
    fn a_pose_rebuilds_its_camera_with_the_lens_it_is_given_and_f9_names_what_it_wrote() {
        let mut log = PathLog::new();
        assert!(log.saved_notice(7, "d").contains("empty path"), "no samples is said");
        let c = camera(6_371_000.5);
        log.record(2_000, &c, 12.0, 16.0);
        log.record(32_000, &camera(6_371_100.5), 3.0, 40.0);
        let lens = Camera::at_surface_point(PlanetFixed(Vec3::new(1.0, 0.0, 0.0)), Vec3::new(0.0, 0.0, 1.0), 0.0, 1.1, 2.0, 0.25);
        let rebuilt = log.samples().next().unwrap().camera(&lens);
        assert_eq!(rebuilt.position, c.position);
        assert_eq!(rebuilt.forward, c.forward);
        assert_eq!(rebuilt.up, c.up);
        assert_eq!((rebuilt.fov_y_rad, rebuilt.aspect, rebuilt.near_m), (1.1, 2.0, 0.25), "the lens is the bundle's, not the pose's");
        let n = log.saved_notice(7, "target/review");
        assert_eq!(n, "F9: wrote frame-7.bundle and frame-7-path.txt (2 poses, 30 s) to target/review");
    }

    // spec: VIEW-024
    #[test]
    fn a_path_file_reads_back_bit_for_bit_and_a_damaged_one_is_refused_with_its_line() {
        let mut log = PathLog::new();
        let c = camera(6_371_000.123_456_789);
        log.record(10, &c, 1.0 / 3.0, 16.666_666_666_666_668);
        log.record(110, &camera(-0.0), 1e-300, 0.0);
        let text = log.to_text();
        assert!(text.starts_with("planet-path 1\nwindow_ms 60000\nperiod_ms 100\nsamples 2\n10 6371000.123456789 "), "{text}");
        let back = PathLog::from_text(&text).unwrap();
        assert_eq!(back, log);
        let first = back.samples().next().unwrap();
        assert_eq!(first.position[0].to_bits(), c.position.0.x.to_bits());
        assert_eq!(first.altitude_m.to_bits(), (1.0f64 / 3.0).to_bits());
        for (bad, why) in [
            ("", "empty"),
            ("planet-path 2\n", "unsupported path header"),
            ("planet-path 1\nwindow_ms 60000\nperiod_ms 100\nsamples 3\n", "header says 3 samples"),
            ("planet-path 1\nwindow_ms 60000\nperiod_ms 100\nsamples 1\n5 1 2 3\n", "line 5"),
            (
                "planet-path 1\nwindow_ms 60000\nperiod_ms 100\nsamples 2\n9 1 2 3 4 5 6 7 8 9 10 11\n5 1 2 3 4 5 6 7 8 9 10 11\n",
                "goes back",
            ),
        ] {
            let e = PathLog::from_text(bad).unwrap_err();
            assert!(e.contains(why), "{bad:?} gave {e}");
        }
    }

    // spec: VIEW-024
    #[test]
    fn the_summary_names_the_span_the_ends_the_lowest_altitude_and_the_slowest_frame() {
        let mut log = PathLog::new();
        assert_eq!(log.summary(), "camera path: no samples");
        log.record(0, &camera(1.0), 120.0, 10.0);
        log.record(2_000, &camera(2.0), 3.5, 40.0);
        log.record(4_000, &camera(3.0), 80.0, 12.0);
        let s = log.summary();
        assert!(s.contains("3 samples over 4.0 s (t 0 to 4000 ms)"), "{s}");
        assert!(s.contains("lowest altitude 3.5 m, slowest frame 40.0 ms"), "{s}");
        assert!(s.contains("from (1.0, 0.5, -3.2) m") || s.contains("from (1.0, 0.5, -3.3) m"), "{s}");
    }
}
