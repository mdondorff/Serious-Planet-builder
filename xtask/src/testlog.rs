//! The full output of every `cargo xtask test` run is kept in `target/test-logs/<command>-<UTC date>-<UTC time>.log` (BUILD-012):
//! the build and the nextest run, stdout and stderr in the order they arrive, so a failing test's name and output are never lost
//! to a summary or to the scrollback. The last [`KEEP`] logs of each command are kept.

use crate::util::Res;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Where the logs go, relative to the repository root.
pub const DIR: &str = "target/test-logs";
/// How many logs of one command are kept.
pub const KEEP: usize = 20;

/// `(year, month, day)` of a day count since 1970-01-01 (proleptic Gregorian; Howard Hinnant's civil-from-days).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `<command>-<YYYYMMDD>-<HHMMSS>.log` (UTC) for `secs` seconds after the Unix epoch.
pub fn log_name(command: &str, secs: u64) -> String {
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    let (y, m, d) = civil(days);
    format!("{command}-{y:04}{m:02}{d:02}-{:02}{:02}{:02}.log", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The command key of a `test` run: `test`, `test-fast`, `test-gpu` or `test-gpu-real`.
pub fn command_key(sub: Option<&str>, real: bool) -> String {
    match (sub, real) {
        (Some(s), true) => format!("test-{s}-real"),
        (Some(s), false) => format!("test-{s}"),
        (None, _) => "test".into(),
    }
}

/// True when `file` is a log of `command`: `<command>-` and a date and a time of digits (`test` does not match `test-gpu-...`).
fn is_log_of(file: &str, command: &str) -> bool {
    let Some(rest) = file.strip_prefix(command).and_then(|r| r.strip_prefix('-')).and_then(|r| r.strip_suffix(".log")) else {
        return false;
    };
    let mut parts = rest.split('-');
    let digits = |p: Option<&str>, n: usize| p.is_some_and(|p| p.len() >= n && p.bytes().all(|b| b.is_ascii_digit()));
    digits(parts.next(), 8) && digits(parts.next(), 6) && parts.next().is_none_or(|n| n.bytes().all(|b| b.is_ascii_digit()))
}

/// Deletes all but the newest `keep` logs of `command` in `dir` (the names sort by time).
pub fn rotate(dir: &Path, command: &str, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut logs: Vec<String> = rd.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|f| is_log_of(f, command)).collect();
    logs.sort();
    for old in logs.iter().take(logs.len().saturating_sub(keep)) {
        let _ = std::fs::remove_file(dir.join(old));
    }
}

/// A new, empty log of `command` in `dir` (created, with older logs rotated out); a second run in the same second gets `-2`, `-3`, ....
pub fn new_log(dir: &Path, command: &str, secs: u64) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let name = log_name(command, secs);
    for n in 1.. {
        let path = if n == 1 { dir.join(&name) } else { dir.join(name.replace(".log", &format!("-{n}.log"))) };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => {
                rotate(dir, command, KEEP);
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("cannot create {}: {e}", path.display())),
        }
    }
    unreachable!()
}

/// Like [`crate::util::run`], and every line of the child's stdout and stderr is also appended to `log`, in arrival order.
pub fn run_logged(root: &Path, program: &str, args: &[&str], env: &[(&str, &str)], log: &Path) -> Res {
    let header = format!("$ {program} {}", args.join(" "));
    eprintln!("{header}");
    let file = std::fs::OpenOptions::new().append(true).open(log).map_err(|e| format!("cannot open {}: {e}", log.display()))?;
    let file = Arc::new(Mutex::new(file));
    let _ = writeln!(file.lock().unwrap(), "{header}");
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(root).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| format!("cannot start `{program}`: {e}"))?;
    let pump = |reader: Box<dyn std::io::Read + Send>, to_stderr: bool| {
        let file = Arc::clone(&file);
        std::thread::spawn(move || {
            for line in BufReader::new(reader).split(b'\n').map_while(Result::ok) {
                let text = String::from_utf8_lossy(&line);
                let text = text.trim_end_matches('\r');
                // A closed console (`| head`) must not stop the pump: the log still gets every line.
                let _ = if to_stderr { writeln!(std::io::stderr(), "{text}") } else { writeln!(std::io::stdout(), "{text}") };
                let _ = writeln!(file.lock().unwrap(), "{text}");
            }
        })
    };
    let out = pump(Box::new(child.stdout.take().expect("piped stdout")), false);
    let err = pump(Box::new(child.stderr.take().expect("piped stderr")), true);
    let status = child.wait().map_err(|e| format!("`{program}` did not finish: {e}"))?;
    let _ = (out.join(), err.join());
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program} {}` failed with {status}", args.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-testlog-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // spec: BUILD-012
    #[test]
    fn a_log_is_named_by_command_and_utc_time_and_a_second_run_in_the_same_second_gets_a_suffix() {
        assert_eq!(log_name("test-fast", 1_792_355_415), "test-fast-20261018-203015.log");
        assert_eq!(log_name("test", 0), "test-19700101-000000.log");
        assert_eq!(log_name("test-gpu-real", 951_782_400 + 86_399), "test-gpu-real-20000229-235959.log", "a leap day, the last second");
        assert_eq!(command_key(None, false), "test");
        assert_eq!(command_key(Some("fast"), false), "test-fast");
        assert_eq!(command_key(Some("gpu"), false), "test-gpu");
        assert_eq!(command_key(Some("gpu"), true), "test-gpu-real");
        let dir = temp("names");
        let (a, b) = (new_log(&dir, "test", 1_792_355_415).unwrap(), new_log(&dir, "test", 1_792_355_415).unwrap());
        assert_eq!(a.file_name().unwrap(), "test-20261018-203015.log");
        assert_eq!(b.file_name().unwrap(), "test-20261018-203015-2.log");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: BUILD-012
    #[test]
    fn only_the_newest_20_logs_of_a_command_are_kept_and_other_commands_are_left_alone() {
        let dir = temp("rotate");
        for i in 0..25u64 {
            std::fs::write(dir.join(log_name("test", 1_792_000_000 + i)), "x").unwrap();
        }
        for i in 0..3u64 {
            std::fs::write(dir.join(log_name("test-gpu", 1_700_000_000 + i)), "x").unwrap();
            std::fs::write(dir.join(log_name("test-gpu-real", 1_700_000_000 + i)), "x").unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let fresh = new_log(&dir, "test", 1_792_100_000).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        names.sort();
        let tests: Vec<&String> = names.iter().filter(|n| is_log_of(n, "test")).collect();
        assert_eq!(tests.len(), KEEP, "{names:?}");
        assert_eq!(tests.last().unwrap().as_str(), fresh.file_name().unwrap().to_str().unwrap(), "the new log is kept");
        assert_eq!(*tests.first().unwrap(), &log_name("test", 1_792_000_000 + 6), "the oldest six are gone (25 + 1 - 20)");
        assert_eq!(names.iter().filter(|n| is_log_of(n, "test-gpu")).count(), 3);
        assert_eq!(names.iter().filter(|n| is_log_of(n, "test-gpu-real")).count(), 3);
        assert!(names.contains(&"notes.txt".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: BUILD-012
    #[test]
    fn a_failing_command_leaves_its_stdout_and_stderr_in_the_log_and_the_error_is_returned() {
        let dir = temp("run");
        let log = new_log(&dir, "test", 1_792_355_415).unwrap();
        let root = std::env::current_dir().unwrap();
        let ok = run_logged(&root, "cargo", &["--version"], &[], &log);
        assert!(ok.is_ok(), "{ok:?}");
        let bad = run_logged(&root, "cargo", &["no-such-subcommand-planet"], &[], &log);
        assert!(bad.is_err(), "a failing child is an error");
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("$ cargo --version"), "{text}");
        assert!(text.lines().any(|l| l.starts_with("cargo ")), "stdout of the first command: {text}");
        assert!(text.contains("$ cargo no-such-subcommand-planet"), "{text}");
        assert!(text.contains("no-such-subcommand-planet") && text.lines().count() >= 4, "stderr of the failing command: {text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
