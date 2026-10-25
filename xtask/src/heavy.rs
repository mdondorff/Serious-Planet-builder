//! The machine-wide heavy-run lock (BUILD-026): `test`, `prepush`, `perf`, `capture`, `accept` and `store` take one lock outside every worktree and
//! wait their turn instead of running in parallel (three lanes' runs each got about one core: serialised, each finishes sooner), and the owner's
//! quiet flag file makes every heavy command wait for as long as it exists, so that the owner can clear the machine without messaging anyone.
//!
//! The lock is an OS file lock (`File::try_lock`), which the system releases when the process ends however it ends, so there is no stale lock to
//! clean up; a sidecar file names the holder for the message of whoever waits. Where the state directory cannot be made, or `PLANET_HEAVY_LOCK=off`,
//! the command runs without the lock and says so: the lock is a courtesy of the tooling, never a reason a command cannot run.
//!
//! The state directory is `PLANET_STATE_DIR`, else `%LOCALAPPDATA%\planet`, else `$XDG_STATE_HOME/planet`, else `~/.local/state/planet`.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How often a waiting command looks again.
const POLL: Duration = Duration::from_secs(2);
/// A waiting command repeats its message this often.
const SAY_EVERY: Duration = Duration::from_secs(30);

/// Whether this process holds the lock (a command that runs another heavy command in-process, as `prepush` does, takes it once).
static HELD: AtomicBool = AtomicBool::new(false);

/// The directory of the lock, the holder file and the quiet flag, if one can be found.
pub fn state_dir() -> Option<PathBuf> {
    let from_env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    from_env("PLANET_STATE_DIR")
        .or_else(|| from_env("LOCALAPPDATA").map(|p| p.join("planet")))
        .or_else(|| from_env("XDG_STATE_HOME").map(|p| p.join("planet")))
        .or_else(|| from_env("HOME").map(|p| p.join(".local").join("state").join("planet")))
}

fn lock_path(dir: &Path) -> PathBuf {
    dir.join("heavy.lock")
}

fn holder_path(dir: &Path) -> PathBuf {
    dir.join("heavy.holder")
}

/// The owner's flag file: while it exists every heavy command waits.
pub fn quiet_path(dir: &Path) -> PathBuf {
    dir.join("quiet")
}

/// The lock while a heavy command runs; dropping it lets the next in the queue go.
pub struct Guard {
    file: Option<File>,
    dir: Option<PathBuf>,
    /// This guard set [`HELD`] and clears it.
    outer: bool,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_file(holder_path(dir));
        }
        // The file lock goes with the handle.
        self.file.take();
        if self.outer {
            HELD.store(false, Ordering::SeqCst);
        }
    }
}

/// Why a try for the lock did not give it.
#[derive(Debug, PartialEq, Eq)]
pub enum Busy {
    /// The quiet flag file exists.
    Quiet(PathBuf),
    /// Another command holds the lock; what its holder file says.
    Held(String),
}

/// Take the lock if nobody holds it and the quiet flag is not up; never waits.
pub fn try_acquire_in(dir: &Path, what: &str) -> std::io::Result<Result<Guard, Busy>> {
    std::fs::create_dir_all(dir)?;
    if quiet_path(dir).exists() {
        return Ok(Err(Busy::Quiet(quiet_path(dir))));
    }
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(lock_path(dir))?;
    match file.try_lock() {
        Ok(()) => {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let worktree =
                std::env::current_dir().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_default();
            let text = format!("{what} (process {}, in {worktree}, since {secs} s after the epoch)", std::process::id());
            std::fs::File::create(holder_path(dir)).and_then(|mut f| f.write_all(text.as_bytes()))?;
            Ok(Ok(Guard { file: Some(file), dir: Some(dir.to_path_buf()), outer: false }))
        }
        Err(TryLockError::WouldBlock) => {
            let holder = std::fs::read_to_string(holder_path(dir)).unwrap_or_default();
            let holder = if holder.is_empty() { "another heavy command (no holder note)".to_string() } else { holder };
            Ok(Err(Busy::Held(holder)))
        }
        Err(TryLockError::Error(e)) => Err(e),
    }
}

/// Wait for the lock (and the quiet flag to go), saying why every 30 s; `give_up` ends the wait with `None` (for tests; a command waits for good).
pub fn acquire_in(
    dir: &Path,
    what: &str,
    poll: Duration,
    give_up: Option<Duration>,
    say: &mut dyn FnMut(&str),
) -> std::io::Result<Option<Guard>> {
    let began = Instant::now();
    let mut last_said: Option<Instant> = None;
    loop {
        match try_acquire_in(dir, what)? {
            Ok(guard) => {
                if last_said.is_some() {
                    say(&format!("the machine is free after {:.0} s: running `{what}`", began.elapsed().as_secs_f64()));
                }
                return Ok(Some(guard));
            }
            Err(busy) => {
                if last_said.is_none_or(|t| t.elapsed() >= SAY_EVERY) {
                    last_said = Some(Instant::now());
                    say(&match &busy {
                        Busy::Quiet(p) => format!(
                            "waiting: the owner's quiet flag {} is up (`cargo xtask quiet off` or deleting the file lets heavy commands run); `{what}` is queued",
                            p.display()
                        ),
                        Busy::Held(holder) => format!("waiting for {holder}; `{what}` is queued"),
                    });
                }
            }
        }
        if give_up.is_some_and(|g| began.elapsed() >= g) {
            return Ok(None);
        }
        std::thread::sleep(poll);
    }
}

/// The lock for a heavy command `what`: waits its turn, prints why it waits, and never fails (a state directory that cannot be made, or
/// `PLANET_HEAVY_LOCK=off`, runs the command unlocked and says so).
pub fn acquire(what: &str) -> Guard {
    let unlocked = |why: &str| {
        eprintln!("heavy-run lock: {why}; `{what}` runs without it");
        Guard { file: None, dir: None, outer: false }
    };
    if std::env::var("PLANET_HEAVY_LOCK").is_ok_and(|v| v == "off") {
        return unlocked("PLANET_HEAVY_LOCK=off");
    }
    if HELD.swap(true, Ordering::SeqCst) {
        // A heavy command inside a heavy command of this process: the outer one holds it.
        return Guard { file: None, dir: None, outer: false };
    }
    let Some(dir) = state_dir() else {
        HELD.store(false, Ordering::SeqCst);
        return unlocked("no state directory (set PLANET_STATE_DIR)");
    };
    match acquire_in(&dir, what, POLL, None, &mut |m| eprintln!("heavy-run lock: {m}")) {
        Ok(Some(mut guard)) => {
            guard.outer = true;
            guard
        }
        Ok(None) => unreachable!("a command waits for good"),
        Err(e) => {
            HELD.store(false, Ordering::SeqCst);
            unlocked(&format!("cannot use {} ({e})", dir.display()))
        }
    }
}

/// Run `f` as the heavy command `what`.
pub fn with<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let _guard = acquire(what);
    f()
}

/// `cargo xtask quiet on|off|status`: the owner's flag file.
pub fn quiet(args: &[String]) -> Result<(), String> {
    let dir = state_dir().ok_or("no state directory (set PLANET_STATE_DIR)")?;
    quiet_in(&dir, args)
}

/// [`quiet`] in the state directory `dir`.
pub fn quiet_in(dir: &Path, args: &[String]) -> Result<(), String> {
    let flag = quiet_path(dir);
    match args.first().map(String::as_str) {
        Some("on") => {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            std::fs::write(&flag, "the owner needs the machine\n").map_err(|e| format!("{}: {e}", flag.display()))?;
            println!("quiet: on ({}): every heavy command waits until `cargo xtask quiet off`", flag.display());
        }
        Some("off") => {
            match std::fs::remove_file(&flag) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", flag.display())),
            }
            println!("quiet: off: queued heavy commands go on");
        }
        Some("status") | None => {
            let holder = std::fs::read_to_string(holder_path(dir)).unwrap_or_default();
            println!(
                "quiet flag: {} ({})\nheavy lock holder note: {}",
                if flag.exists() { "up" } else { "down" },
                flag.display(),
                if holder.is_empty() { "none" } else { holder.trim() }
            );
        }
        Some(other) => return Err(format!("quiet takes on, off or status, not `{other}`")),
    }
    Ok(())
}

/// `cargo xtask heavy <command> [args]`: run any command (a diagnostic example, a capture) under the lock.
pub fn run_wrapped(root: &Path, args: &[String]) -> Result<(), String> {
    let Some(program) = args.first() else {
        return Err("heavy needs a command: `cargo xtask heavy cargo run --release -p planet --example ...`".into());
    };
    let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
    with(&args.join(" "), || crate::util::run(root, program, &rest, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("planet-heavy-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    // spec: BUILD-026
    #[test]
    fn a_second_heavy_command_waits_and_names_the_holder_and_goes_when_the_first_is_done() {
        let d = dir("lock");
        let first = try_acquire_in(&d, "test gpu").unwrap().expect("the machine is free");
        let mut said = Vec::new();
        let second =
            acquire_in(&d, "prepush", Duration::from_millis(10), Some(Duration::from_millis(60)), &mut |m| said.push(m.to_string()))
                .unwrap();
        assert!(second.is_none(), "the lock is held: the second waits");
        assert!(said.iter().any(|m| m.contains("waiting for test gpu") && m.contains("`prepush` is queued")), "{said:?}");
        match try_acquire_in(&d, "x").unwrap() {
            Err(Busy::Held(h)) => assert!(h.starts_with("test gpu (process "), "{h}"),
            other => panic!("{:?}", other.map(|_| ())),
        }
        drop(first);
        let mut said = Vec::new();
        let third =
            acquire_in(&d, "prepush", Duration::from_millis(10), Some(Duration::from_secs(5)), &mut |m| said.push(m.to_string())).unwrap();
        assert!(third.is_some(), "the lock is free again");
        assert!(said.is_empty(), "no message when nobody waited: {said:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn the_owners_quiet_flag_holds_every_heavy_command_until_it_is_removed() {
        let d = dir("quiet");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(quiet_path(&d), "x").unwrap();
        let mut said = Vec::new();
        let got =
            acquire_in(&d, "test", Duration::from_millis(10), Some(Duration::from_millis(50)), &mut |m| said.push(m.to_string())).unwrap();
        assert!(got.is_none(), "the flag is up: nothing runs");
        assert!(said.iter().any(|m| m.contains("quiet flag") && m.contains("`test` is queued")), "{said:?}");
        assert_eq!(try_acquire_in(&d, "x").unwrap().err(), Some(Busy::Quiet(quiet_path(&d))));
        std::fs::remove_file(quiet_path(&d)).unwrap();
        assert!(try_acquire_in(&d, "test").unwrap().is_ok(), "the flag is down: the command goes");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_waiter_in_another_thread_goes_as_soon_as_the_holder_lets_go() {
        let d = dir("thread");
        let first = try_acquire_in(&d, "capture stills").unwrap().expect("free");
        let d2 = d.clone();
        let waiter = std::thread::Builder::new()
            .name("test-waiter".into())
            .spawn(move || {
                acquire_in(&d2, "perf", Duration::from_millis(10), Some(Duration::from_secs(10)), &mut |_| {}).unwrap().is_some()
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        drop(first);
        assert!(waiter.join().unwrap(), "the queued command ran once the first was done");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn the_quiet_command_raises_and_lowers_the_flag() {
        let d = dir("cmd");
        std::fs::create_dir_all(&d).unwrap();
        quiet_in(&d, &["on".to_string()]).unwrap();
        assert!(quiet_path(&d).exists());
        quiet_in(&d, &["status".to_string()]).unwrap();
        quiet_in(&d, &["off".to_string()]).unwrap();
        assert!(!quiet_path(&d).exists());
        quiet_in(&d, &["off".to_string()]).unwrap();
        assert!(quiet_in(&d, &["maybe".to_string()]).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
