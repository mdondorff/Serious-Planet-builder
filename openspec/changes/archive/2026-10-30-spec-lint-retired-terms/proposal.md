## Why
`docs/domain.md` section 8 lists the former terms of the project (the chunk stage among them). Nothing stopped a new spec, status line or briefing from using one again. The owner asked (2026-10-30) for a `spec-lint` guard.

## What changes
`cargo xtask spec-lint` reads the first column of the "Former terms" table (no term is hard-coded) and fails a mention in the main specs, `domain.md` above section 8, `status.md`, `roadmap.md`, `README.md` and the files of `docs/briefings/`. A row marked *code removal pending* only prints a warning count until the marker goes (B6). The failure names file, line, term and the "Now" column. ADRs, the archives, the log, background, learnings, spikes and archived changes are not read.

## Matching
Case-insensitive whole phrases, `-` and `_` read as a space; a first cell lists terms separated by commas; remarks in parentheses are dropped; a single word is skipped unless the row says `Lint: all`; `Lint: a; b` adds spellings and `Lint: none` switches a row off.

## Not changed
`docs/domain.md` (the architect's file). Today no row has a `Lint:` note: "block (regional square)" is a single word and is skipped.
