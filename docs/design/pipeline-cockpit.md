# Pipeline Cockpit: Global Authoring Companion & Step Inspector
Status: design note, non-normative (ag2-antigravity, 2026-11-20); companion to BUILD-063, TEST-142, and BUILD-064.

## 1. Intent & Why Two PRs
Before an in-app Global Authoring UI lands inside the runtime viewer (A3 section 11 / `driver-regions.md`), authoring parameters and pipeline steps are inspected across scattered headless commands (`preview world`, `generate --*-preview`, `stages`, `transects`). A durable local HTML cockpit gives an immediate offline companion for:
- Visualizing the real active terrain & hydrology pipeline across flags and definitions (`--erosion`, `--channels`, `--commit DIR`, `world.height_field`, `turned_noise`, ADR 0007 `stamp` layers),
- Running and side-by-side inspecting headless Global Authoring driver previews (`shelf`, `lithology`, `uplift`, `orography`, `arid`, `ice`, `world`) and stage sheets without needing a GPU or rewriting prose docs.

Scraping `docs/design/terrain-target.md` drifts from live code as flags and `*_VERSION` pins evolve. Instead, the cockpit reads a canonical interface descriptor generated directly from Rust source constants and runtime chain assembly, split into two PRs so the contract and JSON interface can be reviewed before the HTML UI builds on it:

| PR | Branch / IDs | Scope | Gate |
|---|---|---|---|
| **PR 1** | `change/pipeline-descriptor` (`BUILD-063`, `TEST-142`) | Code-generated `pipeline_catalog`, runtime `planet generate --pipeline-json OUT` (embedding `catalog` + `--commit DIR`), `target/review/stages/pipeline.json` in Tier A, `probe::Stage` + `manifest.txt` hook parity, live `spec_ids` check, `HeightField::upstream()` contract test | Merged into `main` (`acf57959`, PR #1615) |
| **PR 2** | `change/xtask-pipeline-cockpit` (`BUILD-064`, current) | `std`-only `cargo xtask cockpit` HTML assembler (`xtask/src/cockpit/`) invoking `planet generate --pipeline-json` and `--run <world\|drivers\|stages\|all>` batch orchestrator over existing preview CLIs + `Stage::ALL_KINDS` variant-coverage check | Tier-1 (`xtask` companion tooling) |

**Scope trim:** No repo-admin/meta context drawer belongs in `cargo xtask cockpit`. It focuses solely on the pipeline graph, active chains, version/key pins, and step outputs.

## 2. Architecture of PR 1 (`BUILD-063`, `TEST-142`)
- **`planet_generators::pipeline_catalog` (`SCHEMA_FORMAT_VERSION = 2`)**: Pure, zero-IO/zero-GPU introspection module referencing all 42 live `*_VERSION` / toggle constants in `planet-world-def` and `planet-generators`, enumerating 20 K-labelled (`K1`..`K11`, `M9-P1`/`M9-P2`), `StageScope`-tagged (`global`, `region`, `tile`) stages, and mapping `PipelineSwitches` (`HeightFieldKind`, `ErosionBakeMode`, `erosion_layer`, `relief_lattice`, `channels`, ADR 0007 stamp stack, water cull level) to predicted `ChainLink` stacks.
- **Real probe-registry derivation & split M9 vs. K5-K7**:
  - `planet_generators::bake::probe::Stage::ALL_KINDS` and `Stage::kind_id` are mapped exhaustively by `pipeline_catalog::stage_id_for_probe()` to `k5-landmass-and-window-cut`, `k5-regional-rounds`, `k6-h1-hydrology-and-lakes`, `k6-regional-sediment`, and `k7-local-refinement`, checked 1-to-1 against `docs/tools/stages/manifest.txt`.
  - M9 layer passes (`m9-pass1-face-carve` [`M9-P1`], `m9-pass2-tile-layer` [`M9-P2`]) are distinct from the K5-K7 regional/local chain, with K5 `window_cut` boundary classification modeled explicitly (`GEN-172`). No speculative `spike/rg-prototype` placeholders are included.
- **No committed JSON & live `spec_ids` (`TEST-142`)**:
  - `target/review/stages/pipeline.json` is written deterministically by Tier-A tests (no `docs/tools/stages/pipeline.json` checked into git and zero changes to `cargo xtask stages`), while `planet generate --pipeline-json OUT` embeds `"catalog"` directly alongside `"base_node_hash"`, `"definition_hash"`, `"erosion_mode"`, `"active_stages"`, and `"walked_chain"`.
  - `every_spec_id_in_catalog_resolves_to_a_live_requirement_in_openspec_specs` fails if any cited ID disappears from `openspec/specs/*/spec.md`.
  - `the_declared_runtime_chain_matches_the_walked_height_field_upstream_chain` instantiates `field_chain_erosion` across all valid M9 layer, K5-K7 commit, stamp, and water-cull configurations and asserts exact equality between `resolve_runtime_chain` and walking `HeightField::upstream()`.

## 3. Consumer in PR 2 (`BUILD-064`, `cargo xtask cockpit`)
`cargo xtask cockpit` prints self-tested living CLI documentation via `-h`/`--help` (and embeds the same guide inside the `CLI Help` modal of `index.html`), invokes `planet generate --pipeline-json` (or reads `--pipeline-json FILE` offline without taking a heavy slot when `--run` is unset), parses `docs/tools/stages/manifest.txt`, indexes PNG `IHDR` dimensions and `.txt` metric files across `target/review/cockpit/previews/`, `target/preview/`, `target/review/stages/`, `docs/tools/stages/sheets/`, and `--chain DIR`, and writes a self-contained single-file HTML report (`target/review/cockpit/index.html`) purely with `std` Rust (no `serde` or external network assets), offering three focused tabs:
1. **Pipeline DAG & Switch Matrix**: Renders the catalog stages by `K1`..`K11` / `M9-P1`..`M9-P2` and `global`/`region`/`tile` scope, highlights active paths per `HeightFieldKind` and `--erosion`/`--channels`/`--commit`/stamp switches, and visualizes `base_node_hash` + `definition_hash`.
2. **Global Authoring & Step Inspector**: Indexes existing preview outputs (and triggers `preview world`, `--{shelf,lithology,uplift,climate,arid,ice}-preview`, `stages`, `transect_sheet` under `--run`), pairing generated PNGs with their `.txt` metrics and side-by-side comparison.
3. **Version & Cache-Key Pin Ledger**: Displays all 42 live `*_VERSION` constants alongside `docs/tools/stages/manifest.txt` probe hashes.

## 4. Constitutional & Layering Guarantees
- **Non-authoritative (`CON-06`, `CON-07`)**: `pipeline_catalog` never feeds into generator math, sample bits, or cache keys; `target/review/stages/pipeline.json` and `--pipeline-json` outputs are strictly derived inspection artifacts.
- **Crate isolation (`CON-02`)**: `xtask` stays `std`-only; all generator knowledge remains inside `planet-generators` (`BUILD-063`) and `planet` CLI (`--pipeline-json`).

## 5. Review Incorporation Record (`opus-architect`, PR #1615)
All review points from `opus-architect` on `e8f19b8` were incorporated:
1. Derived bake hooks directly from `bake::probe::Stage` (`stage_id_for_probe`) matched 1-to-1 against `docs/tools/stages/manifest.txt`, split M9 vs. K5-K7 (`K1`..`K11`, `M9-P1`/`M9-P2`), exposed K5 `window_cut`, and omitted `spike/rg-prototype` placeholders.
2. Audited and enforced live `openspec/specs/` resolution for every `spec_ids` entry via a Tier-A test.
3. Dropped committed `docs/tools/stages/pipeline.json` and `xtask/src/stages.rs` edits (Option 1), added `k_label` + `scope` + `base_node_hash` + `--commit DIR` + embedded `catalog`, and kept look files excluded (`LOOK-001`).
