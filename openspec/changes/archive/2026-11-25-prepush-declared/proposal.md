## Why
- A change that moves goldens on purpose (a look change: lit views for the owner's `/bless`) cannot pass prepush: BUILD-056 fails tier C on any written candidate. PR #1662 (REND-068) was pushed under the lead's stated exception (2026-11-25), and the lead ordered the mode that closes the gap (#204).

## What changes
- BUILD-068: `cargo xtask prepush --declared <proposal.md>`. Tier C passes when the written candidates are exactly the proposal's `Expected golden changes:` list, and every failure is a golden test's golden-check panic or a known class by name (BUILD-056). An extra or missing candidate fails, with both lists.
- Without the flag nothing changes. The mode writes and blesses no golden: the candidates still go to golden-review (ADR 0017).
- TEST-154: tier-A tests of the declared list, the panic location and the verdict.

## Expected golden changes:
- none: xtask only

## Impact
- Spec deltas: BUILD-068 (build-tooling), TEST-154 (testing).
- Code: `xtask/src/knowntierc.rs` (`declared_goldens`, `golden_panic_at`, `judge_declared`), `xtask/src/prepush.rs` and `xtask/src/main.rs` (the flag).
