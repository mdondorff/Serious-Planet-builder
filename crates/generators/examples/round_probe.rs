//! GEN-141 probe: how many cells of a catchment routed alone differ from the same catchment inside the region, per rule set; and the bits of the round solver against the region carve.
use planet_generators::bake::carve::{carve, CarveParams, Schedule};
use planet_generators::bake::route::{catchment_roots, route_rules, MfdClip, RoundRules};
use planet_generators::bake::Grid;

fn terrain(n: usize, salt: u64, terraced: bool) -> Vec<f64> {
    let mut z = vec![0.0; n * n];
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (i as f64, j as f64);
            let h = planet_generators::bake::route::jitter(salt, (j * n + i) as u64, 0);
            let edge = (i.min(j).min(n - 1 - i).min(n - 1 - j) as f64 / 12.0).min(1.0);
            z[j * n + i] = (350.0
                + 500.0 * libm::sin(x * 0.13 + salt as f64) * libm::cos(y * 0.11)
                + 250.0 * libm::sin(0.37 * x + 0.21 * y)
                + 30.0 * h)
                * edge
                + 2.0;
            if terraced {
                z[j * n + i] = libm::floor(z[j * n + i] / 60.0) * 60.0 + 2.0 + 0.3 * h.abs();
            }
        }
    }
    z
}

fn main() {
    if std::env::args().any(|a| a == "diag") {
        diag();
        return;
    }
    let n = 96;
    for terraced in [false, true] {
        println!("terraced {terraced}");
        for (name, rules) in [
            ("off", RoundRules::default()),
            ("canonical", RoundRules { canonical_flood: true, ..Default::default() }),
            ("clip-local", RoundRules { clip_mfd: MfdClip::Local, canonical_flood: false }),
            ("clip-to-receiver+canon", RoundRules { clip_mfd: MfdClip::ToReceiver, canonical_flood: true }),
            ("round (local+canon)", RoundRules::ROUND),
        ] {
            let (mut cells, mut recv_diff, mut area_diff, mut catch) = (0usize, 0usize, 0usize, 0usize);
            for salt in 1..=4u64 {
                let g = Grid::new(n, n, 800.0);
                let z = terrain(n, salt, terraced);
                let outlet = g.outlets(&z, 0.0);
                let r = route_rules(&g, &z, &outlet, 1e-3, salt, 0.0, None, rules);
                let root = catchment_roots(&r.receiver, &r.order);
                let mut roots: Vec<u32> = root.iter().copied().collect();
                roots.sort_unstable();
                roots.dedup();
                for &rt in roots.iter().filter(|&&rt| root.iter().filter(|&&x| x == rt).count() > 8) {
                    let active: Vec<bool> = root.iter().map(|&x| x == rt).collect();
                    let ga = g.clone().with_active(active.clone());
                    let mut out = vec![false; n * n];
                    out[rt as usize] = true;
                    let a = route_rules(&ga, &z, &out, 1e-3, salt, 0.0, None, rules);
                    catch += 1;
                    for p in 0..n * n {
                        if active[p] {
                            cells += 1;
                            recv_diff += usize::from(a.receiver[p] != r.receiver[p]);
                            area_diff += usize::from(a.area[p].to_bits() != r.area[p].to_bits());
                        }
                    }
                }
            }
            println!(
                "alone  {name:<24} catchments {catch:>4} cells {cells:>7} receivers differ {recv_diff:>6} areas differ {area_diff:>6}"
            );
        }
    }
    for (name, rules) in [("off", RoundRules::default()), ("round", RoundRules::ROUND)] {
        let g = Grid::new(n, n, 800.0);
        let z = terrain(n, 3, true);
        let outlet = g.outlets(&z, 0.0);
        let run = |s: Schedule| {
            let mut zz = z.clone();
            carve(&g, &mut zz, &outlet, &CarveParams { steps: 20, round_rules: rules, schedule: s, ..Default::default() }, None, 3);
            zz.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        };
        let base = run(Schedule::Region);
        let fwd = run(Schedule::Catchments { descending: false });
        let rev = run(Schedule::Catchments { descending: true });
        let d = |a: &[u64]| a.iter().zip(&base).filter(|(x, y)| x != y).count();
        println!("round  {name:<8} cells differ: catchments forward {} reverse {}", d(&fwd), d(&rev));
    }
}

#[allow(dead_code)]
pub fn diag() {
    use planet_generators::bake::flood::priority_flood_with;
    let n = 96;
    for salt in 1..=4u64 {
        let g = Grid::new(n, n, 800.0);
        let z = terrain(n, salt, true);
        let outlet = g.outlets(&z, 0.0);
        let r = route_rules(&g, &z, &outlet, 1e-3, salt, 0.0, None, RoundRules::ROUND);
        let fr = priority_flood_with(&g, &z, &outlet, 1e-3, true);
        let root = catchment_roots(&r.receiver, &r.order);
        let mut roots: Vec<u32> = root.clone();
        roots.sort_unstable();
        roots.dedup();
        for &rt in &roots {
            let active: Vec<bool> = root.iter().map(|&x| x == rt).collect();
            if active.iter().filter(|&&a| a).count() <= 8 {
                continue;
            }
            let ga = g.clone().with_active(active.clone());
            let mut out = vec![false; n * n];
            out[rt as usize] = true;
            let a = route_rules(&ga, &z, &out, 1e-3, salt, 0.0, None, RoundRules::ROUND);
            let fa = priority_flood_with(&ga, &z, &out, 1e-3, true);
            for p in 0..n * n {
                if active[p] && (a.receiver[p] != r.receiver[p] || fr[p] != fa[p]) {
                    println!(
                        "salt {salt} root {rt} cell {p} z {:.4} filled region {:.4} alone {:.4} recv region {} alone {}",
                        z[p], fr[p], fa[p], r.receiver[p], a.receiver[p]
                    );
                    for d in 0..8 {
                        if let Some(q) = g.neighbour(p, d) {
                            println!("    nb {q} root {} z {:.3} filled {:.4}", root[q], z[q], fr[q]);
                        }
                    }
                }
            }
        }
    }
}
