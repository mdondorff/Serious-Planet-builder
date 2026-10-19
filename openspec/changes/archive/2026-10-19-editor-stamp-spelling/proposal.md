## Why
The stamp stage (WDEF-010, GEN-026) fixed how an item file spells a stamp: the radius is the file's `reach_m`, a hill has `height_m` (signed), a crater `depth_m`, a plateau `target_m` (an absolute height) and its own `max_delta_m`. The editor's provisional table (`radius_m` as a parameter, a reach of 1.25 radii, `rim_m`, `softness_pct`) made the outline and the terrain disagree by a quarter and offered parameters the stage ignores.

## What Changes
- `planet_editor::params`: each parameter of the table says where it is stored (`Stored`: a parameter, `reach_m`, `max_delta_m`); the table has the stamp stage's keys only; `bound_for` and `fit_stamp_bound` keep a hill's and a crater's bound equal to their amount.
- `planet_editor::shape`: a stamp's outline is its reach; resizing sets the reach (in and out); the height handle edits the kind's amount (a hill up, a crater down from the bottom of the bowl, a plateau's absolute target); handle places likewise.
- `planet_editor::session`: placing a stamp, typing a value and the inspector use the stored places; another kind keeps the generic behaviour (VIEW-017).
- VIEW-015 scenario numbers (a hill of reach 400 m, a plateau of 15 km) and VIEW-017 text.

## No golden moves
Editor-crate logic, tests and spec text only.
