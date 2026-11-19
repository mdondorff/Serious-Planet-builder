## Why
- The owner accepted GEN-140 for main (owner, 2026-11-19): the bands' and creases' per-octave turned lattices become the default.
- Before, the turn was a process switch that only the prototype and the river gate set; main's bake and runtime ran the axis lattice (SPIKE-140: the axis lumps of the base are GEN-140's unturned lattice).

## What changes
- GEN-140: the switch is on by default.
  - Off (`set_turned_noise(false)`), every height is the axis lattice's, the bits from before this change.
  - On, the identity words of the bake inputs and the erosion field's climate word change, as before. `CLASS_SOURCE_VERSION` goes from 8 to 9, so stored class fields re-key.
- The pins that move with the heights are re-pinned to the measured values, each with its reason (CON-20). No floor is lowered (CON-31):
  - the known answers: erosion, the C1 read and field, the class source fingerprint, the class field bits;
  - the view node counts: north-pole-1km +4;
  - the residency slots: four values, at most +0.4 %;
  - the water cut's kept share on coast-3m and coast-glide, lower end 55 to 45. That share is how many water nodes the cut keeps; the sea pixels are unchanged.
- Two tests were updated to follow the switch, because their reference used the axis lattice or ignored the turn:
  - the crease reference sum of `weights_of_one_are_the_plain_sum…`;
  - the eroded field's climate word.
- `gpu_water_coverage`'s coast search is re-aimed from a plain under -8 m to one under -15 m. The turned world's first -8 m plain lies between -12 and 0 m, where no pixel is checked. The views, their bounds and the 2,000-pixel minimum hold.

## Expected golden changes:
- none: tier C on WARP gives no golden candidate with the turn on (measured 2026-11-19 on main 60ae116a).

## Not in this change
- The river gate's `--noise` default (`axis`, BUILD-060). A follow-up turns it when the gate's rows move to the turned base.
- The store rebuild. It follows the merge, because every bake input key moves.

## Impact
- Crates: generators (height.rs, classes.rs) and the tests above.
- Stores made before this change refuse to open by key; that is old stores need not stay compatible (owner, 2026-11-05).
