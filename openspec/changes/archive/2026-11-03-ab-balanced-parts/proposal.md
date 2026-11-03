## Why
- The two A+B parts of BUILD-036 are not balanced. In #1134's last run (92bb2558), Windows part 1 took 16m20s, over BUILD-014's 16 min, and part 2 took 14m08s. The test steps took 535 s and 209 s, from summed test times of 1,563 s and 829 s.
- The count partition splits the test count per binary, not the time.
- The relief-band gate (`oracle_world_preview::seeds_1_to_5_pass_the_relief_bands_of_spike_93_in_the_gate_form`, 137 s, `threads-required = "num-cpus"`) holds all four runner slots for 137 s. The part that draws it is the slowest.

## Measurement
A list-scheduling replay of the measured CI durations on 4 slots, with each partition's real membership from `cargo nextest list --partition`, reproduces today's split exactly (535 s and 209 s). The table gives the test-step wall time per part on Windows, in seconds:

| Partition | Part 1 | Part 2 | Part 3 |
|---|---|---|---|
| count /2 (today) | 535 | 209 | |
| slice /2 | 427 | 317 | |
| hash /2 | 394 | 343 | |
| count /3 | 228 | 189 | 342 |
| slice /3 | 180 | 332 | 247 |
| hash /3 | 187 | 189 | 380 |

Fixed cost per job on Windows, from #1134's step times:

| Item | s |
|---|---|
| setup, build, uploads | about 410 |
| checks part extra (manifest 153, check 49, licences 11, run modes 3) | about 216 |

- No two-part split fits. At perfect balance each part would be about 886 s, a 74 s margin. hash /2 with the checks gives 969 s or 1,017 s.
- Starting the gate first changes the walls by 15 s or less.

## What Changes
- The `test` job runs as three parts per OS: `cargo xtask test -- --partition hash:N/3`.
- hash places a test by its name alone, so an added test moves no other test. count and slice reshuffle the parts on every added test.
- With hash /3 the gate falls in part 3; part 2 keeps the checks.
- BUILD-036 is modified. The repository check test reads the new matrix.
- Tier C (BUILD-018, `count:N/4`) is not changed here.

Expected Windows jobs:

| Part | Estimate |
|---|---|
| 1 | about 10 min (410 + 187 s) |
| 2 | about 13.5 min (410 + 216 + 189 s) |
| 3 | about 13 min (410 + 380 s) |

## Impact
- `.github/workflows/ci.yml` and `xtask/src/repocheck.rs`; BUILD-036 modified.
- One more job per OS per CI round, each with its own build (about 6 min of runner time).
- Job names `<OS> A+B N/2` become `<OS> A+B N/3`. pr-gate is the ruleset's only required check.
