## MODIFIED Requirements

### Requirement: TEST-166 The editor's region boxes are checked headless
Tier-A tests SHALL drive `planet::regionedit` and `EditRuntime` headless: snapping and clamping against `check_box` at every face and band edge, place, move, resize and delete with the file reread, the refusals, the lines on the ground, and the Regions events in the panel rig.
A tier-A test SHALL bake a placed box with `bake_entry` on a small layout of the production shape and see its row go `unbaked`, `ready`, then `stale` after an item edit, with its open command.
A tier-A test SHALL check that the editor's inputs id for `--seed N` with `--items` equals `bake-region`'s.
A tier-A test SHALL bake two adjacent boxes one after the other, open both together (VIEW-079), and see both drop out after an item edit.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E2 gate; D2), VIEW-077, VIEW-079, TEST-165

#### Scenario: End to end headless
- **WHEN** the tests run on the software adapter's tier A
- **THEN** every box the editor put passes `check_box`, the states follow the bake and the edit, and the editor's and the bake's inputs ids agree
