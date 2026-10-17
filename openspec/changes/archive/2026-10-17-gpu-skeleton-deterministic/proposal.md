## Why
`gpu_skeleton::the_upload_flag_selects_the_write_path_and_the_batched_loop_draws_the_same_frames_in_fewer_calls` failed once on a Windows CI run of #215 (batched 106 tiles against 175 per slot) and passed on rerun. The pipelined loop never waits, so how many tiles arrive per frame (and so how many are adjacent in one batch) depends on thread timing, and the two runs of the test saw different tile sets. CLAUDE.md: a test that cannot be made deterministic is not acceptable.

## What Changes
- `test-render --pipelined --lockstep`: after every iteration the script loop waits until the plan worker is idle and no generation job is unfinished, so the frames drawn and the tiles each uploads are a function of the script alone. The free-running loop is unchanged. `Streamer::unfinished_jobs` (dispatched minus jobs whose result has reached the channel) is the one new streaming accessor.
- The test runs both modes with `--lockstep`. Its assertions are unchanged (same settled image, tiles uploaded, calls per tile at least one per tile per slot, batched calls per tile under 0.95 of the per-slot figure) plus one: both modes upload the same number of tiles. Measured over 4 repeats per mode and script: per-slot 430 and 594 calls, batched 26 and 42 calls, for 222 and 310 tiles, identical every time.
- `docs/status.md`: the tracked flake line is removed.

## Classes and bounds (ADR 0035)
No field and no shader value is added; this is test tooling and a harness flag.

## Out of scope
Any change to the free-running loop, the upload budget or the batching itself.
