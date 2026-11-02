## Why
`editor --smoke` on the real store needs an unlocked desktop (TEST-073), so while the desktop is locked the gate gives NOT RUN and no result about the store's logic (SPIKE-84, fix 6).

## What changes
- `cargo xtask gate` on the real store also runs `editor --smoke --edit --no-vsync --adapter software --store DIR` once (no camera path; about 2m45s).
- The result is printed whatever the desktop does; a locked desktop still makes the hardware part NOT RUN (exit 3), a failed software smoke is a failure (exit 1). The exit is the worst of the parts.
- The gate snapshots the store's files before and after the run and fails if any was added, removed, resized or touched.

## Impact
- Spec: TEST-085 (new, reserved by the planner 2026-11-01); TEST-073 stays as it is.
- Code: `xtask/src/gate.rs`.
