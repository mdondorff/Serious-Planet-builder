//! `cargo xtask bake-region` (BUILD-078, WDEF-019): bakes the boxes of a world's `regions.toml` with the region bake (GEN-177) over the production base of the world's definition and its items
//! (CON-05), each into `<regions-parent>/regions/<name>/`, and prints the one editor command that opens every ready region of the file (VIEW-077). A region that is `ready` is skipped unless `--force`.
//!
//! Usage: see `planet_cache::regions_file::BAKE_USAGE`.

use std::time::Instant;

use planet_cache::regions_file::{
    bake_entry_progress, bake_plan, commit_dir, region_inputs_id, BakeArgs, BakeProgress, BakeStep, RegionLayout, RegionsFile,
};

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
            println!("region {}: ready, skipped (--force bakes it again)", entry.name);
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
        // At most one progress line every 10 s (BUILD-078), and the time of each step at the end.
        let mut paced = BakeProgress::new(&entry.name);
        let (mut front_at, mut tiles_at) = (None, None);
        let (commit, report) = bake_entry_progress(&def, &entry, &layout, &dir, threads, &mut |step| {
            let t = t0.elapsed().as_secs_f64();
            match step {
                BakeStep::Front => front_at = Some(t),
                BakeStep::Tiles { done: 0, .. } => tiles_at = Some(t),
                _ => {}
            }
            if let Some(line) = paced.step(t, step) {
                println!("{line}");
            }
        })
        .unwrap_or_else(|e| fail(&format!("region {}: {e}", entry.name)));
        let end = t0.elapsed().as_secs_f64();
        let (f, l) = (front_at.unwrap_or(0.0), tiles_at.unwrap_or(end));
        println!("region {}: steps base lattice {:.1} s, solve and hydrology {:.1} s, L tiles {:.1} s", entry.name, f, l - f, end - l);
        println!(
            "region {}: ready, commit {:016x}, {} L tiles, {} channel cells, {} lakes ({:.1} s), in {}",
            entry.name,
            commit.id,
            report.tiles,
            report.channel_cells,
            report.lakes,
            t0.elapsed().as_secs_f64(),
            dir.display()
        );
    }
    // One line for the run: the editor opens every ready region of the file (VIEW-077).
    println!("open: {}", a.open_command());
}
