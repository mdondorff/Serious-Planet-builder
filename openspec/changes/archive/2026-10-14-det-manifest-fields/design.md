## Decisions

- Mutation hook: `GlobalFieldValues::entries(perturb)` changes one ulp of the value behind a line before hashing (block and grid: the first sample's elevation as `f32`; fields: the `f64` height). The check asserts that exactly that line changes. Computing the values once keeps the test at about 0.5 s.
- The grid-hash perturbation touches a copy: the fields read the unperturbed bake, so one perturbation changes one line.
- Field points are face coordinates, not directions picked by hand, so the face edges and cube corners are exactly the lattice's.
- Raw `f64` bits are written instead of a hash so that a det-diff report shows the numeric difference of a drift.
- The persistent known answers are written from `app` (CON-02) with the literals of `cache`'s tests repeated in a tier-A test next to the writer; a change of either place fails one test.
- Cost: the new section is a 64-cell bake, 96 blocks of 289 samples and 144 field evaluations, well under a second in a release build.
