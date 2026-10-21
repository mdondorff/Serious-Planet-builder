# sonnet-crusader handover, 2026-10-21

State: every change below is merged except the last (globe-click-hint, PR open when this was written). Nothing is queued on this lane.

## Merged this session (newest first)
| PR | Change | Requirement |
|---|---|---|
| globe-click-hint | the overview names what a click does: pointer cursor and hint only where it acts, title notice for an ignored click | VIEW-045 |
| #503 | stale-tile overlay, F3's fifth stop | VIEW-044 |
| #496 | editing light, H | VIEW-042 |
| #493 | value fields step (Up/Down), brush wheel, click-through selection | VIEW-041 |
| #487 | refusal log throttle | VIEW-040 |
| #486 | Tab/focus/Alt+Tab fixes | VIEW-039 |
| #485 | measured runs invalid on a refused plan | VIEW-038 |
| #483 | plan refusal degrades, crash report, F9 frame PNG | VIEW-032, VIEW-037 |

## Open items
- Root cause of the owner's level-15 plan refusal (frame 5640, default world, 256 bake) is NOT found. The tile named in the error is flat (extent 8 to 35 m); the refusing start comes from the worst level-15 node in view (the selection equalises the start over a level). 400,000 random level-15 tiles of the default world have a bound extent of at most 59 m, and the dive script plus 600 poses around the tile are never refused. The owner explains it with the test plan; the crash report (`target/review/crashes/`) and F9 now capture the pose. Explorer's P1-1 (loose plateau and brush bounds) is the likely family.
- Right-button mouse capture failing in the editor mode (owner P1-3): no separate cause found. #486 fixed the stale pointer latch after a capture; needs the owner's retest.
- Keys dead after G (status.md entry): probable cause is #486 (a panel widget kept the keyboard focus after Tab). The entry stays open until the owner has not seen it again.
- The capped tile state (ADR 0074): `TileState::Capped` and its grey exist, nothing assigns them. Set `state = Capped` for capped nodes in `draw_plan_transient` (crates/frame/src/pipeline.rs) and the overlay shows it.
- The editing light uses one sun for terrain, atmosphere and sky (the lit plan has a single `sun_dir`). If the sky looks odd, add a separate sky-sun field to the plan.
- The crash report does not keep the last presented frame (F9 writes `frame-<n>.png`, the scene without the egui panels).
- The window wiring of the brush wheel, the Alt+Tab synthetic-key filter and the cursor icon has no test of its own; the pure parts do.

## Non-obvious decisions and facts
- A plan refusal for the frame's own request never ends the window: `PlanPipeline` keeps the newest plan and counts it (`refused_plans`); `RefusalReporter` prints once per cause (kind and level) per 5 s. Measured and checked runs (editperf, streamperf, pipelined, scheduler ablation) use `ManagedLoop::begin_frame_strict` and end invalid on any refusal.
- `TerrainDraw::state` (`TileState`) is set by the draw plan, carried in the node record as level + 32 x state code, and is not in the plan text or hash. Any renderer shader that shares the fragment code needs a `stale_tint` (the reference renderer has a no-op).
- Only a text field being edited takes keys from the camera and the editor (`Context::text_edit_focused`); Tab outside a text field never reaches egui; winit's `is_synthetic` key presses are ignored.
- Select press and hover take the smallest footprint first (`Session::pick_all`); `Session::pick` (nearest centre in its own radius) is unchanged for everything else.
- Enter keeps the focus in inspector and brush fields, so a field no longer follows the item's value straight after Enter (two panels tests click away first).
- `cargo xtask prepush | tail` hides prepush's exit code. Redirect to a file and read `$?`, or read the log, before trusting "passed".
- In this environment a `cat <<EOF >>` heredoc append and `git rebase` were refused by the permission classifier; use the Edit and Write tools, and cherry-pick onto a fresh branch instead of rebasing.
- Files under crates/ are CRLF; the Edit tool copes, `cargo fmt` normalises.
- A GPU test run with plain `cargo test` in one process can crash (access violation); `cargo xtask test gpu` runs them one process each.

## Where things are
- Crash report: crates/app/src/crash.rs (`PLANET_CRASH_DIR` overrides the directory); hooks in main.rs and window.rs.
- Editing light: `editing_sun_dir` in crates/frame/src/viewer.rs.
- Overlay modes and shader code: crates/frame/src/look.rs `Overlay`; shaders in crates/render/src/compact.rs (`overlay_wire`, `stale_tint`), terrain.rs and atmosphere.rs fragment stages.
- Value fields: `value_field` in crates/app/src/panels.rs; stepping in crates/editor/src/params.rs `stepped`.
- Globe click: `GlobeSession::click_target` in crates/frame/src/globe.rs; `update_click_target` in crates/app/src/window.rs.
