## ADDED Requirements
### Requirement: TEST-132 The plateau rim tests
The plateau rim rule SHALL be tested at tier A: (a) the uplift share is 0.5 on the contour, monotone in the distance inside, 0 from `W` outside and 1 from `W` inside, and 0 when the rule is off; (b) with the rule off `eval` and `eval_rules(OFF)` give the same bits for `e0` and the class weights; (c) the identity word is unchanged while off and moves with `W`; (d) a negative `W` is refused; (e) with the rule on, on a spread of directions the sea and every class weight are untouched, deep in the plateau field the share is at least half, outside it the share is in [0, 1], and at the shore the rule adds little.

Verify: A
Status: planned
Source: GEN-183, architect 2026-11-12
