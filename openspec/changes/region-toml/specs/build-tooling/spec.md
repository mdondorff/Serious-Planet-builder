## ADDED Requirements

### Requirement: BUILD-078 One command bakes the region boxes of a world
`cargo xtask bake-region (--world F [--regions FILE] | --seed N --regions FILE) [--items DIR] [--name NAME] [--force] [--threads T]` SHALL bake the boxes of a regions file (WDEF-019) with the region bake (GEN-177).
It SHALL load the world's `WorldDefinition` (`--world`, or the viewer's single planet of `--seed`) and put every item of `--items` into it (CON-05), check every box before the first bake, bake each box that is not `ready` (all with `--force`, one with `--name`) into its commit folder over the production base, and print for each the editor command that opens its commit (VIEW-069).

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring D1 and E1), GEN-177, VIEW-069, CON-05

#### Scenario: The options
- **WHEN** the command is given `--world` alone, `--world` with `--regions`, `--seed` with `--regions`, `--seed` alone, both, neither, or an unknown option
- **THEN** the regions file is `regions.toml` beside the definition, the given file, the given file, or an error with the usage line naming the problem

#### Scenario: The plan
- **WHEN** the file is empty, names no region of `--name`, or holds a box `check_box` refuses anywhere in it (also with `--name` naming another box)
- **THEN** the command fails before any bake, naming the file, the name or the box; otherwise it bakes each region that is not `ready` and skips the ready ones unless `--force`
