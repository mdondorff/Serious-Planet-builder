# 0096 Rounding auto-bless covers lit, sky and water views, against the owner's last bless
Status: accepted (owner, 2026-11-03: "Please amend"); amends ADR 0017's Auto-bless R and its escalation list
Date: 2026-11-03
Source: owner, 2026-11-03 (two RTX march goldens of 5 and 4 px, max delta 1, escalated only as "a lit view"); ADR 0017, ADR 0061

## Context
- ADR 0017's Auto-bless R covers debug and shaded views only; every lit view escalates, whatever its size.
- On 2026-11-03 two lit goldens changed by 5 and 4 px at max channel delta 1 (a clamp at 1.0000001): rounding by every number, escalated by class alone.
- The risk a class rule guards against in lit, sky and water views is drift: a series of small blesses, each compared only with its predecessor, can move the look the owner approved without any single step showing it.
- The owner's approved look is recorded in git: every owner bless is a commit with an `Owner-approved:` trailer.

## Decision
1. Auto-bless R applies to every class, lit, sky and water included: at most 32 changed pixels and max channel delta at most 1 on that adapter, same driver as `driver.txt`.
2. For a lit, sky or water view the candidate is also compared with the **anchor**: that golden's file at the latest commit carrying an `Owner-approved:` trailer (or, if none, its first commit). If the candidate differs from the anchor by more than 32 pixels or a max channel delta above 1, it escalates ("drift from the owner's bless").
3. Every lit, sky or water auto-bless is listed in the planner's next summary to the owner, with the numbers against both the golden and the anchor.
4. Everything else in ADR 0017 and ADR 0061 stands: first goldens, driver changes, size changes, larger changes and look changes outside the look path escalate.

## Consequences
- Rounding on lit, sky and water views no longer waits for the owner; a slow walk away from the approved look still reaches the owner.
- The `golden-review` tool and skill implement point 2 (an anchor lookup from git history); until they do, lit, sky and water candidates keep escalating.
