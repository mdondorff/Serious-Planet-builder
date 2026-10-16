//! Input-trace format `spike11-trace-v1`: JSON Lines, one object per line, UTF-8, `\n` line ends, numbers in Rust `{:?}` form
//! (shortest round-trip), timestamps `t` in integer microseconds on a monotonic clock that starts at window creation.
//!
//! ```text
//! {"k":"header","fmt":"spike11-trace-v1","sens":0.0015,"source":"auto","box":0.25,"radius":1000.0,"lat":0.3,"lon":0.7,"alt":1.7,"hdg":0.4,"scale":1.5,"w":960.0,"h":540.0,"os":"windows","note":"..."}
//! {"t":1200,"k":"raw","dx":3.0,"dy":-1.0}          DeviceEvent::MouseMotion, device units (counts)
//! {"t":1210,"k":"cursor","x":512.0,"y":270.0}      WindowEvent::CursorMoved, physical pixels in the window
//! {"t":1300,"k":"button","down":true}              left mouse button
//! {"t":1400,"k":"key","code":"KeyW","down":true}   physical key (winit KeyCode name)
//! {"t":1500,"k":"focus","on":false}
//! {"t":1600,"k":"resize","w":960.0,"h":540.0}      physical pixels
//! {"t":1700,"k":"frame","dt_us":16666}             a frame boundary: the accumulated deltas are applied here
//! {"t":1800,"k":"mark","text":"..."}               a checklist step or a remark (ignored by replay)
//! {"t":1900,"k":"note","text":"..."}               what the window did: grab results, recentre failures (ignored by replay)
//! {"k":"final","pos":[x,y,z],"heading":[x,y,z],"pitch":0.1,"total_yaw":1.2}   the live pose at the end, for replay checks
//! ```

use crate::pose::Pose;
use crate::sim::{Config, Ev, Source};

#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Header { cfg: Config, scale: f64, w: f64, h: f64, os: String, note: String },
    Event { t: u64, ev: Ev },
    Mark { t: u64, text: String },
    Note { t: u64, text: String },
    Final { pose: Pose, total_yaw: f64 },
}

fn clean(s: &str) -> String {
    s.replace(['"', '\\', '\n', '\r'], "'")
}

pub fn to_line(l: &Line) -> String {
    match l {
        Line::Header { cfg, scale, w, h, os, note } => format!(
            "{{\"k\":\"header\",\"fmt\":\"spike11-trace-v1\",\"sens\":{:?},\"source\":\"{}\",\"box\":{:?},\"radius\":{:?},\"lat\":{:?},\"lon\":{:?},\"alt\":{:?},\"hdg\":{:?},\"scale\":{:?},\"w\":{:?},\"h\":{:?},\"os\":\"{}\",\"note\":\"{}\"}}",
            cfg.sens, cfg.source.name(), cfg.recenter_box, cfg.radius, cfg.start_lat, cfg.start_lon, cfg.start_alt, cfg.start_heading, scale, w, h, clean(os), clean(note)
        ),
        Line::Event { t, ev } => match ev {
            Ev::Raw { dx, dy } => format!("{{\"t\":{t},\"k\":\"raw\",\"dx\":{dx:?},\"dy\":{dy:?}}}"),
            Ev::Cursor { x, y } => format!("{{\"t\":{t},\"k\":\"cursor\",\"x\":{x:?},\"y\":{y:?}}}"),
            Ev::Button { down } => format!("{{\"t\":{t},\"k\":\"button\",\"down\":{down}}}"),
            Ev::Key { code, down } => format!("{{\"t\":{t},\"k\":\"key\",\"code\":\"{}\",\"down\":{down}}}", clean(code)),
            Ev::Focus { on } => format!("{{\"t\":{t},\"k\":\"focus\",\"on\":{on}}}"),
            Ev::Resize { w, h } => format!("{{\"t\":{t},\"k\":\"resize\",\"w\":{w:?},\"h\":{h:?}}}"),
            Ev::Frame { dt_us } => format!("{{\"t\":{t},\"k\":\"frame\",\"dt_us\":{dt_us}}}"),
            Ev::Set { source, recenter_box, sens } => format!("{{\"t\":{t},\"k\":\"set\",\"source\":\"{}\",\"box\":{recenter_box:?},\"sens\":{sens:?}}}", source.name()),
            Ev::Reset => format!("{{\"t\":{t},\"k\":\"reset\"}}"),
        },
        Line::Mark { t, text } => format!("{{\"t\":{t},\"k\":\"mark\",\"text\":\"{}\"}}", clean(text)),
        Line::Note { t, text } => format!("{{\"t\":{t},\"k\":\"note\",\"text\":\"{}\"}}", clean(text)),
        Line::Final { pose, total_yaw } => format!(
            "{{\"k\":\"final\",\"pos\":[{:?},{:?},{:?}],\"heading\":[{:?},{:?},{:?}],\"pitch\":{:?},\"total_yaw\":{:?}}}",
            pose.pos[0], pose.pos[1], pose.pos[2], pose.heading[0], pose.heading[1], pose.heading[2], pose.pitch, total_yaw
        ),
    }
}

/// The raw text after `"name":` up to the next `,` or `}` (or the closing bracket of an array).
fn raw<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("\"{name}\":");
    let at = line.find(&key)? + key.len();
    let rest = &line[at..];
    if let Some(r) = rest.strip_prefix('"') {
        return r.split('"').next();
    }
    if let Some(r) = rest.strip_prefix('[') {
        return r.split(']').next();
    }
    rest.split([',', '}']).next()
}

fn num(line: &str, name: &str) -> Result<f64, String> {
    raw(line, name).ok_or(format!("missing {name} in {line}"))?.parse().map_err(|e| format!("bad {name} in {line}: {e}"))
}

fn flag(line: &str, name: &str) -> Result<bool, String> {
    match raw(line, name) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err(format!("missing or bad {name} in {line}")),
    }
}

fn vec3(line: &str, name: &str) -> Result<[f64; 3], String> {
    let parts: Vec<f64> = raw(line, name)
        .ok_or(format!("missing {name}"))?
        .split(',')
        .map(|s| s.parse().map_err(|e| format!("bad {name}: {e}")))
        .collect::<Result<_, _>>()?;
    parts.try_into().map_err(|_| format!("{name} needs 3 numbers"))
}

pub fn from_line(line: &str) -> Result<Line, String> {
    let k = raw(line, "k").ok_or(format!("no kind in {line}"))?;
    let t = || num(line, "t").map(|v| v as u64);
    Ok(match k {
        "header" => {
            if raw(line, "fmt") != Some("spike11-trace-v1") {
                return Err("unknown trace format".into());
            }
            Line::Header {
                cfg: Config {
                    sens: num(line, "sens")?,
                    source: Source::parse(raw(line, "source").unwrap_or("auto")),
                    recenter_box: num(line, "box")?,
                    radius: num(line, "radius")?,
                    start_lat: num(line, "lat")?,
                    start_lon: num(line, "lon")?,
                    start_alt: num(line, "alt")?,
                    start_heading: num(line, "hdg")?,
                },
                scale: num(line, "scale")?,
                w: num(line, "w")?,
                h: num(line, "h")?,
                os: raw(line, "os").unwrap_or("").to_string(),
                note: raw(line, "note").unwrap_or("").to_string(),
            }
        }
        "raw" => Line::Event { t: t()?, ev: Ev::Raw { dx: num(line, "dx")?, dy: num(line, "dy")? } },
        "cursor" => Line::Event { t: t()?, ev: Ev::Cursor { x: num(line, "x")?, y: num(line, "y")? } },
        "button" => Line::Event { t: t()?, ev: Ev::Button { down: flag(line, "down")? } },
        "key" => Line::Event { t: t()?, ev: Ev::Key { code: raw(line, "code").ok_or("no code")?.to_string(), down: flag(line, "down")? } },
        "focus" => Line::Event { t: t()?, ev: Ev::Focus { on: flag(line, "on")? } },
        "resize" => Line::Event { t: t()?, ev: Ev::Resize { w: num(line, "w")?, h: num(line, "h")? } },
        "frame" => Line::Event { t: t()?, ev: Ev::Frame { dt_us: num(line, "dt_us")? as u64 } },
        "set" => Line::Event {
            t: t()?,
            ev: Ev::Set { source: Source::parse(raw(line, "source").unwrap_or("auto")), recenter_box: num(line, "box")?, sens: num(line, "sens")? },
        },
        "reset" => Line::Event { t: t()?, ev: Ev::Reset },
        "mark" => Line::Mark { t: t()?, text: raw(line, "text").unwrap_or("").to_string() },
        "note" => Line::Note { t: t()?, text: raw(line, "text").unwrap_or("").to_string() },
        "final" => Line::Final { pose: Pose { pos: vec3(line, "pos")?, heading: vec3(line, "heading")?, pitch: num(line, "pitch")? }, total_yaw: num(line, "total_yaw")? },
        other => return Err(format!("unknown line kind {other}")),
    })
}

/// Replay a trace headlessly through the same simulation the window uses; returns the final pose and total yaw.
pub fn replay(text: &str) -> Result<(Pose, f64, Option<(Pose, f64)>), String> {
    use crate::sim::Sim;
    let mut sim: Option<Sim> = None;
    let mut recorded = None;
    for (n, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        match from_line(l).map_err(|e| format!("line {}: {e}", n + 1))? {
            Line::Header { cfg, w, h, .. } => {
                let mut s = Sim::new(cfg);
                s.apply(0, &Ev::Resize { w, h });
                sim = Some(s);
            }
            Line::Event { t, ev } => {
                sim.as_mut().ok_or("event before the header")?.apply(t, &ev);
            }
            Line::Final { pose, total_yaw } => recorded = Some((pose, total_yaw)),
            Line::Mark { .. } | Line::Note { .. } => {}
        }
    }
    let s = sim.ok_or("no header")?;
    Ok((s.pose, s.total_yaw, recorded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_kind_round_trips() {
        let lines = vec![
            Line::Header { cfg: Config::default(), scale: 1.5, w: 960.0, h: 540.0, os: "windows".into(), note: "a note".into() },
            Line::Event { t: 5, ev: Ev::Raw { dx: 3.0, dy: -1.25 } },
            Line::Event { t: 6, ev: Ev::Cursor { x: 512.5, y: 270.0 } },
            Line::Event { t: 7, ev: Ev::Button { down: true } },
            Line::Event { t: 8, ev: Ev::Key { code: "KeyW".into(), down: false } },
            Line::Event { t: 9, ev: Ev::Focus { on: false } },
            Line::Event { t: 10, ev: Ev::Resize { w: 1280.0, h: 720.0 } },
            Line::Event { t: 11, ev: Ev::Frame { dt_us: 16666 } },
            Line::Event { t: 11, ev: Ev::Set { source: Source::Cursor, recenter_box: 0.5, sens: 0.002 } },
            Line::Event { t: 11, ev: Ev::Reset },
            Line::Mark { t: 12, text: "step 3".into() },
            Line::Note { t: 13, text: "grab Locked failed".into() },
            Line::Final { pose: Pose { pos: [1.0, 2.0, 3.5], heading: [0.1, 0.2, 0.3], pitch: -0.25 }, total_yaw: 1.0 / 3.0 },
        ];
        for l in lines {
            let s = to_line(&l);
            assert_eq!(from_line(&s).unwrap(), l, "{s}");
        }
    }

    #[test]
    fn a_synthetic_trace_replays_to_a_known_pose() {
        // 1000 counts to the right in 10 frames of very different lengths, then 500 counts up: yaw 1.5 rad, pitch 0.75 rad.
        let mut t = String::new();
        let mut push = |l: Line| {
            t.push_str(&to_line(&l));
            t.push('\n');
        };
        push(Line::Header { cfg: Config { source: Source::Raw, ..Config::default() }, scale: 1.0, w: 960.0, h: 540.0, os: "test".into(), note: String::new() });
        push(Line::Event { t: 1, ev: Ev::Button { down: true } });
        for k in 0..10u64 {
            push(Line::Event { t: 10 + k, ev: Ev::Raw { dx: 100.0, dy: 0.0 } });
            push(Line::Event { t: 10 + k, ev: Ev::Frame { dt_us: 1000 * (k + 1) * (k + 1) } });
        }
        push(Line::Event { t: 200, ev: Ev::Raw { dx: 0.0, dy: -500.0 } });
        push(Line::Event { t: 201, ev: Ev::Frame { dt_us: 16_666 } });
        let (pose, yaw, _) = replay(&t).unwrap();
        assert!((yaw - 1.5).abs() < 1e-12, "{yaw}");
        assert!((pose.pitch - 0.75).abs() < 1e-12, "{}", pose.pitch);
        let start = crate::pose::Pose::at(Config::default().start_lat, Config::default().start_lon, 1000.0, 1.7, Config::default().start_heading);
        let turned = (pose.heading_from_north() - start.heading_from_north()).rem_euclid(std::f64::consts::TAU);
        assert!((turned - 1.5).abs() < 1e-9, "{turned}");
    }
}
