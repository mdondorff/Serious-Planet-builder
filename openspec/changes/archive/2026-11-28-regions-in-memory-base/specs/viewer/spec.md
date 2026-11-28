## MODIFIED Requirements

### Requirement: VIEW-077 The editor places, resizes and deletes region boxes and reopens a baked one
`planet editor` SHALL edit the boxes of a regions file (WDEF-019): `--regions FILE`, else the `regions.toml` beside `--world F`.
Without `--store` its base SHALL be computed in memory (as for `--commit`), never read from a default store.
The Sites tab SHALL list each box with its state.
Place and Move SHALL put a box of 2 to 4 L tiles a side at a clicked ground point, snapped and clamped into the allowed tiles; Resize and Delete change it.
Every box put SHALL pass `check_box`.
The file SHALL be rewritten after each change, outside `WorldDefinition`.
The boxes, placement box and face-edge margin SHALL be drawn on the ground.
Open on a `ready` box SHALL print its `--commit` command (VIEW-069).

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E2; the base in memory: the E3 smoke, lead 2026-11-28), WDEF-019, BUILD-078, VIEW-051, VIEW-069, VIEW-075

#### Scenario: Placing at a face edge
- **WHEN** a 2x2 box is placed with the cursor on the first tile of a face
- **THEN** the box sits at the first allowed tile, passes `check_box`, is in the reread file, and the world's hash is unchanged

#### Scenario: Resize, move, delete and refusals
- **WHEN** the box is widened to 4 and once more, moved, placed again under a taken name, and deleted
- **THEN** the side stops at 4 with the reason in the status, the move snaps to the new tile, the taken name is refused, and the file ends empty

#### Scenario: No default store
- **WHEN** `editor --seed 14 --regions F` runs with a default store on disk, and a default store cannot be read
- **THEN** no store is opened, the base is computed in memory; the default store's refusal names `--commit DIR`, `--global-res N` and `cargo xtask store`

#### Scenario: State and reopen
- **WHEN** a box's commit is baked, the definition then changes, and Open is pressed on the ready box
- **THEN** the row reads `unbaked`, `ready`, `stale` in turn, and Open prints the `--commit` command of its folder
