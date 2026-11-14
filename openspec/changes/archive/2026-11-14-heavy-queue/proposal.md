## Why
- No queue order: waiters poll, and whoever polls first after a release wins (crusader waited from 11:45, spikes started at 12:14 and got the lock first; lead, 2026-11-14, explorer #144).
- No overdue warning: a hung holder blocks everyone without a word.

## What changes
- BUILD-026: waiters go in arrival order through tickets in `tickets/` of the state directory (each held under an OS file lock, a note beside it); only the oldest live ticket tries the lock; a newcomer queues behind the tickets even while the lock is free; a dead waiter's ticket (lock free, older than 5 s) is removed.
- A holder past `PLANET_HEAVY_OVERDUE_MIN` (default 90) is named "overdue: <holder note>, held N min" in the waiters' messages; the lock is never broken.

## Impact
xtask (heavy.rs). The lock file, the holder note, the quiet flag and the child pass are unchanged; an older xtask in another worktree ignores the tickets (it takes the lock as before) until it merges main.
