//! Golden review policy (ADR 0017): classify candidate golden images as auto-blessable or escalated to the owner.
//! Pure decisions over numbers and names, so the policy is tested at tier A; file handling is in the `golden-review` bin.

use std::collections::BTreeMap;

/// Changed pixels and max channel delta of one candidate against its golden on one adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub pixels: usize,
    pub max_delta: u8,
}

/// Golden classes of ADR 0009, read from `tests/goldens/classes.txt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldenClass {
    Debug,
    Shaded,
    Lit,
}

impl GoldenClass {
    pub fn parse(s: &str) -> Option<GoldenClass> {
        match s {
            "debug" => Some(GoldenClass::Debug),
            "shaded" => Some(GoldenClass::Shaded),
            "lit" => Some(GoldenClass::Lit),
            _ => None,
        }
    }
}

/// What a candidate looks like to the policy on one adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// The adapter has a golden of this name and a candidate of the same size.
    Changed(Change),
    /// The adapter has a golden of this name and no candidate (its frame still matches).
    Unchanged,
    /// The adapter has no golden of this name (a new test or a new adapter family).
    NoGolden,
    /// The candidate's size differs from the golden's.
    SizeChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Rounding noise on a debug or shaded view: at most [`ROUNDING_MAX_PIXELS`] pixels and a max delta of 1.
    AutoRounding,
    /// A declared, cross-adapter-consistent change of a debug or shaded view.
    AutoExplained,
    Escalate(String),
}

pub const ROUNDING_MAX_PIXELS: usize = 32;
pub const ROUNDING_MAX_DELTA: u8 = 1;

/// Do two numbers agree within +-5 % of the larger, or within `floor`?
fn agree(a: usize, b: usize, floor: usize) -> bool {
    let (lo, hi) = (a.min(b), a.max(b));
    hi - lo <= floor.max(hi / 20)
}

/// Classify the candidate of `name` on `adapter`, given what every adapter that has a golden of this name shows, the
/// golden's class and whether the active change declared it under `Expected golden changes:`.
pub fn classify(adapter: &str, name: &str, seen: &BTreeMap<String, Observation>, class: Option<GoldenClass>, declared: bool) -> Verdict {
    let here = match seen.get(adapter) {
        Some(Observation::Changed(c)) => *c,
        Some(Observation::NoGolden) => {
            return Verdict::Escalate(format!("{name} has no golden on {adapter}: a first golden is the owner's"))
        }
        Some(Observation::SizeChanged) => return Verdict::Escalate(format!("{name} changed size on {adapter}")),
        Some(Observation::Unchanged) | None => return Verdict::Escalate(format!("{name}: no candidate on {adapter}")),
    };
    match class {
        None => return Verdict::Escalate(format!("{name} has no class in tests/goldens/classes.txt")),
        Some(GoldenClass::Lit) => return Verdict::Escalate(format!("{name} is a lit view")),
        Some(_) => {}
    }
    if here.pixels <= ROUNDING_MAX_PIXELS && here.max_delta <= ROUNDING_MAX_DELTA {
        return Verdict::AutoRounding;
    }
    if !declared {
        return Verdict::Escalate(format!("{name} is not listed under 'Expected golden changes:' in the change's proposal"));
    }
    // Shaded views (derivative-drawn lines, adapter-specific rasterisation) are exact per adapter only: their changed-pixel
    // counts are not comparable across adapters, so a declared change is enough.
    if class == Some(GoldenClass::Shaded) {
        return Verdict::AutoExplained;
    }
    for (other, obs) in seen {
        match obs {
            Observation::Changed(c) => {
                if !agree(c.pixels, here.pixels, 8) || !agree(usize::from(c.max_delta), usize::from(here.max_delta), 1) {
                    return Verdict::Escalate(format!(
                        "{name}: adapters disagree ({adapter}: {} px, delta {}; {other}: {} px, delta {})",
                        here.pixels, here.max_delta, c.pixels, c.max_delta
                    ));
                }
            }
            Observation::Unchanged => return Verdict::Escalate(format!("{name} changed on {adapter} but not on {other}")),
            Observation::NoGolden | Observation::SizeChanged => {}
        }
    }
    let changed = seen.values().filter(|o| matches!(o, Observation::Changed(_))).count();
    if changed < 2 {
        return Verdict::Escalate(format!("{name}: only one adapter has a candidate yet; cross-adapter agreement is unchecked"));
    }
    Verdict::AutoExplained
}

/// Golden names listed under `Expected golden changes:` in a proposal (`- <name>: <reason>` lines until the next heading
/// or blank line). A line without a reason does not count.
pub fn declared_goldens(proposal: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in proposal.lines() {
        let t = line.trim();
        if t.trim_start_matches('#').trim().eq_ignore_ascii_case("expected golden changes:")
            || t.eq_ignore_ascii_case("expected golden changes:")
        {
            inside = true;
            continue;
        }
        if inside {
            if t.is_empty() || t.starts_with('#') {
                break;
            }
            if let Some((name, reason)) = t.trim_start_matches(['-', '*']).trim().split_once(':') {
                let name = name.trim().trim_matches('`');
                if !name.is_empty() && !reason.trim().is_empty() {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

/// `tests/goldens/classes.txt`: `<golden name> <debug|shaded|lit>` per line, `#` comments allowed.
pub fn parse_classes(text: &str) -> BTreeMap<String, GoldenClass> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.to_string(), GoldenClass::parse(it.next()?)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(v: &[(&str, Observation)]) -> BTreeMap<String, Observation> {
        v.iter().map(|(a, o)| (a.to_string(), o.clone())).collect()
    }
    fn ch(pixels: usize, max_delta: u8) -> Observation {
        Observation::Changed(Change { pixels, max_delta })
    }

    // spec: TEST-016
    #[test]
    fn rounding_noise_is_auto_blessed_on_debug_and_shaded_views_without_a_declaration() {
        let s = seen(&[("warp", ch(7, 1)), ("nv", ch(400, 9))]);
        assert_eq!(classify("warp", "g", &s, Some(GoldenClass::Debug), false), Verdict::AutoRounding);
        assert_eq!(classify("warp", "g", &s, Some(GoldenClass::Shaded), false), Verdict::AutoRounding);
        assert!(matches!(classify("warp", "g", &s, Some(GoldenClass::Lit), false), Verdict::Escalate(_)), "lit views always escalate");
        assert!(matches!(classify("warp", "g", &s, None, false), Verdict::Escalate(_)), "an unclassified golden escalates");
        assert!(matches!(classify("warp", "g", &seen(&[("warp", ch(33, 1))]), Some(GoldenClass::Debug), false), Verdict::Escalate(_)));
        assert!(matches!(classify("warp", "g", &seen(&[("warp", ch(3, 2))]), Some(GoldenClass::Debug), false), Verdict::Escalate(_)));
    }

    // spec: TEST-016
    #[test]
    fn a_declared_consistent_debug_change_is_auto_blessed() {
        let s = seen(&[("warp", ch(951, 147)), ("nv", ch(938, 148)), ("lavapipe", ch(960, 147))]);
        for a in ["warp", "nv", "lavapipe"] {
            assert_eq!(classify(a, "terrain_ground1m_depth", &s, Some(GoldenClass::Debug), true), Verdict::AutoExplained, "{a}");
        }
        assert_eq!(classify("warp", "x", &s, Some(GoldenClass::Shaded), true), Verdict::AutoExplained);
        // A shaded view skips the cross-adapter check (owner, 2026-10-11: contour lines), but still needs a declaration.
        let apart = seen(&[("warp", ch(951, 147)), ("lavapipe", ch(40, 147))]);
        assert_eq!(classify("lavapipe", "terrain_ground1m_depth", &apart, Some(GoldenClass::Shaded), true), Verdict::AutoExplained);
        assert!(matches!(classify("lavapipe", "terrain_ground1m_depth", &apart, Some(GoldenClass::Shaded), false), Verdict::Escalate(_)));
        assert!(matches!(classify("lavapipe", "terrain_ground1m_depth", &apart, Some(GoldenClass::Debug), true), Verdict::Escalate(_)));
    }

    // spec: TEST-016
    #[test]
    fn everything_else_escalates_with_a_reason() {
        let ok = seen(&[("warp", ch(951, 147)), ("nv", ch(938, 148))]);
        let reason = |v: Verdict| match v {
            Verdict::Escalate(r) => r,
            other => panic!("expected an escalation, got {other:?}"),
        };
        assert!(reason(classify("warp", "g", &ok, Some(GoldenClass::Debug), false)).contains("not listed"));
        assert!(reason(classify("warp", "g", &ok, Some(GoldenClass::Lit), true)).contains("lit"));
        assert!(reason(classify("warp", "g", &ok, None, true)).contains("classes.txt"));
        let disagree = seen(&[("warp", ch(951, 147)), ("nv", ch(700, 148))]);
        assert!(reason(classify("warp", "g", &disagree, Some(GoldenClass::Debug), true)).contains("disagree"));
        let half = seen(&[("warp", ch(951, 147)), ("nv", Observation::Unchanged)]);
        assert!(reason(classify("warp", "g", &half, Some(GoldenClass::Debug), true)).contains("not on nv"));
        let alone = seen(&[("warp", ch(951, 147))]);
        assert!(reason(classify("warp", "g", &alone, Some(GoldenClass::Debug), true)).contains("only one adapter"));
        assert!(reason(classify("warp", "g", &seen(&[("warp", Observation::NoGolden)]), Some(GoldenClass::Debug), true))
            .contains("first golden"));
        assert!(
            reason(classify("warp", "g", &seen(&[("warp", Observation::SizeChanged)]), Some(GoldenClass::Debug), true)).contains("size")
        );
    }

    // spec: TEST-016
    #[test]
    fn declarations_and_classes_parse_strictly() {
        let p = "## What Changes\n- x\n\nExpected golden changes:\n- terrain_ground1m_depth: finer near tiles (LOD-008)\n- `terrain_low2km_morph`: morph to own position\n- terrain_no_reason:\n\n## Impact\n- terrain_after: nope\n";
        assert_eq!(declared_goldens(p), ["terrain_ground1m_depth", "terrain_low2km_morph"]);
        let c = parse_classes("# comment\nterrain_a debug\nterrain_b lit # later\nbad\nterrain_c weird\n");
        assert_eq!(c.get("terrain_a"), Some(&GoldenClass::Debug));
        assert_eq!(c.get("terrain_b"), Some(&GoldenClass::Lit));
        assert_eq!(c.len(), 2);
    }
}
