## ADDED Requirements

### Requirement: VIEW-079 Every ready region of a regions file opens together
`planet editor` with `--regions FILE` (or a `regions.toml` beside `--world F`) and no `--store` or `--commit` SHALL open every `ready` region of it over the class-source base in memory (VIEW-052): over each region's tiles its exact base, layer, lakes and rivers; elsewhere the coarse globe.
A `stale` or `unbaked` region SHALL not open; the log SHALL name it with its state, and give each opened region's time.
The regions' layer tiles SHALL share one cache of `LAYER_CACHE_TILES`; their river and lake caches the budget of one commit.
Regions sharing a tile SHALL be refused. The camera SHALL start over the first.
The open line SHALL be `cargo run --release -p planet -- editor --world F|--seed N [--items DIR] --regions FILE`.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring D2: regions bake one at a time, the editor shows every ready one), lead 2026-11-28 (the 14:45 order), VIEW-052, VIEW-069, VIEW-077, WDEF-019

#### Scenario: Two regions baked one after the other
- **WHEN** two adjacent boxes of a file are baked one after the other and the file is opened
- **THEN** both open over one base, each region's ground is what it is when opened alone, the union of their tiles is the region, and the camera starts over the first

#### Scenario: An edit makes them stale
- **WHEN** an item is put into the world's items folder
- **THEN** both regions read `stale` and none opens until it is baked again

#### Scenario: The seam between adjacent regions
- **WHEN** two ready regions share an edge
- **THEN** each opens with its own ground; each commit is solved alone, so the shared edge may step (a known limit)

#### Scenario: Overlapping regions
- **WHEN** two commits hold one tile
- **THEN** opening them together is refused naming the tile

## MODIFIED Requirements

### Requirement: VIEW-077 The editor places, resizes and deletes region boxes and reopens a baked one
`planet editor` SHALL edit a regions file's boxes (WDEF-019): `--regions FILE`, else the `regions.toml` beside `--world F`.
The Sites tab SHALL list each box with its derived state.
Place and Move SHALL put a box of 2 to 4 L tiles a side where a click hits the ground, snapped to whole tiles and clamped into the allowed tiles; Resize and Delete SHALL change it.
Every box put SHALL pass `check_box`; a refused one leaves the file unchanged.
The file SHALL be rewritten after each change, outside `WorldDefinition`.
The window SHALL draw the boxes by state, the placement box and the face-edge margin on the ground.
Open on a `ready` box SHALL copy the open line (VIEW-079) to the clipboard.

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
- **THEN** the row reads `unbaked`, `ready`, `stale` in turn, and Open copies and prints the open line of the regions file (VIEW-079)

#### Scenario: No default store for region boxes
- **WHEN** `editor --seed 14 --regions F` runs with a default store on disk
- **THEN** no store is opened and the base is computed in memory (VIEW-052)

#### Scenario: The default store's refusal
- **WHEN** a default store cannot be read
- **THEN** the run is refused naming `--commit DIR`, `--global-res N` and `cargo xtask store` (VIEW-052)
