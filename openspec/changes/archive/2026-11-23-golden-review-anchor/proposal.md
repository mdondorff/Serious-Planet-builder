## Why
- ADR 0096 (owner-accepted 2026-11-03, #1112): rounding auto-bless covers lit, sky and water views, checked against the anchor (the owner's last bless of the golden) so that a series of small blesses cannot walk the look away from what the owner approved.
- Until the tool implements point 2, lit, sky and water candidates keep escalating; two RTX march goldens (5 and 4 px, delta 1) escalated on 2026-11-03 only as "a lit view".

## What Changes
- `cargo xtask golden-review`: for a lit, sky or water candidate within rounding of its own golden, the verdict is `AUTO-R` only if the candidate is also within rounding (at most 32 px, delta 1) of the anchor: the golden's file at the latest commit with an `Owner-approved:` trailer, else its first commit (`planet_testkit::review::{anchor_commit, file_at_commit, classify_anchored}`). Drift or no anchor escalates; `report.md` and the console line carry the numbers against both golden and anchor.
- TEST-016 reworded; the `golden-review` skill and CLAUDE.md's bless line updated.
- Tier-A tests with synthetic images and a scripted git history (a throwaway repository): the anchor commit with and without trailers, the verdict rule per class, a slow walk of small blesses reaching the owner.

## Impact
- `crates/testkit` (`review.rs`, `bin/golden-review.rs`), `.claude/skills/golden-review/SKILL.md`, `CLAUDE.md`, `openspec/specs/testing` (TEST-016).
- Verified on the real RTX march candidates of 2026-11-03: both come out `AUTO-R` against the anchor 083de557 (the #1078 bless); nothing applied.
