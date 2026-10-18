---
name: golden-review
description: Review candidate golden images against the owner's policy (ADR 0017, plus ADR 0037 and ADR 0052 for the first goldens of M5 classes), auto-bless rounding noise, declared cross-adapter-consistent debug-view changes and conforming first M5 goldens, and escalate everything else to the owner's /bless. Use whenever a change produces golden candidates (a golden test mismatch or PENDING) before declaring the change done.
---
Policy and thresholds: `docs/adr/0017-golden-review-policy.md`; the M5 window: `docs/adr/0037-m5-new-goldens-and-look-review.md`. Never bless outside them; when unsure, escalate.

1. **Declare.** If the change moves pixels on purpose, its OpenSpec `proposal.md` must have a section
   `Expected golden changes:` with one line per golden: `- <golden name>: <reason>`. Add it before collecting.
   For the first golden of an M5 class (`lit`, later `sky` and `water`, in `tests/goldens/classes.txt`; ADR 0037, a window the
   owner revisits at the M5 gate) the proposal also needs a section `ADR 0037 conditions:` with one line per golden:
   `- <golden name>: debug-view=<golden name of the feature's debug view>; bound-tests=<test>, <test>`. The tests are the ones
   that prove the attribute's declared bound (ADR 0035; the change's spec deltas name class and bound), as `test_name`, `binary::test_name` or `crate::binary::test_name`. The debug view may be a golden of this run (a new view) or a blessed one.
   **Look changes after the M5 review** (ADR 0037 point 2, TEST-036): a change that only edits look parameters (files of `tests/goldens/look_data.txt`, today `*.look.toml`) declares them in `proposal.md` under `Look changes:`, one line each:
   `- <look file>: <key> = <old> -> <new>; affects=<lit|sky|water>[, ...]`, and lists the pictures it moves under `Expected golden changes:`. The tool then blesses the changed lit, sky and water candidates as `AUTO-L` only when the diff touches nothing but look data and inert paths (docs, specs, goldens), every changed line of a look file is one of the declared changes, no debug or shaded golden moved beyond rounding, and the golden is in an affected class and changed on every adapter that has it. A look change plus any code, shader or other data change escalates.
2. **Collect candidates for every adapter that has the golden.** Start from an empty `target/review/candidates/`
   (delete it first) so nothing from an earlier run is reviewed.
   - WARP: `cargo xtask test gpu` (writes `target/review/candidates/dx12-microsoft-basic-render-driver/`). Or download
     CI's `review-Windows` artifact when the machine's WARP version is not the recorded one (the driver gate escalates it).
   - NVIDIA (Rasierklinge): `cargo xtask test gpu --real`.
   - llvmpipe: push the branch, then `gh run download <run> -n review-Linux -D target/review/ci-linux`.
   Golden tests report every mismatch of a run (`GoldenChecks`), so one run per adapter is enough.
   For a first M5 golden also keep this run's test reports: after each `cargo xtask test`, `test gpu` or `test gpu --real`
   copy `target/nextest/default/junit.xml` (`target/nextest/real-gpu/junit.xml` for a `--real` run) to a file of its own (nextest overwrites it), and pass each with `--tests`.
3. **Review.** `cargo xtask golden-review --proposal openspec/changes/<id>/proposal.md [--extra target/review/ci-linux/candidates]... [--tests <junit.xml>]...`.
   It writes `target/review/golden-review/report.md` and a golden | candidate | diff triptych per candidate, and prints
   which adapters took part (an adapter that has goldens but delivered no candidates is printed as NOT RUN and is never
   counted as agreeing), the ADR 0037 evidence per first M5 golden, and one verdict per candidate: `AUTO-R`, `AUTO-E`,
   `AUTO-M5`, `AUTO-L` or `ESCALATE <reason>`. Look at every triptych yourself; if a picture contradicts its verdict (a hole, a seam,
   a colour that should not exist), treat it as `ESCALATE`.
   An `AUTO-L` needs the same oracle check as an `AUTO-E` (commit it as `bless(auto): <tests> [L]`).
   An `AUTO-E` also needs, per ADR 0017: the test's own oracles pass on the candidate frame (no holes, coverage, colour
   rules: every assertion of the golden test other than the golden comparison), and the change's verifier and reviewers
   have no open finding about pixels. An `AUTO-M5` needs the same. If either fails, or any reviewer objects, treat the
   candidate as `ESCALATE`.
   For the debug view of a first M5 golden, candidates that are new in this run agree pairwise when at most 64 pixels
   differ by more than delta 1 (any amount) and any number differ at exactly delta 1 (ADR 0052 as written, which amends
   ADR 0037 point 1(b); `ADR_0052_EDGE_PIXELS`; the planner's 32-px reading of 2026-10-17 was withdrawn because
   RTX-against-software rounding spreads delta 1 across whole views). The same-adapter check of a candidate against its
   golden is unchanged (ADR 0017, 32 px at delta 1). A debug view that already has a blessed golden is still checked by
   ADR 0017. The report prints the worst pair and, on a failure, the cap exceeded and the counts.
   The driver gate is never relaxed, for first goldens too: a candidate rendered by another driver than the one in
   `driver.txt` escalates.
4. **Bless the auto verdicts.** `cargo xtask golden-review ... --apply` copies only `AUTO-R`/`AUTO-E`/`AUTO-M5`/`AUTO-L` candidates
   (never `driver.txt`) into `tests/goldens/<adapter>/`. Commit them on the change's branch as
   `bless(auto): <tests> [R|E|M5]`, with the per-candidate numbers from the report in the message body, and add one line to
   `docs/status.md`. Apply a first M5 golden only when every adapter with goldens took part (the report says NOT RUN otherwise), or accept that the missing adapter later escalates as a new family. Remove a golden's line from `tests/goldens/pending.txt` once every adapter has it.
5. **Escalate the rest.** Leave escalated candidates in place, list them with their reasons and the report path under
   "Waiting for the owner" in `docs/status.md`, and stop work that depends on them. The owner decides with `/bless`.

Never delete or overwrite a golden by hand, never relax a tolerance to make a candidate pass, never bless a lit view
outside ADR 0037's conditions (a first golden of an M5 class; a changed existing lit golden still escalates), a first golden
of another class, or a new adapter family.

Scope: $ARGUMENTS
