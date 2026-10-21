# 0075 Documentation budget: the hot files stay small and current
Status: accepted (opus-planner, 2026-10-21; budget approved by the owner)
Date: 2026-10-21
Source: owner's approval of the documentation budget (2026-10-21)

## Context
- Markdown is 4.2 MB against 6.5 MB of Rust on main; the OpenSpec change archive is 2.1 MB.
- Hot files read by every session: `status.md` 42 KB, `learnings.md` 107 KB, `spikes.md` 38 KB.
- ADR 0055 keeps archived changes (proposal and spec deltas); nothing reads them, and the current specs hold the rules.
- Documentation is a means to maintainable software, not an output.

## Decision
1. **Hot-file caps**, checked by `cargo xtask spec-lint`:
   - `docs/status.md` at most 15 KB;
   - `docs/learnings.md` at most 20 KB;
   - `docs/spikes.md` at most 10 KB, as an index (one row per spike, details in the spike's results file);
   - `CLAUDE.md` and `docs/constitution.md` at most their size when this ADR is accepted.
2. **Archived OpenSpec changes are deleted at milestone close.** The specs hold the rules; git holds the history. This amends ADR 0055 point 5 (keep `proposal.md` and spec deltas).
3. **A learning has one of three fates:** promoted (a spec line, a `CLAUDE.md` line or a test that cites it), kept while still active and surprising, or moved to `docs/archive/<M>/learnings.md` with its milestone.
4. **A Tracked entry in `status.md` has at most 3 lines.** Details live in the change or spike it names. A finished entry is deleted.
5. **Each milestone report states the hot set's size in tokens** (`status.md`, `learnings.md`, `spikes.md`, `CLAUDE.md`, the constitution).

## Consequences
- A session's startup reading is bounded and current; history is found with `git log`.
- Housekeeping must promote, keep or archive learnings and trim status at each milestone close, or spec-lint fails.
- Existing archived changes are deleted once; links into them are fixed in the same change.
- Cap values change only by a new ADR.

## Licences of adopted code or techniques
None.
