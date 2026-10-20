# M6 editor: owner test plan

Purpose: the owner's hands-on review at the M6 gate (tier D: the look and the feel). The automated tests already prove the numbers (`docs/briefings/m6-gate-evidence.md`).
Time: about 45 minutes. Each block takes 5 to 10 minutes; skip any block.

## Start
- Build on main after #479: `cargo run --release -p planet -- editor --edit --items target/owner-items`
- Tab switches the editor panels on and off. F1 or ? lists the keys.
- Something wrong: press **F9** at once (it saves the last minute of camera poses and the frame's repro bundle to `target/review/`), then write down the time and what you saw.
- Keys stop working: read the end of the window title (`keys .. | pointer .. | overview`) before doing anything else.

## 1. Camera and keys (G5, G10)
- [ ] Hold the right button: look around. WASD and QE move. Do moving and looking feel right?
- [ ] Press G (globe), then G again, then move with WASD. Repeat 3 times, with and without clicking a panel first.
- [ ] If the keys die: the title shows keys held but no movement, or shows no keys? Write down the steps.
- [ ] Mouse: does the look speed feel the same in a slow and a fast sweep? Does it stay smooth with the window on a scaled display (if you have one)?

## 2. Stamps (2 crater, 3 hill, 4 plateau)
- [ ] Place one of each, close (about 200 m altitude) and from about 2 km.
- [ ] Is it visible at once (the preview), and does the exact result follow within about a second with no jump?
- [ ] Drag the handles: the circle moves it, the square sets the radius, the light circle the height. Is dragging the main way that works?
- [ ] Type a value in the inspector (for example a radius of `300`). Does it apply?

## 3. Lines and regions (5 ridge, 6 channel, 7 region)
- [ ] Click points, Enter finishes, Backspace takes the last point back.
- [ ] Shift+click adds a point, Ctrl+click removes one, and the amber dots drag.
- [ ] Draw a region and set its surface type. F3 cycles the overlays: does the tint show the type?
- [ ] Set a region's mask to clip a layer: do that layer's edits apply only inside it?

## 4. Brushes (8 raise, 9 lower, 0 smooth, minus flatten)
- [ ] Paint a stroke with each. Raise, lower and flatten preview while you drag; smooth shows when exact (a known limit).
- [ ] Type a radius and a strength in metres. Flatten starts at the ground height under the stroke.
- [ ] Ctrl+Z takes back one stroke.
- [ ] F1 lists every key, the brush keys included.

## 5. Managing edits
- [ ] Hover shows the outline and the name; click selects.
- [ ] Search finds an edit planet-wide, and the list near the camera changes as you fly.
- [ ] The layer list: a split line or a brush drag is one row; double-click flies there; Delete removes the whole item.
- [ ] Undo and redo through a mix of 10 to 20 actions: does everything come back?
- [ ] Close the editor, start it again with the same `--items` folder: are your edits back?

## 6. Fixed today: confirm by eye
- [ ] Sea edge: fly low over a coastal plain (where frames 5161, 6193 and 13883 showed the straight edge). Does the sea reach the whole plain?
- [ ] Atmosphere: descend from orbit to the ground slowly. Do you see rings, bands or a colour jump (especially between 10 and 6 km)?
- [ ] Tiles: while moving, do tiles pop to a coarser level?

## 7. Overall look (G4)
- [ ] Is the panel layout readable and the map-first flow natural?
- [ ] One thing that most needs to change before you build a real world with it?

## Report back
A line per block: ok, or what was wrong (with the F9 frame number). Screenshots worth keeping go to `docs/dev_screenshots/`.
