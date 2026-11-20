//! The machine-wide heavy-run lock (BUILD-026): `test`, `prepush`, `capture`, `accept` and `store` take one of `PLANET_HEAVY_SLOTS` slots (default 2;
//! owner, 2026-11-14: short jobs waited behind long probes) outside every worktree and wait their turn when all are held; `perf`, `gate` (but
//! `--small`), `repro --window` and `heavy --exclusive` wait until no slot is held and hold them all (timing needs the machine alone). The owner's
//! quiet flag file makes every heavy command wait for as long as it exists, so that the owner can clear the machine without messaging anyone.
//!
//! The lock is an OS file lock (`File::try_lock`), which the system releases when the process ends however it ends, so there is no stale lock to
//! clean up; a sidecar file names the holder for the message of whoever waits. Where the state directory cannot be made, or `PLANET_HEAVY_LOCK=off`,
//! the command runs without the lock and says so: the lock is a courtesy of the tooling, never a reason a command cannot run.
//!
//! The state directory is `PLANET_STATE_DIR`, else `SeriousPlanetData/tmp/heavy` in the nearest ancestor of the workspace that has the data
//! root (ADR 0121), else `%LOCALAPPDATA%\planet`, else `$XDG_STATE_HOME/planet`, else `~/.local/state/planet`. The data root comes before
//! `%LOCALAPPDATA%` because the desktop app redirects `%LOCALAPPDATA%` for its sessions: a terminal and a session then saw two different locks.
//!
//! Waiters queue in arrival order (lead, 2026-11-14): a command that finds the lock busy, or others already queued, takes a ticket (a file in
//! `tickets/`, named by its arrival time, held open under an OS file lock), and only the lowest live ticket tries the lock. A ticket whose file
//! lock is free belongs to a process that has ended: the next reader removes it. A holder past [`overdue_limit`] is named overdue in the waiters'
//! messages; the lock is never broken.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How often a waiting command looks again.
const POLL: Duration = Duration::from_secs(2);
/// A waiting command repeats its message this often.
const SAY_EVERY: Duration = Duration::from_secs(30);

/// The owner's quiet flag older than this is ignored (the owner can forget it): the command warns, naming the age, and goes on.
pub const QUIET_EXPIRES: Duration = Duration::from_secs(60 * 60);

/// The environment variable a holder sets for its children: its process id. A child that finds its parent's id in the holder note does not wait.
const PARENT_ENV: &str = "PLANET_HEAVY_HELD";

/// Whether this process holds the lock (a command that runs another heavy command in-process, as `prepush` does, takes it once).
static HELD: AtomicBool = AtomicBool::new(false);
/// The warning about an expired quiet flag is made once a run.
static WARNED_EXPIRED: AtomicBool = AtomicBool::new(false);

/// The directory of the lock, the holder file and the quiet flag, if one can be found.
pub fn state_dir() -> Option<PathBuf> {
    let from_env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    from_env("PLANET_STATE_DIR")
        .or_else(|| data_root_state_dir(Path::new(env!("CARGO_MANIFEST_DIR"))))
        .or_else(|| from_env("LOCALAPPDATA").map(|p| p.join("planet")))
        .or_else(|| from_env("XDG_STATE_HOME").map(|p| p.join("planet")))
        .or_else(|| from_env("HOME").map(|p| p.join(".local").join("state").join("planet")))
}

/// `SeriousPlanetData/tmp/heavy` in the nearest ancestor of `start` that holds the data root `SeriousPlanetData` (ADR 0121), if any. The main
/// checkout and every worktree under `.claude/worktrees/` find the same one.
fn data_root_state_dir(start: &Path) -> Option<PathBuf> {
    start.ancestors().map(|a| a.join("SeriousPlanetData")).find(|d| d.is_dir()).map(|d| d.join("tmp").join("heavy"))
}

/// The lock file of slot `k`: slot 0 keeps the single lock's name, so an older xtask (one slot) shares slot 0 with this one.
fn lock_path(dir: &Path, k: usize) -> PathBuf {
    if k == 0 {
        dir.join("heavy.lock")
    } else {
        dir.join(format!("heavy.lock.{k}"))
    }
}

/// The holder note of slot `k` (slot 0 keeps the single lock's name).
fn holder_path(dir: &Path, k: usize) -> PathBuf {
    if k == 0 {
        dir.join("heavy.holder")
    } else {
        dir.join(format!("heavy.holder.{k}"))
    }
}

/// The highest slot count a reader looks at for holder notes (a `PLANET_HEAVY_SLOTS` above it is capped).
const MAX_SLOTS: usize = 8;

/// The ordinary slots (`PLANET_HEAVY_SLOTS`, default 2, 1 to [`MAX_SLOTS`]; owner, 2026-11-14).
pub fn slot_count() -> usize {
    std::env::var("PLANET_HEAVY_SLOTS").ok().and_then(|v| v.trim().parse::<usize>().ok()).map_or(2, |n| n.clamp(1, MAX_SLOTS))
}

/// What a command asks for: one of `slots` slots, or all of them (`exclusive`: timing and window runs need the machine alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Req {
    pub slots: usize,
    pub exclusive: bool,
}

impl Req {
    /// One slot of the configured count.
    pub fn shared() -> Req {
        Req { slots: slot_count(), exclusive: false }
    }
    /// Every slot of the configured count.
    pub fn exclusive() -> Req {
        Req { slots: slot_count(), exclusive: true }
    }
}

/// The holder notes present, in slot order.
pub fn holder_notes(dir: &Path) -> Vec<String> {
    (0..MAX_SLOTS).filter_map(|k| std::fs::read_to_string(holder_path(dir, k)).ok()).filter(|n| !n.trim().is_empty()).collect()
}

/// The owner's flag file: while it exists every heavy command waits.
pub fn quiet_path(dir: &Path) -> PathBuf {
    dir.join("quiet")
}

/// How old the quiet flag is, `None` when it is down.
pub fn quiet_age(dir: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(quiet_path(dir)).and_then(|m| m.modified()).ok()?;
    Some(std::time::SystemTime::now().duration_since(modified).unwrap_or_default())
}

/// Whether the process whose id the environment names (`parent`, the value of `PLANET_HEAVY_HELD`) is the one the holder note of the lock names:
/// then the caller is its child and the lock is already its own.
pub fn parent_holds(dir: &Path, parent: Option<&str>) -> bool {
    let Some(pid) = parent.filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) else { return false };
    holder_notes(dir).iter().any(|note| note.contains(&format!("(process {pid},")))
}

/// The slots while a heavy command runs; dropping it lets the next in the queue go.
pub struct Guard {
    /// The slot locks held (one, every slot for an exclusive run, none for a child of the holder or an unlocked run).
    files: Vec<File>,
    /// Their holder notes, removed on drop.
    holders: Vec<PathBuf>,
    /// This guard set [`HELD`] and clears it.
    outer: bool,
}

impl Guard {
    fn none() -> Guard {
        Guard { files: Vec::new(), holders: Vec::new(), outer: false }
    }
    /// How many slots this guard holds.
    pub fn slots_held(&self) -> usize {
        self.files.len()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        for h in &self.holders {
            let _ = std::fs::remove_file(h);
        }
        // The file locks go with the handles.
        self.files.clear();
        if self.outer {
            std::env::remove_var(PARENT_ENV);
            HELD.store(false, Ordering::SeqCst);
        }
    }
}

/// Why a try for the lock did not give it.
#[derive(Debug, PartialEq, Eq)]
pub enum Busy {
    /// The quiet flag file exists.
    Quiet(PathBuf),
    /// Other commands hold the slots; their holder notes, joined by " | ".
    Held(String),
    /// Other waiters queued earlier: how many, and the first one's ticket note.
    Queued(usize, String),
}

/// A holder that has held the lock longer than this is named overdue (a parameter: `PLANET_HEAVY_OVERDUE_MIN`, default 90 minutes).
pub const OVERDUE_DEFAULT: Duration = Duration::from_secs(90 * 60);

/// The overdue limit in force.
pub fn overdue_limit() -> Duration {
    std::env::var("PLANET_HEAVY_OVERDUE_MIN")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(OVERDUE_DEFAULT, |m| Duration::from_secs(m * 60))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// How many whole minutes the holder of `note` has held the lock, when that is longer than `limit` (the note's "since N s after the epoch"); `None` when it
/// is not overdue or the note names no start.
pub fn overdue_minutes(note: &str, now: u64, limit: Duration) -> Option<u64> {
    let since: u64 = note.split("since ").nth(1)?.split(' ').next()?.parse().ok()?;
    let held = now.saturating_sub(since);
    (held > limit.as_secs()).then_some(held / 60)
}

/// The overdue phrases of the holders among `holders` (notes joined by " | ") past `limit`, each naming its holder.
pub fn overdue_phrases(holders: &str, now: u64, limit: Duration) -> String {
    holders.split(" | ").filter_map(|h| overdue_minutes(h, now, limit).map(|m| format!("; overdue: {}, held {m} min", h.trim()))).collect()
}

/// The phrase naming the holders (notes joined by " | ") that an xtask older than the slots and the ticket queue wrote: a note of a process that names no slot ("slot k of n" or
/// "exclusive"; a slot without a note is not named: today's xtask writes its note just after the lock). Such a binary (a `target/debug/xtask.exe` run directly instead of through `cargo xtask`) takes `heavy.lock` without
/// queueing (lead #156).
pub fn pre_queue_phrase(holders: &str) -> String {
    let old: Vec<&str> = holders
        .split(" | ")
        .map(str::trim)
        .filter(|h| h.contains("(process ") && !h.contains(" slot ") && !h.contains("exclusive)"))
        .collect();
    if old.is_empty() {
        String::new()
    } else {
        format!(
            "; held by an xtask older than the ticket queue, which does not queue: {} (run heavy commands through `cargo xtask`, which rebuilds it)",
            old.join(" | ")
        )
    }
}

/// The waiting message for `busy`, with each holder past `limit` named overdue.
pub fn waiting_message(busy: &Busy, what: &str, now: u64, limit: Duration) -> String {
    let overdue = |holders: &str| {
        let o = overdue_phrases(holders, now, limit);
        if o.is_empty() {
            o
        } else {
            format!("{o} (the lock is never broken: the holder's lane or the owner ends it)")
        }
    };
    match busy {
        Busy::Quiet(p) => format!(
            "waiting: the owner's quiet flag {} is up (`cargo xtask quiet off` or deleting the file lets heavy commands run); `{what}` is queued",
            p.display()
        ),
        Busy::Held(holder) => format!("waiting for {}; `{what}` is queued{}{}", holder.trim(), overdue(holder), pre_queue_phrase(holder)),
        Busy::Queued(ahead, first) => format!("waiting: {ahead} queued ahead (next: {}); `{what}` is queued", first.trim()),
    }
}

fn tickets_dir(dir: &Path) -> PathBuf {
    dir.join("tickets")
}

/// A ticket younger than this is never taken for dead: its owner may not have locked it yet.
const TICKET_GRACE: Duration = Duration::from_secs(5);

/// Tickets of one process are told apart by this counter.
static TICKET_SEQ: AtomicU64 = AtomicU64::new(0);

/// A waiter's place in the queue: its file, held open and locked while it waits; removed when dropped.
pub struct Ticket {
    file: Option<File>,
    path: PathBuf,
    /// The file name: arrival time in nanoseconds, process id, sequence (zero-padded, so the names sort in arrival order).
    pub name: String,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        // The handle first: a file open elsewhere cannot be removed on Windows.
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(note_path(&self.path));
    }
}

/// The note beside a ticket: a locked file cannot be read through another handle on Windows, so what the waiter is goes in its own file.
fn note_path(ticket: &Path) -> PathBuf {
    let mut p = ticket.as_os_str().to_owned();
    p.push(".note");
    PathBuf::from(p)
}

/// Take a ticket for `what` in the queue of `dir`.
pub fn take_ticket(dir: &Path, what: &str) -> std::io::Result<Ticket> {
    let t = tickets_dir(dir);
    std::fs::create_dir_all(&t)?;
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let name = format!("{nanos:024}-{:010}-{:06}", std::process::id(), TICKET_SEQ.fetch_add(1, Ordering::SeqCst));
    let path = t.join(&name);
    let worktree = std::env::current_dir().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_default();
    std::fs::write(
        note_path(&path),
        format!("{what} (process {}, in {worktree}, queued since {} s after the epoch)", std::process::id(), now_secs()),
    )?;
    let file = OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
    file.lock()?;
    Ok(Ticket { file: Some(file), path, name })
}

/// The live tickets of `dir` in arrival order, `(name, note)`: a ticket whose file lock is free and that is older than [`TICKET_GRACE`] belongs to an ended
/// process and is removed.
pub fn live_tickets(dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(tickets_dir(dir)) else { return Vec::new() };
    let mut names: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(n, _)| !n.ends_with(".note"))
        .collect();
    names.sort();
    let mut live = Vec::new();
    for (name, path) in names {
        // A ticket that cannot be opened (it was removed meanwhile: gone; any other error: kept) still queues its waiter: skipping it would let later arrivals
        // take a slot ahead of it (lead #156).
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                live.push((name, std::fs::read_to_string(note_path(&path)).unwrap_or_default()));
                continue;
            }
        };
        let young = file.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).is_none_or(|age| age < TICKET_GRACE);
        match file.try_lock() {
            Ok(()) if !young => {
                // Its process has ended: the system released its lock.
                drop(file);
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_file(note_path(&path));
            }
            _ => {
                drop(file);
                live.push((name, std::fs::read_to_string(note_path(&path)).unwrap_or_default()));
            }
        }
    }
    live
}

/// Take a free slot (or, exclusive, every slot) if the quiet flag is not up; never waits.
pub fn try_acquire_in(dir: &Path, what: &str, req: Req) -> std::io::Result<Result<Guard, Busy>> {
    std::fs::create_dir_all(dir)?;
    if let Some(age) = quiet_age(dir) {
        if age < QUIET_EXPIRES {
            return Ok(Err(Busy::Quiet(quiet_path(dir))));
        }
        if !WARNED_EXPIRED.swap(true, Ordering::SeqCst) {
            eprintln!(
                "heavy-run lock: the quiet flag {} is {} min old (it expires after {} min): ignored; `cargo xtask quiet off` removes it, `quiet on` renews it",
                quiet_path(dir).display(),
                age.as_secs() / 60,
                QUIET_EXPIRES.as_secs() / 60
            );
        }
    }
    let slots = req.slots.clamp(1, MAX_SLOTS);
    // The slots taken: one (the first free) or, exclusive, all in slot order (released again if any is busy).
    let mut taken: Vec<(usize, File)> = Vec::new();
    let mut busy: Vec<usize> = Vec::new();
    for k in 0..slots {
        let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(lock_path(dir, k))?;
        match file.try_lock() {
            Ok(()) => {
                taken.push((k, file));
                if !req.exclusive {
                    break;
                }
            }
            Err(TryLockError::WouldBlock) => busy.push(k),
            Err(TryLockError::Error(e)) => return Err(e),
        }
    }
    if taken.is_empty() || (req.exclusive && !busy.is_empty()) {
        // Not a slot free, or (exclusive) not every slot: hold none, and name the holders of the busy slots.
        taken.clear();
        let notes: Vec<String> =
            busy.iter().filter_map(|&k| std::fs::read_to_string(holder_path(dir, k)).ok()).filter(|n| !n.trim().is_empty()).collect();
        let holders = if notes.is_empty() { "another heavy command (no holder note)".to_string() } else { notes.join(" | ") };
        return Ok(Err(Busy::Held(holders)));
    }
    let worktree = std::env::current_dir().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_default();
    let mut guard = Guard::none();
    for (k, file) in taken {
        let place = if req.exclusive { "exclusive".to_string() } else { format!("slot {} of {slots}", k + 1) };
        let text = format!("{what} (process {}, in {worktree}, since {} s after the epoch, {place})", std::process::id(), now_secs());
        let h = holder_path(dir, k);
        std::fs::File::create(&h).and_then(|mut f| f.write_all(text.as_bytes()))?;
        guard.files.push(file);
        guard.holders.push(h);
    }
    Ok(Ok(guard))
}

/// Wait for the lock (and the quiet flag to go), saying why every 30 s; `give_up` ends the wait with `None` (for tests; a command waits for good).
pub fn acquire_in(
    dir: &Path,
    what: &str,
    req: Req,
    poll: Duration,
    give_up: Option<Duration>,
    say: &mut dyn FnMut(&str),
) -> std::io::Result<Option<Guard>> {
    let began = Instant::now();
    let mut last_said: Option<Instant> = None;
    let limit = overdue_limit();
    // This waiter's place in the queue, taken the first time it has to wait.
    let mut ticket: Option<Ticket> = None;
    loop {
        let queue = live_tickets(dir);
        let ahead: Vec<&(String, String)> = queue.iter().filter(|(n, _)| ticket.as_ref().is_none_or(|t| n < &t.name)).collect();
        let tried = if let Some((_, first)) = ahead.first() {
            // Others queued earlier: wait behind them (never take the lock ahead of the queue).
            Err(Busy::Queued(ahead.len(), first.clone()))
        } else {
            try_acquire_in(dir, what, req)?
        };
        match tried {
            Ok(guard) => {
                if last_said.is_some() {
                    say(&format!("the machine is free after {:.0} s: running `{what}`", began.elapsed().as_secs_f64()));
                }
                // The ticket goes with the wait.
                drop(ticket);
                return Ok(Some(guard));
            }
            Err(busy) => {
                if ticket.is_none() {
                    ticket = Some(take_ticket(dir, what)?);
                }
                if last_said.is_none_or(|t| t.elapsed() >= SAY_EVERY) {
                    last_said = Some(Instant::now());
                    let mut m = waiting_message(&busy, what, now_secs(), limit);
                    if let Busy::Queued(..) = busy {
                        // A holder may be overdue while the queue waits.
                        m.push_str(&overdue_phrases(&holder_notes(dir).join(" | "), now_secs(), limit));
                    }
                    say(&m);
                }
            }
        }
        if give_up.is_some_and(|g| began.elapsed() >= g) {
            return Ok(None);
        }
        std::thread::sleep(poll);
    }
}

/// [`acquire_in`] for a process that may be a child of the holder: when `parent` (the `PLANET_HEAVY_HELD` of its environment) names the process
/// of the holder note, the lock is already its parent's and it takes nothing and waits for nothing.
#[allow(clippy::too_many_arguments)]
pub fn acquire_dir(
    dir: &Path,
    what: &str,
    req: Req,
    parent: Option<&str>,
    poll: Duration,
    give_up: Option<Duration>,
    say: &mut dyn FnMut(&str),
) -> std::io::Result<Option<Guard>> {
    if parent_holds(dir, parent) {
        return Ok(Some(Guard::none()));
    }
    acquire_in(dir, what, req, poll, give_up, say)
}

/// The slot for a heavy command `what` (all slots when `exclusive`): waits its turn, prints why it waits, and never fails (a state directory that cannot
/// be made, or `PLANET_HEAVY_LOCK=off`, runs the command unlocked and says so).
pub fn acquire(what: &str, exclusive: bool) -> Guard {
    let unlocked = |why: &str| {
        eprintln!("heavy-run lock: {why}; `{what}` runs without it");
        Guard::none()
    };
    if std::env::var("PLANET_HEAVY_LOCK").is_ok_and(|v| v == "off") {
        return unlocked("PLANET_HEAVY_LOCK=off");
    }
    if HELD.swap(true, Ordering::SeqCst) {
        // A heavy command inside a heavy command of this process: the outer one holds it.
        return Guard::none();
    }
    let Some(dir) = state_dir() else {
        HELD.store(false, Ordering::SeqCst);
        return unlocked("no state directory (set PLANET_STATE_DIR)");
    };
    let parent = std::env::var(PARENT_ENV).ok();
    let req = if exclusive { Req::exclusive() } else { Req::shared() };
    match acquire_dir(&dir, what, req, parent.as_deref(), POLL, None, &mut |m| eprintln!("heavy-run lock: {m}")) {
        Ok(Some(mut guard)) => {
            if guard.slots_held() > 0 {
                // This process holds it now: its children (a `cargo xtask` run by a script, a test binary) inherit the id and do not wait.
                guard.outer = true;
                std::env::set_var(PARENT_ENV, std::process::id().to_string());
            } else {
                // A child of the holder: the parent's lock is the one in force; this process holds nothing and sets nothing.
                HELD.store(false, Ordering::SeqCst);
            }
            guard
        }
        Ok(None) => unreachable!("a command waits for good"),
        Err(e) => {
            HELD.store(false, Ordering::SeqCst);
            unlocked(&format!("cannot use {} ({e})", dir.display()))
        }
    }
}

/// Run `f` as the heavy command `what` in one slot.
pub fn with<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let _guard = acquire(what, false);
    f()
}

/// Run `f` as the heavy command `what` with every slot (timing and window runs: the machine alone).
pub fn with_exclusive<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let _guard = acquire(what, true);
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
            // Writing the file again renews it: its age is the time since the last `quiet on`.
            std::fs::write(&flag, "the owner needs the machine\n").map_err(|e| format!("{}: {e}", flag.display()))?;
            println!(
                "quiet: on ({}): every heavy command waits until `cargo xtask quiet off`, or {} min from now (run `quiet on` again to renew it)",
                flag.display(),
                QUIET_EXPIRES.as_secs() / 60
            );
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
            let holder = holder_notes(dir).join("\n  ");
            println!(
                "quiet flag: {} ({})\nheavy lock holders ({} slots):\n  {}",
                match quiet_age(dir) {
                    None => "down".to_string(),
                    Some(a) if a < QUIET_EXPIRES => format!("up, {} min old", a.as_secs() / 60),
                    Some(a) => format!("EXPIRED, {} min old: ignored", a.as_secs() / 60),
                },
                flag.display(),
                slot_count(),
                if holder.is_empty() { "none" } else { holder.trim() }
            );
        }
        Some(other) => return Err(format!("quiet takes on, off or status, not `{other}`")),
    }
    Ok(())
}

/// Whether `heavy`'s arguments ask for the exclusive mode, and the command after the flag.
pub fn wrapped_mode(args: &[String]) -> (bool, &[String]) {
    match args.first().map(String::as_str) {
        Some("--exclusive") => (true, &args[1..]),
        _ => (false, args),
    }
}

/// `cargo xtask heavy [--exclusive] <command> [args]`: run any command (a diagnostic example, a capture) in a slot, or with `--exclusive` with every slot
/// (a timing run).
pub fn run_wrapped(root: &Path, args: &[String]) -> Result<(), String> {
    let (exclusive, args) = wrapped_mode(args);
    let Some(program) = args.first() else {
        return Err("heavy needs a command: `cargo xtask heavy [--exclusive] cargo run --release -p planet --example ...`".into());
    };
    let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
    let run = || crate::util::run(root, program, &rest, &[]);
    if exclusive {
        with_exclusive(&args.join(" "), run)
    } else {
        with(&args.join(" "), run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The single lock of the tests written before the slots: one slot, shared.
    const ONE: Req = Req { slots: 1, exclusive: false };

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("planet-heavy-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    // spec: BUILD-026
    #[test]
    fn the_state_directory_is_the_data_roots_heavy_folder_for_the_checkout_and_every_worktree() {
        let base = dir("data-root");
        let data = base.join("SeriousPlanetData");
        let main = base.join("Serious-Planet-Builder");
        let worktree = main.join(".claude").join("worktrees").join("lane-1").join("xtask");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&worktree).unwrap();
        let want = data.join("tmp").join("heavy");
        assert_eq!(data_root_state_dir(&main.join("xtask")), Some(want.clone()), "the main checkout");
        assert_eq!(data_root_state_dir(&worktree), Some(want), "a worktree finds the same folder");
        // The nearest data root wins over any farther one (the temporary directory itself may sit below a data root on the owner's machine,
        // so the case "no data root at all" is not testable here).
        let inner = main.join("SeriousPlanetData");
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(data_root_state_dir(&worktree), Some(inner.join("tmp").join("heavy")), "the nearest data root wins");
        let _ = std::fs::remove_dir_all(&base);
    }

    // spec: BUILD-026
    #[test]
    fn a_second_heavy_command_waits_and_names_the_holder_and_goes_when_the_first_is_done() {
        let d = dir("lock");
        let first = try_acquire_in(&d, "test gpu", ONE).unwrap().expect("the machine is free");
        let mut said = Vec::new();
        let second =
            acquire_in(&d, "prepush", ONE, Duration::from_millis(10), Some(Duration::from_millis(60)), &mut |m| said.push(m.to_string()))
                .unwrap();
        assert!(second.is_none(), "the lock is held: the second waits");
        assert!(said.iter().any(|m| m.contains("waiting for test gpu") && m.contains("`prepush` is queued")), "{said:?}");
        match try_acquire_in(&d, "x", ONE).unwrap() {
            Err(Busy::Held(h)) => assert!(h.starts_with("test gpu (process "), "{h}"),
            other => panic!("{:?}", other.map(|_| ())),
        }
        drop(first);
        let mut said = Vec::new();
        let third =
            acquire_in(&d, "prepush", ONE, Duration::from_millis(10), Some(Duration::from_secs(5)), &mut |m| said.push(m.to_string()))
                .unwrap();
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
            acquire_in(&d, "test", ONE, Duration::from_millis(10), Some(Duration::from_millis(50)), &mut |m| said.push(m.to_string()))
                .unwrap();
        assert!(got.is_none(), "the flag is up: nothing runs");
        assert!(said.iter().any(|m| m.contains("quiet flag") && m.contains("`test` is queued")), "{said:?}");
        assert_eq!(try_acquire_in(&d, "x", ONE).unwrap().err(), Some(Busy::Quiet(quiet_path(&d))));
        std::fs::remove_file(quiet_path(&d)).unwrap();
        assert!(try_acquire_in(&d, "test", ONE).unwrap().is_ok(), "the flag is down: the command goes");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_waiter_in_another_thread_goes_as_soon_as_the_holder_lets_go() {
        let d = dir("thread");
        let first = try_acquire_in(&d, "capture stills", ONE).unwrap().expect("free");
        let d2 = d.clone();
        let waiter = std::thread::Builder::new()
            .name("test-waiter".into())
            .spawn(move || {
                acquire_in(&d2, "perf", ONE, Duration::from_millis(10), Some(Duration::from_secs(10)), &mut |_| {}).unwrap().is_some()
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        drop(first);
        assert!(waiter.join().unwrap(), "the queued command ran once the first was done");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_child_of_the_holder_does_not_wait_for_its_parents_lock() {
        let d = dir("child");
        let parent = try_acquire_in(&d, "prepush", ONE).unwrap().expect("free");
        let me = std::process::id().to_string();
        // The environment of a child says whose lock it is: it takes nothing and waits for nothing, however short its patience.
        let got = acquire_dir(&d, "test gpu", ONE, Some(&me), Duration::from_millis(10), Some(Duration::from_millis(30)), &mut |m| {
            panic!("waited: {m}")
        })
        .unwrap();
        let child = got.expect("the child goes at once");
        assert_eq!(child.slots_held(), 0, "it holds no lock of its own");
        drop(child);
        assert!(try_acquire_in(&d, "x", ONE).unwrap().is_err(), "the parent still holds the lock after the child is done");
        // Not a child: another process id, an empty or non-numeric value, no value; and no holder any more.
        for env in [Some("1"), Some(""), Some("abc"), Some("12x"), None] {
            assert!(!parent_holds(&d, env), "{env:?}");
            let waited =
                acquire_dir(&d, "other", ONE, env, Duration::from_millis(10), Some(Duration::from_millis(30)), &mut |_| {}).unwrap();
            assert!(waited.is_none(), "{env:?} is no child: it waits for the lock");
        }
        drop(parent);
        assert!(!parent_holds(&d, Some(&me)), "the holder note went with the lock: a stale variable is no pass");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_quiet_flag_older_than_an_hour_is_ignored_and_quiet_on_renews_it() {
        let d = dir("expiry");
        std::fs::create_dir_all(&d).unwrap();
        quiet_in(&d, &["on".to_string()]).unwrap();
        assert!(quiet_age(&d).unwrap() < Duration::from_secs(5));
        assert!(matches!(try_acquire_in(&d, "x", ONE).unwrap(), Err(Busy::Quiet(_))), "a fresh flag holds");
        // The owner forgot it: two hours old.
        let old = std::time::SystemTime::now() - Duration::from_secs(2 * 60 * 60);
        File::options().write(true).open(quiet_path(&d)).unwrap().set_modified(old).unwrap();
        let age = quiet_age(&d).unwrap();
        assert!(age >= Duration::from_secs(2 * 60 * 60 - 5) && age >= QUIET_EXPIRES, "{age:?}");
        let got = try_acquire_in(&d, "x", ONE).unwrap();
        assert!(got.is_ok(), "an expired flag is ignored (with a warning naming its age)");
        drop(got);
        // `quiet on` renews it: the flag holds again.
        quiet_in(&d, &["on".to_string()]).unwrap();
        assert!(quiet_age(&d).unwrap() < Duration::from_secs(5));
        assert!(matches!(try_acquire_in(&d, "x", ONE).unwrap(), Err(Busy::Quiet(_))));
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn waiters_go_in_arrival_order_and_a_newcomer_never_jumps_the_queue() {
        let d = dir("order");
        let holder = try_acquire_in(&d, "test gpu", ONE).unwrap().expect("free");
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let spawn = |name: &'static str| {
            let (d, order) = (d.clone(), order.clone());
            std::thread::spawn(move || {
                let g = acquire_in(&d, name, ONE, Duration::from_millis(5), Some(Duration::from_secs(20)), &mut |_| {})
                    .unwrap()
                    .expect("its turn comes");
                order.lock().unwrap().push(name);
                std::thread::sleep(Duration::from_millis(50));
                drop(g);
            })
        };
        let a = spawn("first");
        // The first is queued (its ticket is taken) before the second arrives.
        while live_tickets(&d).is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let b = spawn("second");
        while live_tickets(&d).len() < 2 {
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(holder);
        a.join().unwrap();
        b.join().unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["first", "second"]);
        assert!(live_tickets(&d).is_empty(), "every ticket went with its wait");
        // The lock is free but a waiter is queued: a newcomer queues behind it instead of taking the lock.
        let queued = take_ticket(&d, "queued earlier").unwrap();
        let mut said = Vec::new();
        let newcomer =
            acquire_in(&d, "newcomer", ONE, Duration::from_millis(5), Some(Duration::from_millis(60)), &mut |m| said.push(m.to_string()))
                .unwrap();
        assert!(newcomer.is_none(), "the newcomer waits behind the queue");
        assert!(said.iter().any(|m| m.contains("1 queued ahead") && m.contains("queued earlier")), "{said:?}");
        drop(queued);
        assert!(acquire_in(&d, "newcomer", ONE, Duration::from_millis(5), Some(Duration::from_secs(5)), &mut |_| {}).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_dead_waiters_ticket_is_removed_and_a_young_one_is_kept() {
        let d = dir("dead");
        std::fs::create_dir_all(tickets_dir(&d)).unwrap();
        // A ticket no process holds (its process ended): unlocked and older than the grace.
        let dead = tickets_dir(&d).join("000000000000000000000001-0000000001-000000");
        std::fs::write(&dead, "").unwrap();
        std::fs::write(note_path(&dead), "prepush (process 1, in gone, queued since 0 s after the epoch)").unwrap();
        File::options().write(true).open(&dead).unwrap().set_modified(SystemTime::now() - Duration::from_secs(60)).unwrap();
        // A ticket just written, not yet locked by its owner: kept for the grace.
        let young = tickets_dir(&d).join("000000000000000000000002-0000000002-000000");
        std::fs::write(&young, "test (process 2)").unwrap();
        let live = live_tickets(&d);
        assert_eq!(live.len(), 1, "{live:?}");
        assert!(live[0].0.ends_with("0000000002-000000"));
        assert!(!dead.exists() && !note_path(&dead).exists(), "the dead ticket and its note are removed");
        // A held ticket is live whatever its age.
        std::fs::remove_file(&young).unwrap();
        let held = take_ticket(&d, "held").unwrap();
        File::options().write(true).open(&held.path).unwrap().set_modified(SystemTime::now() - Duration::from_secs(60)).unwrap();
        assert_eq!(live_tickets(&d).len(), 1, "a locked ticket is live");
        drop(held);
        assert!(live_tickets(&d).is_empty(), "a dropped ticket is gone");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_holder_past_the_limit_is_named_overdue_and_the_lock_is_never_broken() {
        let limit = Duration::from_secs(90 * 60);
        let now = 1_000_000;
        let note = |held_min: u64| format!("cargo xtask prepush (process 7, in lane, since {} s after the epoch)", now - held_min * 60);
        assert_eq!(overdue_minutes(&note(100), now, limit), Some(100));
        assert_eq!(overdue_minutes(&note(90), now, limit), None, "at the limit: not yet");
        assert_eq!(overdue_minutes(&note(10), now, limit), None);
        assert_eq!(overdue_minutes("another heavy command (no holder note)", now, limit), None);
        let m = waiting_message(&Busy::Held(note(100)), "test", now, limit);
        assert!(m.contains("overdue: cargo xtask prepush (process 7") && m.contains("held 100 min") && m.contains("never broken"), "{m}");
        assert!(!waiting_message(&Busy::Held(note(10)), "test", now, limit).contains("overdue"));
        assert_eq!(OVERDUE_DEFAULT, limit, "90 minutes to start");
        // An overdue holder still holds: a waiter waits.
        let d = dir("overdue");
        let holder = try_acquire_in(&d, "stuck", ONE).unwrap().expect("free");
        std::fs::write(holder_path(&d, 0), note(500)).unwrap();
        let got = acquire_in(&d, "test", ONE, Duration::from_millis(5), Some(Duration::from_millis(40)), &mut |_| {}).unwrap();
        assert!(got.is_none(), "the lock is not broken");
        drop(holder);
        let _ = std::fs::remove_dir_all(&d);
    }

    const TWO: Req = Req { slots: 2, exclusive: false };
    const ALL: Req = Req { slots: 2, exclusive: true };

    // spec: BUILD-026
    #[test]
    fn two_ordinary_jobs_run_side_by_side_and_a_third_waits_naming_both() {
        let d = dir("slots");
        let a = try_acquire_in(&d, "prepush", TWO).unwrap().expect("slot 1 is free");
        let b = try_acquire_in(&d, "test gpu", TWO).unwrap().expect("slot 2 is free beside the first");
        assert_eq!((a.slots_held(), b.slots_held()), (1, 1));
        let notes = holder_notes(&d);
        assert!(notes[0].starts_with("prepush (process ") && notes[0].ends_with("slot 1 of 2)"), "{notes:?}");
        assert!(notes[1].starts_with("test gpu (process ") && notes[1].ends_with("slot 2 of 2)"), "{notes:?}");
        match try_acquire_in(&d, "capture", TWO).unwrap() {
            Err(Busy::Held(h)) => assert!(h.contains("prepush (process") && h.contains(" | test gpu (process"), "{h}"),
            other => panic!("{:?}", other.map(|_| ())),
        }
        drop(a);
        let c = try_acquire_in(&d, "capture", TWO).unwrap().expect("the freed slot");
        assert!(holder_notes(&d).iter().any(|n| n.starts_with("capture") && n.ends_with("slot 1 of 2)")));
        drop((b, c));
        assert!(holder_notes(&d).is_empty(), "every note went with its slot");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn an_exclusive_run_waits_for_every_slot_and_holds_them_all() {
        let d = dir("exclusive");
        let a = try_acquire_in(&d, "prepush", TWO).unwrap().expect("free");
        match try_acquire_in(&d, "perf", ALL).unwrap() {
            Err(Busy::Held(h)) => assert!(h.starts_with("prepush (process") && !h.contains(" | "), "only the busy slot is named: {h}"),
            other => panic!("one slot is held: the exclusive run waits ({:?})", other.map(|_| ())),
        }
        assert_eq!(holder_notes(&d).len(), 1, "the failed exclusive try left no note and holds nothing");
        drop(a);
        let e = try_acquire_in(&d, "perf", ALL).unwrap().expect("every slot free");
        assert_eq!(e.slots_held(), 2);
        assert!(holder_notes(&d).iter().all(|n| n.starts_with("perf (process") && n.ends_with("exclusive)")));
        assert!(try_acquire_in(&d, "test", TWO).unwrap().is_err(), "an ordinary job waits while the exclusive run holds both");
        drop(e);
        assert!(try_acquire_in(&d, "test", TWO).unwrap().is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_queued_exclusive_run_blocks_later_ordinary_jobs_so_it_is_not_starved() {
        let d = dir("starve");
        let holder = try_acquire_in(&d, "probe", TWO).unwrap().expect("free");
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let spawn = |name: &'static str, req: Req| {
            let (d, order) = (d.clone(), order.clone());
            std::thread::spawn(move || {
                let g = acquire_in(&d, name, req, Duration::from_millis(5), Some(Duration::from_secs(20)), &mut |_| {})
                    .unwrap()
                    .expect("its turn comes");
                order.lock().unwrap().push(name);
                std::thread::sleep(Duration::from_millis(50));
                drop(g);
            })
        };
        let e = spawn("perf", ALL);
        while live_tickets(&d).is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
        // Slot 2 is free, but the exclusive run queued first: the ordinary job queues behind it.
        let o = spawn("prepush", TWO);
        while live_tickets(&d).len() < 2 {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(50));
        assert!(order.lock().unwrap().is_empty(), "nobody passed the queued exclusive run");
        drop(holder);
        e.join().unwrap();
        o.join().unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["perf", "prepush"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn the_overdue_warning_names_each_overdue_holder() {
        let (now, limit) = (1_000_000u64, Duration::from_secs(90 * 60));
        let note = |what: &str, min: u64| format!("{what} (process 7, in lane, since {} s after the epoch, slot 1 of 2)", now - min * 60);
        let held = format!("{} | {}", note("probe", 120), note("prepush", 10));
        let m = waiting_message(&Busy::Held(held), "test", now, limit);
        assert!(m.contains("overdue: probe (process 7") && m.contains("held 120 min"), "{m}");
        assert!(!m.contains("overdue: prepush"), "{m}");
        assert_eq!(overdue_phrases(&note("prepush", 10), now, limit), "");
    }

    // spec: BUILD-026
    #[test]
    fn heavy_takes_the_exclusive_flag_before_the_command_and_the_slots_are_a_parameter() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let a = s(&["--exclusive", "cargo", "run"]);
        assert_eq!(wrapped_mode(&a), (true, &a[1..]));
        let b = s(&["cargo", "run", "--exclusive"]);
        assert_eq!(wrapped_mode(&b), (false, &b[..]), "only a leading flag");
        assert_eq!(Req::shared().slots, slot_count());
        assert!(Req::exclusive().exclusive && !Req::shared().exclusive);
        assert!((1..=MAX_SLOTS).contains(&slot_count()));
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

    // spec: BUILD-026
    #[test]
    fn a_ticket_that_cannot_be_opened_still_queues_its_waiter_and_a_removed_one_does_not() {
        let d = dir("unopenable");
        let t = tickets_dir(&d);
        std::fs::create_dir_all(&t).unwrap();
        // A ticket name that is a directory: opening it as a file fails with an error other than NotFound.
        std::fs::create_dir_all(t.join("000000000000000000000001-0000000001-000000")).unwrap();
        std::fs::write(
            t.join("000000000000000000000001-0000000001-000000.note"),
            "older waiter (process 1, in w, queued since 1 s after the epoch)",
        )
        .unwrap();
        let live = live_tickets(&d);
        assert_eq!(live.len(), 1, "{live:?}");
        assert!(live[0].1.starts_with("older waiter"), "{live:?}");
        // A newcomer queues behind it even though every slot is free.
        let mut said = Vec::new();
        let got =
            acquire_in(&d, "later", ONE, Duration::from_millis(5), Some(Duration::from_millis(30)), &mut |m| said.push(m.to_string()))
                .unwrap();
        assert!(got.is_none(), "the later arrival waits behind the ticket it could not open");
        assert!(said.iter().any(|m| m.contains("1 queued ahead (next: older waiter")), "{said:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-026
    #[test]
    fn a_holder_note_without_a_slot_is_named_as_an_xtask_older_than_the_queue() {
        let old = "test gpu (process 8008, in w, since 1794748947 s after the epoch)";
        let new = "test fast (process 9, in w, since 1794748947 s after the epoch, slot 2 of 2)";
        let all = "perf (process 7, in w, since 1794748947 s after the epoch, exclusive)";
        let m = waiting_message(&Busy::Held(format!("{old} | {new}")), "prepush", 1_794_748_950, OVERDUE_DEFAULT);
        assert!(m.contains("held by an xtask older than the ticket queue, which does not queue: test gpu (process 8008"), "{m}");
        assert!(!m.contains("queue: test fast"), "{m}");
        for quiet in [new, all, "another heavy command (no holder note)"] {
            assert_eq!(pre_queue_phrase(quiet), "", "{quiet}");
        }
    }
}
