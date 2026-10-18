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
    /// M5 sky and atmosphere views (ADR 0037 names them as later M5 classes).
    Sky,
    /// M5 ocean and shoreline views (ADR 0037).
    Water,
}

/// The M5 window of ADR 0037 (the first golden of a lit, sky or water view blessed under policy) closed at the M5 gate (ADR 0061, owner,
/// 2026-10-18): the list is empty, so the first golden of any class escalates to the owner's `/bless` (ADR 0017). The look path of
/// ADR 0037 point 2 (AUTO-L, TEST-036) stays and names its classes in [`LOOK_CLASSES`].
pub const ADR_0037_M5_CLASSES: [GoldenClass; 0] = [];

/// The classes whose pictures a look parameter changes (ADR 0037 point 2) and that ADR 0017 never auto-blesses by the rounding or
/// declared-change rules: lit views and the sky and water views of M5.
pub const LOOK_CLASSES: [GoldenClass; 3] = [GoldenClass::Lit, GoldenClass::Sky, GoldenClass::Water];

impl GoldenClass {
    pub fn parse(s: &str) -> Option<GoldenClass> {
        match s {
            "debug" => Some(GoldenClass::Debug),
            "shaded" => Some(GoldenClass::Shaded),
            "lit" => Some(GoldenClass::Lit),
            "sky" => Some(GoldenClass::Sky),
            "water" => Some(GoldenClass::Water),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            GoldenClass::Debug => "debug",
            GoldenClass::Shaded => "shaded",
            GoldenClass::Lit => "lit",
            GoldenClass::Sky => "sky",
            GoldenClass::Water => "water",
        }
    }

    /// Is this a class a look parameter changes ([`LOOK_CLASSES`])?
    pub fn is_look_class(self) -> bool {
        LOOK_CLASSES.contains(&self)
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
    /// A declared change of a debug or shaded view, judged on this adapter's candidate against its own golden (ADR 0061).
    AutoExplained,
    /// A declared look change after the M5 review (ADR 0037 point 2) with every condition met; carries the evidence line.
    AutoLook(String),
    Escalate(String),
}

pub const ROUNDING_MAX_PIXELS: usize = 32;
pub const ROUNDING_MAX_DELTA: u8 = 1;

/// Classify the candidate of `name` on `adapter` against that adapter's own golden (ADR 0017 as amended by ADR 0061: no agreement
/// between adapters is needed or checked), given what every adapter that has a golden of this name shows (the look path reads it), the
/// golden's class and whether the active change declared it under `Expected golden changes:`. A first golden of any class escalates.
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
        Some(c) if c.is_look_class() => return Verdict::Escalate(format!("{name} is a {} view", c.as_str())),
        Some(_) => {}
    }
    if here.pixels <= ROUNDING_MAX_PIXELS && here.max_delta <= ROUNDING_MAX_DELTA {
        return Verdict::AutoRounding;
    }
    if !declared {
        return Verdict::Escalate(format!("{name} is not listed under 'Expected golden changes:' in the change's proposal"));
    }
    // Every adapter is judged on its own candidate against its own golden (ADR 0061): hardware and software rasterisers differ at
    // edges and in rounding by design, so the changed-pixel counts of the adapters are not compared.
    Verdict::AutoExplained
}

/// What the nextest JUnit reports of this run say: `(classname, test name, green)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestOutcomes(pub Vec<(String, String, bool)>);

impl TestOutcomes {
    /// Add the testcases of one JUnit report. A testcase is green only without a failure, error, skip or flaky-retry child.
    pub fn add_junit(&mut self, junit: &str) {
        fn attr<'a>(line: &'a str, key: &str) -> Option<&'a str> {
            let at = line.find(&format!(" {key}=\""))? + key.len() + 3;
            Some(&line[at..at + line[at..].find('"')?])
        }
        let unescape = |s: &str| s.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
        let mut open: Option<usize> = None;
        for line in junit.lines().map(str::trim_start) {
            if line.starts_with("<testcase ") {
                let (Some(class), Some(name)) = (attr(line, "classname"), attr(line, "name")) else { continue };
                self.0.push((unescape(class), unescape(name), true));
                open = (!line.trim_end().ends_with("/>")).then(|| self.0.len() - 1);
            } else if line.starts_with("</testcase") {
                open = None;
            } else if let Some(i) = open {
                let bad = ["<failure", "<error", "<skipped", "<flakyFailure", "<flakyError", "<rerunFailure", "<rerunError"];
                if bad.iter().any(|p| line.starts_with(p)) {
                    self.0[i].2 = false;
                }
            }
        }
    }

    /// Did all the named tests run in this run, and is every test of that name green? `Err` names the first one that is
    /// missing or not green.
    pub fn all_green(&self, wanted: &[String]) -> Result<(), String> {
        for w in wanted {
            let hits: Vec<_> = self
                .0
                .iter()
                .filter(|(c, n, _)| {
                    // A bare name, or the name qualified by its binary (`gpu_water::name`) or by crate and binary.
                    let full = format!("{c}::{n}");
                    n == w || full == *w || full.ends_with(&format!("::{w}"))
                })
                .collect();
            if hits.is_empty() {
                return Err(format!("bound test '{w}' did not run in this run (it is in no --tests report)"));
            }
            if let Some((c, n, _)) = hits.iter().find(|h| !h.2) {
                return Err(format!("bound test '{w}' is not green in this run ({c} {n})"));
            }
        }
        Ok(())
    }
}

/// One entry under `Look changes:` in a proposal (ADR 0037 point 2): a look parameter that changes on purpose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookDeclaration {
    /// The look data file the parameter lives in (a `*.look.toml`, or a file of `tests/goldens/look_data.txt`).
    pub path: String,
    /// The parameter's key, as the file spells it (`shore_filter`, `fresnel_f0`, `max_ev100`).
    pub key: String,
    pub old: String,
    pub new: String,
    /// The golden classes the parameter can change: `lit`, `sky`, `water`.
    pub affects: Vec<GoldenClass>,
}

/// The `Look changes:` section of a proposal: `- <path>: <key> = <old> -> <new>; affects=<class>[, <class>]` per line, until a
/// blank line or heading. A line missing a part, an unknown class, or an `affects=` naming a class that is not a look class does
/// not count.
pub fn look_declarations(proposal: &str) -> Vec<LookDeclaration> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in proposal.lines() {
        let t = line.trim();
        if t.trim_start_matches('#').trim().eq_ignore_ascii_case("look changes:") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            break;
        }
        let Some((path, rest)) = t.trim_start_matches(['-', '*']).trim().split_once(": ") else { continue };
        let Some((change, affects)) = rest.split_once(';') else { continue };
        let Some(("affects", classes)) = affects.trim().split_once('=') else { continue };
        let classes: Vec<Option<GoldenClass>> =
            classes.split(',').map(|c| GoldenClass::parse(c.trim()).filter(|c| c.is_look_class())).collect();
        if classes.is_empty() || classes.iter().any(Option::is_none) {
            continue;
        }
        let Some((key, values)) = change.split_once('=') else { continue };
        let Some((old, new)) = values.split_once("->") else { continue };
        let strip = |v: &str| v.trim().trim_matches('`').to_string();
        let (key, old, new) = (strip(key), strip(old), strip(new));
        if key.is_empty() || old.is_empty() || new.is_empty() || old == new {
            continue;
        }
        out.push(LookDeclaration { path: strip(path), key, old, new, affects: classes.into_iter().flatten().collect() });
    }
    out
}

/// Does `path` match one pattern of `tests/goldens/look_data.txt`: an exact path, or `*suffix` for a file whose name ends so?
pub fn is_look_data(path: &str, patterns: &[String]) -> bool {
    let path = path.replace('\\', "/");
    patterns.iter().any(|p| match p.strip_prefix('*') {
        Some(suffix) => path.ends_with(suffix),
        None => path == *p,
    })
}

/// Paths that cannot change a pixel: the docs, the specs and changes, the goldens themselves and the review output.
fn is_inert(path: &str) -> bool {
    ["docs/", "openspec/", "tests/goldens/", "target/"].iter().any(|p| path.starts_with(p))
}

/// ADR 0037 point 2, condition 1: the PR changes look data and nothing that can move a pixel by itself. `changed` is the PR's
/// file list, `diffs` the zero-context unified diff of each look data file (`git diff -U0`), `patterns` the look data files.
/// Every changed file must be inert or look data; in a look data file every added line must carry a declared key and its new value
/// and every removed line a declared key and its old value; every declaration must be found in the diff of its file.
pub fn look_diff_ok(
    changed: &[String],
    diffs: &BTreeMap<String, String>,
    patterns: &[String],
    decls: &[LookDeclaration],
) -> Result<(), String> {
    if decls.is_empty() {
        return Err("no `Look changes:` declaration in the proposal".into());
    }
    for f in changed {
        let f = f.replace('\\', "/");
        if !is_inert(&f) && !is_look_data(&f, patterns) {
            return Err(format!(
                "{f} is not look data (shader and Rust code, or any file that can move a pixel by itself, is the owner's)"
            ));
        }
    }
    // (a removed line with the old value, an added line with the new value) found per declaration
    let mut found = vec![(false, false); decls.len()];
    for (file, diff) in diffs {
        let mine: Vec<(usize, &LookDeclaration)> = decls.iter().enumerate().filter(|(_, d)| d.path == *file).collect();
        for line in diff.lines() {
            let (sign, text) = match line.chars().next() {
                Some(c @ ('+' | '-')) if !line.starts_with("+++") && !line.starts_with("---") => (c, &line[1..]),
                _ => continue,
            };
            // Exactly `key = value` (a trailing comment and comma aside): a longer key, a longer value or a second change on the
            // line is not the declared change.
            let (k, v) = text.split('#').next().unwrap_or("").split_once('=').map_or(("", ""), |(k, v)| (k.trim(), v.trim()));
            let unquote = |x: &str| x.trim().trim_end_matches(',').trim().trim_matches('"').to_string();
            let v = unquote(v);
            let hit = mine.iter().find(|(_, d)| k == d.key && v == unquote(if sign == '-' { &d.old } else { &d.new }));
            match hit {
                Some((i, _)) => {
                    if sign == '-' {
                        found[*i].0 = true;
                    } else {
                        found[*i].1 = true;
                    }
                }
                None => return Err(format!("{file}: the changed line `{}` is not a declared parameter change", line.trim())),
            }
        }
    }
    if let Some(i) = found.iter().position(|f| !(f.0 && f.1)) {
        let d = &decls[i];
        return Err(format!("the declared change {}: {} {} -> {} is not in the diff", d.path, d.key, d.old, d.new));
    }
    Ok(())
}

/// ADR 0037 point 2, condition 2: no debug view changed beyond ADR 0017's rounding on any adapter (`views` holds, per golden of
/// the debug or shaded classes, what each adapter shows). A look parameter changes lit pictures, never the debug views of the
/// features (CON-18); a debug view that moved means the change was not only a look change.
pub fn debug_views_unchanged(views: &[(String, BTreeMap<String, Observation>)]) -> Result<(), String> {
    for (name, seen) in views {
        for (adapter, obs) in seen {
            let moved = match obs {
                Observation::Changed(c) => c.pixels > ROUNDING_MAX_PIXELS || c.max_delta > ROUNDING_MAX_DELTA,
                Observation::SizeChanged => true,
                Observation::Unchanged | Observation::NoGolden => false,
            };
            if moved {
                return Err(format!("the debug view {name} changed on {adapter} beyond rounding"));
            }
        }
    }
    Ok(())
}

/// The evidence for ADR 0037 point 2, gathered once per run.
#[derive(Clone, Debug)]
pub struct LookEvidence {
    pub declarations: Vec<LookDeclaration>,
    /// Condition 1: [`look_diff_ok`].
    pub diff: Result<(), String>,
    /// Condition 2: [`debug_views_unchanged`].
    pub debug_views: Result<(), String>,
}

/// ADR 0037 point 2: classify the candidate of an existing lit, sky or water golden when the change declares look parameter changes.
/// `None` when the path does not apply (nothing declared, not an M5 class, or the adapter has no changed candidate): then
/// [`classify`] decides as before. Otherwise blesses only when all hold: the PR changes only look data (condition 1), the
/// debug views are unchanged (2), the golden is declared under `Expected golden changes:` and in a class the declared
/// parameters affect (3), and every adapter that has the golden shows a change (a look change moves the picture everywhere), at
/// least two. Anything else escalates with the condition that failed. The driver gate is applied by the caller before this.
pub fn classify_look_change(
    adapter: &str,
    name: &str,
    seen: &BTreeMap<String, Observation>,
    class: Option<GoldenClass>,
    declared: bool,
    look: &LookEvidence,
) -> Option<Verdict> {
    let class = class.filter(|c| c.is_look_class())?;
    if look.declarations.is_empty() || !matches!(seen.get(adapter), Some(Observation::Changed(_))) {
        return None;
    }
    let esc = |why: String| Some(Verdict::Escalate(format!("{name}: {why} (ADR 0037 point 2)")));
    if let Err(why) = &look.diff {
        return esc(format!("the change is not only look data: {why}"));
    }
    if let Err(why) = &look.debug_views {
        return esc(format!("a debug view moved: {why}"));
    }
    if !declared {
        return esc("not listed under 'Expected golden changes:' in the change's proposal".into());
    }
    if !look.declarations.iter().any(|d| d.affects.contains(&class)) {
        let named: Vec<&str> = look.declarations.iter().flat_map(|d| d.affects.iter().map(|c| c.as_str())).collect();
        return esc(format!("a {} golden is in no class the declared parameters affect ({})", class.as_str(), named.join(", ")));
    }
    if let Some((other, obs)) = seen.iter().find(|(_, o)| !matches!(o, Observation::Changed(_))) {
        let what = match obs {
            Observation::Unchanged => "does not change",
            Observation::NoGolden => "has no golden",
            _ => "changed size",
        };
        return esc(format!("{other} {what}: a look change moves the picture on every adapter"));
    }
    if seen.len() < 2 {
        return esc("only one adapter has a candidate; cross-adapter agreement is unchecked".into());
    }
    let params: Vec<String> = look.declarations.iter().map(|d| format!("{} {} -> {}", d.key, d.old, d.new)).collect();
    Some(Verdict::AutoLook(format!(
        "declared look change ({}); only look data changed; debug views unchanged; {} golden in an affected class; changed on {} adapters",
        params.join(", "),
        class.as_str(),
        seen.len()
    )))
}

/// A candidate rendered with another driver than the one recorded beside the goldens escalates, whatever its pixels:
/// driver and wgpu upgrades are re-blessed by the owner (ADR 0004, ADR 0009). `None` when the recorded and the candidate
/// driver strings agree (or the candidate has none, e.g. an artefact without its sidecar: then nothing is overwritten).
pub fn driver_gate(adapter: &str, recorded: Option<&str>, candidate: Option<&str>) -> Option<String> {
    match (recorded.map(str::trim), candidate.map(str::trim)) {
        (Some(r), Some(c)) if r != c => Some(format!("driver changed on {adapter}: recorded '{r}', candidate '{c}'")),
        (None, Some(c)) => Some(format!("{adapter} has no recorded driver; candidate driver '{c}'")),
        _ => None,
    }
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
    fn a_declared_debug_change_is_blessed_per_adapter_whatever_the_other_adapters_show() {
        let s = seen(&[("warp", ch(951, 147)), ("nv", ch(938, 148)), ("lavapipe", ch(960, 147))]);
        for a in ["warp", "nv", "lavapipe"] {
            assert_eq!(classify(a, "terrain_ground1m_depth", &s, Some(GoldenClass::Debug), true), Verdict::AutoExplained, "{a}");
        }
        assert_eq!(classify("warp", "x", &s, Some(GoldenClass::Shaded), true), Verdict::AutoExplained);
        // ADR 0061: a declared change that differs a lot across adapters (counts, deltas), that did not change on another adapter, that
        // changed its size there, or that only one adapter has a candidate for, is blessed on each adapter that shows it, debug or shaded.
        let apart = seen(&[("warp", ch(951, 147)), ("lavapipe", ch(40, 147)), ("nv", ch(5000, 3))]);
        let others = seen(&[("warp", ch(951, 147)), ("nv", Observation::Unchanged), ("lavapipe", Observation::SizeChanged)]);
        let alone = seen(&[("warp", ch(951, 147))]);
        for (what, s) in [("apart", &apart), ("others", &others), ("alone", &alone)] {
            for class in [GoldenClass::Debug, GoldenClass::Shaded] {
                assert_eq!(classify("warp", "terrain_ground1m_depth", s, Some(class), true), Verdict::AutoExplained, "{what} {class:?}");
            }
        }
        assert_eq!(classify("lavapipe", "terrain_ground1m_depth", &apart, Some(GoldenClass::Debug), true), Verdict::AutoExplained);
        // ...but still only when declared.
        assert!(matches!(classify("lavapipe", "terrain_ground1m_depth", &apart, Some(GoldenClass::Shaded), false), Verdict::Escalate(_)));
        assert!(matches!(classify("lavapipe", "terrain_ground1m_depth", &apart, Some(GoldenClass::Debug), false), Verdict::Escalate(_)));
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
        assert!(reason(classify("warp", "g", &seen(&[("warp", Observation::NoGolden)]), Some(GoldenClass::Debug), true))
            .contains("first golden"));
        assert!(
            reason(classify("warp", "g", &seen(&[("warp", Observation::SizeChanged)]), Some(GoldenClass::Debug), true)).contains("size")
        );
    }

    // spec: TEST-016
    #[test]
    fn a_driver_change_escalates_whatever_the_pixels() {
        assert_eq!(driver_gate("warp", Some("WARP 10.0\n"), Some("WARP 10.0")), None);
        assert_eq!(driver_gate("warp", Some("WARP 10.0"), None), None);
        assert!(driver_gate("lavapipe", Some("Mesa 25.2.8"), Some("Mesa 25.3.0")).unwrap().contains("driver changed on lavapipe"));
        assert!(driver_gate("nv", None, Some("581.57")).is_some());
    }

    // spec: TEST-016
    #[test]
    fn the_owner_decisions_on_pr_43_are_reproduced() {
        // Changed pixels and max deltas of PR #43's candidates (LOD-008), which the owner approved on 2026-10-11.
        let (w, l, n) = ("dx12-microsoft-basic-render-driver", "vulkan-llvmpipe", "vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu");
        type Case = (&'static str, GoldenClass, [(usize, u8); 3]);
        let cases: [Case; 5] = [
            ("terrain_aerial20km_level", GoldenClass::Debug, [(665, 97), (666, 97), (665, 97)]),
            ("terrain_aerial200km_height", GoldenClass::Shaded, [(7, 2), (7, 1), (12, 1)]),
            ("terrain_ground1m_depth", GoldenClass::Shaded, [(951, 147), (7, 147), (938, 148)]),
            ("terrain_low2km_morph", GoldenClass::Debug, [(1655, 76), (1656, 76), (1657, 76)]),
            ("terrain_cube_corner_normals", GoldenClass::Debug, [(4, 1), (4, 1), (8, 1)]),
        ];
        let (mut with, mut without) = (0, 0);
        for (name, class, px) in cases {
            let s = seen(&[(w, ch(px[0].0, px[0].1)), (l, ch(px[1].0, px[1].1)), (n, ch(px[2].0, px[2].1))]);
            for a in [w, l, n] {
                let declared = name != "terrain_cube_corner_normals";
                assert!(
                    !matches!(classify(a, name, &s, Some(class), declared), Verdict::Escalate(_)),
                    "{a} {name} must auto-bless with the declarations"
                );
                with += 1;
                if matches!(classify(a, name, &s, Some(class), false), Verdict::Escalate(_)) {
                    without += 1;
                }
            }
        }
        assert_eq!((with, without), (15, 10), "15 auto with the declarations; every real change escalates without them");
    }

    fn first(adapters: &[&str]) -> BTreeMap<String, Observation> {
        adapters.iter().map(|a| (a.to_string(), Observation::NoGolden)).collect()
    }

    // spec: TEST-022
    #[test]
    fn a_first_golden_of_any_class_escalates_to_the_owner_and_the_m5_window_is_closed() {
        // ADR 0061: the window of ADR 0037 point 1 is empty; whatever the declaration, the first golden is the owner's (ADR 0017).
        assert!(ADR_0037_M5_CLASSES.is_empty());
        for class in [GoldenClass::Debug, GoldenClass::Shaded, GoldenClass::Lit, GoldenClass::Sky, GoldenClass::Water] {
            for declared in [true, false] {
                for adapters in [&["warp"][..], &["warp", "nv"][..], &["warp", "nv", "lavapipe"][..]] {
                    let v = classify("warp", "new_view", &first(adapters), Some(class), declared);
                    assert!(matches!(&v, Verdict::Escalate(r) if r.contains("first golden")), "{class:?} {declared} {adapters:?}: {v:?}");
                }
            }
        }
        // A golden that exists on another adapter but not on this one is a first golden here too.
        let family = seen(&[("warp", Observation::NoGolden), ("nv", Observation::Unchanged)]);
        assert!(matches!(classify("warp", "v", &family, Some(GoldenClass::Debug), true), Verdict::Escalate(_)));
        // Existing lit, sky and water goldens keep escalating (ADR 0017); they are the look classes of ADR 0037 point 2.
        let changed = seen(&[("warp", ch(500, 40)), ("nv", ch(500, 40))]);
        for class in LOOK_CLASSES {
            assert!(matches!(classify("warp", "lit_x", &changed, Some(class), true), Verdict::Escalate(r) if r.contains(class.as_str())));
            assert!(class.is_look_class());
        }
        assert!(!GoldenClass::Debug.is_look_class() && !GoldenClass::Shaded.is_look_class());
    }

    // spec: TEST-022
    #[test]
    fn the_golden_path_keeps_adr_0017_rounding_and_declaration_thresholds_per_adapter() {
        let d = Some(GoldenClass::Debug);
        assert_eq!(classify("warp", "g", &seen(&[("warp", ch(32, 1))]), d, false), Verdict::AutoRounding);
        assert!(matches!(classify("warp", "g", &seen(&[("warp", ch(64, 1))]), d, false), Verdict::Escalate(_)));
        assert!(matches!(classify("warp", "g", &seen(&[("warp", ch(33, 1))]), d, false), Verdict::Escalate(_)));
        assert!(matches!(classify("warp", "g", &seen(&[("warp", ch(3, 2))]), d, false), Verdict::Escalate(_)));
        // The count of one adapter does not matter to another: 951 px here, 902 px there (49 apart, past the old 5 % / 8 px band).
        let apart = seen(&[("warp", ch(951, 147)), ("nv", ch(951 - 49, 147))]);
        assert_eq!(classify("warp", "g", &apart, d, true), Verdict::AutoExplained);
        assert_eq!(classify("nv", "g", &apart, d, true), Verdict::AutoExplained);
    }

    // spec: TEST-022
    #[test]
    fn bound_tests_count_only_when_this_runs_reports_show_them_green() {
        let mut out = TestOutcomes::default();
        out.add_junit(
            r#"<testsuites><testsuite name="planet-render">
    <testcase name="tone_ok" classname="planet-render::oracle_lit" timestamp="t" time="0.1"/>
    <testcase name="tone_red" classname="planet-render::oracle_lit" timestamp="t" time="0.1">
        <failure message="boom"/>
    </testcase>
    <testcase name="tone_skipped" classname="planet-render::oracle_lit" timestamp="t" time="0.0">
        <skipped/>
    </testcase>
    <testcase name="tone_ok_after_retry" classname="planet-render::oracle_lit" timestamp="t" time="0.1">
        <flakyFailure message="x"/>
    </testcase>
    <testcase name="last" classname="planet-frame::look" timestamp="t" time="0.1"/>
</testsuite></testsuites>"#,
        );
        let w = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(out.all_green(&w(&["tone_ok", "planet-frame::look::last", "last"])).is_ok());
        assert!(out.all_green(&w(&["planet-render::oracle_lit::tone_ok"])).is_ok());
        // A module-qualified name (binary without its crate) matches, but only on a path boundary.
        assert!(out.all_green(&w(&["oracle_lit::tone_ok"])).is_ok());
        assert!(out.all_green(&w(&["oracle_lit::tone_red"])).unwrap_err().contains("not green"));
        assert!(out.all_green(&w(&["racle_lit::tone_ok"])).unwrap_err().contains("did not run"));
        assert!(out.all_green(&w(&["other::tone_ok"])).unwrap_err().contains("did not run"));
        assert!(out.all_green(&w(&["tone_ok", "tone_red"])).unwrap_err().contains("not green"));
        assert!(out.all_green(&w(&["tone_skipped"])).unwrap_err().contains("not green"));
        assert!(out.all_green(&w(&["tone_ok_after_retry"])).unwrap_err().contains("not green"));
        assert!(out.all_green(&w(&["never_ran"])).unwrap_err().contains("did not run"));
        assert!(TestOutcomes::default().all_green(&w(&["tone_ok"])).is_err(), "no report is no evidence");
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
    fn decl(key: &str, old: &str, new: &str, classes: &[GoldenClass]) -> LookDeclaration {
        LookDeclaration {
            path: "worlds/earth.look.toml".into(),
            key: key.into(),
            old: old.into(),
            new: new.into(),
            affects: classes.to_vec(),
        }
    }
    fn look(diff: Result<(), String>, debug_views: Result<(), String>, classes: &[GoldenClass]) -> LookEvidence {
        LookEvidence { declarations: vec![decl("shore_filter", "1", "0", classes)], diff, debug_views }
    }
    fn all_changed() -> BTreeMap<String, Observation> {
        seen(&[("warp", ch(5000, 40)), ("lavapipe", ch(5100, 40)), ("nv", ch(4900, 41))])
    }

    // spec: TEST-036
    #[test]
    fn a_pure_look_change_blesses_the_changed_lit_golden_on_every_adapter() {
        let ev = look(Ok(()), Ok(()), &[GoldenClass::Lit, GoldenClass::Water]);
        for a in ["warp", "lavapipe", "nv"] {
            let v = classify_look_change(a, "lit_orbit_ev15", &all_changed(), Some(GoldenClass::Lit), true, &ev);
            match v {
                Some(Verdict::AutoLook(e)) => assert!(e.contains("shore_filter 1 -> 0") && e.contains("3 adapters"), "{e}"),
                other => panic!("{a}: {other:?}"),
            }
        }
        assert!(matches!(
            classify_look_change("warp", "water_class_orbit", &all_changed(), Some(GoldenClass::Water), true, &ev),
            Some(Verdict::AutoLook(_))
        ));
    }

    // spec: TEST-036
    #[test]
    fn each_failed_condition_escalates_with_its_reason() {
        let why = |v: Option<Verdict>| match v {
            Some(Verdict::Escalate(r)) => r,
            other => panic!("expected an escalation, got {other:?}"),
        };
        let seen3 = all_changed();
        let lit = Some(GoldenClass::Lit);
        // A look change plus a code change.
        let ev = look(Err("crates/render/src/lit.rs is not look data".into()), Ok(()), &[GoldenClass::Lit]);
        assert!(why(classify_look_change("warp", "g", &seen3, lit, true, &ev)).contains("not only look data"));
        // A debug view moved.
        let ev = look(Ok(()), Err("the debug view terrain_orbit_tile_id changed on warp beyond rounding".into()), &[GoldenClass::Lit]);
        assert!(why(classify_look_change("warp", "g", &seen3, lit, true, &ev)).contains("a debug view moved"));
        // A golden outside the classes the parameters affect.
        let ev = look(Ok(()), Ok(()), &[GoldenClass::Water]);
        assert!(
            why(classify_look_change("warp", "lit_orbit_ev15", &seen3, lit, true, &ev)).contains("no class the declared parameters affect")
        );
        // Not declared as an expected golden change.
        let ev = look(Ok(()), Ok(()), &[GoldenClass::Lit]);
        assert!(why(classify_look_change("warp", "g", &seen3, lit, false, &ev)).contains("Expected golden changes"));
        // An adapter that does not change, or a single adapter.
        let half = seen(&[("warp", ch(5000, 40)), ("nv", Observation::Unchanged)]);
        assert!(why(classify_look_change("warp", "g", &half, lit, true, &ev)).contains("nv does not change"));
        let alone = seen(&[("warp", ch(5000, 40))]);
        assert!(why(classify_look_change("warp", "g", &alone, lit, true, &ev)).contains("only one adapter"));
        // The path does not apply without a declaration, for a non-M5 class, or for a first golden.
        let none = LookEvidence { declarations: vec![], diff: Ok(()), debug_views: Ok(()) };
        assert!(classify_look_change("warp", "g", &seen3, lit, true, &none).is_none());
        assert!(classify_look_change("warp", "g", &seen3, Some(GoldenClass::Debug), true, &ev).is_none());
        let first = seen(&[("warp", Observation::NoGolden)]);
        assert!(classify_look_change("warp", "g", &first, lit, true, &ev).is_none());
        // Without the look path an existing lit golden still escalates (ADR 0017).
        assert!(matches!(classify("warp", "g", &seen3, lit, true), Verdict::Escalate(_)));
    }

    // spec: TEST-036
    #[test]
    fn the_look_diff_must_touch_only_look_data_and_exactly_the_declared_lines() {
        let patterns = vec!["*.look.toml".to_string(), "crates/frame/src/look/water.rs".to_string()];
        let d = [decl("shore_filter", "1", "0", &[GoldenClass::Water])];
        let diffs = |text: &str| BTreeMap::from([("worlds/earth.look.toml".to_string(), text.to_string())]);
        let good = diffs("--- a/worlds/earth.look.toml\n+++ b/worlds/earth.look.toml\n@@ -3 +3 @@\n-shore_filter = 1\n+shore_filter = 0\n");
        let files = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let ok = look_diff_ok(
            &files(&[
                "worlds/earth.look.toml",
                "tests/goldens/warp/lit_orbit_ev15.png",
                "docs/log.d/x.md",
                "openspec/changes/c/proposal.md",
            ]),
            &good,
            &patterns,
            &d,
        );
        assert_eq!(ok, Ok(()));
        // A Rust or shader file in the diff.
        let e = look_diff_ok(&files(&["worlds/earth.look.toml", "crates/render/src/shaders/lit.wgsl"]), &good, &patterns, &d).unwrap_err();
        assert!(e.contains("lit.wgsl") && e.contains("not look data"), "{e}");
        // An extra line in the look file that no declaration names.
        let extra = diffs("-shore_filter = 1\n+shore_filter = 0\n-sun_lux = 1\n+sun_lux = 2\n");
        assert!(look_diff_ok(&files(&["worlds/earth.look.toml"]), &extra, &patterns, &d).unwrap_err().contains("sun_lux"));
        // A longer key, a longer value, a second change hidden in a comment: none of them is the declared line.
        for sneaky in [
            "-shore_filter = 1
+shore_filter = 10
",
            "-shore_filter = 1
+max_shore_filter = 0
",
            "-shore_filter = 1
+sun_lux = 9 # shore_filter = 0
",
        ] {
            assert!(look_diff_ok(&files(&["worlds/earth.look.toml"]), &diffs(sneaky), &patterns, &d).is_err(), "{sneaky}");
        }
        // A trailing comment and comma and quotes do not matter.
        let spaced = diffs(
            "-shore_filter=1 # was
+  shore_filter = 0,
",
        );
        assert_eq!(look_diff_ok(&files(&["worlds/earth.look.toml"]), &spaced, &patterns, &d), Ok(()));
        // The wrong value, and a declaration that is not in the diff.
        let wrong = diffs("-shore_filter = 1\n+shore_filter = 2\n");
        assert!(look_diff_ok(&files(&["worlds/earth.look.toml"]), &wrong, &patterns, &d).is_err());
        let missing = diffs("-shore_filter = 1\n");
        assert!(look_diff_ok(&files(&["worlds/earth.look.toml"]), &missing, &patterns, &d).unwrap_err().contains("not in the diff"));
        assert!(look_diff_ok(&files(&["worlds/earth.look.toml"]), &good, &patterns, &[]).unwrap_err().contains("no `Look changes:`"));
        assert!(is_look_data("a\\b.look.toml", &patterns) && !is_look_data("a/b.toml", &patterns));
    }

    // spec: TEST-036
    #[test]
    fn a_debug_view_that_moved_beyond_rounding_voids_the_look_path() {
        let view = |obs: Observation| vec![("terrain_orbit_tile_id".to_string(), seen(&[("warp", obs)]))];
        assert_eq!(debug_views_unchanged(&view(Observation::Unchanged)), Ok(()));
        assert_eq!(debug_views_unchanged(&view(ch(32, 1))), Ok(()), "ADR 0017 rounding is not a move");
        assert!(debug_views_unchanged(&view(ch(33, 1))).unwrap_err().contains("terrain_orbit_tile_id changed on warp"));
        assert!(debug_views_unchanged(&view(ch(3, 2))).is_err());
        assert!(debug_views_unchanged(&view(Observation::SizeChanged)).is_err());
        assert_eq!(debug_views_unchanged(&[]), Ok(()));
    }

    // spec: TEST-036
    #[test]
    fn the_look_changes_section_parses_strictly() {
        let p = "Why\n\nLook changes:\n- worlds/earth.look.toml: shore_filter = 1 -> 0; affects=water, lit\n- `a.look.toml`: `max_ev100` = 16.0 -> 15.0; affects=sky\n- bad.toml: k = 1 -> 1; affects=lit\n- bad.toml: k = 1 -> 2; affects=debug\n- bad.toml: k = 1 -> 2\n- bad.toml: k 1 -> 2; affects=lit\n\nExpected golden changes:\n- g: x\n";
        let d = look_declarations(p);
        assert_eq!(d.len(), 2, "{d:?}");
        assert_eq!(
            (d[0].path.as_str(), d[0].key.as_str(), d[0].old.as_str(), d[0].new.as_str()),
            ("worlds/earth.look.toml", "shore_filter", "1", "0")
        );
        assert_eq!(d[0].affects, [GoldenClass::Water, GoldenClass::Lit]);
        assert_eq!((d[1].path.as_str(), d[1].key.as_str(), d[1].affects.clone()), ("a.look.toml", "max_ev100", vec![GoldenClass::Sky]));
        assert!(look_declarations("no section here").is_empty());
    }
}
