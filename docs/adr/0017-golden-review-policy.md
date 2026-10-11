# 0017 Golden review policy and agent blessing
Status: accepted
Date: 2026-10-11
Source: owner decision 2026-10-11 (bless of PR #43's terrain goldens); constitution CON-18 (amended to v1.2 by this ADR); ADR 0009

## Context
CON-18 (v1.1) let only the owner bless golden images. Each M3 change that moves terrain pixels then waits for the owner, per adapter (WARP, llvmpipe, NVIDIA), and the golden test stopped at its first mismatch, so the llvmpipe set took one CI round per image (PR #43: five goldens, five rounds). The owner asked for more autonomous execution on golden changes. Blessing stays a high-trust act: a wrong golden silently redefines "correct" for every later run.

## Options (with trade-offs)
1. Owner blesses everything (status quo). Safest, slowest; blocks agents at every visual change.
2. **Agents bless under a written policy; everything else escalates** (chosen). Fast for noise and for declared, cross-checked debug-view changes; the owner keeps every judgement call and can revert any auto-bless.
3. Agents bless anything once reviewers pass, owner audits afterwards. Fastest, but a reviewer agent and the author agent share blind spots; a subtly wrong golden would become the reference before anyone looks.

## Decision
Option 2. The `golden-review` skill (`.claude/skills/golden-review/`) collects candidates for every adapter, writes evidence (golden | candidate | diff triptych, changed pixels, max channel delta) to `target/review/golden-review/`, and classifies each candidate:
- **Auto-bless R (rounding):** a debug or shaded view with at most 32 changed pixels and max channel delta at most 1 on that adapter (no declaration needed).
- **Auto-bless E (explained):** all of
  - the golden's name is listed under `Expected golden changes:` in the active OpenSpec change's `proposal.md`, with a one-line reason;
  - the golden is a *debug* or *shaded* view (ADR 0009 classes), never *lit*;
  - for a *debug* view: the changed-pixel count agrees across every adapter that has the golden, within +-5 % (or +-8 pixels, whichever is larger), the max channel deltas agree within +-5 % (or +-1), and at least two adapters have a candidate; a *shaded* view (adapter-specific rasterisation such as derivative-drawn contour lines) is exact per adapter and skips this check (owner, 2026-10-11, after `terrain_ground1m_depth` changed 7 px on llvmpipe against about 940 on WARP and NVIDIA);
  - the test's own oracles pass on the candidate frame (no holes, coverage, colour rules), and the change's verifier report has no open finding about pixels.
- **Escalate to the owner** (`/bless`): a golden for a new test or a new adapter family; any lit view; cross-adapter disagreement; a change not declared in the OpenSpec change; a candidate whose size differs from the golden; any reviewer objection; anything the agent is unsure of.

Auto-blessed goldens are committed separately as `bless(auto): <tests> [R|E]` with the numbers in the message, and one line per batch is added to `docs/status.md`. The owner may revert any auto-bless; a revert turns that golden's next change into an escalation. The owner can suspend the policy at any time by saying so (agents then escalate everything); thresholds or rules change only through a superseding ADR (CON-23).

Golden tests report every mismatch of a run before failing (`testkit::GoldenChecks`), so one CI run yields every candidate of an adapter.

## Consequences
- Constitution v1.2: CON-18 reads "Golden images are approved by the owner, or by an agent under the owner's policy in ADR 0017."
- CLAUDE.md: "Never bless golden images" becomes "Bless goldens only through the `golden-review` skill's policy (ADR 0017); everything it escalates waits for the owner's `/bless`."
- Easier: visual changes no longer stall on rounding and declared debug-view changes. Harder: OpenSpec changes that move pixels must declare their expected golden changes up front.
- Tests: the policy classifier is pure and tested at tier A (`xtask golden-review` unit tests); the collector is tested at tier A in testkit.

## Licences of adopted code or techniques
None.
