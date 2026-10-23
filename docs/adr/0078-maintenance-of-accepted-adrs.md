# 0078 Maintenance of accepted ADRs: links, spec-owned values, domain terms, errata
Status: accepted (owner, 2026-10-23; amends constitution CON-23, v1.5)
Date: 2026-10-23
Source: owner, 2026-10-23 (carve-outs from "never edit an accepted ADR"); `docs/domain.md`; ADR 0055, ADR 0075

## Context
- CON-23 allowed only the status line of an accepted ADR to change; links to moved files broke, and the folder clean-up waited for it.
- Specs are authoritative for current values (ADR 0055); older ADRs still state values the specs have since changed.
- `docs/domain.md` is the shared language; renaming a term left ADRs speaking an outdated language.
- A factual error found later in an ADR stayed unmarked where a reader meets it.

## Decision
1. **Decisions never change.** An accepted ADR's decision, context facts, evidence, date and source are never edited; a new decision is a new ADR that supersedes or amends it.
2. **Links.** A link whose target moved or was renamed is updated to the same content; a link to a newer decision is a supersession, not maintenance.
3. **Spec-owned values.** A decided value now owned by a spec keeps the value at decision time and gains the spec reference ("5 km at decision; current: LOD-014").
4. **Evidence.** Measurements and spike results are never edited.
5. **Domain terms.** A term renamed in `docs/domain.md` is renamed in the ADR only where it names the same concept; quoted owner words stay verbatim with the new term in brackets; `docs/domain.md` lists former names. A new term enters `docs/domain.md` before an ADR uses it.
6. **Errata.** A statement wrong at the time is marked where it stands: `[Erratum <date>: <what is wrong>; see ADR NNNN or <spec ID>]`; the correction itself lives in the newer ADR or the spec.
7. **Trace.** Each maintenance edit adds a line at the end of the ADR: `Maintained: <date>, <links | values | terms | erratum>: <what>`.
8. **One kind per PR.** A maintenance PR is docs-only and holds one kind of edit; spec-guardian checks that the diff is only that kind.

## Consequences
- Constitution v1.5: CON-23 states these carve-outs (applied on the owner's go, 2026-10-23); its cap (BUILD-023) rises to the amended size.
- The `adr` skill, `CLAUDE.md` and the glossary rules name the maintenance kinds.
- spec-lint checks that a change to an accepted ADR beyond its status line adds a `Maintained:` line (BUILD, change `adr-maintenance-lint`).
- The milestone-folder clean-up of `docs/` (2026-10-23) can update the links in ADRs.

## Licences of adopted code or techniques
None.
