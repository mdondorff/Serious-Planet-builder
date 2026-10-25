## Why
The owner cannot see the horizon shadows in the viewer (opus-planner, 2026-10-25): the sun is the look's fixed planet-fixed direction, the editing light (H) puts it 35 degrees up behind the camera, and the Horizon and Shadow shade views exist only in the test harness. A world without a chunk stage (no store, or a store without a coupled run) draws no horizon shadows by design, and nothing in the window says so.

## What Changes
- J cycles the lit shade view: normal, horizon, shadow (`Command::CycleShadeView`).
- `[` and `]` move the sun half an hour earlier or later in local solar time at the camera, over the look's sun; Backslash returns to it. The editing light wins while it is on.
- The title names a shade view other than normal, the local solar time of a moved sun, and a lit window without a chunk stage ("no chunk stage: no horizon shadows (open a store with a coupled run)").
- The help texts (usage, F1) name the keys.

## Impact
- Spec: VIEW-054.
- Code: `frame/viewer.rs` (commands, state, pure helpers), `app/window.rs`, `app/editwin.rs` (help text).
- A viewer without the keys pressed draws as before (sun offset 0, normal view), so goldens stay.
