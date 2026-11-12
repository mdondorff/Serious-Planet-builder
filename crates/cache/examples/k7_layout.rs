//! The layout report of K7 (BUILD-052): the fixture's tile layout against an alternative, one `LAYOUT` line per set of cells. `cargo xtask k7-layout` runs it and compares it with the baseline.

fn main() {
    let threads = std::env::args().nth(1).and_then(|t| t.parse().ok()).unwrap_or(8);
    match planet_cache::region_bake::layout_fixture(threads) {
        Ok(d) => {
            println!(
                "LAYOUTS | canonical tile {} halo {} ({} land tiles) | alternative tile {} halo {} ({} land tiles)",
                d.layouts[0].0, d.layouts[0].1, d.tiles[0], d.layouts[1].0, d.layouts[1].1, d.tiles[1]
            );
            for r in &d.rows {
                println!("{}", r.line());
            }
        }
        Err(e) => {
            eprintln!("k7_layout: {e}");
            std::process::exit(1);
        }
    }
}
