//! `cargo xtask package`: the owner package (TEST-074), the stills and short videos of the owner's pose sites and a `package.md` that says, per item,
//! PASS, FAIL or NOT RUN, in `target/review/owner-package/<date>/`. A step that cannot run is NOT RUN with its reason, never a silent pass; a failed
//! step does not stop the others. The steps run the existing `capture` commands (never a measurement); detectors join as items when they have a log.

use crate::util::Res;
use std::path::{Path, PathBuf};

/// The default output directory under the repository root: `<DIR>/<date>`.
const PACKAGE_DIR: &str = "target/review/owner-package";
/// The sites of the owner's look reviews (`docs/milestones/M8/look-review-2b/`), each drawn at its default look.
const SITES: [&str; 3] = ["hills-b", "river-r1", "hills-r1"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    NotRun,
}

impl Status {
    fn word(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NotRun => "NOT RUN",
        }
    }
}

/// One line of the package: what it is, how it went, why, and the files it wrote (relative to the package directory).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub status: Status,
    pub detail: String,
    pub files: Vec<String>,
}

/// One step: a name, the arguments of the `capture` command that makes it, and the directory (relative to the package) its files go to.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub name: String,
    pub capture_args: Vec<String>,
    pub dir: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Opts {
    pub store: Option<String>,
    pub date: Option<String>,
    pub out: Option<String>,
    pub dry_run: bool,
}

pub fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |what: &str| it.next().cloned().ok_or_else(|| format!("{what} needs a value"));
        match a.as_str() {
            "--store" => o.store = Some(value("--store")?),
            "--date" => o.date = Some(value("--date")?),
            "--out" => o.out = Some(value("--out")?),
            "--dry-run" => o.dry_run = true,
            other => return Err(format!("unknown package option '{other}' (known: --store DIR, --date YYYY-MM-DD, --out DIR, --dry-run)")),
        }
    }
    Ok(o)
}

/// The steps for `package_dir` (the absolute or relative directory of this package) over `store`: the default-look stills of each site, then the
/// coastal flights. Without a store every step is planned with its capture arguments left empty and is NOT RUN when run.
pub fn plan(store: Option<&str>, package_dir: &str) -> Vec<Step> {
    let s = |v: &str| v.to_string();
    let mut steps: Vec<Step> = SITES
        .iter()
        .map(|site| {
            let dir = format!("stills/{site}");
            let mut args = Vec::new();
            if let Some(store) = store {
                args = vec![
                    s("stills"),
                    s("--poses"),
                    format!("docs/milestones/M8/look-review-2b/{site}/default/poses.toml"),
                    s("--store"),
                    s(store),
                    s("--out-dir"),
                    format!("{package_dir}/{dir}"),
                ];
            }
            Step { name: format!("stills {site}"), capture_args: args, dir }
        })
        .collect();
    let mut flights = Vec::new();
    if let Some(store) = store {
        flights = vec![s("flights"), s("--store"), s(store), s("--out-dir"), format!("{package_dir}/flights")];
    }
    steps.push(Step { name: s("flights"), capture_args: flights, dir: s("flights") });
    steps
}

/// Run the steps with `runner` (the `capture` command in production), listing each step's files with `list`. A step with no arguments is NOT RUN
/// (no store); a runner error is FAIL with its message; the others carry on either way.
pub fn run_steps(steps: &[Step], runner: &mut dyn FnMut(&Step) -> Result<(), String>, list: &dyn Fn(&str) -> Vec<String>) -> Vec<Item> {
    steps
        .iter()
        .map(|st| {
            if st.capture_args.is_empty() {
                return Item { name: st.name.clone(), status: Status::NotRun, detail: "no --store given".into(), files: Vec::new() };
            }
            match runner(st) {
                Ok(()) => {
                    let files = list(&st.dir);
                    let (status, detail) = if files.is_empty() {
                        (Status::Fail, "the command succeeded but wrote no file".to_string())
                    } else {
                        (Status::Pass, format!("captured {} file(s)", files.len()))
                    };
                    Item { name: st.name.clone(), status, detail, files }
                }
                Err(e) => Item { name: st.name.clone(), status: Status::Fail, detail: e, files: Vec::new() },
            }
        })
        .collect()
}

/// The `package.md` text: the date and commit, one table row per item, the files of each item, and the counts. A NOT RUN item is named as such.
pub fn render(date: &str, commit: &str, items: &[Item]) -> String {
    let count = |s: Status| items.iter().filter(|i| i.status == s).count();
    let mut out = format!("# Owner package {date}\n\nCommit {commit}.\n\n| Item | Status | Detail |\n|---|---|---|\n");
    for i in items {
        out += &format!("| {} | {} | {} |\n", i.name, i.status.word(), i.detail.replace('|', "/").replace('\n', " "));
    }
    out += "\n";
    for i in items.iter().filter(|i| !i.files.is_empty()) {
        out += &format!("## {}\n", i.name);
        for f in &i.files {
            out += &format!("- {f}\n");
        }
        out += "\n";
    }
    out += &format!(
        "{} items: {} PASS, {} FAIL, {} NOT RUN (a NOT RUN item is not a pass).\n",
        items.len(),
        count(Status::Pass),
        count(Status::Fail),
        count(Status::NotRun)
    );
    out
}

/// Empty the directory of a step before it runs, so files of an earlier run of the same date never count as this run's.
pub fn fresh_dir(base: &Path, dir: &str) -> Res {
    let d = base.join(dir);
    match std::fs::remove_dir_all(&d) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", d.display())),
    }
}

/// The files under `dir`, relative to `base`, with `/` separators, sorted.
pub fn list_files(base: &Path, dir: &str) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if let Ok(rel) = p.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(base, &base.join(dir), &mut out);
    out.sort();
    out
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let o = parse_args(args)?;
    let date = match &o.date {
        Some(d) => d.clone(),
        None => {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            crate::accept::utc_date(secs)
        }
    };
    let rel = format!("{}/{date}", o.out.as_deref().unwrap_or(PACKAGE_DIR));
    let base: PathBuf = if Path::new(&rel).is_absolute() { PathBuf::from(&rel) } else { root.join(&rel) };
    let steps = plan(o.store.as_deref(), &rel);
    if o.dry_run {
        for st in &steps {
            let cmd = if st.capture_args.is_empty() {
                "NOT RUN (no --store)".to_string()
            } else {
                format!("capture {}", st.capture_args.join(" "))
            };
            println!("{}: {cmd}", st.name);
        }
        return Ok(());
    }
    std::fs::create_dir_all(&base).map_err(|e| format!("{}: {e}", base.display()))?;
    let mut runner = |st: &Step| {
        fresh_dir(&base, &st.dir)?;
        crate::testcmd::capture_cmd(root, &st.capture_args)
    };
    let items = run_steps(&steps, &mut runner, &|d| list_files(&base, d));
    let commit = crate::util::capture(root, "git", &["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let md = render(&date, &commit, &items);
    let path = base.join("package.md");
    std::fs::write(&path, &md).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{md}wrote {}", path.display());
    if items.iter().any(|i| i.status == Status::Fail) {
        return Err("the package has a FAIL item (see package.md)".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(name: &str, args: &[&str]) -> Step {
        Step { name: name.into(), capture_args: args.iter().map(|s| s.to_string()).collect(), dir: name.into() }
    }

    // spec: TEST-074
    #[test]
    fn without_a_store_every_step_is_not_run_and_never_a_pass() {
        let steps = plan(None, "target/p");
        assert_eq!(steps.len(), 4, "three sites and the flights");
        let items = run_steps(&steps, &mut |_| panic!("no step may run without a store"), &|_| Vec::new());
        assert!(items.iter().all(|i| i.status == Status::NotRun && i.detail.contains("--store")));
        assert!(render("2026-10-30", "abc", &items).contains("4 items: 0 PASS, 0 FAIL, 4 NOT RUN"));
    }

    // spec: TEST-074
    #[test]
    fn a_failed_step_is_a_fail_with_its_message_and_the_others_still_run() {
        let steps = vec![step("a", &["x"]), step("b", &["y"]), step("c", &["z"])];
        let mut ran = Vec::new();
        let mut runner = |s: &Step| {
            ran.push(s.name.clone());
            if s.name == "b" {
                Err("no chunk landed in 60 s".to_string())
            } else {
                Ok(())
            }
        };
        let items = run_steps(&steps, &mut runner, &|d| vec![format!("{d}/frame.png")]);
        assert_eq!(ran, ["a", "b", "c"]);
        assert_eq!(items.iter().map(|i| i.status).collect::<Vec<_>>(), [Status::Pass, Status::Fail, Status::Pass]);
        assert_eq!(items[1].detail, "no chunk landed in 60 s");
        let md = render("2026-10-30", "abc", &items);
        assert!(md.contains("| b | FAIL | no chunk landed in 60 s |") && md.contains("2 PASS, 1 FAIL, 0 NOT RUN"), "{md}");
    }

    // spec: TEST-074
    #[test]
    fn a_command_that_wrote_nothing_is_a_fail_and_the_files_are_listed_sorted_and_relative() {
        let steps = vec![step("a", &["x"])];
        let items = run_steps(&steps, &mut |_| Ok(()), &|_| Vec::new());
        assert_eq!(items[0].status, Status::Fail);
        let dir = std::env::temp_dir().join(format!("planet-package-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("stills/s")).unwrap();
        std::fs::write(dir.join("stills/s/b.png"), b"1").unwrap();
        std::fs::write(dir.join("stills/s/a.png"), b"1").unwrap();
        assert_eq!(list_files(&dir, "stills"), ["stills/s/a.png", "stills/s/b.png"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // spec: TEST-074
    #[test]
    fn a_step_starts_from_an_empty_directory_so_files_of_an_earlier_run_never_count() {
        let dir = std::env::temp_dir().join(format!("planet-package-fresh-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("flights")).unwrap();
        std::fs::write(dir.join("flights/old.png"), b"1").unwrap();
        fresh_dir(&dir, "flights").unwrap();
        assert!(list_files(&dir, "flights").is_empty());
        fresh_dir(&dir, "never-made").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // spec: TEST-074
    #[test]
    fn the_options_are_parsed_and_an_unknown_one_is_refused() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let o = parse_args(&a(&["--store", "S", "--date", "2026-10-30", "--dry-run"])).unwrap();
        assert_eq!((o.store.as_deref(), o.date.as_deref(), o.dry_run), (Some("S"), Some("2026-10-30"), true));
        assert!(parse_args(&a(&["--wat"])).unwrap_err().contains("--wat"));
        assert!(parse_args(&a(&["--store"])).unwrap_err().contains("needs a value"));
        let steps = plan(Some("S"), "target/p/2026-10-30");
        assert_eq!(steps[0].capture_args[..3], ["stills", "--poses", "docs/milestones/M8/look-review-2b/hills-b/default/poses.toml"]);
        assert_eq!(steps[3].capture_args, ["flights", "--store", "S", "--out-dir", "target/p/2026-10-30/flights"]);
    }
}
