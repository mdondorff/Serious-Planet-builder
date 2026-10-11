//! `cargo xtask golden-review`: evidence and verdicts for candidate golden images under the owner's policy (ADR 0017).
//!
//! Usage: golden-review [--proposal <proposal.md>] [--extra <dir with <adapter>/<name>.png>]... [--apply]
//! Reads candidates from `target/review/candidates/<adapter>/` and every `--extra` directory, compares each with
//! `tests/goldens/<adapter>/<name>.png`, writes golden | candidate | diff triptychs and `report.md` to
//! `target/review/golden-review/`, prints one verdict per candidate and, with `--apply`, copies only the auto verdicts
//! (and the adapter's `driver.txt`) into `tests/goldens/`. Exits 0 when nothing escalates and 3 otherwise (`cargo xtask
//! golden-review` then reports a failure); 2 on bad arguments.

use planet_testkit::review::{classify, declared_goldens, parse_classes, Change, Observation, Verdict};
use planet_testkit::{read_png, repo_root, write_png, Image};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut proposal, mut extras, mut apply) = (None::<PathBuf>, vec![], false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--proposal" | "--extra" => {
                let Some(v) = it.next() else {
                    eprintln!("golden-review: {a} needs a value");
                    std::process::exit(2);
                };
                if a == "--proposal" {
                    proposal = Some(PathBuf::from(v));
                } else {
                    extras.push(PathBuf::from(v));
                }
            }
            "--apply" => apply = true,
            other => {
                eprintln!(
                    "golden-review: unknown argument '{other}'\nusage: golden-review [--proposal <file>] [--extra <dir>]... [--apply]"
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
    let declared: BTreeSet<String> = proposal
        .as_ref()
        .map(|p| declared_goldens(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display()))))
        .unwrap_or_default()
        .into_iter()
        .collect();

    // Candidates: (adapter, name) -> (path, directory holding driver.txt). Later sources override earlier ones.
    let mut candidates: BTreeMap<(String, String), PathBuf> = BTreeMap::new();
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

    let mut report = String::from("# Golden review (ADR 0017)\n\n| Adapter | Golden | Changed px | Max delta | Class | Declared | Verdict |\n|---|---|---|---|---|---|---|\n");
    let mut escalated = 0;
    let mut applied = vec![];
    for name in &names {
        // What every adapter shows for this golden.
        let mut seen: BTreeMap<String, Observation> = BTreeMap::new();
        let mut images: BTreeMap<String, (Image, Image)> = BTreeMap::new();
        for adapter in &adapters {
            let golden = goldens.join(adapter).join(format!("{name}.png"));
            let cand = candidates.get(&(adapter.clone(), name.clone()));
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
                        images.insert(adapter.clone(), (g, c));
                        Observation::Changed(ch)
                    }
                }
            };
            seen.insert(adapter.clone(), obs);
        }
        for adapter in adapters.iter().filter(|a| candidates.contains_key(&((*a).clone(), name.clone()))) {
            let class = classes.get(name).copied();
            let verdict = classify(adapter, name, &seen, class, declared.contains(name));
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
                Verdict::Escalate(r) => format!("ESCALATE {r}"),
            };
            println!("{adapter} {name}: {px} px, max delta {delta}: {v}");
            report.push_str(&format!(
                "| {adapter} | {name} | {px} | {delta} | {} | {} | {v} |\n",
                class.map_or("none".into(), |c| format!("{c:?}").to_lowercase()),
                if declared.contains(name) { "yes" } else { "no" }
            ));
            if matches!(verdict, Verdict::Escalate(_)) {
                escalated += 1;
            } else if apply {
                let src = &candidates[&(adapter.clone(), name.clone())];
                let dst = goldens.join(adapter).join(format!("{name}.png"));
                std::fs::copy(src, &dst).expect("copy candidate");
                let driver = src.parent().map(|d| d.join("driver.txt"));
                if let Some(d) = driver.filter(|d| d.is_file()) {
                    std::fs::copy(d, goldens.join(adapter).join("driver.txt")).expect("copy driver.txt");
                }
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
