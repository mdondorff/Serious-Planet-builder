//! Lead #162 (spike branch only, never merged): where R1h's uphill river steps and its graph/raster agreement misses lie, by class, from the river gate's own routing (`CRUSADER_DIAG_DIR`
//! dumps of `river_gate`), the final surface, the canonical graph and the stage fields of explorer's chain run.
//!
//! `g1f_diag --surface F --s0 F --graph F --stages DIR --diag DIR --origin X0,Y0 --dx M`
//! - uphill: a river cell inside (gate area >= 75 km2, land) whose river receiver lies more than 0.25 m higher on the surface (BUILD-057's rule);
//! - agreement miss: a routed river cell inside (area >= the threshold, land) with no rasterised graph cell within one cell (BUILD-060's rule), at 1 and 75 km2.
use std::collections::HashMap;

use planet_cache::river_gate_graph::{budget_field, read_stages};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::river_graph::{rasterise, Graph, NodeKind};
use planet_generators::bake::Grid;

fn value<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn f64s(path: &str) -> Result<Vec<f64>, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(b.chunks_exact(8).map(|x| f64::from_le_bytes(x.try_into().unwrap())).collect())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let need = |k: &str| value(&args, k).ok_or(format!("{k} is required"));
    let z = f64s(need("--surface")?)?;
    let s0 = f64s(need("--s0")?)?;
    let n = z.len();
    let w = (n as f64).sqrt() as usize;
    let h = n / w;
    if w * h != n || s0.len() != n {
        return Err(format!("a square window: {n} cells, s0 {}", s0.len()));
    }
    let dx: f64 = need("--dx")?.parse().map_err(|_| "--dx")?;
    let org: Vec<u64> = need("--origin")?.split(',').map(|x| x.parse().unwrap()).collect();
    let (x0, y0) = (org[0], org[1]);
    let diag = need("--diag")?;
    let rd = |name: &str| std::fs::read(format!("{diag}/{name}")).map_err(|e| format!("{diag}/{name}: {e}"));
    let area: Vec<f64> = rd("gate-area.f32")?.chunks_exact(4).map(|x| f64::from(f32::from_le_bytes(x.try_into().unwrap()))).collect();
    let receiver: Vec<u32> = rd("gate-receiver.u32")?.chunks_exact(4).map(|x| u32::from_le_bytes(x.try_into().unwrap())).collect();
    let inside: Vec<bool> = rd("gate-inside.u8")?.iter().map(|&x| x != 0).collect();
    if area.len() != n || receiver.len() != n || inside.len() != n {
        return Err("the gate's dumps do not fit the surface".into());
    }
    let g = Graph::parse(&std::fs::read_to_string(need("--graph")?).map_err(|e| e.to_string())?)?;
    let (stages, _) = read_stages(std::path::Path::new(need("--stages")?), n)?;
    let local = |id: u64| -> Option<usize> {
        let (gx, gy) = (id & 0xFFFF_FFFF, id >> 32);
        (gx >= x0 && gy >= y0 && gx < x0 + w as u64 && gy < y0 + h as u64).then(|| (gy - y0) as usize * w + (gx - x0) as usize)
    };
    // the graph's raster (trunk reaches apart) and node anchors by kind, each dilated by one cell
    let mut on_graph = vec![false; n];
    let mut trunk_cell = vec![false; n];
    let reach_parent: HashMap<u64, bool> = g.reaches.iter().map(|r| (r.id, r.parent.is_some())).collect();
    for (&cell, &(rid, _)) in &rasterise(&g) {
        if let Some(c) = local(cell) {
            on_graph[c] = true;
            trunk_cell[c] = reach_parent.get(&rid).copied().unwrap_or(false);
        }
    }
    let dilate = |m: &[bool], k: i64| -> Vec<bool> {
        let mut o = vec![false; n];
        for c in (0..n).filter(|&c| m[c]) {
            let (x, y) = ((c % w) as i64, (c / w) as i64);
            for dy in -k..=k {
                for dxx in -k..=k {
                    let (a, b) = (x + dxx, y + dy);
                    if a >= 0 && b >= 0 && a < w as i64 && b < h as i64 {
                        o[b as usize * w + a as usize] = true;
                    }
                }
            }
        }
        o
    };
    let mut near_kind: HashMap<&'static str, Vec<bool>> = HashMap::new();
    for (label, kinds) in [
        ("exception", vec![NodeKind::Exception]),
        ("lake_inlet", vec![NodeKind::LakeInlet, NodeKind::LakeSpill]),
        ("junction", vec![NodeKind::TribJunction, NodeKind::Confluence]),
        ("tile_crossing", vec![NodeKind::TileCrossing]),
    ] {
        let mut m = vec![false; n];
        for nd in g.nodes.iter().filter(|x| kinds.contains(&x.kind)) {
            if let Some(c) = local(nd.anchor) {
                m[c] = true;
            }
        }
        near_kind.insert(label, dilate(&m, 1));
    }
    // the budget used: the stages' lowering against the budget of S0 (G0 contracts section 6; within one 0.1 m step)
    let budget = budget_field(&s0, w, h, 8, dx, 25_000.0);
    let lower_m = |c: usize| -> f64 { stages.iter().map(|s| f64::from(s.lower[c]) * 0.1).sum() };
    let full = |c: usize| budget.len() == n && lower_m(c) >= budget[c] - 0.1;
    let land = |c: usize| z[c] > 0.0;
    println!(
        "DIAG inputs: {w} x {h} cells, budget field {} values, stages {:?}",
        budget.len(),
        stages.iter().map(|s| s.name.clone()).collect::<Vec<_>>()
    );
    let filled = {
        let outlet: Vec<bool> = (0..n).map(|c| !land(c) || c % w == 0 || c / w == 0 || c % w == w - 1 || c / w == h - 1).collect();
        priority_flood_with(&Grid::new(w, h, dx), &z, &outlet, 1e-3, false)
    };
    let g3 = dilate(&on_graph, 3);
    let kc_near: Vec<bool> = match stages.iter().find(|s| s.name.starts_with("kc")) {
        Some(k) => dilate(&k.lower.iter().map(|&v| v > 0).collect::<Vec<_>>(), 1),
        None => vec![false; n],
    };
    let cause = |c: usize, t: f64| -> &'static str {
        if filled[c] - z[c] > 0.1 {
            "lake_interior"
        } else if kc_near[c] {
            "kc_breach"
        } else if area[c] < 2.0 * t {
            "near_threshold"
        } else if g3[c] {
            "graph_offset_2_3"
        } else {
            "other"
        }
    };
    let mut off_cause: HashMap<&str, usize> = HashMap::new();
    let mut lake_sub: HashMap<&str, (usize, Vec<f64>)> = HashMap::new();
    let kc_fill: Option<&Vec<u16>> = stages.iter().find(|s| s.name.starts_with("kc")).and_then(|s| s.fill.as_ref());
    // ---- uphill steps on 75 km2 rivers
    let river = |c: usize| area[c] >= 75.0e6 && land(c);
    let order = ["exception", "lake_inlet", "junction", "tile_crossing", "budget_full", "on_graph_trunk", "on_graph_trib", "off_graph"];
    let mut primary: HashMap<&str, (usize, Vec<f64>)> = HashMap::new();
    let mut labels: HashMap<&str, usize> = HashMap::new();
    let (mut steps, mut up) = (0usize, 0usize);
    for c in (0..n).filter(|&c| inside[c] && river(c)) {
        let r = receiver[c];
        if r == u32::MAX || !river(r as usize) {
            continue;
        }
        steps += 1;
        let r = r as usize;
        let rise = z[r] - z[c];
        if rise <= 0.25 {
            continue;
        }
        up += 1;
        let mut ls: Vec<&str> = Vec::new();
        for k in ["exception", "lake_inlet", "junction", "tile_crossing"] {
            if near_kind[k][c] || near_kind[k][r] {
                ls.push(k);
            }
        }
        if full(c) || full(r) {
            ls.push("budget_full");
        }
        ls.push(if on_graph[c] || on_graph[r] {
            if trunk_cell[c] || trunk_cell[r] {
                "on_graph_trunk"
            } else {
                "on_graph_trib"
            }
        } else {
            "off_graph"
        });
        for l in &ls {
            *labels.entry(l).or_default() += 1;
        }
        let p = order.iter().find(|o| ls.contains(o)).unwrap();
        if *p == "off_graph" {
            let k = cause(c, 75.0e6);
            *off_cause.entry(k).or_default() += 1;
            if k == "lake_interior" {
                // what KC did at the pit: filled to the budget (clamped), filled less, or nothing (a protected basin, or a pit KC does not see)
                let fill = kc_fill.map_or(0.0, |f| f64::from(f[c]) * 0.1);
                let sub = if fill <= 0.0 {
                    "kc_untouched"
                } else if budget.len() == n && fill >= budget[c] - 0.15 {
                    "kc_fill_at_budget"
                } else {
                    "kc_fill_partial"
                };
                let e = lake_sub.entry(sub).or_insert((0, Vec::new()));
                e.0 += 1;
                e.1.push(filled[c] - z[c]);
            }
        }
        let e = primary.entry(p).or_insert((0, Vec::new()));
        e.0 += 1;
        e.1.push(rise);
    }
    println!("DIAG uphill (75 km2 rivers inside, rise > 0.25 m): {up} of {steps} steps ({:.2} %)", 100.0 * up as f64 / steps.max(1) as f64);
    for o in order {
        if let Some((k, v)) = primary.get_mut(o) {
            v.sort_by(f64::total_cmp);
            println!(
                "DIAG uphill primary {o:<15} {k:>6} ({:.1} %); rise p50 {:.2} p90 {:.2} max {:.2} m; label count {}",
                100.0 * *k as f64 / up.max(1) as f64,
                v[v.len() / 2],
                v[v.len() * 9 / 10],
                v[v.len() - 1],
                labels.get(o).copied().unwrap_or(0)
            );
        }
    }
    let mut oc: Vec<(&&str, &usize)> = off_cause.iter().collect();
    oc.sort();
    println!("DIAG uphill off_graph by cause: {oc:?}");
    for (k, (cnt, v)) in lake_sub.iter_mut() {
        v.sort_by(f64::total_cmp);
        println!("DIAG uphill off_graph lake_interior {k:<18} {cnt:>5}; depression depth at the step p50 {:.2} p90 {:.2} max {:.2} m", v[v.len() / 2], v[v.len() * 9 / 10], v[v.len() - 1]);
    }
    // ---- agreement misses at 1 and 75 km2
    let g1 = dilate(&on_graph, 1);
    for t in [1.0e6, 75.0e6] {
        let order = ["lake_interior", "kc_breach", "near_threshold", "graph_offset_2_3", "other"];
        let mut cnt: HashMap<&str, usize> = HashMap::new();
        let (mut routed, mut miss) = (0usize, 0usize);
        for c in (0..n).filter(|&c| inside[c] && land(c) && area[c] >= t) {
            routed += 1;
            if g1[c] {
                continue;
            }
            miss += 1;
            let k = cause(c, t);
            *cnt.entry(k).or_default() += 1;
        }
        println!(
            "DIAG agreement {} km2: {miss} of {routed} routed river cells miss the graph ({:.2} % agree)",
            t / 1.0e6,
            100.0 * (routed - miss) as f64 / routed.max(1) as f64
        );
        for o in order {
            let k = cnt.get(o).copied().unwrap_or(0);
            println!("DIAG agreement {} km2 miss {o:<17} {k:>8} ({:.1} % of misses)", t / 1.0e6, 100.0 * k as f64 / miss.max(1) as f64);
        }
    }
    Ok(())
}
