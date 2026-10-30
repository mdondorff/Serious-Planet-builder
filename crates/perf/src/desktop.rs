//! A locked desktop (TEST-072, owner decision 2026-10-30): real-GPU window checks report NOT RUN while the Windows session is locked, instead of
//! failing or timing out. With the session locked the present and acquire calls of a window stall for seconds and the device is lost (measured:
//! the dense-cluster sweep takes 26 s unlocked and times out at 300 s locked), so the result says nothing about the code.
//!
//! Detection is a cheap, locale-independent look for the lock screen's process (`LogonUI.exe`, which exists while the session is locked or at the
//! sign-in screen) in `tasklist /FO CSV`; elsewhere the desktop is "unlocked". The detector is a plain function value, so tests stub it.
//!
//! A NOT RUN is a reported, reasoned outcome and never a PASS: the check prints a `NOT RUN` line and leaves a marker file in the directory named by
//! `PLANET_NOT_RUN_DIR` (set by `cargo xtask`), which the xtask summaries count and `accept` turns into INCOMPLETE.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The environment variable that names the marker directory.
pub const MARKER_ENV: &str = "PLANET_NOT_RUN_DIR";

/// The state of the desktop at one look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Desktop {
    Unlocked,
    /// Locked, with what showed it.
    Locked(String),
}

/// The detector: how a caller looks at the desktop. Tests pass their own.
pub type Detector<'a> = &'a dyn Fn() -> Desktop;

/// Whether `tasklist /FO CSV /NH` output lists the lock screen process (case-insensitive, the image name only).
pub fn lists_lock_screen(tasklist_csv: &str) -> bool {
    tasklist_csv.lines().any(|l| l.trim_start_matches('"').to_ascii_lowercase().starts_with("logonui.exe"))
}

/// The real detector: Windows only (`tasklist`), else unlocked. A failure to look is "unlocked": a check never hides behind a broken probe.
pub fn detect() -> Desktop {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist").args(["/FI", "IMAGENAME eq LogonUI.exe", "/FO", "CSV", "/NH"]).output();
        if let Ok(out) = out {
            if lists_lock_screen(&String::from_utf8_lossy(&out.stdout)) {
                return Desktop::Locked("the lock screen (LogonUI.exe) is running".into());
            }
        }
    }
    Desktop::Unlocked
}

/// The marker directory of this process, from the environment.
pub fn marker_dir() -> Option<PathBuf> {
    std::env::var_os(MARKER_ENV).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `HH:MM:SS` (UTC) of a time in seconds since the epoch.
pub fn clock(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
}

/// The one line a NOT RUN prints. `started` and `ended` are seconds since the epoch (UTC clock in the text); `during` says the lock began mid-run.
pub fn not_run_line(name: &str, reason: &str, started: u64, ended: u64, during: bool) -> String {
    let when = if during {
        format!("the desktop was locked by the end of the run, which ran {} to {} UTC", clock(started), clock(ended))
    } else {
        format!("checked at {} UTC", clock(started))
    };
    format!("NOT RUN {name}: desktop locked ({reason}); {when}")
}

/// Print the NOT RUN line and leave the marker (one file per check, named after it).
pub fn report(name: &str, reason: &str, started: u64, ended: u64, during: bool, dir: Option<&Path>) -> String {
    let line = not_run_line(name, reason, started, ended, during);
    println!("{line}");
    if let Some(dir) = dir {
        let mut file: String = name.chars().take(48).map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        // The whole name, hashed (FNV-1a), keeps two names that differ in punctuation or beyond the cut apart.
        let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
        file.push_str(&format!("-{hash:016x}"));
        if std::fs::create_dir_all(dir).is_ok() {
            let _ = std::fs::write(dir.join(format!("{file}.txt")), format!("{line}\n"));
        }
    }
    line
}

/// Run `f`, a real-GPU window check named `name`, unless the desktop is locked. `hardware` is false for the software adapter, which is not
/// affected: `f` runs. Returns `None` for a NOT RUN: before the run when the desktop is already locked, or after a failure (a panic of `f`) when it
/// is locked by then (the failure is the lock's, reported with the times). Any other panic goes on.
pub fn guarded<R>(name: &str, hardware: bool, detect: Detector<'_>, dir: Option<&Path>, f: impl FnOnce() -> R) -> Option<R> {
    if !hardware {
        return Some(f());
    }
    let started = now_s();
    if let Desktop::Locked(reason) = detect() {
        report(name, &reason, started, started, false, dir);
        return None;
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => Some(r),
        Err(payload) => {
            if let Desktop::Locked(reason) = detect() {
                report(name, &reason, started, now_s(), true, dir);
                return None;
            }
            std::panic::resume_unwind(payload)
        }
    }
}

/// Watch a test that a nextest timeout would kill (TEST-072): poll the desktop every `poll`; once it has been seen locked and `fire_after` has passed since
/// `started`, report a NOT RUN with the times and return true. Returns false when `stop` is set first. A timeout that kills the process leaves no
/// chance to look at the desktop afterwards, so the look is taken while the test runs.
pub fn watch_until(
    name: &str,
    detect: Detector<'_>,
    poll: std::time::Duration,
    fire_after: std::time::Duration,
    started: std::time::Instant,
    dir: Option<&Path>,
    stop: &std::sync::atomic::AtomicBool,
) -> bool {
    let begun = now_s();
    let mut seen: Option<String> = None;
    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(poll);
        if let Desktop::Locked(reason) = detect() {
            seen.get_or_insert(reason);
        }
        if started.elapsed() >= fire_after {
            if let Some(reason) = &seen {
                report(name, reason, begun, now_s(), true, dir);
                return true;
            }
            // Not locked so far: the test goes on to its own limit.
            return false;
        }
    }
    false
}

/// Start the watchdog of a real-GPU window test in the background, once per process: when the desktop has been locked and the test is still running
/// after `fire_after`, the NOT RUN is reported and the process ends with success (nextest would otherwise terminate it as a timeout).
pub fn start_watchdog(name: &str, hardware: bool, fire_after: std::time::Duration) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if !hardware {
        return;
    }
    let name = name.to_string();
    ONCE.call_once(move || {
        let started = std::time::Instant::now();
        let dir = marker_dir();
        std::thread::spawn(move || {
            let stop = std::sync::atomic::AtomicBool::new(false);
            if watch_until(&name, &detect, std::time::Duration::from_secs(15), fire_after, started, dir.as_deref(), &stop) {
                std::process::exit(0);
            }
        });
    });
}

/// The marker files in `dir`: their lines, sorted.
pub fn markers(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Remove the markers (before a run).
pub fn clear(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("planet-notrun-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    // spec: TEST-072
    #[test]
    fn the_lock_screen_is_found_in_tasklist_output_in_any_locale() {
        assert!(lists_lock_screen("\"LogonUI.exe\",\"1234\",\"Console\",\"1\",\"35.000 K\"\r\n"));
        assert!(lists_lock_screen("\"logonui.EXE\",\"1\"\n"));
        // The "no task" message of any language starts with something else.
        assert!(!lists_lock_screen("INFO: No tasks are running which match the specified criteria.\n"));
        assert!(!lists_lock_screen("INFORMATION : aucune tâche en service ne correspond aux critères spécifiés.\n"));
        assert!(!lists_lock_screen("\"notLogonUI.exe\",\"2\"\n"));
        assert!(!lists_lock_screen(""));
    }

    // spec: TEST-072
    #[test]
    fn a_locked_desktop_is_a_not_run_with_its_reason_and_a_marker_and_an_unlocked_one_runs_the_check() {
        let dir = temp("locked");
        let ran = Cell::new(false);
        let locked = || Desktop::Locked("test lock".into());
        let r = guarded("gpu_window smoke", true, &locked, Some(&dir), || ran.set(true));
        assert!(r.is_none() && !ran.get(), "a locked desktop does not run the check");
        let lines = markers(&dir);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("NOT RUN gpu_window smoke: desktop locked (test lock); checked at "), "{}", lines[0]);
        // Unlocked: it runs and leaves no marker.
        clear(&dir);
        let unlocked = || Desktop::Unlocked;
        assert_eq!(guarded("x", true, &unlocked, Some(&dir), || 7), Some(7));
        assert!(markers(&dir).is_empty());
        // The software adapter is not affected, locked or not.
        assert_eq!(guarded("x", false, &locked, Some(&dir), || 8), Some(8));
        assert!(markers(&dir).is_empty());
        clear(&dir);
    }

    // spec: TEST-072
    #[test]
    fn the_watchdog_reports_a_not_run_when_the_desktop_was_seen_locked_and_the_test_is_still_running_at_the_time_it_would_be_cut_off() {
        use std::sync::atomic::AtomicBool;
        use std::time::{Duration, Instant};
        let dir = temp("watch");
        let stop = AtomicBool::new(false);
        let ms = Duration::from_millis;
        let locked = || Desktop::Locked("test lock".into());
        assert!(watch_until("sweep", &locked, ms(5), ms(30), Instant::now(), Some(&dir), &stop), "locked and still running: fired");
        let lines = markers(&dir);
        assert!(lines.len() == 1 && lines[0].contains("locked by the end of the run"), "{lines:?}");
        clear(&dir);
        let unlocked = || Desktop::Unlocked;
        assert!(!watch_until("sweep", &unlocked, ms(5), ms(30), Instant::now(), Some(&dir), &stop), "never locked: the test goes on");
        assert!(markers(&dir).is_empty());
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            !watch_until("sweep", &locked, ms(5), ms(30), Instant::now(), Some(&dir), &stop),
            "a test that ended first is not reported"
        );
        // Two names that differ only in punctuation leave two markers.
        report("a b", "r", 1, 1, false, Some(&dir));
        report("a-b", "r", 1, 1, false, Some(&dir));
        assert_eq!(markers(&dir).len(), 2);
        clear(&dir);
    }

    // spec: TEST-072
    #[test]
    fn a_failure_after_the_desktop_locked_mid_run_is_a_not_run_with_the_times_and_any_other_failure_goes_on() {
        let dir = temp("mid");
        // Unlocked at the start, locked at the end: the panic of the check is the lock's.
        let calls = Cell::new(0);
        let flips = || {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Desktop::Unlocked
            } else {
                Desktop::Locked("test lock".into())
            }
        };
        let r: Option<()> = guarded("sweep", true, &flips, Some(&dir), || panic!("timed out"));
        assert!(r.is_none());
        let lines = markers(&dir);
        assert!(
            lines.len() == 1 && lines[0].contains("locked by the end of the run, which ran ") && lines[0].contains(" UTC"),
            "{lines:?}"
        );
        clear(&dir);
        // Unlocked throughout: the failure stays a failure.
        let unlocked = || Desktop::Unlocked;
        let caught = std::panic::catch_unwind(|| guarded::<()>("sweep", true, &unlocked, None, || panic!("a real failure")));
        assert!(caught.is_err(), "a failure with the desktop unlocked is not a NOT RUN");
        assert!(markers(&dir).is_empty());
    }
}
