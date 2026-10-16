//! The input adapter plus pose integration, shared by the live window and the headless replay (so a recorded trace reproduces the
//! live pose). Events carry a monotonic microsecond timestamp; decisions (capture, release, recentre, which source counts) live
//! here; the app only executes the returned commands.

use crate::pose::{step, Input, Pose};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Raw,
    Cursor,
    /// Raw while raw events keep arriving (one within the last 500 ms), the cursor otherwise.
    Auto,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Raw => "raw",
            Source::Cursor => "cursor",
            Source::Auto => "auto",
        }
    }
    pub fn parse(s: &str) -> Source {
        match s {
            "raw" => Source::Raw,
            "cursor" => Source::Cursor,
            _ => Source::Auto,
        }
    }
    pub fn next(self) -> Source {
        match self {
            Source::Raw => Source::Cursor,
            Source::Cursor => Source::Auto,
            Source::Auto => Source::Raw,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Radians per raw count or per physical pixel. Never multiplied by dt.
    pub sens: f64,
    pub source: Source,
    /// Recentre when the cursor is farther than this fraction of the half-extent from the window centre (0 = after every
    /// event, 1 = never).
    pub recenter_box: f64,
    pub radius: f64,
    pub start_lat: f64,
    pub start_lon: f64,
    pub start_alt: f64,
    pub start_heading: f64,
}

impl Default for Config {
    fn default() -> Config {
        Config { sens: 0.0015, source: Source::Auto, recenter_box: 0.25, radius: 1000.0, start_lat: 0.3, start_lon: 0.7, start_alt: 1.7, start_heading: 0.4 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    Raw { dx: f64, dy: f64 },
    Cursor { x: f64, y: f64 },
    Button { down: bool },
    Key { code: String, down: bool },
    Focus { on: bool },
    Resize { w: f64, h: f64 },
    Frame { dt_us: u64 },
    /// A live change of the input configuration (recorded so a trace stays replayable).
    Set { source: Source, recenter_box: f64, sens: f64 },
    /// Back to the start pose.
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// Grab the cursor (confined, hidden) or release it.
    Grab(bool),
    Recenter(f64, f64),
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub raw_events: u64,
    pub cursor_events: u64,
    pub recenters: u64,
    pub recenter_failures: u64,
    pub dropped_while_free: u64,
}

pub struct Sim {
    pub cfg: Config,
    pub pose: Pose,
    pub captured: bool,
    pub focused: bool,
    pub center: (f64, f64),
    pub stats: Stats,
    pub total_yaw: f64,
    pub total_pitch_input: f64,
    keys: BTreeSet<String>,
    accum: (f64, f64),
    last_raw_us: Option<u64>,
    last_cursor: Option<(f64, f64)>,
    pending_recenter: Option<u32>,
    now_us: u64,
}

fn held(keys: &BTreeSet<String>, k: &str) -> f64 {
    f64::from(u8::from(keys.contains(k)))
}

impl Sim {
    pub fn new(cfg: Config) -> Sim {
        let pose = Pose::at(cfg.start_lat, cfg.start_lon, cfg.radius, cfg.start_alt, cfg.start_heading);
        Sim {
            cfg,
            pose,
            captured: false,
            focused: true,
            center: (480.0, 270.0),
            stats: Stats::default(),
            total_yaw: 0.0,
            total_pitch_input: 0.0,
            keys: BTreeSet::new(),
            accum: (0.0, 0.0),
            last_raw_us: None,
            last_cursor: None,
            pending_recenter: None,
            now_us: 0,
        }
    }

    pub fn reset_pose(&mut self) {
        self.pose = Pose::at(self.cfg.start_lat, self.cfg.start_lon, self.cfg.radius, self.cfg.start_alt, self.cfg.start_heading);
        self.total_yaw = 0.0;
    }

    /// The source that counts at time `t`.
    pub fn active_source(&self) -> Source {
        match self.cfg.source {
            Source::Auto => match self.last_raw_us {
                Some(r) if self.now_us.saturating_sub(r) <= 500_000 => Source::Raw,
                _ => Source::Cursor,
            },
            s => s,
        }
    }

    fn release(&mut self, out: &mut Vec<Command>) {
        if self.captured {
            self.captured = false;
            out.push(Command::Grab(false));
        }
        self.last_cursor = None;
        self.pending_recenter = None;
        self.accum = (0.0, 0.0);
    }

    fn capture(&mut self, out: &mut Vec<Command>) {
        self.captured = true;
        self.last_cursor = None;
        self.accum = (0.0, 0.0);
        out.push(Command::Grab(true));
        out.push(Command::Recenter(self.center.0, self.center.1));
        self.pending_recenter = Some(0);
        self.stats.recenters += 1;
    }

    /// Feed one event at monotonic time `t_us`; returns what the window should do.
    pub fn apply(&mut self, t_us: u64, ev: &Ev) -> Vec<Command> {
        self.now_us = t_us;
        let mut out = Vec::new();
        match ev {
            Ev::Raw { dx, dy } => {
                self.last_raw_us = Some(t_us);
                self.stats.raw_events += 1;
                if self.captured && self.active_source() == Source::Raw {
                    self.accum.0 += dx;
                    self.accum.1 += dy;
                } else if !self.captured {
                    self.stats.dropped_while_free += 1;
                }
            }
            Ev::Cursor { x, y } => {
                self.stats.cursor_events += 1;
                let p = (*x, *y);
                let mut d = match self.last_cursor {
                    Some(prev) => (p.0 - prev.0, p.1 - prev.1),
                    None => (0.0, 0.0),
                };
                if let Some(late) = self.pending_recenter {
                    let (cx, cy) = self.center;
                    if (p.0 - cx).abs() <= 2.0 && (p.1 - cy).abs() <= 2.0 {
                        // The recentre landed: the jump to the centre is ours, not the user's.
                        d = (0.0, 0.0);
                        self.pending_recenter = None;
                    } else if late >= 8 {
                        self.stats.recenter_failures += 1;
                        self.pending_recenter = None;
                    } else {
                        self.pending_recenter = Some(late + 1);
                    }
                }
                self.last_cursor = Some(p);
                if self.captured {
                    if self.active_source() == Source::Cursor {
                        self.accum.0 += d.0;
                        self.accum.1 += d.1;
                    }
                    let (cx, cy) = self.center;
                    let b = self.cfg.recenter_box;
                    if self.pending_recenter.is_none() && ((p.0 - cx).abs() > b * cx || (p.1 - cy).abs() > b * cy) {
                        out.push(Command::Recenter(cx, cy));
                        self.pending_recenter = Some(0);
                        self.stats.recenters += 1;
                    }
                } else {
                    self.stats.dropped_while_free += 1;
                }
            }
            Ev::Button { down } => {
                if *down && self.focused && !self.captured {
                    self.capture(&mut out);
                }
            }
            Ev::Key { code, down } => {
                if *down && code == "Escape" {
                    self.release(&mut out);
                }
                if *down {
                    self.keys.insert(code.clone());
                } else {
                    self.keys.remove(code);
                }
            }
            Ev::Focus { on } => {
                self.focused = *on;
                // Policy: focus loss releases the grab and forgets everything pending; regaining focus needs a click, so there
                // is no stored position to jump from.
                self.release(&mut out);
                self.keys.clear();
            }
            Ev::Resize { w, h } => {
                self.center = (w / 2.0, h / 2.0);
            }
            Ev::Set { source, recenter_box, sens } => {
                self.cfg.source = *source;
                self.cfg.recenter_box = *recenter_box;
                self.cfg.sens = *sens;
            }
            Ev::Reset => self.reset_pose(),
            Ev::Frame { dt_us } => {
                let dt = *dt_us as f64 / 1.0e6;
                let (dx, dy) = std::mem::take(&mut self.accum);
                // Mouse: counts times sensitivity, once per frame, never times dt.
                let mut dyaw = dx * self.cfg.sens;
                let mut dpitch = -dy * self.cfg.sens;
                // Keys are rates, so they do use dt.
                dyaw += (held(&self.keys, "ArrowRight") - held(&self.keys, "ArrowLeft")) * 1.5 * dt;
                dpitch += (held(&self.keys, "ArrowUp") - held(&self.keys, "ArrowDown")) * 1.5 * dt;
                let alt = self.pose.altitude(self.cfg.radius).max(0.0);
                let speed = 5.0 * (alt / 2.0).max(1.0);
                let input = Input {
                    dyaw,
                    dpitch,
                    forward: (held(&self.keys, "KeyW") - held(&self.keys, "KeyS")) * speed * dt,
                    right: (held(&self.keys, "KeyD") - held(&self.keys, "KeyA")) * speed * dt,
                    up: (held(&self.keys, "KeyE") - held(&self.keys, "KeyQ")) * speed * dt,
                };
                self.pose = step(&self.pose, &input);
                self.total_yaw += dyaw;
                self.total_pitch_input += dpitch;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim() -> Sim {
        let mut s = Sim::new(Config { source: Source::Raw, ..Config::default() });
        s.apply(0, &Ev::Resize { w: 960.0, h: 540.0 });
        s
    }

    fn click(s: &mut Sim, t: u64) {
        s.apply(t, &Ev::Button { down: true });
    }

    #[test]
    fn raw_motion_turns_by_counts_times_sensitivity_whatever_the_frame_rate() {
        for frames in [1u64, 7, 60, 500] {
            let mut s = sim();
            click(&mut s, 1);
            let start = s.pose.heading_from_north();
            for k in 0..1000u64 {
                s.apply(10 + k, &Ev::Raw { dx: 2.0, dy: 0.0 });
                if k % (1000 / frames).max(1) == 0 {
                    // Wildly different frame times: the mouse part must not depend on dt.
                    s.apply(10 + k, &Ev::Frame { dt_us: 1 + (k * 37) % 90_000 });
                }
            }
            s.apply(5000, &Ev::Frame { dt_us: 16_666 });
            assert!((s.total_yaw - 2000.0 * s.cfg.sens).abs() < 1e-12, "frames {frames}: {}", s.total_yaw);
            let turned = (s.pose.heading_from_north() - start).rem_euclid(std::f64::consts::TAU);
            assert!((turned - (2000.0 * s.cfg.sens).rem_euclid(std::f64::consts::TAU)).abs() < 1e-9);
        }
    }

    #[test]
    fn motion_before_the_click_is_ignored_and_focus_loss_releases() {
        let mut s = sim();
        s.apply(1, &Ev::Raw { dx: 500.0, dy: 500.0 });
        s.apply(2, &Ev::Frame { dt_us: 16_000 });
        assert_eq!(s.total_yaw, 0.0);
        click(&mut s, 3);
        assert!(s.captured);
        let cmds = s.apply(4, &Ev::Focus { on: false });
        assert!(!s.captured && cmds == vec![Command::Grab(false)]);
        // Motion while unfocused, then regaining focus: nothing pending, no jump.
        s.apply(5, &Ev::Raw { dx: 900.0, dy: 0.0 });
        s.apply(6, &Ev::Focus { on: true });
        s.apply(7, &Ev::Frame { dt_us: 16_000 });
        assert_eq!(s.total_yaw, 0.0);
        click(&mut s, 8);
        s.apply(9, &Ev::Cursor { x: 100.0, y: 100.0 }); // first cursor event after a capture only sets the reference
        s.apply(10, &Ev::Frame { dt_us: 16_000 });
        assert_eq!(s.total_yaw, 0.0);
    }

    #[test]
    fn cursor_deltas_survive_recentring_and_its_echo() {
        let mut s = Sim::new(Config { source: Source::Cursor, recenter_box: 0.25, ..Config::default() });
        s.apply(0, &Ev::Resize { w: 960.0, h: 540.0 });
        click(&mut s, 1);
        // Capture asked for a recentre; the echo arrives at the centre.
        s.apply(2, &Ev::Cursor { x: 480.0, y: 270.0 });
        let mut x = 480.0;
        let mut total = 0.0;
        let mut recenters = 0;
        for k in 0..100u64 {
            x += 30.0;
            total += 30.0;
            let cmds = s.apply(10 + k, &Ev::Cursor { x, y: 270.0 });
            if cmds.contains(&Command::Recenter(480.0, 270.0)) {
                recenters += 1;
                // The system moves the cursor back; we see the echo at the centre.
                s.apply(10 + k, &Ev::Cursor { x: 480.0, y: 270.0 });
                x = 480.0;
            }
        }
        s.apply(500, &Ev::Frame { dt_us: 16_000 });
        assert!(recenters > 5, "lazy recentring happened {recenters} times");
        assert!((s.total_yaw - total * s.cfg.sens).abs() < 1e-9, "{} vs {}", s.total_yaw, total * s.cfg.sens);
    }

    #[test]
    fn auto_falls_back_to_the_cursor_when_raw_events_stop() {
        let mut s = Sim::new(Config { source: Source::Auto, recenter_box: 1.0, ..Config::default() });
        s.apply(0, &Ev::Resize { w: 960.0, h: 540.0 });
        click(&mut s, 1);
        s.apply(2, &Ev::Cursor { x: 480.0, y: 270.0 });
        // Raw present: raw counts, cursor ignored (no double counting).
        s.apply(10, &Ev::Raw { dx: 10.0, dy: 0.0 });
        s.apply(11, &Ev::Cursor { x: 490.0, y: 270.0 });
        s.apply(20, &Ev::Frame { dt_us: 16_000 });
        assert!((s.total_yaw - 10.0 * s.cfg.sens).abs() < 1e-12);
        // One second later no raw events: the cursor counts.
        s.apply(2_000_000, &Ev::Cursor { x: 510.0, y: 270.0 });
        s.apply(2_000_100, &Ev::Frame { dt_us: 16_000 });
        assert!((s.total_yaw - 30.0 * s.cfg.sens).abs() < 1e-12, "{}", s.total_yaw);
    }

    #[test]
    fn a_recentre_that_never_lands_is_counted_and_deltas_stay_continuous() {
        let mut s = Sim::new(Config { source: Source::Cursor, recenter_box: 0.25, ..Config::default() });
        s.apply(0, &Ev::Resize { w: 960.0, h: 540.0 });
        click(&mut s, 1);
        s.apply(2, &Ev::Cursor { x: 480.0, y: 270.0 });
        let mut total = 0.0;
        let mut x = 480.0;
        for k in 0..40u64 {
            x += 10.0;
            total += 10.0;
            s.apply(10 + k, &Ev::Cursor { x, y: 270.0 });
        }
        s.apply(100, &Ev::Frame { dt_us: 16_000 });
        assert!(s.stats.recenter_failures >= 1);
        assert!((s.total_yaw - total * s.cfg.sens).abs() < 1e-9);
    }
}
