## Why
The `/bless` apply step was a manual copy of candidates, and it read only `target/review/candidates/`: the WARP and llvmpipe candidates of the CI artifacts could not be applied by the tool (#303, opus-planner 2026-10-18).

## What Changes
- `xtask::blesscmd`: `cargo xtask bless` lists the candidates of every adapter, including `--extra` directories; `bless --apply <names> --approved-by <who>` promotes exactly the named goldens on every adapter, honours the driver gate, writes nothing when any candidate is refused, and clears `pending.txt`.
- The `bless` skill uses it instead of a copy. BUILD-013 added; tier-A tests on the sources, the promotion, the refusals and the arguments.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling. No golden moves.

## Out of scope
`golden-review --apply` (auto verdicts only, unchanged), the policy (ADR 0017), the owner-approval record beyond `--approved-by` and the commit body.
