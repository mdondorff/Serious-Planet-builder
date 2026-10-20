## Why
The M6 gate evidence list (docs/briefings/m6-gate-evidence.md, G8 and G9) names tests that were missing: a brush over a stamp end to end, the editor's refusal of typed numbers, the part budget of a split line, and hover and redo after a split line is deleted whole (opus-planner, 2026-10-20).

## What Changes
- Tier-A tests of the editor (`crates/editor/tests/line_edge_cases.rs`, VIEW-033, VIEW-016): `Editor::set_line_params` refuses NaN, infinite, negative, zero and absurd values and changes nothing; `split_line` refuses a width at the part budget (`WidthTooLarge`) and splits one just inside it; hover and redo after a split line is deleted whole.
- A generators test (`brush_stage.rs`, GEN-030): a brush stage over a stamp stage adds its delta to the stamped height, and a flatten lands on its target on the line whatever the hill below.
- Fix found by the hover test: deleting a split line cleared the hover only when the hovered item was the selected part; the hover of another part of the line was left pointing at a deleted item (`Session::delete_selected`).
- AUTH-003 (pinning and freeze/bake) is not implemented: no pin, freeze or bake exists in any crate (only the `frozen` layer name and the item-side version record with `Item::check_versions`). No citing test is possible; reported to opus-planner, not built.

## Classes and bounds (ADR 0035)
No field, no shader value and no golden is touched.

## Out of scope
AUTH-003 itself; the M3-condition tests (parked by opus-planner).
