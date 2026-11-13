//! Read-only probe (#125): the rises along the stored channel lines of a commit. For every tile of the commit's region, every channel of the tile's feed, every line cell inside the tile's core:
//! the composed height at the cell centre against the line cell before it. Prints the rises over 0.25 m with the tile, the cell, the step index and the height of the bed level there.
//! `cargo run --release -p planet --example k7_rises -- <commit dir> <out file>`
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::Face;
use planet_generators::bake::k7::line_cells;
use planet_frame::river::RiverSource;
use std::io::Write;

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    let (commit, out) = (&a[1], &a[2]);
    let def = planet_world_def::WorldDefinition::single_planet(6_371_000.0, 14);
    let world = planet::worldfield::WorldTerrain::open_commit_in_memory(def, std::path::Path::new(commit), 8)?;
    let field = world.field()?;
    let region = world.commit_region().ok_or("no commit region")?.clone();
    let rivers = world.rivers().ok_or("no rivers")?.clone();
    let grid = rivers.grid();
    let n = f64::from(grid.cells);
    let tile = i64::from(grid.tile);
    let h = |face: Face, x: i64, y: i64| {
        field.height(face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0))
    };
    let mut f = std::fs::File::create(out).map_err(|e| e.to_string())?;
    writeln!(f, "# face ti tj channel step x y prev_m here_m rise_m steps_from_start line_len up_min_m (lowest height so far on the line) edge_dist").unwrap();
    let (mut rises, mut cells, mut big) = (0usize, 0usize, 0usize);
    let mut worst: Vec<f64> = Vec::new();
    for &(fc, ti, tj) in &region.tiles {
        let face = Face(fc);
        let chans = rivers.tile_channels(face, ti, tj)?;
        let (x0, y0) = (i64::from(ti) * tile, i64::from(tj) * tile);
        let core = |x: i64, y: i64| x >= x0 && y >= y0 && x < x0 + tile && y < y0 + tile;
        if a.len() == 5 && a[3].parse::<u32>().ok() == Some(ti) && a[4].parse::<u32>().ok() == Some(tj) {
            // The offset search: rises over 0.25 m along the lines of this tile shifted by (dx, dy) cells.
            let mut res: Vec<(usize, i64, i64, usize)> = Vec::new();
            for dy in -12..=12i64 {
                for dx in -12..=12i64 {
                    let (mut r, mut c) = (0, 0);
                    for ch in chans.iter() {
                        let line = line_cells(ch);
                        let mut prev: Option<f64> = None;
                        for &(x, y) in &line {
                            let v = h(face, x + dx, y + dy);
                            if let Some(p) = prev {
                                if core(x, y) && v > 0.0 && p > 0.0 {
                                    c += 1;
                                    r += usize::from(v - p > 0.25);
                                }
                            }
                            prev = Some(v);
                        }
                    }
                    res.push((r, dx, dy, c));
                }
            }
            res.sort();
            for (r, dx, dy, c) in res.iter().take(8) {
                println!("offset {dx} {dy}: {r} rises of {c}");
            }
            println!("offset 0 0: {:?}", res.iter().find(|t| t.1 == 0 && t.2 == 0));
            return Ok(());
        }
        if a.len() > 5 && a[3].parse::<u32>().ok() == Some(ti) && a[4].parse::<u32>().ok() == Some(tj) {
            let ch = &chans[a[5].parse::<usize>().unwrap()];
            for (k, p) in ch.points.iter().enumerate() {
                println!("point {k} {:.1} {:.1} level {:.2} q {:.1}", p.0, p.1, ch.levels[k], ch.q.get(k).copied().unwrap_or(f64::NAN));
            }
            for (k, &(x, y)) in line_cells(ch).iter().enumerate() {
                let mut lo = f64::INFINITY;
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        lo = lo.min(h(face, x + dx, y + dy));
                    }
                }
                println!("cell {k} {x} {y} field {:.2} min5x5 {:.2}", h(face, x, y), lo);
            }
            return Ok(());
        }
        for (ci, ch) in chans.iter().enumerate() {
            let line = line_cells(ch);
            let hs: Vec<f64> = line.iter().map(|&(x, y)| h(face, x, y)).collect();
            let mut lo = f64::INFINITY;
            for k in 0..line.len() {
                lo = lo.min(hs[k]);
                let (x, y) = line[k];
                if k == 0 || !core(x, y) || hs[k] <= 0.0 {
                    continue;
                }
                cells += 1;
                let rise = hs[k] - hs[k - 1];
                if rise > 0.25 {
                    rises += 1;
                    worst.push(rise);
                    big += usize::from(rise > 5.0);
                    let edge = (x - x0).min(y - y0).min(x0 + tile - 1 - x).min(y0 + tile - 1 - y);
                    writeln!(
                        f,
                        "{fc} {ti} {tj} {ci} {k} {x} {y} {:.2} {:.2} {rise:.2} {k} {} {lo:.2} {edge}",
                        hs[k - 1],
                        hs[k],
                        line.len()
                    )
                    .unwrap();
                }
            }
        }
    }
    worst.sort_by(f64::total_cmp);
    let p = |q: f64| worst.get(((worst.len() as f64 * q) as usize).min(worst.len().saturating_sub(1))).copied().unwrap_or(0.0);
    let line = format!(
        "line cells in cores {cells}; rises over 0.25 m {rises} (over 5 m {big}); rise p50 {:.2} p99 {:.2} max {:.2} m",
        p(0.5),
        p(0.99),
        worst.last().copied().unwrap_or(0.0)
    );
    writeln!(f, "# {line}").unwrap();
    println!("{line}");
    Ok(())
}
