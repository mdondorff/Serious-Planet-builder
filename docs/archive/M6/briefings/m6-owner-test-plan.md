# M6 editor: owner test plan

Purpose: the owner's hands-on review at the M6 gate (tier D: the look and the feel). The automated tests already prove the numbers (`docs/briefings/m6-gate-evidence.md`).
Time: about 45 minutes. Each block takes 5 to 10 minutes; skip any block.

## Start
- Build on main after #479: `cargo run --release -p planet -- editor --edit --items target/owner-items`
- Tab switches the editor panels on and off. F1 or ? lists the keys.
  - MD: Tab does not turn editor on or off. Tab currently cycles focus on UI elements. Correction: Tab toggles Editor when the mouse is captured. Alt+tab to the application toggles the editor mode (note: it should not do that).
  - MD: Mouse capturing works only when editor is off. Turning editor on/off is only possible with alt+tab initially. Then it works as long as the mouse stays captured. Once in the editor UI mode, mouse can't be captured anymore and also toggle does not work anymore.
  - MD: F1 or ?: Works well.
- Something wrong: press **F9** at once (it saves the last minute of camera poses and the frame's repro bundle to `target/review/`), then write down the time and what you saw.
- Keys stop working: read the end of the window title (`keys .. | pointer .. | overview`) before doing anything else.

## Assorted feedback / notes

- MD: Slight atmosphere (?) shading lag. Going quickly from surface to orbit to surface and back to orbit: There is a faint shift in colors on the sphere. Taking about <1s to stabilize. The globe gets slightly lighter.
- MD: Lighting: Parts of the planet are just plain black. Nice for visuals, but hard for editing, eventually. Shading is important though to understand shape of the landscape. Maybe having a editing lighting mode where the sun's direction / position is always relative to the camera and thus will always show "12 o'clock" or "sundawn" indepdendent of where the camera actually is. Think of defining the sun's directional vector in the local camera space and then updating the world space directional vector afterwards. This might be an inelegant idea and other things could work as well.

## 1. Camera and keys (G5, G10)
- [x] Hold the right button: look around. WASD and QE move. Do moving and looking feel right?
  - MD: Potential UX consideration: Let right mouse button look interact wherever the cursor is, also on UI. This might be a poor idea though, as the right mouse button might be needed on the UI elements.
- [x] Press G (globe), then G again, then move with WASD. Repeat 3 times, with and without clicking a panel first.
  - MD: The 'click to move' feature in the globe view is very usable. It feels slightly awkward so to understand when this mode is active. E.g. moving to a orbital altitude and clicking doesn't do anything, which feels slightly off. Not a huge concern though.
- [ ] If the keys die: the title shows keys held but no movement, or shows no keys? Write down the steps.
- [x] Mouse: does the look speed feel the same in a slow and a fast sweep? Does it stay smooth with the window on a scaled display (if you have one)?
  - MD: No scaled display , otherwise good.

## 2. Stamps (2 crater, 3 hill, 4 plateau)
- [x] Place one of each, close (about 200 m altitude) and from about 2 km.
  - [x] Crater
  - [x] Hill
  - [x] Plateu
- [x] Is it visible at once (the preview), and does the exact result follow within about a second with no jump?
  - MD: Generally the speed and UI lag is just well. There are noticable movements and re-generation is happening and sometimes there is a tile missing visually. But I don't think this is looking very concerning, ask me for a video (2026-10-20-edit-latency-tile-refresh.mp4).
- [x] Type a value in the inspector (for example a radius of `300`). Does it apply?
  - MD: UX: Typing a value and hitting enter: The focus should stay in that input field. Further: Cursor up and down should move the values. However potentially also dependend a bit on the scale. Maybe shift makes it 10x or so.

## 3. Lines and regions (5 ridge, 6 channel, 7 region)
- [x] Click points, Enter finishes, Backspace takes the last point back.
- [x] Shift+click adds a point, Ctrl+click removes one, and the amber dots drag.
- [x] Draw a region and set its surface type. F3 cycles the overlays: does the tint show the type?
- [ ] Set a region's mask to clip a layer: do that layer's edits apply only inside it?
  - MD: No. It looks like to be inversed, as if the layer "stamp" (the only selectable layer atm afaict) is being removed from the inside. At the edges of that clip region the other edited terrain is harshly clipped. I'm atm not sure of the value of that current version and for what I would use a hard clip like this for. Low priority to fix , but important to track.

## 4. Brushes (8 raise, 9 lower, 0 smooth, minus flatten)
- [x] Paint wwa stroke with each. Raise, lower and flatten preview while you drag; smooth shows when exact (a known limit).
  - MD: UX improvement: Mouse scroll changes radius, shift+scroll changes strength.
  - MD: After some edits with the 'smooth' brush: Instead of smoothening, it starts to sharpen (Example: Brush 23, a 'smooth' brush. If that one is removed, it appears 'more smooth', with it applied it appears sharpened)
- [x] Type a radius and a strength in metres. Flatten starts at the ground height under the stroke.
- [x] Ctrl+Z takes back one stroke.
- [x] F1 lists every key, the brush keys included.

## 5. Managing edits
- [x] Hover shows the outline and the name; click selects.
  - MD: Sometimes hard to pick: Stamps close to each others. Eg. a big plateau and some hills places inside ("Plateau 8" and "Hill 2"). Caveat: Might be that the plateau places after the hill makes the hilld be 'under' the terrain and thus not clickable. Not sure what the correct behavior should be when another larger edit is placed with 'lower' edits contained.
- [x] Search finds an edit planet-wide, and the list near the camera changes as you fly.
  - MD: Potential: Scalability concern with _many_ edits. But non-issue atm.
  - MD: Otherwise: Curently very quick and functional 'near' view.
- [x] The layer list: a split line or a brush drag is one row; double-click flies there; Delete removes the whole item.
- [x] Undo and redo through a mix of 10 to 20 actions: does everything come back?
- [x] Close the editor, start it again with the same `--items` folder: are your edits back?

## 6. Fixed today: confirm by eye
- [x] Sea edge: fly low over a coastal plain (where frames 5161, 6193 and 13883 showed the straight edge). Does the sea reach the whole plain?
- [ ] Atmosphere: descend from orbit to the ground slowly. Do you see rings, bands or a colour jump (especially between 10 and 6 km)?
  - MD: At altitude 100km there appears to be a very tiny colour jump, when decending. So between 95km and 100km.
- [ ] Tiles: while moving, do tiles pop to a coarser level?
  - Yes, especially noticable near the sea levels and when using the debug view. When moving, something is different as to when staying still. Ask me for a video (2026-10-20-bug-tile-lod-flashing-on-movement.mp4).

## 7. Overall look (G4)
- [x] Is the panel layout readable and the map-first flow natural?
- [ ] One thing that most needs to change before you build a real world with it?
  - MD: Stability. And then potentially some way to update the generated world as well.

## Report back
A line per block: ok, or what was wrong (with the F9 frame number). Screenshots worth keeping go to `docs/dev_screenshots/`.


## Retest after the P1 fixes (2026-10-21)
Pull main first (#483 to #489). Crashes now write `target/review/crashes/<time>.txt`; F9 also saves `frame-<n>.png`.
- [ ] P1-1 (#489): fly to "Plateau 7" and the other plateaus: no "too steep" lines in the console and no "plans refused" in the title.
- [ ] P1-2 (#489): smooth over an area several times (and brush 23): it smooths, never sharpens.
- [ ] P1-3 (#486): Tab toggles the editor (unless typing in a field); Alt+Tab into the window does not; right-button look works in the editor mode.
- [ ] Keys after G (#486): G, G, then WASD, after clicking a panel button.
- [ ] P1-5 (#487, #489): with no coder building, does editing still lag?
- [ ] P1-4: if tiles flash while moving, press F9 right after the flash and note the frame number.

## Crashes

### CRSH-0001

Steps done:
- Editing with a crater stamp, one stamp, several updates to that stamp.
- Crash as I was approaching the ground (altitude maybe below 30m).

```
error: plan for frame 13483: invalid LOD parameters: tile TileId(1369094391616068718) (level 19): morph interval [290.9, 248.0] m is empty because the height extent of its neighbourhood leaves no room; this terrain is too steep for cells 32 and viewport_h_px / tau_px 240: raise viewport_h_px / tau_px or reduce cells
error: process didn't exit successfully: `target\release\planet.exe editor --edit --items target/owner-items` (exit code: 1)
```
c:\DEV\Serious-Planet-Builder\target\review\run-201403.log


### CRSH-0002

Steps done:
- In an earlier session: placed several plataeu stamps. During that a crash (not recorded).
- Restart, navigation back to the plataeu area.

```
error: plan for frame 1233: invalid LOD parameters: tile TileId(7782220156112864104) (level 12): morph interval [30475.2, 31738.7] m is empty because the height extent of its neighbourhood leaves no room; this terrain is too steep for cells 32 and viewport_h_px / tau_px 240: raise viewport_h_px / tau_px or reduce cells
error: process didn't exit successfully: `target\release\planet.exe editor --edit --items target/owner-items` (exit code: 1)
```

### CRSH-0003

Steps done:
- Place plateau near a sea ("Plateau 9")
- Crash
- Note: Upon re-opening and visiting the same plateau: No crash. Moving plateau worked.

```
error: plan for frame 8661: invalid LOD parameters: tile TileId(1080863911470671201) (level 15): morph interval [4055.5, 3967.3] m is empty because the height extent of its neighbourhood leaves no room; this terrain is too steep for cells 32 and viewport_h_px / tau_px 240: raise viewport_h_px / tau_px or reduce cells
error: process didn't exit successfully: `target\release\planet.exe editor --edit --items target/owner-items` (exit code: 1)
```

run-204203.log

### CRSH-0004

Steps done:
- Brush a bit
- Place a nearby plateu. ("Plateu 7"))
- Crash

Reproduction:
- Fly to "Plateau 7"
- Crash

```
error: plan for frame 1400: invalid LOD parameters: tile TileId(7782220156112863447) (level 12): morph interval [30475.2, 31738.7] m is empty because the height extent of its neighbourhood leaves no room; this terrain is too steep for cells 32 and viewport_h_px / tau_px 240: raise viewport_h_px / tau_px or reduce cells
error: process didn't exit successfully: `target\release\planet.exe editor --edit --items target/owner-items` (exit code: 1)
```
