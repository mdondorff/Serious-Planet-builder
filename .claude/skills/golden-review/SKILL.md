---
name: golden-review
description: Review candidate golden images against the owner's policy (ADR 0017), auto-bless rounding noise and declared, cross-adapter-consistent debug-view changes, and escalate everything else to the owner's /bless. Use whenever a change produces golden candidates (a golden test mismatch or PENDING) before declaring the change done.
---
Policy and thresholds: `docs/adr/0017-golden-review-policy.md`. Never bless outside it; when unsure, escalate.

1. **Declare.** If the change moves pixels on purpose, its OpenSpec `proposal.md` must have a section
   `Expected golden changes:` with one line per golden: `- <golden name>: <reason>`. Add it before collecting.
2. **Collect candidates for every adapter that has the golden.** Start from an empty `target/review/candidates/`
   (delete it first) so nothing from an earlier run is reviewed.
   - WARP: `cargo xtask test gpu` (writes `target/review/candidates/dx12-microsoft-basic-render-driver/`).
   - NVIDIA (Rasierklinge): `cargo xtask test gpu --real`.
   - llvmpipe: push the branch, then `gh run download <run> -n review-Linux -D target/review/ci-linux`.
   Golden tests report every mismatch of a run (`GoldenChecks`), so one run per adapter is enough.
3. **Review.** `cargo xtask golden-review --proposal openspec/changes/<id>/proposal.md [--extra target/review/ci-linux/candidates]`.
   It writes `target/review/golden-review/report.md` and a golden | candidate | diff triptych per candidate, and prints
   one verdict per candidate: `AUTO-R`, `AUTO-E` or `ESCALATE <reason>`. Look at every triptych yourself; if a picture
   contradicts its verdict (a hole, a seam, a colour that should not exist), treat it as `ESCALATE`.
   An `AUTO-E` also needs, per ADR 0017: the test's own oracles pass on the candidate frame (no holes, coverage, colour
   rules: every assertion of the golden test other than the golden comparison), and the change's verifier and reviewers
   have no open finding about pixels. If either fails, or any reviewer objects, treat the candidate as `ESCALATE`.
4. **Bless the auto verdicts.** `cargo xtask golden-review ... --apply` copies only `AUTO-R`/`AUTO-E` candidates (and
   the adapter's `driver.txt`) into `tests/goldens/<adapter>/`. Commit them on the change's branch as
   `bless(auto): <tests> [R|E]`, with the per-candidate numbers from the report in the message body, and add one line to
   `docs/status.md`.
5. **Escalate the rest.** Leave escalated candidates in place, list them with their reasons and the report path under
   "Waiting for the owner" in `docs/status.md`, and stop work that depends on them. The owner decides with `/bless`.

Never delete or overwrite a golden by hand, never relax a tolerance to make a candidate pass, never bless a lit view,
a new test's first golden or a new adapter family.

Scope: $ARGUMENTS
