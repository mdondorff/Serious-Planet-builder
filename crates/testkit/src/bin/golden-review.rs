//! `cargo xtask golden-review`: evidence and verdicts for candidate golden images under the owner's policy (ADR 0017) and,
//! for the first goldens of M5 classes, ADR 0037's conditions.
//!
//! Usage: golden-review [--proposal <proposal.md>] [--extra <dir with <adapter>/<name>.png>]... [--tests <junit.xml>]... [--apply]
//! Reads candidates from `target/review/candidates/<adapter>/` and every `--extra` directory, compares each with
//! `tests/goldens/<adapter>/<name>.png`, writes golden | candidate | diff triptychs and `report.md` to
//! `target/review/golden-review/`, prints one verdict per candidate and, with `--apply`, copies only the auto verdicts
//! into `tests/goldens/`. A candidate whose `driver.txt` differs from the recorded one always escalates. `--tests` names the
//! nextest JUnit reports of this run (`target/nextest/default/junit.xml`, copied after each `cargo xtask test` run): the
//! bound tests that the proposal declares under `ADR 0037 conditions:` must be green in them. An adapter takes part in the
//! run when it delivered candidates; the adapters that did not are printed and never counted as agreeing. Exits 0 when
//! nothing escalates and 3 otherwise (`cargo xtask golden-review` then reports a failure); 2 on bad arguments.

use planet_testkit::review::{
    classify, classify_m5_first, debug_view_agreement, declared_goldens, driver_gate, m5_declarations, parse_classes, view_diff, Change,
    GoldenClass, M5Evidence, Observation, TestOutcomes, Verdict, ViewDiff, ViewState,
};
use planet_testkit::{read_png, repo_root, write_png, Image};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn change(a: &Image, b: &Image) -> Change {
    let mut pixels = 0;
    let mut max_delta = 0u8;
    for (p, q) in a.pixels().zip(b.pixels()) {
        if p != q {
            pixels += 1;
            max_delta = max_delta.max((0..4).map(|c| p[c].abs_diff(q[c])).max().unwrap_or(0));
        }
    }
    Change { pixels, max_delta }
}

/// golden | candidate | diff (changed pixels red, others dimmed), side by side, scaled 3x.
fn triptych(a: &Image, b: &Image) -> Image {
    let (w, h, gap, s) = (a.width, a.height, 4u32, 3u32);
    let mut out = Image::new((3 * w + 2 * gap) * s, h * s, vec![255; ((3 * w + 2 * gap) * s * h * s * 4) as usize]);
    for y in 0..h {
        for x in 0..w {
            let (p, q) = (a.pixel(x, y), b.pixel(x, y));
            let d = if p == q {
                let g = ((u32::from(q[0]) + u32::from(q[1]) + u32::from(q[2])) / 6) as u8;
                [g, g, g, 255]
            } else {
                [255, 0, 0, 255]
            };
            for (k, px) in [(0, p), (1, q), (2, d)] {
                for dy in 0..s {
                    for dx in 0..s {
                        out.set_pixel((k * (w + gap) + x) * s + dx, y * s + dy, px);
                    }
                }
            }
        }
    }
    out
}

type Candidates = BTreeMap<(String, String), PathBuf>;

/// What every adapter shows for golden `name`, and the golden | candidate images of the adapters whose candidate changed.
fn observe(
    goldens: &Path,
    adapters: &[String],
    candidates: &Candidates,
    name: &str,
) -> (BTreeMap<String, Observation>, BTreeMap<String, (Image, Image)>) {
    let mut seen: BTreeMap<String, Observation> = BTreeMap::new();
    let mut images: BTreeMap<String, (Image, Image)> = BTreeMap::new();
    for adapter in adapters {
        let golden = goldens.join(adapter).join(format!("{name}.png"));
        let cand = candidates.get(&(adapter.clone(), name.to_string()));
        let obs = match (golden.is_file(), cand) {
            (false, Some(_)) => Observation::NoGolden,
            (false, None) => continue,
            (true, None) => Observation::Unchanged,
            (true, Some(c)) => {
                let (g, c) = (read_png(&golden).expect("golden"), read_png(c).expect("candidate"));
                if (g.width, g.height) != (c.width, c.height) {
                    Observation::SizeChanged
                } else {
                    let ch = change(&g, &c);
                    if ch.pixels == 0 {
                        // A candidate equal to its golden (a CI artefact beside a golden blessed since) changes nothing.
                        seen.insert(adapter.clone(), Observation::Unchanged);
                        continue;
                    }
                    images.insert(adapter.clone(), (g, c));
                    Observation::Changed(ch)
                }
            }
        };
        seen.insert(adapter.clone(), obs);
    }
    (seen, images)
}

/// Everything the verdicts of one run need.
struct Run<'a> {
    goldens: &'a Path,
    adapters: &'a [String],
    candidates: &'a Candidates,
    classes: &'a BTreeMap<String, GoldenClass>,
    declared: &'a BTreeSet<String>,
    ran: &'a BTreeSet<String>,
    outcomes: &'a TestOutcomes,
    tests_given: bool,
    m5: &'a BTreeMap<String, planet_testkit::review::M5Declaration>,
}

impl Run<'_> {
    fn read(&self, p: PathBuf) -> Option<String> {
        std::fs::read_to_string(p).ok()
    }

    /// The driver gate (ADR 0017, not relaxed by ADR 0037) for the candidate of `name` on `adapter`.
    fn gate(&self, adapter: &str, name: &str) -> Option<String> {
        let src = self.candidates.get(&(adapter.to_string(), name.to_string()))?;
        let recorded = self.read(self.goldens.join(adapter).join("driver.txt"));
        let cand = src.parent().and_then(|d| self.read(d.join("driver.txt")));
        driver_gate(adapter, recorded.as_deref(), cand.as_deref())
    }

    /// ADR 0037 point 1(b) for the debug view `view`, over the adapters that took part in this run.
    fn view_agreement(&self, view: &str) -> Result<String, String> {
        let (seen, _) = observe(self.goldens, self.adapters, self.candidates, view);
        let class = self.classes.get(view).copied();
        let mut states: BTreeMap<String, ViewState> = BTreeMap::new();
        for adapter in self.ran {
            let state = match seen.get(adapter) {
                None => ViewState::Missing,
                Some(Observation::Unchanged) => ViewState::Matches,
                Some(_) if self.gate(adapter, view).is_some() => ViewState::Differs { auto: false },
                Some(Observation::NoGolden) => ViewState::New,
                Some(Observation::SizeChanged) => ViewState::Differs { auto: false },
                Some(Observation::Changed(_)) => {
                    let auto = !matches!(classify(adapter, view, &seen, class, self.declared.contains(view)), Verdict::Escalate(_));
                    ViewState::Differs { auto }
                }
            };
            states.insert(adapter.clone(), state);
        }
        // New candidates are compared with one another: compared pairwise under ADR 0052 (at most 64 px beyond delta 1, any number at delta 1).
        let news: Vec<&String> = states.iter().filter(|(_, s)| **s == ViewState::New).map(|(a, _)| a).collect();
        let mut pairs = vec![];
        for (i, a) in news.iter().enumerate() {
            for b in &news[i + 1..] {
                let (ia, ib) = (
                    read_png(&self.candidates[&((*a).clone(), view.to_string())]).expect("candidate"),
                    read_png(&self.candidates[&((*b).clone(), view.to_string())]).expect("candidate"),
                );
                let c = if (ia.width, ia.height) == (ib.width, ib.height) {
                    view_diff(&ia, &ib)
                } else {
                    ViewDiff { at_rounding: usize::MAX, beyond_rounding: usize::MAX, max_delta: u8::MAX }
                };
                pairs.push(((*a).clone(), (*b).clone(), c));
            }
        }
        debug_view_agreement(view, &states, &pairs, &self.not_run())
    }

    /// Adapters that have goldens but delivered no candidate in this run.
    fn not_run(&self) -> Vec<String> {
        self.adapters.iter().filter(|a| !self.ran.contains(*a)).cloned().collect()
    }

    fn evidence(&self, name: &str) -> M5Evidence {
        let declaration = self.m5.get(name).cloned();
        let (bound_tests, debug_view) = match &declaration {
            None => (Err("no declaration".to_string()), Err("no declaration".to_string())),
            Some(d) => (
                if self.tests_given {
                    self.outcomes.all_green(&d.bound_tests)
                } else {
                    Err("no --tests report given, so no bound test is known to have run".to_string())
                },
                self.view_agreement(&d.debug_view),
            ),
        };
        M5Evidence { declaration, bound_tests, debug_view }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut proposal, mut extras, mut tests, mut apply) = (None::<PathBuf>, vec![], vec![], false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--proposal" | "--extra" | "--tests" => {
                let Some(v) = it.next() else {
                    eprintln!("golden-review: {a} needs a value");
                    std::process::exit(2);
                };
                match a.as_str() {
                    "--proposal" => proposal = Some(PathBuf::from(v)),
                    "--extra" => extras.push(PathBuf::from(v)),
                    _ => tests.push(PathBuf::from(v)),
                }
            }
            "--apply" => apply = true,
            other => {
                eprintln!(
                    "golden-review: unknown argument '{other}'\nusage: golden-review [--proposal <file>] [--extra <dir>]... [--tests <junit.xml>]... [--apply]"
                );
                std::process::exit(2);
            }
        }
    }
    let root = repo_root();
    let goldens = root.join("tests").join("goldens");
    let out = root.join("target").join("review").join("golden-review");
    let _ = std::fs::create_dir_all(&out);
    let classes = parse_classes(&std::fs::read_to_string(goldens.join("classes.txt")).unwrap_or_default());
    let proposal_text = proposal
        .as_ref()
        .map(|p| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display())))
        .unwrap_or_default();
    let declared: BTreeSet<String> = declared_goldens(&proposal_text).into_iter().collect();
    let m5 = m5_declarations(&proposal_text);
    let mut outcomes = TestOutcomes::default();
    for t in &tests {
        let p = root.join(t);
        outcomes.add_junit(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display())));
    }

    // Candidates: (adapter, name) -> path. Later sources override earlier ones.
    let mut candidates: Candidates = BTreeMap::new();
    for dir in std::iter::once(root.join("target").join("review").join("candidates")).chain(extras.iter().map(|e| root.join(e))) {
        let Ok(adapters) = std::fs::read_dir(&dir) else { continue };
        for a in adapters.flatten().filter(|e| e.path().is_dir()) {
            let adapter = a.file_name().to_string_lossy().to_string();
            for f in std::fs::read_dir(a.path()).into_iter().flatten().flatten() {
                let p = f.path();
                if p.extension().is_some_and(|x| x == "png") {
                    let name = p.file_stem().unwrap().to_string_lossy().to_string();
                    candidates.insert((adapter.clone(), name), p);
                }
            }
        }
    }
    if candidates.is_empty() {
        println!("golden-review: no candidates");
        return;
    }
    let names: BTreeSet<String> = candidates.keys().map(|(_, n)| n.clone()).collect();
    let adapters: Vec<String> = std::fs::read_dir(&goldens)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .chain(candidates.keys().map(|(a, _)| a.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let ran: BTreeSet<String> = candidates.keys().map(|(a, _)| a.clone()).collect();
    let run = Run {
        goldens: &goldens,
        adapters: &adapters,
        candidates: &candidates,
        classes: &classes,
        declared: &declared,
        ran: &ran,
        outcomes: &outcomes,
        tests_given: !tests.is_empty(),
        m5: &m5,
    };

    // Which adapters took part: a missing one is said so and never counts as agreeing (ADR 0037 (b)).
    let mut report = String::from("# Golden review (ADR 0017, ADR 0037 for first goldens of M5 classes)\n\n");
    let took_part = format!("Adapters that took part (delivered candidates): {}\n", ran.iter().cloned().collect::<Vec<_>>().join(", "));
    let not_run = run.not_run();
    let missing = if not_run.is_empty() {
        "All adapters with goldens took part.\n".to_string()
    } else {
        format!("NOT RUN in this review (no candidates; never counted as agreeing): {}\n", not_run.join(", "))
    };
    print!("{took_part}{missing}");
    report.push_str(&format!(
        "{took_part}\n{missing}\nBound-test reports: {} testcase(s) from {} file(s).\n\n",
        outcomes.0.len(),
        tests.len()
    ));
    report.push_str("| Adapter | Golden | Changed px | Max delta | Class | Declared | Verdict |\n|---|---|---|---|---|---|---|\n");
    let mut escalated = 0;
    let mut applied = vec![];
    let mut conditions = String::new();
    for name in &names {
        let (seen, images) = observe(&goldens, &adapters, &candidates, name);
        let class = classes.get(name).copied();
        // ADR 0037 evidence only for a first golden of an M5 class (it is computed once per golden).
        let m5_evidence = class.filter(|c| c.is_m5()).map(|_| run.evidence(name));
        if let Some(e) = &m5_evidence {
            conditions.push_str(&format!(
                "- **{name}**: bound tests: {}; {}\n",
                match &e.bound_tests {
                    Ok(()) => format!("green in this run ({})", e.declaration.as_ref().map_or(String::new(), |d| d.bound_tests.join(", "))),
                    Err(r) => format!("NOT SATISFIED: {r}"),
                },
                match &e.debug_view {
                    Ok(l) => l.clone(),
                    Err(r) => format!("debug view NOT SATISFIED: {r}"),
                }
            ));
        }
        for adapter in adapters.iter().filter(|a| candidates.contains_key(&((*a).clone(), name.clone()))) {
            let src = &candidates[&(adapter.clone(), name.clone())];
            let is_declared = declared.contains(name);
            let verdict = match run.gate(adapter, name) {
                Some(reason) => Verdict::Escalate(reason),
                None => m5_evidence
                    .as_ref()
                    .and_then(|e| classify_m5_first(adapter, name, &seen, class, is_declared, e))
                    .unwrap_or_else(|| classify(adapter, name, &seen, class, is_declared)),
            };
            let (px, delta) = match seen.get(adapter) {
                Some(Observation::Changed(c)) => (c.pixels.to_string(), c.max_delta.to_string()),
                _ => ("-".into(), "-".into()),
            };
            if let Some((g, c)) = images.get(adapter) {
                let path = out.join(adapter).join(format!("{name}.png"));
                write_png(&path, &triptych(g, c)).expect("triptych");
            }
            let v = match &verdict {
                Verdict::AutoRounding => "AUTO-R".to_string(),
                Verdict::AutoExplained => "AUTO-E".to_string(),
                Verdict::AutoM5(e) => format!("AUTO-M5 {e}"),
                Verdict::Escalate(r) => format!("ESCALATE {r}"),
            };
            println!("{adapter} {name}: {px} px, max delta {delta}: {v}");
            report.push_str(&format!(
                "| {adapter} | {name} | {px} | {delta} | {} | {} | {v} |\n",
                class.map_or("none".into(), |c| c.as_str().to_string()),
                if is_declared { "yes" } else { "no" }
            ));
            if matches!(verdict, Verdict::Escalate(_)) {
                escalated += 1;
            } else if apply {
                // The driver gate passed, so the recorded driver.txt stays as it is.
                let dst = goldens.join(adapter).join(format!("{name}.png"));
                std::fs::copy(src, &dst).expect("copy candidate");
                applied.push(format!("{adapter}/{name}"));
            }
        }
    }
    if !conditions.is_empty() {
        report.push_str(&format!("\n## ADR 0037 conditions (first goldens of M5 classes)\n\n{conditions}"));
        print!("ADR 0037 conditions:\n{conditions}");
    }
    report.push_str(&format!("\nTriptychs (golden | candidate | diff): `{}`\n", out.display()));
    std::fs::write(out.join("report.md"), &report).expect("report");
    if apply {
        println!("applied {} auto verdicts: {}", applied.len(), applied.join(", "));
    }
    println!("report: {}", out.join("report.md").display());
    if escalated > 0 {
        println!("{escalated} candidate(s) escalated to the owner (/bless)");
        std::process::exit(3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-022
    #[test]
    fn a_candidate_equal_to_its_golden_is_unchanged_not_a_zero_pixel_change() {
        let dir = std::env::temp_dir().join(format!("golden-review-observe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("g/warp")).unwrap();
        let img = Image::new(2, 2, vec![10; 16]);
        write_png(&dir.join("g/warp/v.png"), &img).unwrap();
        let mut other = img.clone();
        other.rgba[0] = 12;
        write_png(&dir.join("cand_same.png"), &img).unwrap();
        write_png(&dir.join("cand_diff.png"), &other).unwrap();
        let adapters = vec!["warp".to_string()];
        let observe_with = |file: &str| {
            let cands: Candidates = [(("warp".to_string(), "v".to_string()), dir.join(file))].into();
            observe(&dir.join("g"), &adapters, &cands, "v").0["warp"].clone()
        };
        assert_eq!(observe_with("cand_same.png"), Observation::Unchanged);
        assert_eq!(observe_with("cand_diff.png"), Observation::Changed(Change { pixels: 1, max_delta: 2 }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
