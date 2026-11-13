## ADDED Requirements
### Requirement: BUILD-040 A change of a gate or an owner decision leaves a contract-change line
Spec-lint SHALL fail a branch that adds, removes or changes a row of the gate table under `### 12.1` of `docs/design/terrain-target.md` or of the table under `## Owner decisions` of `docs/briefings/terrain-prototype.md`, against the merge base with `origin/main`, unless the same branch adds to the briefing's `## Contract changes` list a line `- YYYY-MM-DD | gate or section | what changed | who decided` whose second field contains the row's first cell (without a trailing parenthesis). A malformed line SHALL cover nothing. A missing briefing with a changed row SHALL fail.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94

#### Scenario: A gate changes
- **WHEN** a branch changes the G-SURF row and adds no line, adds a line naming G-CON, adds a malformed line, or adds a dated line naming G-SURF
- **THEN** the first three are refused naming G-SURF and the last passes; a line already on the base covers no new change

### Requirement: BUILD-041 A gate ledger holds every measurement of a gate
`docs/gates/ledger.md` SHALL hold one row per measurement of a gate of `docs/design/terrain-target.md` 12.1: gate, seed, region, resolution, commit, value, verdict (`pass`, `fail`, `partial`, `reported`, `not run`) and the lane that measured it. Spec-lint SHALL fail a gate of 12.1 with no row, a row of a gate not in 12.1, a bad verdict or an empty cell, and a measured row whose commit does not resolve; a `not run` row SHALL have `-` as commit. `cargo xtask gates` SHALL print per gate the newest verdict, the rows and the lanes, and name the gates measured by one lane only. CI SHALL check out the history spec-lint reads (BUILD-040).

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (#1259 reached green with its gates unmeasured)

#### Scenario: The ledger
- **WHEN** a gate has no row, a row names an unknown gate, a measured row has a commit that does not resolve, or all gates have rows including `not run`
- **THEN** the first three are refused naming the gate or commit and the last passes; the summary names the gates measured by one lane

### Requirement: BUILD-042 An item that becomes done moves the re-estimate
Spec-lint SHALL fail a branch that turns an item of the table under `## Items` of `docs/briefings/terrain-prototype.md` to a State starting with `done`, against the merge base with `origin/main`, unless the briefing's `Re-estimate: YYYY-MM-DD[ HH:MM] (...)` line holds a stamp later than the base's.
- A stamp is a date with an optional `HH:MM`; a date alone reads as 00:00, and an invalid time reads as none.
- Stamps compare as a whole: the same or an earlier stamp is refused, a later one passes, a later time on the same day included.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (items were marked done without the remaining work re-estimated), inbox #122 (a second re-estimate on one day)

#### Scenario: An item is done
- **WHEN** an item becomes done with the re-estimate stamp unchanged, earlier, with no line, or later (a later date, or a later time on the same day)
- **THEN** the first three are refused naming the item and the last passes

#### Scenario: A date alone
- **WHEN** the base's re-estimate is a date alone and the new line is the same day at 00:00, at 00:01, or the date alone again; or the base has a time and the new line is the date alone
- **THEN** 00:01 passes and the others are refused (a date alone is 00:00)

### Requirement: BUILD-043 The lanes are listed with their state
`cargo xtask lanes` SHALL print, for each lane of `docs/lanes.txt` (a lane name and the prefix of its worktree directory under `.claude/worktrees/`; no machine paths), the last commit of its worktree with its age, the open PR of its branch, the highest `## #N` heading of its inbox file (`PLANET_INBOX_DIR`, else `SeriousPlanetData/lanes` beside the main checkout) and the running `cargo`, `rustc`, `nextest` or `target/` executables whose path lies in its worktree; other build processes SHALL be listed as not attributed. A missing piece SHALL print `-` and the command SHALL NOT fail for it.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94

#### Scenario: A lane block
- **WHEN** a lane has no worktree, or a worktree with no commit, PR, inbox file or process
- **THEN** its block prints `-` for each missing piece, and a process under a lane's worktree is listed under that lane

### Requirement: BUILD-044 The pr-gate requires the verifier line on a code PR
`cargo xtask pr-body-check` SHALL read `PR_BODY`, `BASE_SHA` and `HEAD_SHA` from the environment and fail when the PR changes a path that is not docs-only (the paths `ci.yml` ignores) and its body has no line `Verifier: PASS <sha>` (at least 7 hex digits) naming a commit of `BASE..HEAD`. A PR whose every path is docs-only SHALL pass. The `pr-gate` workflow SHALL run it on every pull-request event including a body edit, with the full history, and a sha that is not the head SHALL pass with a note naming the later commits.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (#1259 reached green with the verifier not run)

#### Scenario: The verifier line
- **WHEN** a code PR has no line, names a commit that is not in the PR, names its head, or names an earlier commit, and a docs-only PR has no line
- **THEN** the first two are refused, the head passes, the earlier commit passes with a note, and the docs-only PR passes
