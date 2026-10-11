## Why

The owner asked for more autonomous execution on golden changes (2026-10-11). PR #43's five goldens needed the owner on three adapters, and the golden test stopped at its first mismatch, so llvmpipe took five CI rounds.

## What Changes

- ADR 0017 (accepted by the owner): golden review policy; constitution v1.2 amends CON-18; CLAUDE.md and `/bless` updated.
- `testkit::GoldenChecks`: a test checks every golden before failing; `gpu_terrain` and `gpu_debug_views` use it.
- `testkit::review`: the pure policy classifier; `golden-review` bin (triptychs, report, `--apply`); `cargo xtask golden-review`.
- `tests/goldens/classes.txt` (ADR 0009 classes; `terrain_ground1m_depth` and `terrain_aerial200km_height` are shaded, owner 2026-10-11).
- `.claude/skills/golden-review/`.
- Added: TEST-016.

Expected golden changes:

(none: no pixel changes)

## Impact

`crates/testkit`, `crates/render/tests`, `xtask`, docs. Dry run on PR #43's approved candidates: 15 of 15 auto (5 R, 10 E) with the declarations, 10 escalations without them.
