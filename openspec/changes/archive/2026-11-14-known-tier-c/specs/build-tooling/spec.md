## ADDED Requirements
### Requirement: BUILD-056 Prepush passes local tier C failures of known classes only, with a summary
When `cargo xtask prepush` runs `test gpu` and it fails, prepush SHALL read that run's log and pass the step only when the run ended, at least one test failed, every failed test matches a line of `xtask/known-tier-c.txt` (a test pattern, an outcome: `abort 0x<code>`, `timeout` or `panic at <file>:<line>`, and the bold title of the `docs/status.md` Tracked entry it cites, which must be present) and no candidate golden was written. It SHALL then print "tier C: N known-class failure(s), listed" and one line per failure, for the PR body. Any other failure, any candidate, or a run without a summary SHALL fail the step. The list is local only: CI runs tier C in full on every push.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #149), codifying the architect's rule of 2026-11-09 (known local tier-C classes: WARP aborts 0xc0000005, gpu_skeleton timeouts, gpu_preflight's 8 s bound); BUILD-016, BUILD-026
Detail: It changes no test and no tolerance. A list line whose Tracked entry is gone is ignored with a warning. A failure of a listed test with another outcome (another exit code, another test of the same binary timing out, a panic at another line) is not covered.

#### Scenario: Known classes only
- **WHEN** tier C fails with a WARP abort 0xc0000005, the gpu_skeleton pipelined-loop timeout and gpu_preflight's panic at line 22, and no candidate is written
- **THEN** prepush passes the step and prints "tier C: 3 known-class failure(s), listed" with one line each

#### Scenario: Anything else
- **WHEN** another test fails beside them, or a candidate golden is written, or the run has no summary
- **THEN** prepush fails the step and names the uncovered failure
