//! The automatic crash report (VIEW-037): on a panic, an error that ends the run, or a lost GPU device, one text file
//! `target/review/crashes/<UTC time>.txt` holds the message, the location, the backtrace, the build commit, the command line, the camera
//! path of the last minute (the F9 path log) and the recent log lines, and its path is printed, so nobody copies logs by hand.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

/// Log lines kept for a report.
const LOG_LINES: usize = 200;

#[derive(Default)]
struct Context {
    args: Vec<String>,
    log: VecDeque<String>,
    path_text: String,
    written: bool,
}

static CONTEXT: Mutex<Context> = Mutex::new(Context { args: Vec::new(), log: VecDeque::new(), path_text: String::new(), written: false });

fn with<T>(f: impl FnOnce(&mut Context) -> T) -> T {
    let mut g = CONTEXT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut g)
}

/// Where the reports go: `PLANET_CRASH_DIR`, else `target/review/crashes`.
pub fn crash_dir() -> PathBuf {
    std::env::var_os("PLANET_CRASH_DIR").map_or_else(|| PathBuf::from("target/review/crashes"), PathBuf::from)
}

/// Install the panic hook and remember the command line. The default hook still runs (the message reaches stderr as before).
pub fn install(args: &[String]) {
    with(|c| c.args = args.to_vec());
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crate::gate::note_panic();
        previous(info);
        let location = info.location().map_or_else(|| "unknown location".to_string(), ToString::to_string);
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic with a non-text payload".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        report("panic", &message, &location, &backtrace);
    }));
}

/// Keep a log line for a report and print it to stderr.
pub fn note(line: &str) {
    eprintln!("{line}");
    with(|c| {
        if c.log.len() >= LOG_LINES {
            c.log.pop_front();
        }
        c.log.push_back(line.to_string());
    });
}

/// The camera path text (`PathLog::to_text`) a report carries; the window refreshes it about once a second.
pub fn set_path_text(text: String) {
    with(|c| c.path_text = text);
}

/// Write the report of a failure that ends the run, once per run (a panic after an error does not overwrite it), and print its path.
pub fn report(kind: &str, message: &str, location: &str, backtrace: &str) -> Option<PathBuf> {
    write_report(kind, message, location, backtrace, true)
}

/// A report of a failure the run survives (a plan refusal the window degrades from): written every time it is asked for, and it does not
/// use up the once-per-run report of a failure that ends the run.
pub fn snapshot(kind: &str, message: &str) -> Option<PathBuf> {
    write_report(kind, message, "", "", false)
}

fn write_report(kind: &str, message: &str, location: &str, backtrace: &str, once: bool) -> Option<PathBuf> {
    let (args, log, path_text) = with(|c| {
        if once && c.written {
            return None;
        }
        c.written |= once;
        Some((c.args.clone(), c.log.iter().cloned().collect::<Vec<_>>(), c.path_text.clone()))
    })?;
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let survived = !once;
    let input =
        Input { kind, survived, message, location, backtrace, commit: &commit(), args: &args, path_text: &path_text, log: &log, secs };
    let dir = crash_dir();
    let path = dir.join(format!("{}.txt", file_stamp(secs)));
    match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, format_report(&input))) {
        Ok(()) => {
            eprintln!("{} report: {}", noun(survived), path.display());
            Some(path)
        }
        Err(e) => {
            eprintln!("{} report could not be written to {}: {e}", noun(survived), path.display());
            None
        }
    }
}

/// What the report of a failure is called: a crash ends the run, a refusal does not.
fn noun(survived: bool) -> &'static str {
    if survived {
        "refusal"
    } else {
        "crash"
    }
}

/// The build commit: `git rev-parse HEAD` of the working directory, best effort.
pub(crate) fn commit() -> String {
    // Asked once per run: a flight over steep ground writes many reports, and each must not start a process.
    static COMMIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    COMMIT
        .get_or_init(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map_or_else(|| "unknown".to_string(), |o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .clone()
}

/// The commit a bundle records (TEST-071): [`commit`], with `+dirty` when the working tree has uncommitted changes (the commit then does
/// not reproduce the build), `None` when git cannot say.
pub(crate) fn bundle_commit() -> Option<String> {
    static ID: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let c = commit();
        if c == "unknown" {
            return None;
        }
        let dirty = std::process::Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=no"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .is_some_and(|o| !o.stdout.iter().all(u8::is_ascii_whitespace));
        Some(if dirty { format!("{c}+dirty") } else { c })
    })
    .clone()
}

pub struct Input<'a> {
    pub kind: &'a str,
    /// The run went on after the failure (a plan refusal): the report is called a refusal report, not a crash report.
    pub survived: bool,
    pub message: &'a str,
    pub location: &'a str,
    pub backtrace: &'a str,
    pub commit: &'a str,
    pub args: &'a [String],
    pub path_text: &'a str,
    pub log: &'a [String],
    pub secs: u64,
}

/// The text of a report.
pub fn format_report(i: &Input) -> String {
    let mut s = format!(
        "planet {} report\nkind: {}\ntime: {}\nbuild commit: {}\nversion: {}\ncommand line: {}\n\nmessage:\n{}\n\nlocation: {}\n\nbacktrace:\n{}\n",
        noun(i.survived),
        i.kind,
        iso_stamp(i.secs),
        i.commit,
        env!("CARGO_PKG_VERSION"),
        i.args.join(" "),
        i.message,
        i.location,
        if i.backtrace.is_empty() { "(none)" } else { i.backtrace }
    );
    s += &format!("\nrecent log lines ({}):\n", i.log.len());
    for l in i.log {
        s += l;
        s.push('\n');
    }
    s += "\ncamera path of the last minute (the F9 path file; replay with `test-render --repro BUNDLE --replay-path`):\n";
    s += if i.path_text.is_empty() { "(no poses recorded)\n" } else { i.path_text };
    s
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn parts(secs: u64) -> (i64, u32, u32, u64, u64, u64) {
    let (y, m, d) = civil((secs / 86_400) as i64);
    let t = secs % 86_400;
    (y, m, d, t / 3600, t / 60 % 60, t % 60)
}

/// `2026-10-20T17:49:01Z`.
pub fn iso_stamp(secs: u64) -> String {
    let (y, m, d, h, mi, s) = parts(secs);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// `20261020-174901Z`, safe in a file name.
pub fn file_stamp(secs: u64) -> String {
    let (y, m, d, h, mi, s) = parts(secs);
    format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: VIEW-037
    #[test]
    fn the_time_is_utc_and_the_report_holds_what_a_diagnosis_needs() {
        assert_eq!(iso_stamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_stamp(1_792_518_541), "2026-10-20T17:49:01Z");
        assert_eq!(file_stamp(1_792_518_541), "20261020-174901Z");
        let args = vec!["editor".to_string(), "--smoke".to_string()];
        let log = vec!["a line".to_string()];
        let text = format_report(&Input {
            kind: "plan refusal",
            survived: false,
            message: "plan for frame 5640: too steep",
            location: "pipeline.rs:869",
            backtrace: "0: frame",
            commit: "abc123",
            args: &args,
            path_text: "planet-camera-path\n",
            log: &log,
            secs: 1_792_518_541,
        });
        assert!(text.starts_with("planet crash report"), "{text}");
        for want in ["plan refusal", "too steep", "pipeline.rs:869", "0: frame", "abc123", "editor --smoke", "a line", "planet-camera-path"]
        {
            assert!(text.contains(want), "missing {want}: {text}");
        }
    }

    // spec: VIEW-037
    #[test]
    fn a_report_is_written_once_to_the_crash_directory() {
        let dir = std::env::temp_dir().join(format!("planet-crash-test-{}", std::process::id()));
        std::env::set_var("PLANET_CRASH_DIR", &dir);
        note("log line before the crash");
        let p = report("test", "boom", "here", "").expect("written");
        assert!(p.starts_with(&dir) && std::fs::read_to_string(&p).unwrap().contains("log line before the crash"));
        assert!(report("test", "second", "here", "").is_none(), "once per run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: VIEW-037
    #[test]
    fn the_report_of_a_refusal_the_window_survived_is_called_a_refusal_report() {
        let input = |survived| Input {
            kind: "plan refusal",
            survived,
            message: "m",
            location: "",
            backtrace: "",
            commit: "c",
            args: &[],
            path_text: "",
            log: &[],
            secs: 0,
        };
        assert!(format_report(&input(true)).starts_with("planet refusal report\n"));
        assert!(format_report(&input(false)).starts_with("planet crash report\n"));
        assert_eq!((noun(true), noun(false)), ("refusal", "crash"));
    }
}
