## Why
`windows (WARP) C` has a CI median of 16.9 min (p90 18.2) against the 16 min budget (BUILD-014, `cargo xtask ci-stats`, 2026-10-19). Its test step is about 1,000 s; the build and cache steps take under 1 minute. Improvement 2 of the M5 process review.

## What Changes
- The Windows leg of the `gpu` CI job runs in two halves: `cargo xtask test gpu -- --partition count:N/2` for N = 1, 2 (nextest places every test in exactly one half); the Linux leg stays whole (8 min).
- Artifacts of a half carry its number: `test-logs-Windows-gpu-N`, `review-Windows-N`; the `golden-review` skill downloads both halves.
- BUILD-018 and a tier-A test of the workflow file.

## Classes and bounds (ADR 0035)
No field and no shader value is added; CI configuration.

## Out of scope
Sharding tier A+B (13.2 min median, under budget), changes to any test, changes to the 300 s test limit.
