## Why
Every CI round trip costs 10 to 20 minutes, and most failures it finds (fmt, clippy, a golden in a crate the author did not run tier C for) could have been found locally in a few minutes. opus-planner and the owner (2026-10-19) asked for one command that runs what CI would run, and for the one runner fault that fails jobs without a code failure to be retried.

## What Changes
- `cargo xtask prepush` (BUILD-016): `check`, `test fast`, and `test gpu` when the diff touches a golden-producing path; the time of each step and one verdict line.
- CI (BUILD-017): the tool install is tried a second time after a failure (actions/partner-runner-images#169).
- The CLAUDE.md Git rule "run `cargo xtask prepush` before every push to a PR branch, while waiting is cheap; lavapipe-only golden differences stay CI's job" is committed by opus-planner (the owner's decision); its text is in the PR description.

## Not in this change
- `oracle_stream_smoke`'s configuration-cap test (status item 9): measured and reported separately; no limit is changed here.
