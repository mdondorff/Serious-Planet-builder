## MODIFIED Requirements
### Requirement: GEN-140 The band and crease noise can turn its lattice per octave
The noise of the hill band, the mountain band and the creases SHALL be switchable to a lattice turned by a fixed rotation per octave (axis and angle from `libm`, different for every octave), so no lattice plane of one octave is a plane of another or of the world axes.
- The switch SHALL be on by default (owner, 2026-11-19); off, every height SHALL be the axis lattice's. On, it SHALL change the bake inputs' identity words and the erosion field's climate word.
- The bounds of the turned noise SHALL be the bounds of the same cap in the turned frame and SHALL hold.

Verify: A
Status: active
Source: SPIKE-104 D-C, planner inbox #91 (owner, SPIKE-105 decision 1 = A conditional); the default on (owner, 2026-11-19, SPIKE-140)

#### Scenario: Planes
- **WHEN** the octaves of the untouched noise and of the turned noise are summed and their slope is measured across the world planes `x = k / f`
- **THEN** the untouched sum is flat across them and the turned sum is not

#### Scenario: Default and switch off
- **WHEN** a process starts
- **THEN** the switch is on and an identity word it folds is changed
- **WHEN** the switch is turned off
- **THEN** the identity words are unchanged and the turn is a rotation (lengths kept) that differs for every octave
