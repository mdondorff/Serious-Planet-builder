## MODIFIED Requirements
### Requirement: BUILD-026 Heavy commands queue on one machine-wide lock and obey the owner's quiet flag
`cargo xtask test` (every form), `prepush`, `perf`, `capture`, `accept`, `store` and `heavy <command>` SHALL take one lock outside every worktree (the file `heavy.lock` in the state directory: `PLANET_STATE_DIR`, else `SeriousPlanetData/tmp/heavy` in the nearest ancestor of the workspace that holds the data root (ADR 0121), else `%LOCALAPPDATA%\planet`, else `$XDG_STATE_HOME/planet`, else `~/.local/state/planet`) before they run, and a second one SHALL wait for the first instead of running beside it, saying "waiting for <holder>" (what, process, worktree) at the start of the wait and every 30 seconds. The lock SHALL be an operating-system file lock, so that it is released when the holder's process ends however it ends.

Verify: A
Status: active
Source: opus-planner (2026-10-25): three lanes' runs each got about one core; the owner needs to clear the machine without messaging anyone; lead 2026-11-14 (queue order, overdue warning: a later waiter took the lock first; a hung holder blocks everyone without a word)

#### Detail
The data root comes before `%LOCALAPPDATA%` because the desktop app redirects `%LOCALAPPDATA%` for its sessions, and a terminal and a session must see one lock.
Waiters SHALL go in arrival order: a command that finds the lock held, or other waiters queued, SHALL take a ticket (a file in `tickets/` of the state directory named by its arrival time, held under an operating-system file lock, with a note beside it), and only the oldest live ticket SHALL try the lock; a newcomer SHALL queue behind the tickets even while the lock is free. A ticket whose file lock is free and that is older than 5 seconds belongs to an ended process and SHALL be removed by the next reader. A holder that has held the lock longer than the overdue limit (`PLANET_HEAVY_OVERDUE_MIN`, default 90 minutes) SHALL be named in the waiters' messages as "overdue: <holder note>, held N min"; the lock SHALL never be broken automatically.
While the file `quiet` exists in the state directory every heavy command SHALL wait before it takes the lock and say that the flag is up (`cargo xtask quiet on|off|status` makes and removes it); a command already running SHALL finish. A flag older than 60 minutes SHALL be ignored, with a warning that names its age (the owner can forget it), and `quiet on` SHALL renew it. A heavy command SHALL set `PLANET_HEAVY_HELD` to its process id for its children, and a process whose `PLANET_HEAVY_HELD` names the process of the holder note SHALL take nothing and wait for nothing (a stale or wrong value is no pass). A state directory that cannot be used, or `PLANET_HEAVY_LOCK=off`, SHALL run the command without the lock and say so, and a heavy command run inside another in the same process SHALL take the lock once. `check`, `spec-lint`, `layering` and the other light commands SHALL not take it.

#### Scenario: A queue
- **WHEN** a second heavy command starts while the first runs, and then the first ends
- **THEN** the second says it is waiting for the first's holder note and runs once the first has let go, and no message is printed when nobody waited

#### Scenario: The quiet flag
- **WHEN** the quiet flag file exists and a heavy command starts, and the file is then removed
- **THEN** the command waits and says the flag is up, and goes once the flag is down; `quiet on`, `status` and `off` raise, show and lower it

#### Scenario: A flag the owner forgot
- **WHEN** the flag is two hours old, and then `quiet on` is run
- **THEN** the first is ignored with a warning naming the age and the second holds again

#### Scenario: Arrival order
- **WHEN** two commands queue behind a holder one after the other, and a third arrives while the lock is free but one is still queued
- **THEN** the first queued runs first, the second next, and the third waits saying how many are queued ahead; a dead waiter's ticket is removed

#### Scenario: An overdue holder
- **WHEN** the holder has held the lock 100 minutes with the limit at 90
- **THEN** the waiters' message says "overdue: <holder note>, held 100 min", and the waiters still wait

#### Scenario: A child of the holder
- **WHEN** a process whose `PLANET_HEAVY_HELD` is the holder's id asks for the lock, and then ones with another id, an empty or a non-numeric value, and none
- **THEN** the first goes at once and holds no lock of its own, the others wait, and after the holder is done the id is no pass
