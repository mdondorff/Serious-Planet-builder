## Why
TEST-069 (merged in #889) compared black frames in two of its three cases: the look's own sun is below the horizon at the coast view (night: the lit target is all zeros and the picture black; only the sun-ahead case drew a daytime picture). The PASS reported for the plain and the overlay case was vacuous. The exposure also stood at its start value there (an empty histogram keeps the current EV), which the test took for an adapted exposure.

## What Changes
1. Every case of `lit_still_frames` sets a sun above the horizon (from the side at 35 degrees; ahead of the camera at 17 degrees for the glint).
2. The test asserts content per case: more than 16 colours in the picture and light in more than half of the lit target (a night, or nothing drawn, now fails), and writes the first frame of each case to `target/review/still-flicker/`.
3. The TEST-068 test keeps the first frame of each debug view there too and prints its colour and tile counts.
4. TEST-069's Detail says the look's sun is below the horizon at this coast and what each case proves.

## Impact
- Result on main after the repair: PASS, with real pictures (4713, 4820 and 8560 colours in the first frames; the exposure left its start value: 15.85, 2.75 and 16.00 EV). No renderer change.
- Lesson: a still-frame detector must assert content (colours, a non-empty lit target, one frame looked at) before equality means anything.
- Code: `app/litperf.rs`, `app/tests/gpu_lit_still_flicker.rs`, `app/tests/gpu_still_flicker.rs`. Spec: TEST-069 Detail.
