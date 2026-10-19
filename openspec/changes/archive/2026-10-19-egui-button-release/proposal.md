## Why
The pointer look-around of the editor mode (right button held) captures the pointer on the press. While the pointer is captured the window gives egui no pointer event (VIEW-011 routing), so egui never sees the release: the secondary button stays down for it (hover and clicks wait for some later event). The test shows it: after the press and a few silent passes the secondary button is still down.

## What Changes
- `ui::release_all_buttons` and `EditorUi::release_buttons`: a move far from the cursor (egui counts a click by the moves, not by the position of the release), then every button up; `apply_grab` sends them when a capture starts and when it ends.
- VIEW-022: one bullet and one scenario.

## No golden moves
Editor-only UI.
