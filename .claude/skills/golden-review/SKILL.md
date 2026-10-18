---
name: golden-review
description: Review candidate golden images against the owner's policy (ADR 0017 as amended by ADR 0061), auto-bless rounding noise, declared changes of debug and shaded views (judged per adapter) and pure look changes, and escalate everything else (first goldens of any class, lit, sky and water views, driver changes) to the owner's /bless. Use whenever a change produces golden candidates (a golden test mismatch or PENDING) before declaring the change done.
---
Policy and thresholds: `docs/adr/0017-golden-review-policy.md`; goldens after M5: `docs/adr/0061-goldens-after-m5.md` (the window of ADR 0037 point 1 is closed; ADR 0052 is superseded). Never bless outside them; when unsure, escalate.

1. **Declare.** If the change moves pixels on purpose, its OpenSpec `proposal.md` must have a section
   `Expected golden changes:` with one line per golden: `- <golden name>: <reason>`. Add it before collecting.
   A first golden of any class (a golden with no file yet on an adapter) is never blessed by the tool: it escalates to the owner's `/bless`.
   A declared change also needs a section `Bound tests:` with one line per golden: `- <golden name>: <test>, <test>` (the tests that prove the attribute's declared bound, ADR 0035; as `test_name`, `binary::test_name` or `crate::binary::test_name`). The tool enforces them (ADR 0061 point 3): an `AUTO-E` or `AUTO-L` needs every one green in the `--tests` reports of the same run, else it escalates naming the test.
   **Look changes** (ADR 0037 point 2, TEST-036): a change that only edits look parameters (files of `tests/goldens/look_data.txt`, today `*.look.toml`) declares them in `proposal.md` under `Look changes:`, one line each:
   `- <look file>: <key> = <old> -> <new>; affects=<lit|sky|water>[, ...]`, and lists the pictures it moves under `Expected golden changes:`. The tool then blesses the changed lit, sky and water candidates as `AUTO-L` only when the diff touches nothing but look data and inert paths (docs, specs, goldens), every changed line of a look file is one of the declared changes, no debug or shaded golden moved beyond rounding, and the golden is in an affected class and changed on every adapter that has it. A look change plus any code, shader or other data change escalates.
2. **Collect candidates for every adapter that has the golden.** Start from an empty `target/review/candidates/`
   (delete it first) so nothing from an earlier run is reviewed.
   - WARP: `cargo xtask test gpu` (writes `target/review/candidates/dx12-microsoft-basic-render-driver/`). Or download
     CI's `review-Windows` artifact when the machine's WARP version is not the recorded one (the driver gate escalates it).
   - NVIDIA (Rasierklinge): `cargo xtask test gpu --real`.
   - llvmpipe: push the branch, then `gh run download <run> -n review-Linux -D target/review/ci-linux`.
   Golden tests report every mismatch of a run (`GoldenChecks`), and `test gpu` runs every test (`--no-fail-fast`), so one run per adapter is enough.
   Keep this run's test reports (the bound tests of the proposal must be green in them): after each `cargo xtask test`, `test gpu` or `test gpu --real`
   copy `target/nextest/default/junit.xml` (`target/nextest/real-gpu/junit.xml` for a `--real` run) to a file of its own (nextest overwrites it), and pass each with `--tests`.
3. **Review.** `cargo xtask golden-review --proposal openspec/changes/<id>/proposal.md [--extra target/review/ci-linux/candidates]... [--tests <junit.xml>]...`.
   It writes `target/review/golden-review/report.md` and a golden | candidate | diff triptych per candidate, and prints
   which adapters took part (an adapter that has goldens but delivered no candidates is printed as NOT RUN) and one verdict per candidate: `AUTO-R`, `AUTO-E`,
   `AUTO-L` or `ESCALATE <reason>`. Each adapter's candidate is judged against that adapter's own golden: agreement between adapters
   is not a condition (ADR 0061), so a declared change of a debug or shaded view is `AUTO-E` on each adapter that shows it, whatever the others show.
   Look at every triptych yourself; if a picture contradicts its verdict (a hole, a seam, a colour that should not exist), treat it as `ESCALATE`.
   An `AUTO-L` needs the same oracle check as an `AUTO-E` (commit it as `bless(auto): <tests> [L]`).
   An `AUTO-E` also needs, per ADR 0017 and ADR 0061 point 3: at least one adapter with a candidate, the declared bound tests green in the same run (enforced by the tool from the `--tests` reports), the test's own oracles passing on the candidate frame (no holes, coverage, colour
   rules, every assertion of the golden test other than the golden comparison; the agent checks this), and the change's verifier and reviewers
   have no open finding about pixels. If either fails, or any reviewer objects, treat the candidate as `ESCALATE`.
   The driver gate is never relaxed, for first goldens too: a candidate rendered by another driver than the one in
   `driver.txt` escalates.
4. **Bless the auto verdicts.** `cargo xtask golden-review ... --apply` copies only `AUTO-R`/`AUTO-E`/`AUTO-L` candidates
   (never `driver.txt`) into `tests/goldens/<adapter>/`. Commit them on the change's branch as
   `bless(auto): <tests> [R|E|L]`, with the per-candidate numbers from the report in the message body, and add one line to
   `docs/status.md`. Remove a golden's line from `tests/goldens/pending.txt` once every adapter has it.
5. **Escalate the rest.** Leave escalated candidates in place, list them with their reasons and the report path under
   "Waiting for the owner" in `docs/status.md`, and stop work that depends on them. The owner decides with `/bless`.

Never delete or overwrite a golden by hand, never relax a tolerance to make a candidate pass, never bless a lit, sky or water view
outside the look path (a changed existing one escalates), a first golden of any class, or a new adapter family.

Scope: $ARGUMENTS
