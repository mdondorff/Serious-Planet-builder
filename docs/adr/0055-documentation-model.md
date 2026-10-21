# 0055 Documentation model: specs hold current rules, ADRs record why
Status: accepted (owner, 2026-10-17, opus-planner session); point 5 amended by ADR 0075 (archived changes are deleted at milestone close)
Date: 2026-10-17
Source: owner's review of the side documentation (2026-10-17); word counts on main at 4d4c54c+

## Context
- Side documentation is about 1.6 times the production source: production source 284k words; change archive 239k, live specs 70k, ADRs 49k, working files 23k.
- ADRs hold current numbers (budgets, sizes, limits). Because accepted ADRs are immutable, a changed number needs a new ADR that amends the old one. 15 of 54 ADRs amend another, so a reader has to follow a chain (0033 → 0048 → 0054) to find today's value.
- Six "readings" ADRs (0027, 0044, 0048, 0050, 0051, 0053) record how a design ADR was built, which duplicates the change's design notes.
- The writing style drifts into narration and asides.

## Decision
1. **Specs are the single home of current rules.** Every normative number, budget, limit and format lives in `openspec/specs/`, edited through spec deltas. Code and tests cite the requirement ID.
2. **ADRs record why, and stay immutable.**
   - At most about one page: Context (at most 5 bullets), Decision (numbered, one or two lines each), Consequences.
   - Numbers appear in an ADR only as the reason for a choice; the spec carries the value.
   - A changed decision is a new ADR; the old ADR's status line says `superseded by NNNN` or `point N amended by NNNN`.
   - The rule "never edit an accepted ADR except its status line" stays.
3. **No readings ADRs.** How a decision was built goes in the change's design notes. A new ADR is written only when the build departs from an accepted ADR or makes a new decision.
4. **Writing style for specs, ADRs and working files:**
   - definitions, not narration;
   - one statement per line or bullet;
   - numbers in tables;
   - no conversational asides, and no "we" or "I";
   - who decided goes on the status line, not in the prose.
5. **Lean archive.** An archived change keeps `proposal.md` and its spec deltas. Its `design.md` and `tasks.md` are deleted in the archiving PR; git history keeps them.
6. **One-time consolidation in the M5 housekeeping:**
   - rewrite the live specs in the style of point 4;
   - move the current values of every ADR amendment chain into the specs;
   - mark the readings ADRs `superseded by 0055 (content in specs)`;
   - prune `design.md` and `tasks.md` from earlier archived changes.

## Consequences
- To find a current value, read the spec. To find why, read the ADR named in the spec's `Source:` line.
- ADRs and archives stop growing with every feature; specs change more often.
- spec-lint gains checks for the style rules that can be counted (one example: a requirement's statement over 120 words fails) in the M5 housekeeping.
- The consolidation must not change any requirement's meaning; any meaning change is a spec delta with its own review.

## Licences of adopted code or techniques
None.
