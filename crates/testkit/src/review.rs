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

/// The M5 window of ADR 0037 (accepted 2026-10-15, owner policy): the first golden of one of these classes is blessed under
/// policy when [`classify_m5_first`]'s conditions all hold. Every other class, and every golden that already exists, keeps
/// ADR 0017 exactly. The owner revisits this window at the M5 gate ("after the M5 gate the owner decides whether this rule
/// continues"); ending it means emptying this list through a superseding ADR. ADR 0052 (accepted 2026-10-17) amended point
/// 1(b) of this window with [`ADR_0052_EDGE_PIXELS`]; it ends with the window.
pub const ADR_0037_M5_CLASSES: [GoldenClass; 3] = [GoldenClass::Lit, GoldenClass::Sky, GoldenClass::Water];

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

    /// Is this a class of ADR 0037's M5 window?
    pub fn is_m5(self) -> bool {
        ADR_0037_M5_CLASSES.contains(&self)
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
    /// The first golden of an M5 class with every condition of ADR 0037 point 1 met; carries the evidence line.
    AutoM5(String),
    /// A declared look change after the M5 review (ADR 0037 point 2) with every condition met; carries the evidence line.
    AutoLook(String),
    Escalate(String),
}

pub const ROUNDING_MAX_PIXELS: usize = 32;
pub const ROUNDING_MAX_DELTA: u8 = 1;

/// ADR 0052 point 1 (amends ADR 0037 point 1(b)), as written: the candidates of a new debug view of a first M5 golden agree
/// pairwise when every pixel is within [`ROUNDING_MAX_DELTA`] except at most this many pixels, which may differ by any amount
/// (adapters rasterise silhouettes and band edges differently). Any number of pixels may differ at delta 1: hardware and
/// software adapters round floats differently across a whole view, and the bound tests against the f64 references guard the
/// values. The planner's 32-pixel cap on delta-1 pixels (2026-10-17) was withdrawn for exactly that reason; the same-adapter
/// check of a candidate against its golden (ADR 0017, [`ROUNDING_MAX_PIXELS`]) is unchanged. Part of the ADR 0037 window.
pub const ADR_0052_EDGE_PIXELS: usize = 64;

/// How two candidates of the same view differ, for the cross-adapter check of a new debug view (ADR 0052).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewDiff {
    /// Pixels whose largest channel delta is exactly [`ROUNDING_MAX_DELTA`]; reported, not capped (ADR 0052 as written).
    pub at_rounding: usize,
    /// Pixels whose largest channel delta exceeds [`ROUNDING_MAX_DELTA`]; capped by [`ADR_0052_EDGE_PIXELS`].
    pub beyond_rounding: usize,
    pub max_delta: u8,
}

/// How two same-sized candidates of a new debug view differ (ADR 0052): the pixels at delta 1, those beyond delta 1 (largest
/// channel delta, alpha included) and the largest delta. (`Change` is the same count without the split, used for
/// candidates against their goldens.)
pub fn view_diff(a: &crate::Image, b: &crate::Image) -> ViewDiff {
    let mut d = ViewDiff { at_rounding: 0, beyond_rounding: 0, max_delta: 0 };
    for (p, q) in a.pixels().zip(b.pixels()) {
        if p != q {
            let delta = (0..4).map(|c| p[c].abs_diff(q[c])).max().unwrap_or(0);
            d.at_rounding += usize::from(delta <= ROUNDING_MAX_DELTA);
            d.beyond_rounding += usize::from(delta > ROUNDING_MAX_DELTA);
            d.max_delta = d.max_delta.max(delta);
        }
    }
    d
}

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
        Some(c) if c.is_m5() => return Verdict::Escalate(format!("{name} is a {} view", c.as_str())),
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
            Observation::SizeChanged => return Verdict::Escalate(format!("{name} changed size on {other}")),
            Observation::NoGolden => {}
        }
    }
    let changed = seen.values().filter(|o| matches!(o, Observation::Changed(_))).count();
    if changed < 2 {
        return Verdict::Escalate(format!("{name}: only one adapter has a candidate yet; cross-adapter agreement is unchecked"));
    }
    Verdict::AutoExplained
}

/// One golden's entry under `ADR 0037 conditions:` in the change's proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct M5Declaration {
    /// The golden that is the feature's debug view (CON-18); may be the golden itself.
    pub debug_view: String,
    /// The tests that prove the attribute's declared bound (ADR 0035), as `test_name` or `binary::test_name`.
    pub bound_tests: Vec<String>,
}

/// The `ADR 0037 conditions:` section of a proposal: `- <golden>: debug-view=<golden>; bound-tests=<test>, <test>` per line,
/// until a blank line or heading. An entry without both keys (and at least one test) does not count.
pub fn m5_declarations(proposal: &str) -> BTreeMap<String, M5Declaration> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    for line in proposal.lines() {
        let t = line.trim();
        if t.trim_start_matches('#').trim().eq_ignore_ascii_case("adr 0037 conditions:") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            break;
        }
        let Some((name, rest)) = t.trim_start_matches(['-', '*']).trim().split_once(':') else { continue };
        let (mut view, mut tests) = (None, Vec::new());
        for part in rest.split(';') {
            match part.trim().split_once('=') {
                Some(("debug-view", v)) => view = Some(v.trim().trim_matches('`').to_string()),
                Some(("bound-tests", v)) => {
                    tests = v.split(',').map(|x| x.trim().trim_matches('`').to_string()).filter(|x| !x.is_empty()).collect()
                }
                _ => {}
            }
        }
        if let Some(debug_view) = view.filter(|v| !v.is_empty()) {
            if !tests.is_empty() {
                out.insert(name.trim().trim_matches('`').to_string(), M5Declaration { debug_view, bound_tests: tests });
            }
        }
    }
    out
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

/// What a debug view shows on one adapter that took part in this run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewState {
    /// A blessed golden exists and the run left no candidate (the frame still matches).
    Matches,
    /// A blessed golden exists and the candidate differs; `auto` is whether ADR 0017's own rules bless that change.
    Differs { auto: bool },
    /// No golden yet and a candidate (the view is new in this run).
    New,
    /// No golden and no candidate: the view does not exist on this adapter.
    Missing,
}

/// ADR 0037 point 1(b): the feature's debug view exists and agrees across the adapters under ADR 0017's cross-adapter rule.
/// `states` holds only the adapters that took part in this run; `not_run` those that have goldens but did not, which are
/// reported and never counted as agreeing. A view with a blessed golden is checked by ADR 0017's own classification of its
/// candidates (`Matches` or an auto `Differs`); a view that is new is checked candidate against candidate (ADR 0052):
/// in every pair of `new_pairs` (adapter, adapter, difference of their candidates)
/// at most [`ADR_0052_EDGE_PIXELS`] pixels differ beyond [`ROUNDING_MAX_DELTA`], any number at that delta. At least two
/// adapters must take part.
/// `Ok` carries the evidence line for the report.
pub fn debug_view_agreement(
    view: &str,
    states: &BTreeMap<String, ViewState>,
    new_pairs: &[(String, String, ViewDiff)],
    not_run: &[String],
) -> Result<String, String> {
    let note = if not_run.is_empty() { String::new() } else { format!("; NOT RUN, not counted as agreeing: {}", not_run.join(", ")) };
    if let Some((a, _)) = states.iter().find(|(_, s)| **s == ViewState::Missing) {
        return Err(format!("debug view {view} does not exist on {a}{note}"));
    }
    if let Some((a, _)) = states.iter().find(|(_, s)| **s == ViewState::Differs { auto: false }) {
        return Err(format!("debug view {view} has a candidate on {a} that ADR 0017 does not bless (changed without a blessable change, resized, or rendered by another driver){note}"));
    }
    if states.len() < 2 {
        return Err(format!("debug view {view}: {} adapter(s) took part, ADR 0017's cross-adapter rule needs two{note}", states.len()));
    }
    let new = states.values().filter(|s| **s == ViewState::New).count();
    if new != 0 && new != states.len() {
        return Err(format!("debug view {view} is new on some adapters and blessed on others{note}"));
    }
    let apart: Vec<String> = new_pairs
        .iter()
        .filter(|(_, _, c)| c.beyond_rounding > ADR_0052_EDGE_PIXELS)
        .map(|(a, b, c)| {
            format!(
                "{a} vs {b}: {} px beyond delta {ROUNDING_MAX_DELTA} exceed the cap of {ADR_0052_EDGE_PIXELS} (counts: {} beyond, {} at delta {ROUNDING_MAX_DELTA}; max delta {})",
                c.beyond_rounding, c.beyond_rounding, c.at_rounding, c.max_delta
            )
        })
        .collect();
    if !apart.is_empty() {
        return Err(format!(
            "debug view {view}: the adapters disagree ({}; allowed per pair: {ADR_0052_EDGE_PIXELS} px beyond delta {ROUNDING_MAX_DELTA}, any number at delta {ROUNDING_MAX_DELTA}, ADR 0052){note}",
            apart.join("; ")
        ));
    }
    let worst = new_pairs.iter().fold((0, 0, 0), |w, (_, _, c)| (w.0.max(c.beyond_rounding), w.1.max(c.at_rounding), w.2.max(c.max_delta)));
    let kind = if new == 0 { "matches its blessed golden or an auto-blessed change" } else { "candidates agree pairwise" };
    let adapters = states.keys().cloned().collect::<Vec<_>>().join(", ");
    Ok(format!(
        "debug view {view}: {kind} on {adapters} (worst pair {} px beyond delta {ROUNDING_MAX_DELTA} of {ADR_0052_EDGE_PIXELS}, {} px at delta {ROUNDING_MAX_DELTA}, max delta {}){note}",
        worst.0, worst.1, worst.2
    ))
}

/// The evidence for ADR 0037 point 1, gathered by the caller for one golden.
#[derive(Clone, Debug)]
pub struct M5Evidence {
    /// The golden's entry under `ADR 0037 conditions:`, if the proposal has one.
    pub declaration: Option<M5Declaration>,
    /// Point 1(a), second half: the declared bound tests ran green in this run ([`TestOutcomes::all_green`]).
    pub bound_tests: Result<(), String>,
    /// Point 1(b): [`debug_view_agreement`] for the declared debug view.
    pub debug_view: Result<String, String>,
}

/// ADR 0037 point 1: classify the candidate of `name` on `adapter` when it is the first golden of an M5 class. `None` when
/// this path does not apply (not an M5 class, or this adapter already has the golden): [`classify`] then decides exactly as
/// under ADR 0017. Escalates unless (a) the class is an M5 class and the declared bound tests passed in this run, (b) the
/// debug view agrees across the adapters and (c) the proposal declares the golden under `Expected golden changes:`. A golden
/// that exists on another adapter is a new adapter family and escalates as before. The driver gate ([`driver_gate`]) is
/// applied by the caller before this and is not relaxed.
pub fn classify_m5_first(
    adapter: &str,
    name: &str,
    seen: &BTreeMap<String, Observation>,
    class: Option<GoldenClass>,
    declared: bool,
    evidence: &M5Evidence,
) -> Option<Verdict> {
    let class = class.filter(|c| c.is_m5())?;
    if !matches!(seen.get(adapter), Some(Observation::NoGolden)) {
        return None;
    }
    let esc = |why: String| Some(Verdict::Escalate(why));
    if seen.values().any(|o| !matches!(o, Observation::NoGolden)) {
        return esc(format!("{name} has a golden on another adapter: a new adapter family is the owner's"));
    }
    if !declared {
        return esc(format!("{name} is not listed under 'Expected golden changes:' in the change's proposal (ADR 0037 (c))"));
    }
    if evidence.declaration.is_none() {
        return esc(format!("{name} has no entry under 'ADR 0037 conditions:' naming its debug view and bound tests (ADR 0037 (a), (b))"));
    }
    if let Err(why) = &evidence.bound_tests {
        return esc(format!("{name}: {why} (ADR 0037 (a))"));
    }
    match &evidence.debug_view {
        Err(why) => esc(format!("{name}: {why} (ADR 0037 (b))")),
        Ok(line) => Some(Verdict::AutoM5(format!("first {} golden under ADR 0037; bound tests green; {line}", class.as_str()))),
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
/// blank line or heading. A line missing a part, an unknown class, or an `affects=` naming a class that is not an M5 class does
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
        let classes: Vec<Option<GoldenClass>> = classes.split(',').map(|c| GoldenClass::parse(c.trim()).filter(|c| c.is_m5())).collect();
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
            let hit = mine.iter().find(|(_, d)| text.contains(&d.key) && text.contains(if sign == '-' { &d.old } else { &d.new }));
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

/// ADR 0037 point 2: classify the candidate of an existing M5-class golden when the change declares look parameter changes.
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
    let class = class.filter(|c| c.is_m5())?;
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
        let resized = seen(&[("warp", ch(951, 147)), ("nv", ch(938, 148)), ("lavapipe", Observation::SizeChanged)]);
        assert!(reason(classify("warp", "g", &resized, Some(GoldenClass::Debug), true)).contains("size on lavapipe"));
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
    fn good() -> M5Evidence {
        M5Evidence {
            declaration: Some(M5Declaration { debug_view: "v".into(), bound_tests: vec!["t".into()] }),
            bound_tests: Ok(()),
            debug_view: Ok("debug view v: ok".into()),
        }
    }
    fn m5(class: Option<GoldenClass>, declared: bool, ev: &M5Evidence) -> Option<Verdict> {
        classify_m5_first("warp", "lit_x", &first(&["warp", "nv"]), class, declared, ev)
    }
    fn why(v: Option<Verdict>) -> String {
        match v {
            Some(Verdict::Escalate(r)) => r,
            other => panic!("expected an escalation, got {other:?}"),
        }
    }

    // spec: TEST-022
    #[test]
    fn a_first_m5_golden_with_all_three_conditions_is_blessed_under_policy() {
        for class in ADR_0037_M5_CLASSES {
            assert!(matches!(m5(Some(class), true, &good()), Some(Verdict::AutoM5(_))), "{class:?}");
        }
    }

    // spec: TEST-022
    #[test]
    fn each_missing_condition_escalates_with_its_reason() {
        let lit = Some(GoldenClass::Lit);
        // (c) no declaration, in the proposal's golden list or in the conditions section.
        assert!(why(m5(lit, false, &good())).contains("Expected golden changes"));
        let no_entry = M5Evidence { declaration: None, ..good() };
        assert!(why(m5(lit, true, &no_entry)).contains("ADR 0037 conditions"));
        // (a) bound tests missing or red.
        let red = M5Evidence { bound_tests: Err("bound test 't' is not green in this run".into()), ..good() };
        assert!(why(m5(lit, true, &red)).contains("not green"));
        let gone = M5Evidence { bound_tests: Err("bound test 't' did not run".into()), ..good() };
        assert!(why(m5(lit, true, &gone)).contains("did not run"));
        // (b) the debug view is missing or the adapters disagree.
        let view = M5Evidence { debug_view: Err("debug view v does not exist on nv".into()), ..good() };
        assert!(why(m5(lit, true, &view)).contains("(ADR 0037 (b))"));
    }

    // spec: TEST-022
    #[test]
    fn the_m5_path_leaves_every_other_golden_to_adr_0017() {
        // A class outside the M5 window: the path does not apply, classify() escalates the first golden as before.
        for class in [GoldenClass::Debug, GoldenClass::Shaded] {
            assert_eq!(m5(Some(class), true, &good()), None, "{class:?}");
            assert!(why(Some(classify("warp", "lit_x", &first(&["warp"]), Some(class), true))).contains("first golden"));
        }
        assert_eq!(m5(None, true, &good()), None);
        // An existing M5 golden that changes keeps escalating (ADR 0017: no lit view is auto-blessed).
        let changed = seen(&[("warp", ch(500, 40)), ("nv", ch(500, 40))]);
        assert_eq!(classify_m5_first("warp", "lit_x", &changed, Some(GoldenClass::Lit), true, &good()), None);
        assert!(why(Some(classify("warp", "lit_x", &changed, Some(GoldenClass::Lit), true))).contains("lit"));
        // A new adapter family of an M5 golden that exists elsewhere is still the owner's.
        let family = seen(&[("warp", Observation::NoGolden), ("nv", Observation::Unchanged)]);
        assert!(why(classify_m5_first("warp", "lit_x", &family, Some(GoldenClass::Lit), true, &good())).contains("new adapter family"));
        // Existing classes still parse; the window names exactly the M5 classes.
        assert!(GoldenClass::Lit.is_m5() && GoldenClass::Sky.is_m5() && GoldenClass::Water.is_m5());
        assert!(!GoldenClass::Debug.is_m5() && !GoldenClass::Shaded.is_m5());
    }

    fn states(v: &[(&str, ViewState)]) -> BTreeMap<String, ViewState> {
        v.iter().map(|(a, s)| (a.to_string(), *s)).collect()
    }
    /// A pair differing in `beyond` pixels of delta `max_delta` (> 1, or 0 with none) plus `ones` pixels of delta 1.
    fn pair(a: &str, b: &str, beyond: usize, ones: usize, max_delta: u8) -> (String, String, ViewDiff) {
        (a.into(), b.into(), ViewDiff { at_rounding: ones, beyond_rounding: beyond, max_delta })
    }

    // spec: TEST-022
    #[test]
    fn a_missing_adapter_is_reported_and_never_counted_as_agreeing() {
        let none: Vec<String> = vec![];
        // Three adapters took part, one did not: the pass names the one that did not.
        let ok = debug_view_agreement(
            "v",
            &states(&[("warp", ViewState::New), ("nv", ViewState::New)]),
            &[pair("warp", "nv", 0, 0, 0)],
            &["vulkan-llvmpipe".to_string()],
        )
        .unwrap();
        assert!(ok.contains("NOT RUN") && ok.contains("vulkan-llvmpipe"), "{ok}");
        // One adapter alone is not agreement, and the missing ones are named in the reason.
        let alone = debug_view_agreement("v", &states(&[("warp", ViewState::New)]), &[], &["nv".to_string(), "llvmpipe".to_string()]);
        let e = alone.unwrap_err();
        assert!(e.contains("needs two") && e.contains("nv, llvmpipe"), "{e}");
        assert!(debug_view_agreement("v", &states(&[]), &[], &none).is_err());
    }

    // spec: TEST-022
    #[test]
    fn the_debug_view_must_exist_and_agree_across_the_adapters() {
        let none: Vec<String> = vec![];
        // A blessed view that still matches, or whose change ADR 0017 blesses, agrees.
        let blessed = states(&[("warp", ViewState::Matches), ("nv", ViewState::Differs { auto: true })]);
        assert!(debug_view_agreement("v", &blessed, &[], &none).is_ok());
        // A view that does not exist on a participating adapter, or whose change is not blessed, does not.
        let gone = states(&[("warp", ViewState::New), ("nv", ViewState::Missing)]);
        assert!(debug_view_agreement("v", &gone, &[], &none).unwrap_err().contains("does not exist on nv"));
        let bad = states(&[("warp", ViewState::Matches), ("nv", ViewState::Differs { auto: false })]);
        assert!(debug_view_agreement("v", &bad, &[], &none).unwrap_err().contains("does not bless"));
        // New candidates (ADR 0052 as written): at most 64 px beyond delta 1 (any delta), any number at delta 1.
        let new = states(&[("warp", ViewState::New), ("nv", ViewState::New), ("llvm", ViewState::New)]);
        let zero = pair("warp", "nv", 0, 0, 0);
        let agree = |p: (String, String, ViewDiff)| debug_view_agreement("v", &new, &[zero.clone(), p], &none);
        // Boundary: 63 and exactly 64 pixels of any delta agree, 65 disagree.
        assert!(agree(pair("nv", "llvm", 63, 0, 95)).is_ok());
        assert!(agree(pair("nv", "llvm", 64, 0, 95)).is_ok());
        let e = agree(pair("nv", "llvm", 65, 0, 95)).unwrap_err();
        assert!(e.contains("disagree") && e.contains("65 px beyond delta 1"), "{e}");
        // Delta-1 pixels are not capped (the withdrawn 32-px reading): 33 and 5000 of them agree, alone or beside 64 beyond.
        assert!(agree(pair("nv", "llvm", 64, 32, 95)).is_ok());
        assert!(agree(pair("nv", "llvm", 0, 33, 1)).is_ok());
        assert!(agree(pair("nv", "llvm", 64, 5000, 95)).is_ok());
        // A whole-image one-level shift agrees on this path: ADR 0052 as written, the bound tests guard the values.
        assert!(agree(pair("nv", "llvm", 0, 1_000_000, 1)).is_ok());
        // The 65th pixel beyond delta 1 escalates whatever the delta-1 count, and the message names the cap and the counts.
        let e = agree(pair("nv", "llvm", 65, 5000, 95)).unwrap_err();
        assert!(e.contains("65 px beyond delta 1 exceed the cap of 64") && e.contains("65 beyond, 5000 at delta 1"), "{e}");
        assert!(!e.contains("exceed the cap of 32"), "{e}");
        // One delta-2 pixel outside the 64 escalates.
        let e = agree(pair("nv", "llvm", 65, 0, 2)).unwrap_err();
        assert!(e.contains("disagree"), "{e}");
        // Any one pair beyond the limit fails the view, not only the worst-looking one.
        let near = [pair("warp", "nv", 64, 3, 11), pair("warp", "llvm", 0, 0, 0), pair("nv", "llvm", 65, 0, 2)];
        assert!(debug_view_agreement("v", &new, &near, &none).is_err());
        let ok = debug_view_agreement("v", &new, &near[..2], &none).unwrap();
        assert!(ok.contains("worst pair 64 px beyond delta 1 of 64, 3 px at delta 1, max delta 11"), "{ok}");
        // New on one adapter and blessed on another is not one view.
        let mixed = states(&[("warp", ViewState::New), ("nv", ViewState::Matches)]);
        assert!(debug_view_agreement("v", &mixed, &[], &none).unwrap_err().contains("new on some"));
    }

    // spec: TEST-022
    #[test]
    fn view_diff_counts_pixels_beyond_delta_1_by_their_largest_channel_including_alpha() {
        let base = crate::Image::new(100, 100, vec![100; 100 * 100 * 4]);
        let mut other = base.clone();
        let set = |img: &mut crate::Image, i: usize, ch: usize, v: u8| img.rgba[i * 4 + ch] = v;
        for i in 0..64 {
            set(&mut other, i, 0, 195); // delta 95
        }
        for i in 64..96 {
            set(&mut other, i, 1, 101); // delta 1 in a single channel
        }
        let d = view_diff(&base, &other);
        assert_eq!((d.at_rounding, d.beyond_rounding, d.max_delta), (32, 64, 95));
        // Two small deltas in different channels are still delta 1 per pixel (the max, not the sum).
        set(&mut other, 6000, 0, 101);
        set(&mut other, 6000, 2, 99);
        let d = view_diff(&base, &other);
        assert_eq!((d.at_rounding, d.beyond_rounding), (33, 64));
        // A delta-2 alpha difference is the 65th pixel beyond delta 1.
        set(&mut other, 7000, 3, 102);
        let d = view_diff(&base, &other);
        assert_eq!((d.at_rounding, d.beyond_rounding, d.max_delta), (33, 65, 95));
        let none: Vec<String> = vec![];
        let new = states(&[("warp", ViewState::New), ("nv", ViewState::New)]);
        assert!(debug_view_agreement("v", &new, &[("warp".into(), "nv".into(), d)], &none).is_err());
        assert_eq!(view_diff(&base, &base), ViewDiff { at_rounding: 0, beyond_rounding: 0, max_delta: 0 });
    }

    // spec: TEST-022
    #[test]
    fn the_existing_golden_path_keeps_adr_0017_thresholds() {
        // ADR 0052 touches only new-vs-new candidates: rounding noise stays 32 px and delta 1, and the 8 px agreement floor
        // for declared changes is unchanged, for a debug view with a blessed golden.
        let d = Some(GoldenClass::Debug);
        assert_eq!(classify("warp", "g", &seen(&[("warp", ch(32, 1))]), d, false), Verdict::AutoRounding);
        assert!(matches!(classify("warp", "g", &seen(&[("warp", ch(64, 1))]), d, false), Verdict::Escalate(_)));
        let apart = seen(&[("warp", ch(951, 147)), ("nv", ch(951 - 49, 147))]);
        assert!(matches!(classify("warp", "g", &apart, d, true), Verdict::Escalate(_)));
        // A blessed view still matches or changes only as ADR 0017 blesses; the tolerance does not reach it.
        let none: Vec<String> = vec![];
        let bad = states(&[("warp", ViewState::Matches), ("nv", ViewState::Differs { auto: false })]);
        assert!(debug_view_agreement("v", &bad, &[], &none).is_err());
        assert_eq!(ADR_0052_EDGE_PIXELS, 64);
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

    // spec: TEST-022
    #[test]
    fn the_m5_conditions_section_parses_strictly() {
        let p = "## What\n- x\n\nADR 0037 conditions:\n- lit_a: debug-view=lit_bands; bound-tests=t1, `planet-render::b::t2`\n- lit_b: debug-view=lit_bands\n- lit_c: bound-tests=t1\n\n## After\n- lit_d: debug-view=x; bound-tests=t\n";
        let d = m5_declarations(p);
        assert_eq!(d.len(), 1, "an entry needs both keys");
        assert_eq!(
            d["lit_a"],
            M5Declaration { debug_view: "lit_bands".into(), bound_tests: vec!["t1".into(), "planet-render::b::t2".into()] }
        );
        assert!(m5_declarations("no section").is_empty());
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
