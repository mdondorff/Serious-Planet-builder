## MODIFIED Requirements

### Requirement: VIEW-052 The editor opens the production store
Without `--store`, `--commit`, `--regions`, `--world` or `--global-res`, outside a smoke run, the editor SHALL open the first existing directory of: `PLANET_STORE`, the pointer file `target/stores/default`'s, `target/stores/global16384`, `target/accept/global16384` (under the repo root); with none, the in-memory preview bake.
With `--commit` or `--regions` it SHALL compute the base in memory (1,024 cells; `--regions --global-res N`: N).
An unreadable default store SHALL be refused naming `--commit DIR`, `--global-res N` and `cargo xtask store`.
The title SHALL name the source: `store <dir>` or `preview bake <N> cells`.
`cargo xtask store [--res N] [--relief-scale S] [--link DIR]` SHALL build a store of N cells a face edge (default 16384) into `target/stores/global<N>` and write the pointer file; `--link` only writes the pointer to an existing store.

Verify: A
Status: active
Source: owner (2026-10-22, via opus-planner): the editor showed a flat world because it ran on the 256-cell preview bake

#### Scenario: The order
- **WHEN** a root holds none, then an accept store, a stores store, a pointer file and an environment variable, in that order
- **THEN** the choice is none, the accept store, the stores store, the pointer's, the variable's, and a variable naming no directory falls through

### Requirement: VIEW-077 The editor places, resizes and deletes region boxes and reopens a baked one
`planet editor` SHALL edit the boxes of a regions file (WDEF-019): `--regions FILE`, else the `regions.toml` beside `--world F`.
The Sites tab SHALL list each box with its derived state.
Place and Move SHALL put a box of 2 to 4 L tiles a side where a click hits the ground, snapped to whole tiles and clamped into the allowed tiles; Resize and Delete SHALL change it.
Every box put SHALL pass `check_box`; a refused one leaves the file unchanged.
The file SHALL be rewritten after each change, outside `WorldDefinition`.
The window SHALL draw the boxes by state, the placement box and the face-edge margin on the ground.
Open on a `ready` box SHALL print its `--commit` command (VIEW-069).

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E2), WDEF-019, BUILD-078, VIEW-052, VIEW-069, VIEW-075

#### Scenario: Placing at a face edge
- **WHEN** a 2x2 box is placed with the cursor on the first tile of a face
- **THEN** the box sits at the first allowed tile, passes `check_box`, is in the reread file, and the world's hash is unchanged

#### Scenario: Resize, move, delete and refusals
- **WHEN** the box is widened to 4 and once more, moved, placed again under a taken name, and deleted
- **THEN** the side stops at 4 with the reason in the status, the move snaps to the new tile, the taken name is refused, and the file ends empty

#### Scenario: State and reopen
- **WHEN** a box's commit is baked, the definition then changes, and Open is pressed on the ready box
- **THEN** the row reads `unbaked`, `ready`, `stale` in turn, and Open prints the `--commit` command of its folder

#### Scenario: No default store for region boxes
- **WHEN** `editor --seed 14 --regions F` runs with a default store on disk
- **THEN** no store is opened and the base is computed in memory (VIEW-052)

#### Scenario: The default store's refusal
- **WHEN** a default store cannot be read
- **THEN** the run is refused naming `--commit DIR`, `--global-res N` and `cargo xtask store` (VIEW-052)
