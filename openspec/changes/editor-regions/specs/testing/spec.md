## ADDED Requirements

### Requirement: TEST-166 The editor's region boxes are checked headless
Tier-A tests SHALL drive `planet::regionedit` and `EditRuntime` without a window: snapping and clamping against `check_box` at every face and at the band's edges in the production layout, place, move, resize and delete with the file reread after each, the refusals, the outlines and the margin projected on the ground, and the Sites tab's Regions events in the headless panel rig.
A tier-A test SHALL bake a placed box with `bake_entry` on a small layout of the production shape and check its row `unbaked`, then `ready`, then `stale` after an item is put into the definition, and the printed open command.
A tier-A test SHALL check that the inputs id the editor derives for `--seed N` (with an `--items` folder) equals the one `bake-region` derives for the same options.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E2 gate), VIEW-077, TEST-165

#### Scenario: End to end headless
- **WHEN** the tests run on the software adapter's tier A
- **THEN** every box the editor put passes `check_box`, the states follow the bake and the edit, and the editor's and the bake's inputs ids agree
