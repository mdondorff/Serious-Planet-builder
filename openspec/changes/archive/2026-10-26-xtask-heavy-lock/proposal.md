## Why
Three lanes' heavy runs (prepush, tests, GPU tests, stills, perf) started in parallel on one machine each got about one core and timed out under the load; serialised, each finishes sooner. The owner also needs the machine quiet at times (opus-planner, 2026-10-25) and had to ask by message.

## What Changes
- One machine-wide heavy-run lock outside every worktree (`%LOCALAPPDATA%\planet\heavy.lock`, an OS file lock released when the process ends, with a sidecar file naming the holder): `cargo xtask test`, `prepush`, `perf`, `capture`, `accept`, `store` and the new `heavy <command>` (any diagnostic example or replay) wait their turn and print "waiting for <holder>" while queued.
- The owner's flag file `%LOCALAPPDATA%\planet\quiet` (`cargo xtask quiet on|off|status`): while it exists every heavy command waits; commands already running finish.
- A state directory that cannot be made, or `PLANET_HEAVY_LOCK=off`, runs the command without the lock and says so; a heavy command run inside another in the same process (prepush runs test fast) takes the lock once.

## Impact
- Spec: build-tooling (BUILD-026).
- Code: `xtask/src/heavy.rs` (new), `xtask/src/main.rs`. `check`, `spec-lint`, `layering` and the light commands stay free.
