## ADDED Requirements

### Requirement: BUILD-064 The offline pipeline cockpit and preview batch runner
`cargo xtask cockpit` SHALL print living usage documentation and verified examples via `-h` or `--help` (and inside the HTML report), validate schema-v2 pipeline JSON (`BUILD-063`), parse probe rows from `docs/stages/manifest.txt`, index existing PNG headers and text reports across preview and stage directories, and write a single-file offline `index.html` containing a live switch simulator, side-by-side preview comparator, and searchable version-pin and probe ledger using `std`-only code without external assets or network requests (`CON-02`, `CON-06`). With `--run <world|drivers|stages|all>`, `--compare-seed`, or `--chain`, it SHALL plan and execute matching CPU preview, stage-manifest, and transect commands sequentially under the shared heavy lock (`BUILD-026`) before assembling the report.

Verify: A
Status: active
Source: owner (2026-11-20, Global Authoring & pipeline debugger MVP), ag2-antigravity, BUILD-026, BUILD-063

#### Scenario: Offline cockpit assembly and batch preview planning
- **WHEN** `cargo xtask cockpit` runs with `--help`, reads a valid schema-v2 pipeline JSON, or runs `--run all --compare-seed N --chain DIR`
- **THEN** `-h`/`--help` and `--pipeline-json` without `--run` skip the heavy lock, `-h`/`--help` prints live flags and validated examples, HTML indexes PNG dimensions and probe hooks, `--run` builds CPU preview and sheet commands, and invalid flag combinations or outdated schema versions are refused
