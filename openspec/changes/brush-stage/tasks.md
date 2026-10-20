## Tasks
(The brush item, its caps and its hash are the change `brush-items`.)
1. [ ] generators: `stroke_delta` (raise, lower, flatten, smooth) and `apply_stamps_with(items, radius, dir, base, base_at)`; `apply_stamps` stays and reads nothing else; tests first (profile values, order, bounds of each kind, smooth on a slope, tile seam equality).
2. [ ] generators: `delta_range` for the brush; `height_bounds` test (the range holds the stage's output on a dense sample of the cap, for each kind).
3. [ ] generators: the stage passes `self.base` as `base_at` in `add_stamps`; a contract test with a recording reader (the brush reads its layer's footprint only).
4. [ ] numerics-reviewer, spec-guardian, verifier; archive.
