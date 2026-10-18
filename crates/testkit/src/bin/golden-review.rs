//! `cargo xtask golden-review`: evidence and verdicts for candidate golden images under the owner's policy (ADR 0017 as amended by
//! ADR 0061: every adapter's candidate is judged against its own golden, the first golden of any class is the owner's) and the look
//! path of ADR 0037 point 2.
//!
//! Usage: golden-review [--proposal <proposal.md>] [--extra <dir with <adapter>/<name>.png>]... [--tests <junit.xml>]... [--apply]
//! Reads candidates from `target/review/candidates/<adapter>/` and every `--extra` directory, compares each with
//! `tests/goldens/<adapter>/<name>.png`, writes golden | candidate | diff triptychs and `report.md` to
//! `target/review/golden-review/`, prints one verdict per candidate and, with `--apply`, copies only the auto verdicts
//! into `tests/goldens/`. A candidate whose `driver.txt` differs from the recorded one always escalates. `--tests` names the
//! nextest JUnit reports of this run (`target/nextest/default/junit.xml`, `target/nextest/real-gpu/junit.xml` for a `--real` run, copied after each `cargo xtask test` run): the
//! report counts their testcases as the evidence of the bound tests. An adapter takes part in the run when it delivered candidates;
//! the adapters that did not are printed (the look path needs every adapter with goldens to take part). Exits 0 when
//! nothing escalates and 3 otherwise (`cargo xtask golden-review` then reports a failure); 2 on bad arguments.

use planet_testkit::review::{
    classify, classify_look_change, debug_views_unchanged, declared_goldens, driver_gate, look_declarations, look_diff_ok, parse_classes,
    Change, GoldenClass, LookEvidence, Observation, TestOutcomes, Verdict,
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

/// The ref a branch is compared with: `origin/main` when it exists (a worktree's local `main` can be far behind), else `main`.
fn diff_base(root: &Path) -> &'static str {
    let ok = |r: &str| {
        std::process::Command::new("git")
            .args(["rev-parse", "--verify", "--quiet", r])
            .current_dir(root)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if ok("origin/main") {
        "origin/main"
    } else {
        "main"
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// ADR 0037 point 2: the evidence for a change that declares look parameter changes under `Look changes:`. The files the change
/// touches (against the base branch, plus the working tree), the zero-context diff of each look data file
/// (`tests/goldens/look_data.txt`), and whether any debug or shaded golden moved beyond rounding on any adapter.
fn look_evidence(root: &Path, run: &Run<'_>, proposal: &str) -> LookEvidence {
    let declarations = look_declarations(proposal);
    // The list of look data files comes from the base branch, never from the change under review: a change could add `*.rs` to its
    // own list (an edit of the list in the change takes effect only after it is merged).
    let base = diff_base(root);
    let patterns: Vec<String> = git(root, &["show", &format!("{base}:tests/goldens/look_data.txt")])
        .as_str()
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let mut changed: BTreeSet<String> = git(root, &["diff", "--name-only", base]).lines().map(str::to_string).collect();
    changed.extend(git(root, &["ls-files", "--others", "--exclude-standard"]).lines().map(str::to_string));
    let changed: Vec<String> = changed.into_iter().filter(|f| !f.is_empty()).collect();
    let mut diffs = BTreeMap::new();
    for f in changed.iter().filter(|f| planet_testkit::review::is_look_data(f, &patterns)) {
        diffs.insert(f.clone(), git(root, &["diff", "-U0", base, "--", f]));
    }
    let views: Vec<(String, BTreeMap<String, Observation>)> = run
        .classes
        .iter()
        .filter(|(_, c)| matches!(c, GoldenClass::Debug | GoldenClass::Shaded))
        .map(|(n, _)| (n.clone(), observe(run.goldens, run.adapters, run.candidates, n).0))
        .collect();
    // The debug views are judged only on the candidates a run delivered: an adapter that delivered none (NOT RUN) proves nothing about
    // them, so the look path needs every adapter with goldens to take part.
    let not_run = run.not_run();
    let debug_views = if not_run.is_empty() {
        debug_views_unchanged(&views)
    } else {
        Err(format!("the adapters {} delivered no candidates (NOT RUN), so their debug views are unchecked", not_run.join(", ")))
    };
    LookEvidence { diff: look_diff_ok(&changed, &diffs, &patterns, &declarations), debug_views, declarations }
}

/// Everything the verdicts of one run need.
struct Run<'a> {
    goldens: &'a Path,
    adapters: &'a [String],
    candidates: &'a Candidates,
    classes: &'a BTreeMap<String, GoldenClass>,
    ran: &'a BTreeSet<String>,
}

impl Run<'_> {
    fn read(&self, p: PathBuf) -> Option<String> {
        std::fs::read_to_string(p).ok()
    }

    /// The driver gate (ADR 0017, not relaxed by ADR 0061) for the candidate of `name` on `adapter`.
    fn gate(&self, adapter: &str, name: &str) -> Option<String> {
        let src = self.candidates.get(&(adapter.to_string(), name.to_string()))?;
        let recorded = self.read(self.goldens.join(adapter).join("driver.txt"));
        let cand = src.parent().and_then(|d| self.read(d.join("driver.txt")));
        driver_gate(adapter, recorded.as_deref(), cand.as_deref())
    }

    /// Adapters that have goldens but delivered no candidate in this run.
    fn not_run(&self) -> Vec<String> {
        self.adapters.iter().filter(|a| !self.ran.contains(*a)).cloned().collect()
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
    let run = Run { goldens: &goldens, adapters: &adapters, candidates: &candidates, classes: &classes, ran: &ran };

    // Which adapters took part: a missing one is said so (the look path needs every adapter with goldens).
    let mut report = String::from("# Golden review (ADR 0017, ADR 0061; ADR 0037 point 2 for look changes)\n\n");
    let took_part = format!("Adapters that took part (delivered candidates): {}\n", ran.iter().cloned().collect::<Vec<_>>().join(", "));
    let not_run = run.not_run();
    let missing = if not_run.is_empty() {
        "All adapters with goldens took part.\n".to_string()
    } else {
        format!("NOT RUN in this review (no candidates): {}\n", not_run.join(", "))
    };
    print!("{took_part}{missing}");
    report.push_str(&format!(
        "{took_part}\n{missing}\nBound-test reports: {} testcase(s) from {} file(s).\n\n",
        outcomes.0.len(),
        tests.len()
    ));
    report.push_str("| Adapter | Golden | Changed px | Max delta | Class | Declared | Verdict |\n|---|---|---|---|---|---|---|\n");
    // ADR 0037 point 2: the look path is computed once, and only when the proposal declares look changes.
    let look = (!look_declarations(&proposal_text).is_empty()).then(|| look_evidence(&root, &run, &proposal_text));
    let mut escalated = 0;
    let mut applied = vec![];
    for name in &names {
        let (seen, images) = observe(&goldens, &adapters, &candidates, name);
        let class = classes.get(name).copied();
        for adapter in adapters.iter().filter(|a| candidates.contains_key(&((*a).clone(), name.clone()))) {
            let src = &candidates[&(adapter.clone(), name.clone())];
            let is_declared = declared.contains(name);
            let verdict = match run.gate(adapter, name) {
                Some(reason) => Verdict::Escalate(reason),
                None => look
                    .as_ref()
                    .and_then(|l| classify_look_change(adapter, name, &seen, class, is_declared, l))
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
                Verdict::AutoLook(e) => format!("AUTO-L {e}"),
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
