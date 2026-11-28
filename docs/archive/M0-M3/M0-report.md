# M0 Foundations: report (2026-10-10)

## Summary
Workspace, `cargo xtask`, golden harness, performance-harness skeleton, headless hello triangle on WARP, CI workflow, 12 proposed ADRs and 67 seeded requirements (15 active) are in place. Change `m0-foundations` is archived.

## Criteria
| Exit criterion (report §15) | Result | Evidence |
|---|---|---|
| Windows-native build and test green locally | pass | `cargo xtask accept M0` (target/review/accept-M0.md) |
| `test fast` under 10 s | pass (warm cache, ~1 s; ~6 s with build) | accept run |
| `spec-lint` passes | pass | 67 requirements, 10 specs |
| Headless hello-triangle PNG on Windows | pass (candidate, not blessed) | target/review/candidates/dx12-microsoft-basic-render-driver/hello_triangle.png |
| Hello triangle on Linux (lavapipe) | needs CI | no pushed branch, `gh` missing |
| CI green on Windows and Linux | needs CI | workflow written, never run |
| Perf harness protocol | skeleton only; refusal logic tested | needs the owner's go for a real run |

## Verifier and guardian
Verifier: 15/15 active requirements pass. Spec-guardian: 2 high findings (strict-golden env override, testkit as a normal dependency of app) fixed; mediums fixed or recorded in ADR 0008/0009 addenda; remaining lows are listed in the guardian report (e.g. CI calling `cargo deny`/`cargo run` directly instead of xtask, WDEF-005 wording, COORD-007 canonical border rule: consider a spike in M1).

## Proposed ADRs awaiting the owner
0001 to 0012 (see docs/status.md).

## Recommendation
Proceed to M1 (`m1-coordinates`, then walking skeleton). Owner actions: install `gh`, push, bless the candidate, accept ADRs.
