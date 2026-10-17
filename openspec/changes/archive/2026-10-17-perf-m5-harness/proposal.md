## Why
Window 6 (first M5 window): three streaming scenes were refused by the readiness gate on CPU load left by the previous heavy child; coast-2km, coast-3m and the flights were GPU-invalid because the clock read the floor once in the warm-up and fell below it in the first run; a single-scene `--out DIR` would have written a file named DIR after a 137 s case.

## What Changes
- `perf m5`: after each child, poll the system CPU load (one gate window per poll, at most 30 polls) until it is below the gate's limit; log the wait.
- Lit views and flights: the unmeasured warm-load phase ends after 3 consecutive readings at the floor, with chunks of at least 1 s; capped as before.
- Single-scene `--out`: a directory gets `<scene>[-lit].json`; a `.json` path is the file; anything else is refused by the option check.

## Impact
Spec: TEST-028 to TEST-030 added (the lit warm-up stop condition of TEST-029 overrides the one-reading rule for the lit views). No golden, no version change.
