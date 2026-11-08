//! The disk gate (BUILD-039): `prepush` and the bake entry points check the free space of the drive of the worktree first. Below 20 GB they
//! refuse with the free space, the largest folders of the worktree and the prune command; below 50 GB they warn. std only: the free space
//! comes from `GetDiskFreeSpaceExW` on Windows and `df` elsewhere.

use crate::util::Res;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const GB: u64 = 1 << 30;
/// Below this many bytes free the gate refuses.
pub const REFUSE_BELOW: u64 = 20 * GB;
/// Below this many bytes free the gate warns.
pub const WARN_BELOW: u64 = 50 * GB;
/// The command that frees what the guardrails know how to free.
pub const PRUNE_COMMAND: &str = "cargo xtask worktrees --prune";

#[derive(Debug, PartialEq)]
pub enum Verdict {
    Ok,
    Warn(String),
    Refuse(String),
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / GB as f64)
}

/// The verdict for `free` bytes, with the largest own folders (name, bytes) for the message.
pub fn judge(free: u64, folders: &[(String, u64)]) -> Verdict {
    if free >= WARN_BELOW {
        return Verdict::Ok;
    }
    let largest: Vec<String> = folders.iter().take(5).map(|(n, b)| format!("{n} {}", gb(*b))).collect();
    let largest = if largest.is_empty() { "none measured".to_string() } else { largest.join(", ") };
    if free < REFUSE_BELOW {
        Verdict::Refuse(format!(
            "only {} free on the drive (the gate refuses below {}); largest folders of this worktree: {largest}; free space with `{PRUNE_COMMAND}` and by deleting target/ folders you no longer need",
            gb(free),
            gb(REFUSE_BELOW)
        ))
    } else {
        Verdict::Warn(format!(
            "{} free on the drive (a warning below {}, a refusal below {}); largest folders of this worktree: {largest}; `{PRUNE_COMMAND}` removes finished worktrees",
            gb(free),
            gb(WARN_BELOW),
            gb(REFUSE_BELOW)
        ))
    }
}

/// Free bytes on the drive that holds `path` (which must exist).
#[cfg(windows)]
pub fn free_bytes(path: &Path) -> Res<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is a NUL-terminated UTF-16 string and the three out-pointers are valid for the call.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) };
    if ok == 0 {
        return Err(format!("cannot read the free space of {}: {}", path.display(), std::io::Error::last_os_error()));
    }
    Ok(available)
}

#[cfg(not(windows))]
pub fn free_bytes(path: &Path) -> Res<u64> {
    let out = crate::util::capture(path, "df", &["-Pk", "."])?;
    parse_df(&out).ok_or_else(|| format!("cannot read the free space from `df`: {out}"))
}

#[cfg_attr(windows, allow(dead_code))]
/// The available kilobytes of the second line of `df -Pk`, in bytes.
pub fn parse_df(out: &str) -> Option<u64> {
    let line = out.lines().nth(1)?;
    line.split_whitespace().nth(3)?.parse::<u64>().ok().map(|k| k * 1024)
}

/// The size of a folder in bytes, and whether the walk finished before `deadline` (symbolic links are not followed).
pub fn dir_size(path: &Path, deadline: Instant) -> (u64, bool) {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(d) = stack.pop() {
        if Instant::now() > deadline {
            return (total, false);
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_symlink() {
                continue;
            }
            if t.is_dir() {
                stack.push(e.path());
            } else if let Ok(m) = e.metadata() {
                total += m.len();
            }
        }
    }
    (total, true)
}

/// The immediate folders of `root/target`, largest first, measured within about `budget`.
pub fn own_folders(root: &Path, budget: Duration) -> Vec<(String, u64)> {
    let deadline = Instant::now() + budget;
    let mut v: Vec<(String, u64)> = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join("target")) else { return v };
    for e in rd.flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            let (b, _) = dir_size(&e.path(), deadline);
            v.push((format!("target/{}", e.file_name().to_string_lossy()), b));
        }
    }
    v.sort_by_key(|f| std::cmp::Reverse(f.1));
    v
}

/// The drive probe path: the worktree itself (it exists, and `target/` is on its drive unless `CARGO_TARGET_DIR` moved it).
fn probe(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).filter(|p| p.exists()).unwrap_or_else(|| root.to_path_buf())
}

/// The gate: refuses below 20 GB free, warns below 50 GB. A free space that cannot be read is a warning, not a refusal.
pub fn check(root: &Path, what: &str) -> Res {
    let free = match free_bytes(&probe(root)) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{what}: disk gate skipped ({e})");
            return Ok(());
        }
    };
    let folders = if free < WARN_BELOW { own_folders(root, Duration::from_secs(8)) } else { Vec::new() };
    match judge(free, &folders) {
        Verdict::Ok => Ok(()),
        Verdict::Warn(m) => {
            eprintln!("{what}: WARNING {m}");
            Ok(())
        }
        Verdict::Refuse(m) => Err(format!("{what} refused: {m}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders() -> Vec<(String, u64)> {
        vec![("target/debug".into(), 30 * GB), ("target/test-tmp".into(), 2 * GB)]
    }

    // spec: BUILD-039
    #[test]
    fn the_gate_refuses_below_20_gb_warns_below_50_gb_and_passes_above() {
        assert_eq!(judge(80 * GB, &folders()), Verdict::Ok);
        assert_eq!(judge(50 * GB, &folders()), Verdict::Ok);
        let Verdict::Warn(w) = judge(35 * GB, &folders()) else { panic!("35 GB warns") };
        assert!(w.contains("35.0 GB") && w.contains(PRUNE_COMMAND), "{w}");
        let Verdict::Refuse(r) = judge(12 * GB, &folders()) else { panic!("12 GB refuses") };
        assert!(r.contains("12.0 GB") && r.contains("target/debug 30.0 GB") && r.contains(PRUNE_COMMAND), "{r}");
        assert!(matches!(judge(20 * GB - 1, &[]), Verdict::Refuse(_)) && matches!(judge(20 * GB, &[]), Verdict::Warn(_)));
    }

    // spec: BUILD-039
    #[test]
    fn the_free_space_is_read_from_df_output() {
        let out = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/sda1 1000 400 600 40% /\n";
        assert_eq!(parse_df(out), Some(600 * 1024));
        assert_eq!(parse_df("nothing"), None);
    }

    // spec: BUILD-039
    #[test]
    fn the_real_free_space_of_the_worktree_is_readable() {
        assert!(free_bytes(&crate::util::repo_root()).unwrap() > 0);
    }

    // spec: BUILD-039
    #[test]
    fn a_folder_is_measured_and_an_expired_deadline_stops_the_walk() {
        let d = std::env::temp_dir().join(format!("disk-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a").join("f"), [0u8; 1000]).unwrap();
        std::fs::write(d.join("g"), [0u8; 24]).unwrap();
        assert_eq!(dir_size(&d, Instant::now() + Duration::from_secs(60)), (1024, true));
        assert!(!dir_size(&d, Instant::now() - Duration::from_secs(1)).1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
