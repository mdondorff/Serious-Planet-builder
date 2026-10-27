## Why
The envelope trial settles standing still and flies 12 s with a chunk edge half way (TEST-054). A chunk becomes wanted at 6 s - D / v after the start and lands a job latency J (5 to 7 s) later, so above v* = D / 6 the stand-in share exceeds the bound at any speed, whatever the prefetch or the worker count (m8-chunk-lead). The 115 m/s at 300 m measures a cold start, not a steady flight.

## What Changes
- A second curve, the steady flight: the same altitudes with a run-in of `RUN_IN_S` (12 s) at the speed under test, chunk stage and prefetch running, before the measured 12 s window; the chunk edge is half way into the window. The first curve (the first visit after a stop) stays as it is.
- The log gives `run_in_s`, `steady_curve` and, per trial, `first_wanted_s` (the earliest time a chunk that landed in the flight was first wanted): in the cold trial the model says 6 s - D / v, in the steady one it lies before the window.
- Which curve the 200 m/s floor of criterion 7 is judged on is the owner's call.

## Impact
- Spec: TEST-057 (new). Code: `app/envelopeperf.rs`, `app/chunkstage.rs`. No golden. The measurement is the architect's window.
