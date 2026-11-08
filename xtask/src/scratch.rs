//! Test scratch under `target/` (BUILD-038): `cargo xtask test`, `prepush` and `accept` point the temp directory of every child
//! (`TMP`, `TEMP`, `TMPDIR`, which `std::env::temp_dir` reads) at one folder of the worktree, cleared when the next run starts, so a
//! run, even a killed one, leaves nothing in the system temp directory and the last run stays for diagnosis until the next one.

use crate::util::Res;
use std::path::{Path, PathBuf};

/// The scratch folder of a worktree, relative to its root.
pub const DIR: &str = "target/test-tmp";

/// The variables the standard library and the tools read for the temp directory.
pub const VARS: [&str; 3] = ["TMP", "TEMP", "TMPDIR"];

pub fn dir(root: &Path) -> PathBuf {
    root.join(DIR)
}

/// The variable assignments that move the temp directory to `dir`.
pub fn env_pairs(dir: &Path) -> Vec<(&'static str, String)> {
    VARS.iter().map(|v| (*v, dir.to_string_lossy().into_owned())).collect()
}

/// Clears the folder of the previous run and creates it empty. A folder that cannot be cleared (a file still held open) is a warning, not
/// a failure: the run goes on in what is left.
pub fn prepare(root: &Path) -> Res<PathBuf> {
    let d = dir(root);
    if d.exists() {
        if let Err(e) = std::fs::remove_dir_all(&d) {
            eprintln!("scratch: {} was not fully cleared ({e}); the run goes on in what is left", d.display());
        }
    }
    std::fs::create_dir_all(&d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    Ok(d)
}

/// Starts a run's scratch: clears the previous one, drops the git worktree records of temporary worktrees that were in it, and exports the
/// variables, which every child process inherits. Called once at the start of a run, inside the heavy-run lock.
pub fn begin(root: &Path) -> Res {
    let had = dir(root).exists();
    let d = prepare(root)?;
    if had {
        let _ = crate::util::capture(root, "git", &["worktree", "prune"]);
    }
    for (k, v) in env_pairs(&d) {
        std::env::set_var(k, v);
    }
    eprintln!("scratch: temp directory {} (cleared at the start of the next run)", d.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("scratch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    // spec: BUILD-038
    #[test]
    fn the_scratch_is_under_target_and_the_next_run_clears_what_the_last_one_left() {
        let root = work("clear");
        let d = prepare(&root).unwrap();
        assert_eq!(d, root.join("target").join("test-tmp"));
        std::fs::create_dir_all(d.join("planet-x-123")).unwrap();
        std::fs::write(d.join("planet-x-123").join("a.bin"), b"left by a killed run").unwrap();
        let again = prepare(&root).unwrap();
        assert_eq!(again, d);
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0, "the leftovers of the last run are gone");
        let _ = std::fs::remove_dir_all(&root);
    }

    // spec: BUILD-038
    #[test]
    fn all_three_temp_variables_point_at_the_scratch() {
        let pairs = env_pairs(Path::new("/w/target/test-tmp"));
        let names: Vec<&str> = pairs.iter().map(|(k, _)| *k).collect();
        assert_eq!(names, ["TMP", "TEMP", "TMPDIR"]);
        assert!(pairs.iter().all(|(_, v)| v.ends_with("test-tmp")));
    }

    // spec: BUILD-038
    #[test]
    fn no_source_names_a_system_temp_path_of_its_own() {
        // The tests honour the redirection only if they ask `std::env::temp_dir` and never name a system folder.
        let needles = [["\"/t", "mp/"].concat(), ["Local", "\\\\Temp"].concat(), ["\"C:", "\\\\Temp"].concat()];
        let mut files = Vec::new();
        let root = crate::util::repo_root();
        for sub in ["crates", "xtask"] {
            crate::util::files_with_ext(&root.join(sub), "rs", &mut files);
        }
        let mut hits = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap_or_default();
            for n in &needles {
                if text.contains(n.as_str()) {
                    hits.push(format!("{} names {n}", f.display()));
                }
            }
        }
        assert!(hits.is_empty(), "paths that bypass the scratch: {hits:?}");
    }
}
