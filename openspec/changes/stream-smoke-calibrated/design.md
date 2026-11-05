## Decisions
- Reference: the run's own `record_ms` median. Measured shares of the scheduler median to it: 0.04 (development machine: 0.10 ms of 2.5 ms), 0.0115 (the failed lavapipe run: 1.516 ms of 132.7 ms), 0.0081 (1.132 of 140.3). The share limit 0.1 is 2.5 times the highest healthy share and 9 times the highest observed on the loaded runner.
- Why `plan_ms` is not the reference: it was not inflated on the failed run (0.34 ms), so the ratio to it would be 4.4 against 0.32 locally and the test would still fail.
- Why `load_run_us_median` is not the reference: the workers ran at their usual speed; only the main thread was stalled.
- What it still catches: on a quiet machine (frame a few ms) the limit stays 0.5 ms, so a scheduler ten times the measured 0.044 to 0.10 ms fails as before (checked by a test at 1.0 ms of 2.5 ms); on a loaded runner a scheduler step above a tenth of the frame fails (14 ms of 132.7 ms in the test). The cap that M4 found (a worker or `ahead` limit) is judged by the concurrency, pool and job counters, which are unchanged.
- The limit is inclusive; a log without `record_ms` has the floor alone.
