## Shared
- All checks run in spec-lint's tier (std only, `xtask/`), on the diff of HEAD against the merge base with `origin/main` (the function the changes-read test already uses). Without git or a base they pass with a note, as the existing diff checks do.
- Tables are read as lines starting with `|`; a row's key is its first cell with a trailing parenthesis removed (`G-SURF-I (diagnostic)` is `G-SURF-I`).
- Each check has a tier-A test on injected text (before and after) and one on a real repository.

## BUILD-040 Contract changes
- Watched: the rows of the table under `### 12.1 Gates` in `docs/design/terrain-target.md`; the rows of the table under `## Owner decisions` in `docs/briefings/terrain-prototype.md`.
- A row added, removed or changed against the base needs a line in the briefing's `## Contract changes` list added by the same diff: `- YYYY-MM-DD | gate or section | what changed | who decided`. The second field must contain the row's key; the date must parse; the last field must not be empty.
- A briefing that no longer exists (archived at the block's close) ends the rule; a gate-table change then fails with "restore the briefing or amend BUILD-040".
- Seed: the list starts with the 2026-11-09 owner decisions (G-SURF, memory, sediment), as they stand on main.

## BUILD-041 Gate ledger
- `docs/gates/ledger.md`: a pipe table `gate | seed | region | resolution | commit | value | verdict | lane`. Verdicts: `pass`, `fail`, `partial`, `reported`, `not run`.
- Spec-lint: every gate key of 12.1 has at least one row; every row's gate is a key of 12.1; a row that is not `not run` has a commit that resolves (`git cat-file -e`, skipped without git) and a non-empty value and lane. A `not run` row has `-` as commit.
- `cargo xtask gates`: per gate the verdict of the newest row, the number of rows and the lanes that measured it; the lines "gate measured by one lane only" mark the exit list's independence item.
- Seed: from `docs/log.d`, the spikes' results and the specs on main; the commit is the squash commit of the PR that carried the measurement (`git log --grep '(#N)'`).

## BUILD-042 Re-estimate
- Watched: the briefing's items table, column State. An item whose State starts with `done` in HEAD and not in the base is newly done.
- A newly done item needs the briefing's line `Re-estimate: YYYY-MM-DD (...)` to hold a date different from the base's and not in the future.

## BUILD-043 Lanes
- `docs/lanes.txt` (config; names only): `lane | worktree directory name`. The inbox directory is `PLANET_INBOX_DIR`, else `planner-inbox` beside the repository's main checkout.
- Per lane: last commit subject and age of the worktree's HEAD, the open PR of its branch (`gh`), the highest `## #N` of `<inbox>/<lane>.md`, and the processes that are `cargo`, `rustc`, `nextest` or an executable under a `target` folder (`tasklist` on Windows, `ps` elsewhere), with their working set.
- Missing pieces print `-`; the command never fails for them.

## BUILD-044 Verifier line in the PR body
- `cargo xtask pr-body-check` reads `PR_BODY`, `BASE_SHA`, `HEAD_SHA` from the environment (the body never reaches a shell command line).
- A PR is docs-only when every changed path matches `ci.yml`'s `paths-ignore` (a tier-A test reads the workflow and compares). Docs-only passes.
- Otherwise the body needs a line `Verifier: PASS <sha>` with at least 7 hex digits, and the sha must be a commit of `BASE..HEAD`. A sha that is not the head passes with a note naming the later commits.
- `pr-gate.yml` gets `edited` among its event types (a fixed body re-runs it), `fetch-depth: 0`, and runs the command.
