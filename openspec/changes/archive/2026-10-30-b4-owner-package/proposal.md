## Why
Option B item B4-5 (design PR #910; planner order 2026-10-30): the owner reads results remotely, so the stills and short videos of his pose sites need one command that writes them with a verdict page. The planner sends the directory to the owner's phone (SendUserFile); the command has no upload or scheduler of its own.

## What Changes
1. `cargo xtask package [--store DIR] [--date D] [--out DIR] [--dry-run]`: runs the existing `capture stills` for the default look of the three look-review sites and `capture flights`, into `target/review/owner-package/<date>/`, and writes `package.md` with PASS, FAIL or NOT RUN per item.
2. A step that cannot run (no store) is NOT RUN with its reason; a failed step is FAIL with its message and the others still run; a command that wrote no file is FAIL.
3. Detectors (B4-2 and later) join as items when they have a log; that is a later change.

## Impact
- Spec: `testing` gains TEST-074.
- Code: `xtask/src/package.rs` (new), `main.rs` (subcommand, usage), `accept.rs` (`utc_date` crate-visible).
- No GPU needed for the tests; the real run needs the real store on a quiet, unlocked machine.
