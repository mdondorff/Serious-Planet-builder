# SPIKE-11 manual check: mouse look (about 10 minutes, locally and over Chrome Remote Desktop)

Run from `C:\DEV\Serious-Planet-Builder\.claude\worktrees\sonnet-spikes\spikes\mouse-look`:

    target\release\mouse-look.exe

(or `cargo run --release`). A window opens on a small checkered planet (white cap at the north pole, red at the south, an orange line on the
zero meridian, a small crosshair in the middle). **The window title is the read-out**: `src <chosen>(<in use>) grab <mode> cap <YES/no>`, `raw N/s cur N/s`
(events per second from each source), `rec`/`fail` (recentres and recentres that did not take effect), `yaw`/`pitch`/`hdg` in degrees, fps, the display scale
`xN`, and the last thing the window did (grab results, errors) at the end.

## Keys
Click in the window = capture the mouse (cursor hidden). `Esc` = release. `W A S D` move, `Q` down, `E` up (speed grows with altitude), arrow keys look.
`F1` input source: auto / raw / cursor. `F2` grab: confined / locked-then-confined / none (applies at the next click). `F3` recentre box: 0.25 / 1 (never) / 0 (every event).
`F4` save the trace (to `target\traces\`) and start a new one. `F5` back to the start. `F6` slow frames (50 ms) on/off. `F7` vsync on/off. `F8` mark a step in the trace.
`-` / `=` sensitivity.

## Local (sitting at the laptop)
1. Start. Click in the window. Press `F8`. Move the mouse slowly to the right, about 10 cm. **The view must turn right smoothly.** Note the title: is `raw` above 0? Is `src` `auto(raw)`?
2. Press `F5`, then move the mouse right 10 cm at a steady speed and note `yaw` in the title (A). Press `F6` (slow frames), `F5`, repeat the same 10 cm, note `yaw` (B). Press `F6` off, `F7` (no vsync, high fps), `F5`, repeat, note `yaw` (C). **A, B and C should be about equal** (a few percent from your hand, not a factor of 2).
3. Spin the mouse in several full circles in both directions, fast. The view must keep turning without sticking at the screen edge or drifting when the mouse stops.
4. Press `F1` until `src` shows `cursor`, then `raw`, each time `F8` and then repeat steps 1 and 3. Note any jerk, stuck direction, drift while the mouse is still, or a sudden jump when the cursor is recentred (`rec` counts up).
5. Press `F2` to `locked>confined`, `Esc`, click again: the title's last note says whether Locked was refused and Confined was used. Then `none`, click, and try to move the cursor out of the window: can you? Return to `confined`.
6. Focus: with the mouse captured, press Alt-Tab to another window and back, and also click on another window and back. **The view must not jump**, the cursor must be visible while away, and after returning nothing turns until you click again.
7. Poles: hold `E` for 2 seconds (higher is faster), then `W` until you reach the white cap at the north, and look around with the mouse while above it and while crossing it. **The horizon stays level, nothing flips, nothing spins suddenly.** Look straight up and straight down: the view stops short of vertical.
8. If your mouse polls at 1000 Hz or more: spin fast and note `raw N/s` and whether the frame rate drops or the motion stutters. If the display scale is not 100 % (`x1.5` etc.): does the turn per cm feel the same as in step 1?
9. Press `F4` to save the trace. Do `F4` again after each of the source changes in step 4 if you can, so each source has its own trace.

## Remote (Chrome Remote Desktop: the app runs on the machine you connect to; you sit at the other computer)
Repeat steps 1, 3, 4, 5 and 6 (skip 7 to 9). What to look for: does `raw` stay at 0? Does the view turn in `cursor` mode? Does it turn at all in `raw` mode? Does recentring work (`fail` stays 0) or does the view creep or jump (`fail` grows)? If the Chrome Remote Desktop client offers a mouse option (relative mouse, pointer lock, capture), try with and without and say which. Press `F4` after each source.

## What to send back
The title text after steps 1, 3 and 4 (a screenshot of the title is enough), the answers in bold, and the files in `target\traces\` (they also hold the counters and every note the window made).
