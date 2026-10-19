//! SPIKE-18 (spike branch only): a per-frame detector of the drawn tiles getting coarser while the camera approaches.
//! A fixed set of probe points on the sphere; for each probe the level of the drawn node that covers it (the node whose tile is the
//! probe's tile at its level). A violation: the level drops between two frames while the camera is nearer to the probe.
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::TerrainPlan;
use std::collections::HashSet;
use std::io::Write;

pub fn enabled() -> bool {
    std::env::var("COARSEN_LOG").is_ok()
}

pub struct Detector {
    probes: Vec<Vec3>,
    prev: Vec<Option<(u8, TileId, f64)>>,
    out: std::fs::File,
    pub violations: u64,
    pub frames_with: u64,
    radius: f64,
    pub pairs: u64,
    pub finer: u64,
    pub same: u64,
    last_kind: String,
    ever: HashSet<u64>,
}

impl Detector {
    pub fn open(radius: f64) -> Option<Detector> {
        let p = std::env::var("COARSEN_LOG").ok()?;
        let n = 3000usize;
        let probes: Vec<Vec3> = (0..n)
            .map(|i| {
                let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
                let r = libm::sqrt(1.0 - z * z);
                let a = std::f64::consts::PI * (3.0 - libm::sqrt(5.0)) * i as f64;
                Vec3::new(r * libm::cos(a), r * libm::sin(a), z)
            })
            .collect();
        Some(Detector { prev: vec![None; probes.len()], probes, out: std::fs::File::create(p).ok()?, violations: 0, frames_with: 0, radius, pairs: 0, finer: 0, same: 0, last_kind: String::new(), ever: HashSet::new() })
    }

    /// Probes denser around `centre` (a ring of rings), added to the global set.
    pub fn densify(&mut self, centre: Vec3, angular_radius: f64, n: usize) {
        let helper = if centre.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let east = centre.cross(helper).normalized();
        let north = centre.cross(east).normalized();
        for i in 0..n {
            let rr = angular_radius * libm::sqrt((i as f64 + 0.5) / n as f64);
            let a = std::f64::consts::PI * (3.0 - libm::sqrt(5.0)) * i as f64;
            self.probes.push((centre + east * (rr * libm::cos(a)) + north * (rr * libm::sin(a))).normalized());
            self.prev.push(None);
        }
    }

    pub fn frame(&mut self, k: u64, kind: &str, subst: usize, blank: usize, cam: Vec3, plan: &TerrainPlan, resident: &dyn Fn(TileId) -> bool) {
        let set: HashSet<u64> = plan.nodes.iter().map(|n| n.tile.raw()).collect();
        for n in &plan.nodes {
            if let Some(p) = n.tile.parent() {
                for c in p.children().into_iter().flatten() {
                    if resident(c) {
                        self.ever.insert(c.raw());
                    }
                }
            }
        }
        let alt = cam.length() - self.radius;
        let map = TangentWarp;
        let maxl = plan.nodes.iter().map(|n| n.level).max().unwrap_or(0);
        let mut found = 0;
        for (i, p) in self.probes.iter().enumerate() {
            let mut cur = None;
            for l in 0..=maxl {
                let t = TileId::from_direction(&map, *p, l);
                if set.contains(&t.raw()) {
                    cur = Some((l, t, (cam - *p * self.radius).length()));
                    break;
                }
            }
            if let (Some((l0, t0, d0)), Some((l1, t1, d1))) = (self.prev[i], cur) {
                self.pairs += 1;
                if l1 > l0 { self.finer += 1; } else if l1 == l0 { self.same += 1; }
                if l1 < l0 && d1 < d0 - 1.0 {
                    found += 1;
                    self.violations += 1;
                    if found <= 3 {
                        let pk = self.last_kind.clone();
                        let kids: Vec<String> = t1
                            .children()
                            .map(|c| {
                                c.iter()
                                    .map(|x| format!("{}{}", if resident(*x) { "R" } else { "-" }, if self.ever.contains(&x.raw()) { "e" } else { "n" }))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let up_res = (resident(t0), t0.parent().map(|p| resident(p)));
                        let _ = writeln!(
                            self.out,
                            "VIOLATION frame {k} alt {alt:.0} prev-kind {pk} kind {kind} subst {subst} blank {blank}: probe {i} L{l0}->L{l1} {t0:?} -> {t1:?} dist {d0:.0} -> {d1:.0}; prev tile resident now {up_res:?}, children of the new tile resident {kids:?}"
                        );
                    }
                }
            }
            if cur.is_some() {
                self.prev[i] = cur;
            } else {
                self.prev[i] = None;
            }
        }
        self.last_kind = kind.to_string();
        if found > 0 {
            self.frames_with += 1;
            let _ = writeln!(self.out, "  frame {k}: {found} probes got coarser");
        }
        if (645..=651).contains(&k) || (940..=944).contains(&k) { let _ = writeln!(self.out, "kind-trace frame {k} {kind} nodes {} maxL {maxl} subst {subst} blank {blank}", plan.nodes.len()); }
        if k % 120 == 0 {
            let _ = writeln!(self.out, "frame {k} alt {alt:.0} kind {kind} nodes {} maxL {maxl} violations so far {} pairs {} finer {} same {}", plan.nodes.len(), self.violations, self.pairs, self.finer, self.same);
        }
    }
}
