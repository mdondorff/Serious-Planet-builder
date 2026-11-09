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
Spec-lint SHALL fail a branch that turns an item of the table under `## Items` of `docs/briefings/terrain-prototype.md` to a State starting with `done`, against the merge base with `origin/main`, unless the briefing's `Re-estimate: YYYY-MM-DD (...)` line holds a valid date other than the base's. A briefing without the line SHALL fail. An item already done on the base, or going back from done, SHALL ask for nothing.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (items were marked done without the remaining work re-estimated)

#### Scenario: An item is done
- **WHEN** an item becomes done with the re-estimate date unchanged, with no line, or with a new date
- **THEN** the first two are refused naming the item and the last passes

### Requirement: BUILD-043 The lanes are listed with their state
`cargo xtask lanes` SHALL print, for each lane of `docs/lanes.txt` (a lane name and the prefix of its worktree directory under `.claude/worktrees/`; no machine paths), the last commit of its worktree with its age, the open PR of its branch, the highest `## #N` heading of its inbox file (`PLANET_INBOX_DIR`, else `planner-inbox` beside the main checkout) and the running `cargo`, `rustc`, `nextest` or `target/` executables whose path lies in its worktree; other build processes SHALL be listed as not attributed. A missing piece SHALL print `-` and the command SHALL NOT fail for it.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94

#### Scenario: A lane block
- **WHEN** a lane has no worktree, or a worktree with no commit, PR, inbox file or process
- **THEN** its block prints `-` for each missing piece, and a process under a lane's worktree is listed under that lane
