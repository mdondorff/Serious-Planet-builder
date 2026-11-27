## ADDED Requirements
### Requirement: BUILD-072 One command archives a finished milestone's log and spikes
`cargo xtask archive-milestone <M> [--before D] [--dry-run]` SHALL do the housekeeping skill's mechanical archive steps for milestone `M`, compute every new file text before it writes any, and delete only the log fragments it merged.

Verify: A
Status: active
Source: Lead 2026-11-27 (the P3 closure plan's S1 and D3's precondition; both reviewers), the housekeeping skill's step 2, BUILD-010
Detail:
| Step | What it does |
|---|---|
| log | the fragments of `docs/log.d/` (dated before `D`, when given) merged into `docs/archive/<M>/log.md` in `cargo xtask log`'s order (newest first; an existing archive's entries merged in, its header kept), then the merged fragments deleted |
| spikes | the rows of `docs/spikes.md`'s index whose status starts with `closed`, byte for byte, into `docs/archive/<M>/spikes.md` (appended after an existing table's last row, a table header first when it has none; a row it already holds is not added again) |
| pointers | `docs/archive/<M>/log.md` added to the header sentence of `docs/log.md` that lists the archives, and `docs/archive/<M>/spikes.md` to the last header line of `docs/spikes.md` that names an archive, each only when missing |
| `--dry-run` | the plan printed (the files written, the fragments deleted), nothing written |
- Refused before any write: a malformed fragment (BUILD-010), a header without the sentence or line the pointer goes on, a milestone name with other than letters, digits and `-`, an unknown option.
- A second run with the same arguments has nothing to do.
- An existing log archive is rewritten as its header (the lines before its first entry) and its entries; a fragment dated `D` stays; files are written with LF line ends.
- Nothing else is deleted: no change archive, no image, no ADR.

#### Scenario: A milestone archived
- **WHEN** the command runs for `M2` before 2026-02-01 on a docs tree with three fragments (one later) and a register with two closed, one open and one planned spike
- **THEN** the archive holds the two entries newest first, the later fragment stays, the two closed rows move byte for byte, the pointers are added once, and a second run has nothing to do
