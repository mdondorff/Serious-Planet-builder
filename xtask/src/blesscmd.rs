//! `cargo xtask bless`: the owner's `/bless` apply step (BUILD-013). Lists the candidate goldens of every adapter (the local run's
//! `target/review/candidates/` and any `--extra` directory of `<adapter>/<name>.png`, such as the CI artifacts) and, with
//! `--apply <name>[,<name>]... --approved-by <who>`, promotes the candidates of exactly those goldens on every adapter that has one.
//! The policy (ADR 0017) stays in the `bless` skill: nothing is promoted without the owner's answer there, and this command only
//! writes what it is told the owner approved. The driver gate is not relaxed: a candidate from another driver than the recorded one
//! is refused, and nothing is written when any candidate is refused.

use crate::util::Res;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `(adapter, golden name)` to the candidate's PNG.
pub type Candidates = BTreeMap<(String, String), PathBuf>;

/// The candidate goldens waiting: `<root>/target/review/candidates/<adapter>/<name>.png`, then each `extra` directory
/// (`<extra>/<adapter>/<name>.png`; a later source replaces an earlier one). A candidate byte-equal to its golden is not waiting.
pub fn collect(root: &Path, extras: &[PathBuf]) -> Candidates {
    let mut out = Candidates::new();
    let sources = std::iter::once(root.join("target").join("review").join("candidates")).chain(extras.iter().map(|e| root.join(e)));
    for dir in sources {
        let Ok(adapters) = std::fs::read_dir(&dir) else { continue };
        for a in adapters.flatten().filter(|e| e.path().is_dir()) {
            let adapter = a.file_name().to_string_lossy().into_owned();
            for f in std::fs::read_dir(a.path()).into_iter().flatten().flatten() {
                let p = f.path();
                if p.extension().is_some_and(|x| x == "png") {
                    let name = p.file_stem().unwrap().to_string_lossy().into_owned();
                    out.insert((adapter.clone(), name), p);
                }
            }
        }
    }
    out.retain(|(adapter, name), path| {
        let golden = root.join("tests").join("goldens").join(adapter).join(format!("{name}.png"));
        !(golden.is_file() && std::fs::read(&golden).ok() == std::fs::read(path).ok())
    });
    out
}

/// The driver gate (ADR 0017): `Some(reason)` when the candidate's `driver.txt` (beside it) differs from the recorded one.
fn driver_problem(root: &Path, adapter: &str, candidate: &Path) -> Option<String> {
    let read = |p: PathBuf| std::fs::read_to_string(p).ok().map(|s| s.trim().to_string());
    let recorded = read(root.join("tests").join("goldens").join(adapter).join("driver.txt"));
    let cand = candidate.parent().and_then(|d| read(d.join("driver.txt")));
    match (recorded, cand) {
        (Some(r), Some(c)) if r != c => Some(format!("driver changed on {adapter}: recorded '{r}', candidate '{c}'")),
        (None, Some(c)) => Some(format!("{adapter} has no recorded driver; candidate driver '{c}'")),
        (Some(r), None) => Some(format!("{adapter} has recorded driver '{r}' but the candidate has no driver.txt beside it")),
        _ => None,
    }
}

/// Promotes the candidates of `names` on every adapter that has one and removes the names from `tests/goldens/pending.txt`.
/// Returns the `adapter/name` promoted. A name without a candidate, or a candidate failing the driver gate (a different driver, or
/// none beside it where one is recorded), is an error and nothing is written; an I/O error while copying stops there (what was
/// copied stays, and the error says so) because there is no staging.
pub fn apply(root: &Path, names: &[String], extras: &[PathBuf]) -> Res<Vec<String>> {
    let all = collect(root, extras);
    let mut plan: Vec<(String, String, PathBuf)> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    for name in names {
        let found: Vec<_> = all.iter().filter(|((_, n), _)| n == name).collect();
        if found.is_empty() {
            problems.push(format!("{name}: no candidate on any adapter (give the CI artifact directories with --extra)"));
        }
        for ((adapter, _), path) in found {
            match driver_problem(root, adapter, path) {
                Some(why) => problems.push(format!("{adapter}/{name}: {why}")),
                None => plan.push((adapter.clone(), name.clone(), path.clone())),
            }
        }
    }
    if !problems.is_empty() {
        return Err(format!("nothing promoted:\n  {}", problems.join("\n  ")));
    }
    let mut done = Vec::new();
    for (adapter, name, path) in plan {
        let dst = root.join("tests").join("goldens").join(&adapter).join(format!("{name}.png"));
        std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| format!("cannot create {}: {e}", dst.display()))?;
        std::fs::copy(&path, &dst).map_err(|e| format!("cannot copy {} to {}: {e}", path.display(), dst.display()))?;
        done.push(format!("{adapter}/{name}"));
    }
    let pending = root.join("tests").join("goldens").join("pending.txt");
    if let Ok(text) = std::fs::read_to_string(&pending) {
        let kept: Vec<&str> = text.lines().filter(|l| !names.iter().any(|n| l.trim() == n)).collect();
        std::fs::write(&pending, kept.join("\n") + "\n")
            .map_err(|e| format!("promoted {}, but cannot update {}: {e}", done.join(", "), pending.display()))?;
    }
    Ok(done)
}

const USAGE: &str =
    "usage: cargo xtask bless [--extra <dir with <adapter>/<name>.png>]... [--apply <name>[,<name>]... --approved-by <who>]";

/// `cargo xtask bless [...]`.
pub fn run(root: &Path, args: &[String]) -> Res {
    let (mut extras, mut names, mut by) = (Vec::<PathBuf>::new(), Vec::<String>::new(), None::<String>);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |what: &str| it.next().cloned().ok_or(format!("{a} needs {what}\n{USAGE}"));
        match a.as_str() {
            "--extra" => extras.push(PathBuf::from(value("a directory")?)),
            "--apply" => names.extend(value("golden names")?.split(',').filter(|s| !s.is_empty()).map(String::from)),
            "--approved-by" => by = Some(value("who approved")?),
            other => return Err(format!("unknown bless argument '{other}'\n{USAGE}")),
        }
    }
    if names.is_empty() {
        if by.is_some() {
            return Err(format!("--approved-by without --apply\n{USAGE}"));
        }
        return list(root, &extras);
    }
    let Some(by) = by.filter(|b| !b.trim().is_empty()) else {
        return Err(format!("--apply needs --approved-by <who>: only what the owner approved in /bless is promoted\n{USAGE}"));
    };
    let done = apply(root, &names, &extras)?;
    eprintln!("promoted {} golden(s), approved by {by}: {}", done.len(), done.join(", "));
    eprintln!("commit them as `bless: {}` with `Owner-approved: {by}` in the message body", names.join(", "));
    Ok(())
}

fn list(root: &Path, extras: &[PathBuf]) -> Res {
    let all = collect(root, extras);
    if all.is_empty() {
        eprintln!("no candidate goldens in target/review/candidates/ or the --extra directories");
        return Ok(());
    }
    for (adapter, name) in all.keys() {
        let golden = root.join("tests").join("goldens").join(adapter).join(format!("{name}.png"));
        eprintln!("{adapter}/{name}: {}", if golden.is_file() { "differs from golden" } else { "no golden yet" });
    }
    eprintln!("Promotion is owner-only: run /bless in a Claude Code session and answer each prompt.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-bless-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(path: PathBuf, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Goldens `v` on warp, llvmpipe and rtx (old); a candidate for rtx under `target/review/candidates`, for warp and
    /// llvmpipe in two extra directories; all with driver.txt.
    fn fixture(name: &str) -> PathBuf {
        let r = root(name);
        for a in ["warp", "llvmpipe", "rtx"] {
            put(r.join("tests/goldens").join(a).join("v.png"), "old");
            put(r.join("tests/goldens").join(a).join("driver.txt"), "drv-1\n");
        }
        put(r.join("tests/goldens/pending.txt"), "# list\nv\nother\n");
        put(r.join("target/review/candidates/rtx/v.png"), "new-rtx");
        put(r.join("target/review/candidates/rtx/driver.txt"), "drv-1");
        put(r.join("target/review/ci-windows/candidates/warp/v.png"), "new-warp");
        put(r.join("target/review/ci-windows/candidates/warp/driver.txt"), "drv-1");
        put(r.join("target/review/ci-linux/candidates/llvmpipe/v.png"), "new-llvmpipe");
        put(r.join("target/review/ci-linux/candidates/llvmpipe/driver.txt"), "drv-1");
        r
    }

    fn extras() -> Vec<PathBuf> {
        vec!["target/review/ci-windows/candidates".into(), "target/review/ci-linux/candidates".into()]
    }

    // spec: BUILD-013
    #[test]
    fn candidates_come_from_the_local_run_and_every_extra_directory_and_equal_ones_are_not_waiting() {
        let r = fixture("collect");
        put(r.join("target/review/candidates/warp/same.png"), "s");
        put(r.join("tests/goldens/warp/same.png"), "s");
        let all = collect(&r, &extras());
        let keys: Vec<_> = all.keys().map(|(a, n)| format!("{a}/{n}")).collect();
        assert_eq!(keys, ["llvmpipe/v", "rtx/v", "warp/v"]);
        assert_eq!(collect(&r, &[]).len(), 1, "without --extra only the local run's candidate");
        // A later source replaces an earlier one: the extra's rtx candidate wins over the local run's.
        put(r.join("target/review/ci-linux/candidates/rtx/v.png"), "later");
        let later = collect(&r, &extras());
        assert_eq!(std::fs::read_to_string(&later[&("rtx".to_string(), "v".to_string())]).unwrap(), "later");
        let _ = std::fs::remove_dir_all(&r);
    }

    // spec: BUILD-013
    #[test]
    fn apply_promotes_the_named_golden_on_every_adapter_from_every_source_and_leaves_the_rest() {
        let r = fixture("apply");
        put(r.join("target/review/candidates/rtx/w.png"), "other-golden");
        let done = apply(&r, &["v".to_string()], &extras()).unwrap();
        assert_eq!(done, ["llvmpipe/v", "rtx/v", "warp/v"]);
        let read = |a: &str, n: &str| std::fs::read_to_string(r.join("tests/goldens").join(a).join(n)).unwrap();
        assert_eq!(
            (read("warp", "v.png"), read("llvmpipe", "v.png"), read("rtx", "v.png")),
            ("new-warp".into(), "new-llvmpipe".into(), "new-rtx".into())
        );
        assert!(!r.join("tests/goldens/rtx/w.png").exists(), "a golden that was not named is not promoted");
        assert_eq!(read("rtx", "driver.txt"), "drv-1\n", "driver.txt is never touched");
        assert_eq!(std::fs::read_to_string(r.join("tests/goldens/pending.txt")).unwrap(), "# list\nother\n", "the name leaves pending.txt");
        let _ = std::fs::remove_dir_all(&r);
    }

    // spec: BUILD-013
    #[test]
    fn a_changed_driver_or_an_unknown_name_promotes_nothing() {
        let r = fixture("refuse");
        put(r.join("target/review/ci-linux/candidates/llvmpipe/driver.txt"), "drv-2");
        let err = apply(&r, &["v".to_string()], &extras()).unwrap_err();
        assert!(err.contains("llvmpipe/v: driver changed on llvmpipe"), "{err}");
        let err = apply(&r, &["nope".to_string()], &extras()).unwrap_err();
        assert!(err.contains("nope: no candidate"), "{err}");
        put(r.join("target/review/ci-linux/candidates/llvmpipe/driver.txt"), "drv-1");
        std::fs::remove_file(r.join("target/review/ci-windows/candidates/warp/driver.txt")).unwrap();
        let err = apply(&r, &["v".to_string()], &extras()).unwrap_err();
        assert!(err.contains("warp/v: warp has recorded driver 'drv-1' but the candidate has no driver.txt"), "{err}");
        put(r.join("target/review/ci-windows/candidates/warp/driver.txt"), "drv-1");
        std::fs::remove_file(r.join("tests/goldens/rtx/driver.txt")).unwrap();
        let err = apply(&r, &["v".to_string()], &extras()).unwrap_err();
        assert!(err.contains("rtx/v: rtx has no recorded driver; candidate driver 'drv-1'"), "{err}");
        assert_eq!(
            std::fs::read_to_string(r.join("tests/goldens/rtx/v.png")).unwrap(),
            "old",
            "all or nothing: the good adapters were not written"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    // spec: BUILD-013
    #[test]
    fn apply_needs_an_approval_and_the_arguments_are_checked() {
        let r = fixture("args");
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let err = run(&r, &a(&["--apply", "v"])).unwrap_err();
        assert!(err.contains("--approved-by"), "{err}");
        assert!(run(&r, &a(&["--approved-by", "owner"])).is_err(), "an approval without names");
        assert!(run(&r, &a(&["--apply", "v", "--approved-by", " "])).is_err(), "a blank approval");
        assert!(run(&r, &a(&["--bogus"])).is_err());
        assert_eq!(std::fs::read_to_string(r.join("tests/goldens/rtx/v.png")).unwrap(), "old", "nothing was written");
        run(
            &r,
            &a(&[
                "--extra",
                "target/review/ci-windows/candidates",
                "--extra",
                "target/review/ci-linux/candidates",
                "--apply",
                "v",
                "--approved-by",
                "owner 2026-10-18",
            ]),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(r.join("tests/goldens/warp/v.png")).unwrap(), "new-warp");
        let _ = std::fs::remove_dir_all(&r);
    }
}
