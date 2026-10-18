//! VIEW-010: the SPIKE-11 traces of the owner's local check (`spikes/mouse-look/results/traces/`, format `spike11-trace-v1`) replayed through
//! the product's mouse look and free camera. The final pose is an independent reference written here: the yaw is the sum of the counts
//! times the sensitivity, the pitch the same sum clamped frame by frame. The recorded `final` line of a trace is not the reference: it
//! belongs to the spike's own pose function and keys, which the product does not share.

use planet_core::{PlanetFixed, Vec3};
use planet_frame::freecam::{free_step, FreeInput, FreePose, MAX_PITCH_RAD};
use planet_frame::mouselook::{MouseLook, SENSITIVITY_RAD_PER_COUNT};
use planet_frame::ControllerParams;
use std::path::Path;

/// The number after `"key":` in a trace line.
fn num(line: &str, key: &str) -> Option<f64> {
    let at = line.find(&format!("\"{key}\":"))? + key.len() + 3;
    line[at..].split([',', '}']).next()?.trim().parse().ok()
}

fn kind(line: &str) -> &str {
    let at = line.find("\"k\":\"").map_or(0, |i| i + 5);
    line[at..].split('"').next().unwrap_or("")
}

fn trace(name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spikes/mouse-look/results/traces").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).lines().map(String::from).collect()
}

/// The start pose of a trace (its header: latitude, longitude, altitude and radius; the heading does not matter here).
fn start_pose(header: &str) -> FreePose {
    let (lat, lon, alt, radius) =
        (num(header, "lat").unwrap(), num(header, "lon").unwrap(), num(header, "alt").unwrap(), num(header, "radius").unwrap());
    let dir = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    FreePose::new(PlanetFixed(dir * (radius + alt)), Vec3::new(0.0, 0.0, 1.0), 0.0)
}

/// The signed angle (radians, to the right) from heading `a` to heading `b` about the vertical of `pose`.
fn turn_between(pose: &FreePose, a: Vec3, b: Vec3) -> f64 {
    let up = pose.up();
    let right = a.cross(up);
    libm::atan2(b.dot(right), b.dot(a))
}

struct Replay {
    frames: u64,
    consumed_events: u64,
    pose: FreePose,
    /// The independent reference: total yaw to the right and the clamped pitch.
    yaw: f64,
    pitch: f64,
    /// The pose and the reference at the end of each stretch of the trace (before a `reset`, and at the end).
    stretches: Vec<(FreePose, f64, f64)>,
}

/// Replay a trace. `click_at_start`: the trace has no button event of its own (the spike captured by a key), so the replay captures
/// first. A `button` event with `down` captures; a `reset` event puts the pose back.
fn replay(lines: &[String], click_at_start: bool) -> Replay {
    let start = start_pose(&lines[0]);
    let params = ControllerParams::default();
    let mut mouse = MouseLook::new();
    if click_at_start {
        mouse.click(true);
    }
    let mut r = Replay { frames: 0, consumed_events: 0, pose: start, yaw: 0.0, pitch: 0.0, stretches: Vec::new() };
    let mut counts = [0.0f64; 2];
    for line in lines {
        match kind(line) {
            "button" if line.contains("\"down\":true") => mouse.click(true),
            "reset" => {
                r.stretches.push((r.pose, r.yaw, r.pitch));
                r.pose = start;
                r.yaw = 0.0;
                r.pitch = 0.0;
                mouse.take_delta();
                counts = [0.0; 2];
            }
            "raw" => {
                let (dx, dy) = (num(line, "dx").unwrap(), num(line, "dy").unwrap());
                if mouse.captured() {
                    r.consumed_events += 1;
                    counts[0] += dx;
                    counts[1] += dy;
                }
                mouse.motion(dx, dy);
            }
            "frame" => {
                let dt = num(line, "dt_us").unwrap() * 1e-6;
                let input = FreeInput { look_delta_rad: mouse.take_delta(), ..Default::default() };
                r.pose = free_step(r.pose, &input, dt, 1.7, &params);
                r.frames += 1;
                // The reference: counts x sensitivity once per frame; pitch clamped per frame like the camera's.
                r.yaw += counts[0] * SENSITIVITY_RAD_PER_COUNT;
                r.pitch = (r.pitch - counts[1] * SENSITIVITY_RAD_PER_COUNT).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);
                counts = [0.0; 2];
            }
            _ => {}
        }
    }
    r.stretches.push((r.pose, r.yaw, r.pitch));
    r
}

/// Every stretch of the trace ended where the reference says; returns the largest total yaw of a stretch.
fn assert_final_poses(r: &Replay, start: &FreePose) -> f64 {
    let mut largest = 0.0f64;
    for (pose, yaw, pitch) in &r.stretches {
        // The position never moves (no movement keys): the camera turned in place.
        assert!((pose.position.0 - start.position.0).length() < 1e-9);
        assert!((pose.pitch - pitch).abs() < 1e-9, "pitch {} against the reference {pitch}", pose.pitch);
        // The heading turned by the reference yaw, modulo a full turn.
        let turned = turn_between(start, start.heading, pose.heading);
        let expected = yaw.rem_euclid(std::f64::consts::TAU);
        let got = turned.rem_euclid(std::f64::consts::TAU);
        let diff = (got - expected).abs().min(std::f64::consts::TAU - (got - expected).abs());
        assert!(diff < 1e-6, "heading turned by {got} rad, the reference says {expected} rad (total yaw {yaw})");
        largest = largest.max(yaw.abs());
    }
    largest
}

// spec: VIEW-010
#[test]
fn the_raw_source_trace_replays_to_the_reference_pose() {
    let lines = trace("raw-source.jsonl");
    let start = start_pose(&lines[0]);
    let r = replay(&lines, true);
    assert_eq!(r.frames, 2347, "every recorded frame was replayed");
    assert!(r.consumed_events > 1000, "the mouse events of the session were used: {}", r.consumed_events);
    assert!(r.stretches.len() >= 2, "the trace has resets: {} stretches", r.stretches.len());
    let largest = assert_final_poses(&r, &start);
    assert!(largest > 1.0, "a stretch of the session turned the view by more than a radian: {largest}");
}

// spec: VIEW-010
#[test]
fn the_cursor_source_trace_only_counts_the_raw_events_after_its_click() {
    let lines = trace("cursor-source.jsonl");
    let start = start_pose(&lines[0]);
    let before_click: u64 = lines.iter().take_while(|l| kind(l) != "button").filter(|l| kind(l) == "raw").count() as u64;
    let total_raw = lines.iter().filter(|l| kind(l) == "raw").count() as u64;
    let r = replay(&lines, false);
    // The raw events before the first click are dropped (the pointer is free), the ones after it are used.
    assert!(before_click > 0 && before_click < total_raw, "the trace has raw events both before and after its click");
    assert!(r.consumed_events <= total_raw - before_click, "{} consumed, {} after the click", r.consumed_events, total_raw - before_click);
    assert!(r.consumed_events > 0);
    assert_final_poses(&r, &start);
}
