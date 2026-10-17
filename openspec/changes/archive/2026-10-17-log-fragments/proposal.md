## Why
Every PR prepended a line to `docs/log.md`, so parallel writers conflicted on it daily; PR #222 merged unresolved markers this way. The `housekeeping` skill names one fragment per change as the fix.

## What changes
- A log entry is a file `docs/log.d/<YYYY-MM-DD>-<id>.md` holding one line.
- `cargo xtask log [--since YYYY-MM-DD] [--limit N]` prints fragments and `docs/log.md` entries merged, newest first.
- `cargo xtask spec-lint` rejects a malformed fragment (BUILD-010).
- `docs/log.md` keeps its history; CLAUDE.md and the `adr`, `accept`, `spike`, `learning` and `housekeeping` skills point to fragments.
