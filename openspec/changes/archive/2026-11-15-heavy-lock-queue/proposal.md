## Why
- A docs-only prepush, which runs spec-lint alone in seconds, waited over 45 minutes behind E3 bakes for a slot (lead #156, item 1).
- At 14:22:27 and 14:23:18 on 2026-11-15, spikes' bake_eroded runs took both slots while two older live tickets waited: 17044 (14:03:41) and 9588 (14:03:48) (lead #156, item 2).
  - Today's code has no path that lets a later arrival take a slot ahead of a live ticket: every heavy command goes through `acquire_in`, which queues behind all live tickets.
  - Two ways remain:
    - an xtask built before the ticket queue (#1510) takes `heavy.lock` (slot 0's file) without queueing. `crusader-main/target/debug/xtask.exe` dates from 2026-11-14 13:17, four hours before the queue merged, and runs so when called directly;
    - `live_tickets` skipped a ticket it could not open, so its waiter became invisible.
  - The binary that ran at 14:22 cannot be identified: spikes' worktree rebuilt its xtask at 20:49.

## What changes
- BUILD-026:
  - a prepush whose plan is spec-lint alone takes no slot;
  - a ticket that cannot be opened (but one that is gone) counts as live;
  - a holder note that names no slot is named in the waiting message as an xtask older than the queue, which does not queue.
- Tests: tier A in `xtask/src/heavy.rs` and `xtask/src/prepush.rs`.

## Impact
xtask only (`heavy.rs`, `prepush.rs`, `main.rs`). The slot and ticket files keep their names, so older xtask binaries still share slot 0.
