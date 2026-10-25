//! SPIKE-42 throwaway: local main-stem routing in a chunk (A) against the coarse path with a capped running-minimum bed (B).
//! `router <dump.bin> <out_dir> <tag> [threads]` prints the table rows; writes `<out_dir>/<tag>_paths.txt` (one path per line, core indices) and the ground.
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io::Read;
use std::time::Instant;

struct Chunk {
    per: usize,
    cell_m: f64,
    z: Vec<f64>,
    anchor: Vec<f64>,
    area: Vec<f64>,
    corridor: Vec<bool>,
    next: Vec<i32>,
    exits: Vec<[i32; 4]>,
    entries: Vec<[i32; 4]>,
}

fn rd(buf: &[u8], pos: &mut usize, n: usize) -> Vec<[u8; 8]> {
    let v = (0..n).map(|k| buf[*pos + 8 * k..*pos + 8 * k + 8].try_into().unwrap()).collect();
    *pos += 8 * n;
    v
}
fn rd_i32(buf: &[u8], pos: &mut usize, n: usize) -> Vec<i32> {
    let v = (0..n).map(|k| i32::from_le_bytes(buf[*pos + 4 * k..*pos + 4 * k + 4].try_into().unwrap())).collect();
    *pos += 4 * n;
    v
}

fn load(path: &str) -> Chunk {
    let mut buf = vec![];
    std::fs::File::open(path).unwrap().read_to_end(&mut buf).unwrap();
    let per = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let cell_m = f64::from_le_bytes(buf[4..12].try_into().unwrap());
    let mut pos = 12 + 12;
    let n = per * per;
    let f = |b: Vec<[u8; 8]>| b.into_iter().map(f64::from_le_bytes).collect::<Vec<f64>>();
    let z = f(rd(&buf, &mut pos, n));
    let anchor = f(rd(&buf, &mut pos, n));
    let area = f(rd(&buf, &mut pos, n));
    let corridor = rd_i32(&buf, &mut pos, n).into_iter().map(|x| x != 0).collect();
    let next = rd_i32(&buf, &mut pos, n);
    let ne = rd_i32(&buf, &mut pos, 1)[0] as usize;
    let ex = rd_i32(&buf, &mut pos, 4 * ne);
    let nn = rd_i32(&buf, &mut pos, 1)[0] as usize;
    let en = rd_i32(&buf, &mut pos, 4 * nn);
    Chunk {
        per,
        cell_m,
        z,
        anchor,
        area,
        corridor,
        next,
        exits: ex.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
        entries: en.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
    }
}

const NB: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
fn step_len(k: usize) -> i64 {
    if k < 4 {
        1000
    } else {
        1414
    }
}

fn neighbours(per: usize, c: usize) -> impl Iterator<Item = (usize, usize)> {
    let (i, j) = ((c % per) as i64, (c / per) as i64);
    NB.iter().enumerate().filter_map(move |(k, (di, dj))| {
        let (a, b) = (i + di, j + dj);
        (a >= 0 && b >= 0 && (a as usize) < per && (b as usize) < per).then_some((b as usize * per + a as usize, k))
    })
}

/// Distance (octile, in 1/1000 cells) from the nearest guide cell, limited to `limit`.
fn band(per: usize, guide: &[usize], limit: i64) -> Vec<i64> {
    let mut d = vec![i64::MAX; per * per];
    let mut h = BinaryHeap::new();
    for &g in guide {
        d[g] = 0;
        h.push(Reverse((0i64, g)));
    }
    while let Some(Reverse((dc, c))) = h.pop() {
        if dc > d[c] {
            continue;
        }
        for (n, k) in neighbours(per, c) {
            let nd = dc + step_len(k);
            if nd <= limit && nd < d[n] {
                d[n] = nd;
                h.push(Reverse((nd, n)));
            }
        }
    }
    d
}

/// Least-cost path from `s` to `e` inside the band; cost of a step = rise (only positive) in 1/1000 m + lambda * length (1/1000 m per m).
fn route(ch: &Chunk, inband: &[i64], s: usize, e: usize, lambda: f64) -> Vec<usize> {
    let per = ch.per;
    let mut d = vec![i64::MAX; per * per];
    let mut from = vec![u32::MAX; per * per];
    let mut h = BinaryHeap::new();
    d[s] = 0;
    h.push(Reverse((0i64, s)));
    while let Some(Reverse((dc, c))) = h.pop() {
        if dc > d[c] {
            continue;
        }
        if c == e {
            break;
        }
        for (n, k) in neighbours(per, c) {
            if inband[n] == i64::MAX {
                continue;
            }
            let rise = ((ch.z[n] - ch.z[c]).max(0.0) * 1000.0) as i64;
            let len = (step_len(k) as f64 * ch.cell_m / 1000.0 * lambda * 1000.0) as i64;
            let nd = dc + rise + len;
            if nd < d[n] {
                d[n] = nd;
                from[n] = c as u32;
                h.push(Reverse((nd, n)));
            }
        }
    }
    let mut p = vec![e];
    while *p.last().unwrap() != s {
        let f = from[*p.last().unwrap()];
        if f == u32::MAX {
            return vec![];
        }
        p.push(f as usize);
    }
    p.reverse();
    p
}

fn cum_len(ch: &Chunk, p: &[usize]) -> Vec<f64> {
    let mut c = vec![0.0; p.len()];
    for k in 1..p.len() {
        let (a, b) = (p[k - 1], p[k]);
        let diag = (a % ch.per != b % ch.per) && (a / ch.per != b / ch.per);
        c[k] = c[k - 1] + ch.cell_m * if diag { std::f64::consts::SQRT_2 } else { 1.0 };
    }
    c
}

fn pos(ch: &Chunk, c: usize) -> (f64, f64) {
    ((c % ch.per) as f64 * ch.cell_m, (c / ch.per) as f64 * ch.cell_m)
}

fn windows(ch: &Chunk, p: &[usize], win: f64) -> (f64, f64, usize) {
    let cum = cum_len(ch, p);
    let mut v = vec![];
    for k in 0..p.len() {
        if let Some(j) = (k..p.len()).find(|&j| cum[j] - cum[k] >= win) {
            let (a, b) = (pos(ch, p[k]), pos(ch, p[j]));
            let chord = (a.0 - b.0).hypot(a.1 - b.1);
            if chord > 0.0 {
                v.push((cum[j] - cum[k]) / chord);
            }
        }
    }
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        return (f64::NAN, f64::NAN, 0);
    }
    (v[v.len() / 2], v[v.len() * 9 / 10], v.len())
}

/// Bed from the ground along the path: running minimum from upstream, limited to `cap` metres under the ground (cap = INF: uncapped).
fn bed(ch: &Chunk, p: &[usize], cap: f64) -> Vec<f64> {
    let mut b: Vec<f64> = Vec::with_capacity(p.len());
    for (k, &c) in p.iter().enumerate() {
        let g = ch.z[c];
        let run = if k == 0 { g } else { b[k - 1].min(g) };
        b.push(run.max(g - cap));
    }
    b
}

struct Row {
    name: String,
    n: usize,
    length: f64,
    sin1: (f64, f64),
    sin4: (f64, f64),
    g_climb: (usize, f64, f64),
    cut: (f64, f64, f64, f64, f64),
    capped: Vec<(f64, usize, f64, f64)>,
    trib: [(usize, f64, f64); 2],
    ms: f64,
    hash: u64,
}

fn climbs(v: &[f64], tol: f64) -> (usize, f64, f64) {
    let (mut n, mut tot, mut mx) = (0, 0.0, 0.0f64);
    for w in v.windows(2) {
        let r = w[1] - w[0];
        if r > tol {
            n += 1;
            tot += r;
            mx = mx.max(r);
        }
    }
    (n, tot, mx)
}

/// Fine tributaries: priority flood from the stem (at its bed) and the core border (at the ground); count side entries to the stem with at least `thr` m2.
fn tributaries(ch: &Chunk, stem: &[usize], bed: &[f64], thr: &[f64; 2]) -> [(usize, f64, f64); 2] {
    let per = ch.per;
    let n = per * per;
    let mut level = vec![f64::INFINITY; n];
    let mut done = vec![false; n];
    let mut rec = vec![u32::MAX; n];
    let mut h: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
    let key = |z: f64| ((z + 1000.0) * 1.0e6) as i64;
    let mut on_stem = vec![false; n];
    for (k, &c) in stem.iter().enumerate() {
        level[c] = bed[k];
        on_stem[c] = true;
        h.push(Reverse((key(bed[k]), c)));
    }
    for c in 0..n {
        let (i, j) = (c % per, c / per);
        if (i == 0 || j == 0 || i == per - 1 || j == per - 1) && !on_stem[c] {
            level[c] = ch.z[c];
            h.push(Reverse((key(ch.z[c]), c)));
        }
    }
    let mut order = vec![];
    while let Some(Reverse((_, c))) = h.pop() {
        if done[c] {
            continue;
        }
        done[c] = true;
        order.push(c);
        for (nb, _) in neighbours(per, c) {
            if done[nb] || level[nb] != f64::INFINITY {
                continue;
            }
            level[nb] = ch.z[nb].max(level[c] + 1e-6);
            rec[nb] = c as u32;
            h.push(Reverse((key(level[nb]), nb)));
        }
    }
    let mut acc = vec![ch.cell_m * ch.cell_m; n];
    for &c in order.iter().rev() {
        if rec[c] != u32::MAX {
            acc[rec[c] as usize] += acc[c];
        }
    }
    let km2 = n as f64 * ch.cell_m * ch.cell_m / 1e6;
    let mut out = [(0, 0.0, 0.0); 2];
    for (t, &th) in thr.iter().enumerate() {
        let joins = (0..n).filter(|&c| !on_stem[c] && acc[c] >= th && rec[c] != u32::MAX && on_stem[rec[c] as usize]).count();
        let len: f64 = (0..n).filter(|&c| !on_stem[c] && acc[c] >= th).count() as f64 * ch.cell_m;
        out[t] = (joins, joins as f64 / km2, len / 1000.0 / km2);
    }
    out
}

fn hash(p: &[usize]) -> u64 {
    p.iter().fold(0xcbf29ce484222325u64, |h, &c| (h ^ c as u64).wrapping_mul(0x100000001b3))
}

fn analyse(ch: &Chunk, name: &str, p: &[usize], t0: Instant) -> Row {
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let g: Vec<f64> = p.iter().map(|&c| ch.z[c]).collect();
    let b = bed(ch, p, f64::INFINITY);
    let cum = cum_len(ch, p);
    let cuts: Vec<f64> = g.iter().zip(&b).map(|(g, b)| g - b).collect();
    let step = |k: usize| if k + 1 < p.len() { cum[k + 1] - cum[k] } else { 0.0 };
    let len_over = |th: f64| (0..p.len()).filter(|&k| cuts[k] > th).map(step).sum::<f64>();
    let cut = (
        cuts.iter().cloned().fold(0.0, f64::max),
        cuts.iter().sum::<f64>() / cuts.len() as f64,
        len_over(1.0),
        len_over(5.0),
        len_over(10.0),
    );
    let mut capped = vec![];
    for cap in [10.0, 30.0] {
        let bc = bed(ch, p, cap);
        let (n, _, mx) = climbs(&bc, 0.05);
        let at_cap = (0..p.len()).filter(|&k| (g[k] - bc[k]) >= cap - 1e-9).map(step).sum::<f64>();
        capped.push((cap, n, mx, at_cap));
    }
    let tb = tributaries(ch, p, &b, &[1e5, 1e6]);
    Row {
        name: name.to_string(),
        n: p.len(),
        length: *cum.last().unwrap(),
        sin1: { let w = windows(ch, p, 1000.0); (w.0, w.1) },
        sin4: { let w = windows(ch, p, 4000.0); (w.0, w.1) },
        g_climb: climbs(&g, 0.05),
        cut,
        capped,
        trib: tb,
        ms,
        hash: hash(p),
    }
}

fn print_row(r: &Row) {
    println!(
        "{:<22} cells {:>4} len {:>6.0} m | sin1km p50 {:.2} p90 {:.2} sin4km p50 {:.2} p90 {:.2} | ground climbs {:>3} (sum {:>6.1}, max {:>5.1} m) | uncapped cut max {:>5.1} mean {:>5.1} m, len>1m {:>5.0} >5m {:>5.0} >10m {:>5.0} | cap10: bed climbs {:>3} (max {:>4.1} m) at-cap {:>5.0} m | cap30: bed climbs {:>3} (max {:>4.1}) at-cap {:>5.0} m | trib 1e5: {} ({:.2}/km2, {:.2} km/km2) 1e6: {} ({:.2}/km2) | {:.0} ms | {:016x}",
        r.name, r.n, r.length, r.sin1.0, r.sin1.1, r.sin4.0, r.sin4.1, r.g_climb.0, r.g_climb.1, r.g_climb.2, r.cut.0, r.cut.1, r.cut.2, r.cut.3, r.cut.4,
        r.capped[0].1, r.capped[0].2, r.capped[0].3, r.capped[1].1, r.capped[1].2, r.capped[1].3, r.trib[0].0, r.trib[0].1, r.trib[0].2, r.trib[1].0, r.trib[1].1, r.ms, r.hash
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ch = load(&a[1]);
    let out_dir = &a[2];
    let tag = &a[3];
    let threads: usize = a.get(4).map_or(1, |s| s.parse().unwrap());
    let per = ch.per;
    // The main stem: from the entry (or head) with the largest river area along `next` to the exit.
    let corr: Vec<usize> = (0..per * per).filter(|&c| ch.corridor[c]).collect();
    let has_in: std::collections::HashSet<i32> = corr.iter().filter(|&&c| ch.next[c] >= 0).map(|&c| ch.next[c]).collect();
    let heads: Vec<usize> = corr.iter().cloned().filter(|&c| !has_in.contains(&(c as i32))).collect();
    let start = *heads.iter().max_by(|&&x, &&y| ch.area[x].total_cmp(&ch.area[y]).then(y.cmp(&x))).expect("a head");
    let mut stem = vec![start];
    while ch.next[*stem.last().unwrap()] >= 0 && stem.len() < per * per {
        stem.push(ch.next[*stem.last().unwrap()] as usize);
    }
    let (s, e) = (stem[0], *stem.last().unwrap());
    println!("# {tag}: {} corridor cells in core, {} heads, stem {} cells from {} (area {:.2e}) to {} (area {:.2e}); exits {:?} entries {:?}", corr.len(), heads.len(), stem.len(), s, ch.area[s], e, ch.area[e], ch.exits, ch.entries);
    // Today's bed (H1): climbs of the anchor along every corridor link.
    let mut links = 0;
    let mut climb_links = 0;
    for &c in &corr {
        if ch.next[c] >= 0 {
            links += 1;
            if ch.anchor[ch.next[c] as usize] > ch.anchor[c] + 0.05 {
                climb_links += 1;
            }
        }
    }
    let eff = |c: usize| ch.anchor[c].min(ch.z[c]);
    let eff_climbs = corr.iter().filter(|&&c| ch.next[c] >= 0 && eff(ch.next[c] as usize) > eff(c) + 0.05).count();
    println!("# today effective bed min(anchor, ground): climbing links {eff_climbs} of {links}; median anchor over ground {:.1} m; stem entry ground {:.1} m, exit ground {:.1} m, stem min {:.1} m", {
        let mut v: Vec<f64> = corr.iter().map(|&c| ch.anchor[c] - ch.z[c]).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }, ch.z[s], ch.z[e], stem.iter().map(|&c| ch.z[c]).fold(f64::INFINITY, f64::min));
    let ground_links = corr.iter().filter(|&&c| ch.next[c] >= 0 && ch.z[ch.next[c] as usize] > ch.z[c] + 0.05).count();
    println!("# today (H1 bed on all corridor links in core): links {links}, anchor climbs {climb_links}, ground climbs {ground_links}");
    let mut rows: Vec<Row> = vec![];
    let mut paths: Vec<(String, Vec<usize>)> = vec![];
    let t0 = Instant::now();
    paths.push(("B coarse path".to_string(), stem.clone()));
    rows.push(analyse(&ch, "B coarse path", &stem, t0));
    // Variants run on `threads` workers; the result must not depend on it.
    let variants: Vec<(f64, f64)> = [500.0, 1000.0, 2000.0].iter().flat_map(|&w| [0.005, 0.02, 0.1].iter().map(move |&l| (w, l))).collect();
    let results: Vec<(usize, String, Vec<usize>, f64)> = {
        let chr = &ch;
        let stem_r = &stem;
        let vr = &variants;
        std::thread::scope(|sc| {
            let hs: Vec<_> = (0..threads)
                .map(|t| {
                    sc.spawn(move || {
                        let mut v = vec![];
                        for (idx, &(w, l)) in vr.iter().enumerate().filter(|(i, _)| i % threads == t) {
                            let t1 = Instant::now();
                            let b = band(chr.per, stem_r, (w / chr.cell_m * 1000.0) as i64);
                            let p = route(chr, &b, s, e, l);
                            v.push((idx, format!("A W={w:.0} lam={l}"), p, t1.elapsed().as_secs_f64() * 1000.0));
                        }
                        v
                    })
                })
                .collect();
            let mut all: Vec<_> = hs.into_iter().flat_map(|h| h.join().unwrap()).collect();
            all.sort_by_key(|x| x.0);
            all
        })
    };
    for (_, name, p, ms) in results {
        let t1 = Instant::now();
        let mut r = analyse(&ch, &name, &p, t1);
        r.ms = ms;
        rows.push(r);
        paths.push((name, p));
    }
    // Gate variant: the exit may slide along its border edge to the lowest ground within +-G cells (a rule both neighbours can evaluate from the
    // halo rows; the symmetric rule is not built here, so the agreement is unverified).
    let (ei, ej) = (e % per, e / per);
    for gate_m in [500.0, 1000.0] {
        let g = (gate_m / ch.cell_m) as i64;
        let on_edge = |c: usize| {
            let (i, j) = (c % per, c / per);
            i == 0 || j == 0 || i == per - 1 || j == per - 1
        };
        if !on_edge(e) {
            continue;
        }
        let along_i = ej == 0 || ej == per - 1;
        let cands: Vec<usize> = (-g..=g)
            .filter_map(|d| {
                let (i, j) = if along_i { (ei as i64 + d, ej as i64) } else { (ei as i64, ej as i64 + d) };
                (i >= 0 && j >= 0 && (i as usize) < per && (j as usize) < per).then_some(j as usize * per + i as usize)
            })
            .collect();
        let e2 = *cands.iter().min_by(|&&x, &&y| ch.z[x].total_cmp(&ch.z[y]).then(x.cmp(&y))).unwrap();
        for (w, l) in [(1000.0, 0.02)] {
            let t1 = Instant::now();
            let mut guide = stem.clone();
            guide.extend(cands.iter().cloned());
            let b = band(per, &guide, (w / ch.cell_m * 1000.0) as i64);
            let p = route(&ch, &b, s, e2, l);
            let name = format!("A gate G={gate_m:.0} W={w:.0}");
            let mut r = analyse(&ch, &name, &p, t1);
            r.ms = t1.elapsed().as_secs_f64() * 1000.0;
            println!("# gate: exit moved {} cells along the edge, exit ground {:.1} m -> {:.1} m", (e2 as i64 - e as i64).abs().max(1), ch.z[e], ch.z[e2]);
            paths.push((name, p));
            rows.push(r);
        }
    }
    // Free head: a river that begins inside the chunk (no entry) may begin at the highest ground within W of the coarse head.
    let (si, sj) = ((s % per) as i64, (s / per) as i64);
    let on_edge_s = si == 0 || sj == 0 || si == per as i64 - 1 || sj == per as i64 - 1;
    if !on_edge_s {
        for w in [500.0, 1000.0, 2000.0] {
            let r_cells = (w / ch.cell_m) as i64;
            let mut best = s;
            for dj in -r_cells..=r_cells {
                for di in -r_cells..=r_cells {
                    let (i, j) = (si + di, sj + dj);
                    if i < 0 || j < 0 || i >= per as i64 || j >= per as i64 || di * di + dj * dj > r_cells * r_cells {
                        continue;
                    }
                    let c = j as usize * per + i as usize;
                    if ch.z[c] > ch.z[best] || (ch.z[c] == ch.z[best] && c < best) {
                        best = c;
                    }
                }
            }
            let t1 = Instant::now();
            let mut guide = stem.clone();
            guide.push(best);
            let b = band(per, &guide, (1000.0 / ch.cell_m * 1000.0) as i64);
            let p = route(&ch, &b, best, e, 0.02);
            let name = format!("A free head R={w:.0} W=1000");
            let mut r = analyse(&ch, &name, &p, t1);
            r.ms = t1.elapsed().as_secs_f64() * 1000.0;
            println!("# free head R={w:.0}: head ground {:.1} m -> {:.1} m, moved {:.0} m", ch.z[s], ch.z[best], (((best % per) as f64 - si as f64).hypot((best / per) as f64 - sj as f64)) * ch.cell_m);
            paths.push((name, p));
            rows.push(r);
        }
    }
    // A2: free head and a gate on the exit together.
    if !on_edge_s && (ei == 0 || ej == 0 || ei == per - 1 || ej == per - 1) {
        let along_i = ej == 0 || ej == per - 1;
        for (rw, gate_m) in [(1000.0, 500.0), (1000.0, 1000.0), (2000.0, 1000.0)] {
            let r_cells = (rw / ch.cell_m) as i64;
            let g = (gate_m / ch.cell_m) as i64;
            let mut best = s;
            for dj in -r_cells..=r_cells {
                for di in -r_cells..=r_cells {
                    let (i, j) = (si + di, sj + dj);
                    if i < 0 || j < 0 || i >= per as i64 || j >= per as i64 || di * di + dj * dj > r_cells * r_cells {
                        continue;
                    }
                    let c = j as usize * per + i as usize;
                    if ch.z[c] > ch.z[best] || (ch.z[c] == ch.z[best] && c < best) {
                        best = c;
                    }
                }
            }
            let cands: Vec<usize> = (-g..=g)
                .filter_map(|d| {
                    let (i, j) = if along_i { (ei as i64 + d, ej as i64) } else { (ei as i64, ej as i64 + d) };
                    (i >= 0 && j >= 0 && (i as usize) < per && (j as usize) < per).then_some(j as usize * per + i as usize)
                })
                .collect();
            let e2 = *cands.iter().min_by(|&&x, &&y| ch.z[x].total_cmp(&ch.z[y]).then(x.cmp(&y))).unwrap();
            let t1 = Instant::now();
            let mut guide = stem.clone();
            guide.push(best);
            guide.extend(cands.iter().cloned());
            let b = band(per, &guide, (1000.0 / ch.cell_m * 1000.0) as i64);
            let p = route(&ch, &b, best, e2, 0.02);
            let name = format!("A2 head R={rw:.0} gate G={gate_m:.0}");
            let mut r = analyse(&ch, &name, &p, t1);
            r.ms = t1.elapsed().as_secs_f64() * 1000.0;
            paths.push((name, p));
            rows.push(r);
        }
    }
    for r in &rows {
        print_row(r);
    }
    let all_hash = rows.iter().fold(0u64, |h, r| h.rotate_left(7) ^ r.hash);
    println!("# hash of all paths ({threads} threads): {all_hash:016x}");
    let mut f = String::new();
    for (name, p) in &paths {
        f += &format!("{name}: {}\n", p.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" "));
    }
    std::fs::write(format!("{out_dir}/{tag}_paths.txt"), f).unwrap();
    let zb: Vec<u8> = ch.z.iter().flat_map(|z| (*z as f32).to_le_bytes()).collect();
    std::fs::write(format!("{out_dir}/{tag}_ground.f32"), zb).unwrap();
}
