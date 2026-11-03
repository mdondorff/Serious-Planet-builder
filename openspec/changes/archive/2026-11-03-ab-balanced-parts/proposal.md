## Why
The two A+B parts of BUILD-036 are not balanced. In #1134's last run (92bb2558) Windows part 1 took 16m20s, over BUILD-014's 16 min, and part 2 took 14m08s. Their test steps: 535 s against 209 s (summed test time 1,563 s against 829 s). The count partition splits the test count per binary, not the time. The relief-band gate (`oracle_world_preview::seeds_1_to_5_pass_the_relief_bands_of_spike_93_in_the_gate_form`, 137 s, `threads-required = "num-cpus"`) holds all four runner slots for 137 s, so whichever part draws it is the slowest (opus-planner, 2026-11-03, inbox #79).

## Measurement
A list-scheduling model replays the measured CI durations on 4 slots, using each partition's real membership from `cargo nextest list --partition`. For the current split it gives exactly 535 s and 209 s. Test-step wall time per part (Windows):

| Partition | Part 1 | Part 2 | Part 3 |
|---|---|---|---|
| count /2 (today) | 535 | 209 | |
| slice /2 | 427 | 317 | |
| hash /2 | 394 | 343 | |
| count /3 | 228 | 189 | 342 |
| slice /3 | 180 | 332 | 247 |
| hash /3 | 187 | 189 | 380 |

- Per job the fixed cost is about 410 s: setup about 40 s, build about 360 s, uploads about 10 s. The checks part adds about 216 s: manifest 153 s, check 49 s, licences 11 s, run modes 3 s.
- No two-part split fits. At perfect balance each part would be about 886 s, a 74 s margin, and no built-in scheme comes near that: hash /2 with the checks gives 969 s or 1,017 s.
- Starting the gate first changes the walls by 15 s or less.

## What Changes
- The `test` job runs as three parts per OS, `cargo xtask test -- --partition hash:N/3`.
- Hash places a test by its name alone, so a new test does not move the others. Count and slice reshuffle the parts whenever a test is added, which is how today's split drifted.
- With hash /3 the gate falls in part 3. Part 2 keeps the checks.
- Expected Windows jobs: part 1 about 10 min (410 + 187 s), part 2 about 13.5 min (410 + 216 + 189 s), part 3 about 13 min (410 + 380 s). Linux runs faster.
- BUILD-036 is modified; the repository check test reads the new matrix.

## Impact
- `.github/workflows/ci.yml`, `xtask/src/repocheck.rs`; BUILD-036 modified.
- Two more jobs per CI round (one per OS), each with its own build of about 6 min runner time.
- The job names change from `<OS> A+B N/2` to `<OS> A+B N/3`; main has no required check names (pr-gate is the ruleset's check).
