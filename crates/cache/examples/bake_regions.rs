//! `cargo xtask bake-region` (BUILD-078, WDEF-019): bakes the boxes of a world's `regions.toml` with the region bake (GEN-177) over the production base of the world's definition and its items
//! (CON-05), each into `<regions-parent>/regions/<name>/`, and prints the editor command that opens each commit (VIEW-069). A region that is `ready` is skipped unless `--force`.
//!
//! Usage: see `planet_cache::regions_file::BAKE_USAGE`.

use std::time::Instant;

use planet_cache::regions_file::{bake_entry, bake_plan, commit_dir, region_inputs_id, BakeArgs, RegionLayout, RegionsFile};

fn fail(e: &str) -> ! {
    eprintln!("bake-region: {e}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = BakeArgs::parse(&args).unwrap_or_else(|e| fail(&e));
    let (threads, note) = planet_streaming::topology::bake_threads(a.threads).unwrap_or_else(|e| fail(&e));
    let (def, seed, _radius) = a.definition().unwrap_or_else(|e| fail(&e));
    let items = def.item_ids(planet_world_def::BodyId::PLANET).len();
    if items > 0 {
        println!("bake-region: {items} items in the definition: they enter the commits' identity, not the base heights (the base reads no items yet, WDEF-019)");
    }
    let file = RegionsFile::read(&a.regions).unwrap_or_else(|e| fail(&e));
    let layout = RegionLayout::PRODUCTION;
    let plan = bake_plan(&file, &a, &layout, region_inputs_id(&def, seed)).unwrap_or_else(|e| fail(&e));
    println!("bake-region: {} ({} regions, seed {seed}, threads {note})", a.regions.display(), plan.len());
    for (entry, state, bake) in plan {
        let dir = commit_dir(&a.regions, &entry.name);
        if !bake {
            println!("region {}: ready, skipped (--force bakes it again)\n  open: {}", entry.name, a.open_command(&dir));
            continue;
        }
        let t0 = Instant::now();
        println!(
            "region {}: {} -> baking face {} tiles ({}, {})..=({}, {}) into {}",
            entry.name,
            state.id(),
            entry.face,
            entry.i0,
            entry.j0,
            entry.i1,
            entry.j1,
            dir.display()
        );
        let (commit, report) =
            bake_entry(&def, &entry, &layout, &dir, threads).unwrap_or_else(|e| fail(&format!("region {}: {e}", entry.name)));
        println!(
            "region {}: ready, commit {:016x}, {} L tiles, {} channel cells, {} lakes ({:.1} s)\n  open: {}",
            entry.name,
            commit.id,
            report.tiles,
            report.channel_cells,
            report.lakes,
            t0.elapsed().as_secs_f64(),
            a.open_command(&dir)
        );
    }
}
