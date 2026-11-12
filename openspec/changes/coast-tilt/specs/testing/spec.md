## ADDED Requirements
### Requirement: TEST-128 The coast tilt tests
The coast tilt SHALL be tested at tier A: (a) the added height is 0 at the coast, monotone in the distance, at most `t x D` and exactly that from `D`, with the slope `2 t` at the coast, scaled by the weight; (b) with the tilt off `eval` and `eval_with(OFF)` give the same bits for `e0` and the class weights; (c) on a spread of directions the tilt lifts land within its bound, leaves the sea and the points without plain weight untouched, and leaves every class weight the same bits; (d) the identity word is unchanged while off and moves with `t` and `D`; (e) a negative `t` and a zero `D` are refused; (f) a plain far inland gains nearly `t x D` for `D` beyond the shelf's cap, and the points at the field's coast gain almost nothing.

Verify: A
Status: planned
Source: GEN-180, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `coast_tilt` run
- **THEN** each of (a) to (f) has an assertion that cites GEN-180
