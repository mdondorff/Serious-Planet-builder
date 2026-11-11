//! K7, the local pass in the new chain (GEN-148, A3 sections 3.1 to 3.3, gate G-SURF): a tile of the local grid (102 m at Earth) starts from `B + C1(change)`, where `B` is the base read at every
//! cell (the store lattice's C1 read; the stride-4 window of pass 2 is retired) and the change is the regional solve's (a C1 read of the regional grid). The channels of K6 are burned in (the cells
//! along each segment are lowered to the segment's level profile, rasterised 8-connected at the local grid), then the breach of pass 1 runs at the local grid, the incising carve, thermal, the deposition
//! sweep and the pit spreading of pass 2. `L` is the result minus `B`, stored in steps of 0.25 m. The pit share and depth are reported after each stage, and G-SURF compares the runtime's `B + L`
//! with the bake's surface at the lattice nodes.

use super::carve::{carve, CarveParams};
use super::deposit::{deposit_on_own_route_flood, spread_pits, DepositParams, DepositReport, Kc};
use super::flood::priority_flood_with;
use super::hydro::Hydro;
pub use super::hydro::QUnit;
use super::map::catmull_rom;
use super::route::{MfdClip, RoundRules, Routing, NONE};
use super::thermal::thermal;
use super::tile::{sample_window, window_grid, Pass2Params, TileGrid, TileId, QUANT_STEP_M};
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};

/// A field on a window of a face's grid (the regional grid at 814 m): the Catmull-Rom (C1) read of the cells' values, clamped to the four cells around the point as `map::change_in_face` is, the
/// window's edge clamped.
pub struct RegionField {
    face: Face,
    cells: usize,
    i0: i64,
    j0: i64,
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl RegionField {
    /// `data` row-major, `w x h`, cell `(0, 0)` the cell `(i0, j0)` of the face's grid of `cells` cells an edge.
    pub fn new(face: Face, cells: usize, (i0, j0): (i64, i64), w: usize, h: usize, data: Vec<f32>) -> RegionField {
        assert!(w >= 4 && h >= 4 && data.len() == w * h, "a window of at least 4 x 4 cells");
        RegionField { face, cells, i0, j0, w, h, data }
    }

    /// The value at a direction on this face's plane.
    pub fn at(&self, d: Vec3) -> f64 {
        let w = d.dot(self.face.normal());
        assert!(w > 0.0 && w.is_finite(), "direction {d:?} is not in front of face {}", self.face.0);
        let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
        let (s, t) = (plane(d.dot(self.face.u_axis()) / w), plane(d.dot(self.face.v_axis()) / w));
        let n = self.cells as f64;
        let x = ((s + 1.0) * 0.5 * n - 0.5 - self.i0 as f64).clamp(0.0, (self.w - 1) as f64);
        let y = ((t + 1.0) * 0.5 * n - 0.5 - self.j0 as f64).clamp(0.0, (self.h - 1) as f64);
        let (i, j) = ((x as usize).min(self.w - 2), (y as usize).min(self.h - 2));
        let (fx, fy) = (x - i as f64, y - j as f64);
        let at =
            |a: i64, b: i64| f64::from(self.data[b.clamp(0, self.h as i64 - 1) as usize * self.w + a.clamp(0, self.w as i64 - 1) as usize]);
        let (ii, jj) = (i as i64, j as i64);
        let row = |dj: i64| catmull_rom([at(ii - 1, jj + dj), at(ii, jj + dj), at(ii + 1, jj + dj), at(ii + 2, jj + dj)], fx);
        let v = catmull_rom([row(-1), row(0), row(1), row(2)], fy);
        let around = [at(ii, jj), at(ii + 1, jj), at(ii, jj + 1), at(ii + 1, jj + 1)];
        v.clamp(around.iter().copied().fold(f64::INFINITY, f64::min), around.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}

/// A channel segment on the local grid: points in the face's cell-index space (cell `i` has its centre at `i`) and the bed level at each point (not rising downstream).
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub points: Vec<(f64, f64)>,
    pub levels: Vec<f64>,
    /// The routed discharge `Q_w` in m3/s at each point (K6's), which the width law turns into the width of the trench; empty: every point is one cell wide.
    pub q: Vec<f64>,
    /// The unit of `q`: the width law refuses an area (m2), which is what H1 returns without the runoff.
    pub q_unit: QUnit,
}

/// The channels of a hydrology on the local grid (the in-memory feed of the bake writer, GEN-167): one [`Channel`] per segment, its points the cells from `up` to `down` (both included) mapped by `to_local`
/// (H1 cell to a point in the face's local cell-index space), the bed level the running minimum of `zs` (not rising downstream) and `q` the routed discharge `hydro.qw` at each cell. `qw` must still be there:
/// run it before the discharges are released. The unit of `qw` is m3/s when H1 ran with runoff.
pub fn channels_from_hydro(hydro: &Hydro, routing: &Routing, zs: &[f64], to_local: &dyn Fn(usize) -> (f64, f64)) -> Vec<Channel> {
    assert!(hydro.qw.len() == zs.len(), "the channels need Q_w: run before the discharges are released");
    let mut order: Vec<usize> = (0..hydro.segments.len()).collect();
    order.sort_by_key(|&i| hydro.segments[i].id);
    let channels: Vec<Channel> = order
        .iter()
        .map(|&i| &hydro.segments[i])
        .map(|sg| {
            let mut cells = vec![sg.up as usize];
            while *cells.last().expect("a cell") != sg.down as usize {
                let r = routing.receiver[*cells.last().expect("a cell")];
                assert!(r != NONE, "a segment reaches its end cell");
                cells.push(r as usize);
            }
            let mut level = f64::INFINITY;
            let levels = cells
                .iter()
                .map(|&c| {
                    level = level.min(zs[c]);
                    level
                })
                .collect();
            Channel {
                points: cells.iter().map(|&c| to_local(c)).collect(),
                levels,
                q: cells.iter().map(|&c| hydro.qw[c]).collect(),
                q_unit: hydro.q_unit,
            }
        })
        .collect();
    upstream_first(channels)
}

/// The channels in the tree order of GEN-169 (architect, 2026-11-11): every channel after the channels that end at its first cell (its tributaries), ties by the order given (the segment-id order). A channel `U` is a tributary of
/// `C` when the rounded cell of `U`'s last point is the rounded cell of `C`'s first point. The burn and the final pass require this order; the producers of channels apply it.
pub fn upstream_first(channels: Vec<Channel>) -> Vec<Channel> {
    let cell = |p: (f64, f64)| (libm::round(p.0) as i64, libm::round(p.1) as i64);
    let n = channels.len();
    let mut ending: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (i, c) in channels.iter().enumerate() {
        let last = cell(*c.points.last().expect("a point"));
        // a channel inside one cell is nobody's tributary: whatever it joins is read from the channels that end in that cell (a one-cell channel is the only way two channels can end in each other's first cell)
        if last != cell(c.points[0]) {
            ending.entry(last).or_default().push(i);
        }
    }
    // indegree: the tributaries of each channel; the channels with none are ready, taken in the order given
    let tributaries = |i: usize| ending.get(&cell(channels[i].points[0])).map_or(0, |v| v.iter().filter(|&&u| u != i).count());
    let mut waiting: Vec<usize> = (0..n).map(tributaries).collect();
    let mut ready: std::collections::BTreeSet<usize> = (0..n).filter(|&i| waiting[i] == 0).collect();
    let mut starts: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (i, c) in channels.iter().enumerate() {
        starts.entry(cell(c.points[0])).or_default().push(i);
    }
    let mut order = Vec::with_capacity(n);
    let mut placed = vec![false; n];
    let mut cursor = 0;
    // malformed input only (a cycle of channels has no upstream; hydrology gives none, one-cell channels are not tributaries): the first unplaced one in the order given is taken
    while let Some(u) = ready.pop_first().or_else(|| {
        while cursor < n && placed[cursor] {
            cursor += 1;
        }
        (cursor < n).then_some(cursor)
    }) {
        placed[u] = true;
        order.push(u);
        let last = cell(*channels[u].points.last().expect("a point"));
        let joins = if last == cell(channels[u].points[0]) { None } else { starts.get(&last) };
        for &c in joins.into_iter().flatten().filter(|&&c| c != u) {
            waiting[c] = waiting[c].saturating_sub(1);
            if waiting[c] == 0 && !placed[c] {
                ready.insert(c);
            }
        }
    }
    assert!(order.len() == n, "every channel is placed once");
    let mut slots: Vec<Option<Channel>> = channels.into_iter().map(Some).collect();
    order.into_iter().map(|i| slots[i].take().expect("each once")).collect()
}

/// The one river width law (architect D6 point 2, D19/D20): the width of a channel is `w = a * Q_w^b` metres for a discharge in m3/s. The burn-in cuts a trench of `max(1 cell, w)` and the drawing uses `w`,
/// so the ground and the water agree by construction. `a = 7.2`, `b = 0.5` (Moody and Troutman 2002); both are bake parameters and part of the key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidthLaw {
    pub a: f64,
    pub b: f64,
}

impl WidthLaw {
    /// The standard law.
    pub const STANDARD: WidthLaw = WidthLaw { a: 7.2, b: 0.5 };

    /// The width in metres of a channel carrying `q` m3/s (zero for no discharge).
    pub fn width_m(&self, q: f64) -> f64 {
        if q > 0.0 {
            self.a * libm::pow(q, self.b)
        } else {
            0.0
        }
    }
}

/// The cells of a channel's line in order, each with the level of the profile there (a cell repeated in a row is listed once, at the lowest level of its samples), in face cell-index space: the cells
/// [`burn_channels_wide`] burns the line to.
fn line_with_levels(ch: &Channel) -> Vec<((i64, i64), f64)> {
    let mut out: Vec<((i64, i64), f64)> = Vec::new();
    let mut push = |x: f64, y: f64, level: f64| {
        let c = (libm::round(x) as i64, libm::round(y) as i64);
        match out.last_mut() {
            Some(last) if last.0 == c => last.1 = last.1.min(level),
            _ => out.push((c, level)),
        }
    };
    if ch.points.len() == 1 {
        push(ch.points[0].0, ch.points[0].1, ch.levels[0]);
    }
    for k in 0..ch.points.len().saturating_sub(1) {
        let ((x0, y0), (x1, y1)) = (ch.points[k], ch.points[k + 1]);
        let (l0, l1) = (ch.levels[k], ch.levels[k + 1]);
        let steps = libm::fabs(x1 - x0).max(libm::fabs(y1 - y0)).max(1.0).ceil() as i64;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            if s == steps {
                push(x1, y1, l1);
            } else {
                push(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, l0 + (l1 - l0) * t);
            }
        }
    }
    out
}

/// The cells of a channel's line in order (a cell repeated in a row is listed once), in face cell-index space.
pub fn line_cells(ch: &Channel) -> Vec<(i64, i64)> {
    line_with_levels(ch).into_iter().map(|(c, _)| c).collect()
}

/// For every channel and every cell of its line, the lowest `value` over the cells within `carry` steps upstream of it along the channel and its tributaries (the channel tree), the cell itself included
/// (GEN-169): a tributary is a channel whose last cell is this channel's first cell. A function of the values within `carry` line steps only, so a window that holds all of them returns the same bits
/// for the cell whatever else it holds (one line step moves one cell at most). `channels` must be listed upstream first (a tributary before the channel it joins).
fn tree_carry(lines: &[Vec<(i64, i64)>], value: &[Vec<f64>], carry: usize) -> Vec<Vec<f64>> {
    let n = lines.len();
    let l = carry.min(lines.iter().map(Vec::len).sum::<usize>());
    let mut ending: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (c, line) in lines.iter().enumerate() {
        if let Some(&last) = line.last().filter(|&&l| Some(&l) != line.first()) {
            ending.entry(last).or_default().push(c);
        }
    }
    // up[c][s]: the lowest value over the tributaries' cells within s steps upstream of the first cell of channel c.
    let mut up: Vec<Vec<f64>> = Vec::with_capacity(n);
    for c in 0..n {
        let mut v = vec![f64::INFINITY; l + 1];
        if let Some(first) = lines[c].first() {
            for &u in ending.get(first).into_iter().flatten().filter(|&&u| u < c) {
                let nu = lines[u].len();
                let mut run = f64::INFINITY;
                for (s, slot) in v.iter_mut().enumerate() {
                    if s < nu {
                        run = run.min(value[u][nu - 1 - s]);
                    }
                    let mut m = run;
                    if s + 1 >= nu {
                        m = m.min(up[u][s + 1 - nu]);
                    }
                    *slot = slot.min(m);
                }
            }
        }
        up.push(v);
    }
    (0..n)
        .map(|c| {
            let len = value[c].len();
            (0..len)
                .map(|k| {
                    let mut m = value[c][k.saturating_sub(l)..=k].iter().copied().fold(f64::INFINITY, f64::min);
                    if k <= l {
                        m = m.min(up[c][l - k]);
                    }
                    m
                })
                .collect()
        })
        .collect()
}

/// [`tree_carry`] with no limit, in O(cells): the lowest value over everything upstream of the cell along the tree (the report of where the cutoff binds).
fn tree_min_unbounded(lines: &[Vec<(i64, i64)>], value: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut ending: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (c, line) in lines.iter().enumerate() {
        if let Some(&last) = line.last().filter(|&&l| Some(&l) != line.first()) {
            ending.entry(last).or_default().push(c);
        }
    }
    // whole[c]: the lowest value over channel c and everything upstream of it
    let mut whole: Vec<f64> = Vec::with_capacity(lines.len());
    let mut out = Vec::with_capacity(lines.len());
    for c in 0..lines.len() {
        let mut up = f64::INFINITY;
        if let Some(first) = lines[c].first() {
            for &u in ending.get(first).into_iter().flatten().filter(|&&u| u < c) {
                up = up.min(whole[u]);
            }
        }
        let mut run = up;
        let prefix: Vec<f64> = value[c]
            .iter()
            .map(|&v| {
                run = run.min(v);
                run
            })
            .collect();
        whole.push(prefix.last().copied().unwrap_or(up));
        out.push(prefix);
    }
    out
}

/// Burns the channels into the window `z` (`w x w`, cell `(0, 0)` the face cell `(i0, j0)`): along each segment between consecutive points the cells of the 8-connected line are lowered to the
/// level interpolated linearly along the segment (never raised). Returns the number of cells lowered. Every channel is one cell wide: [`burn_channels_wide`] with no width law.
pub fn burn_channels(z: &mut [f64], w: usize, origin: (i64, i64), channels: &[Channel]) -> usize {
    burn_channels_wide(z, w, origin, channels, None)
}

/// [`burn_channels`] with the width law `(law, cell_m)`, `cell_m` the metres of a local cell: where the width `w = a Q_w^b` of the channel (the discharge interpolated linearly along the segment) is more
/// than one cell, every cell whose centre lies within `w / 2` of the point of the line is lowered to the line's level there (never raised): a trench of `max(1 cell, w)`. A channel without discharges, or
/// narrower than a cell, burns exactly as the one-cell line (the bits of K7 before the law). The line is always burnt and the disc comes on top, so the trench of a wider river contains the one-cell line (the width is
/// measured in face cell-index space: on the cube-face grid a cell is up to 0.7 to 1.2 times the nominal size near the edges and corners, a known limit that the drawing must share). The profile carries every
/// hole upstream down the line with no limit: [`burn_channels_bounded`] with `carry` unbounded.
pub fn burn_channels_wide(z: &mut [f64], w: usize, origin: (i64, i64), channels: &[Channel], law: Option<(&WidthLaw, f64)>) -> usize {
    burn_channels_bounded(z, w, origin, channels, law, usize::MAX)
}

/// [`burn_channels_wide`] with the profile of GEN-169 (architect, 2026-11-11): the target at a cell of a line is the lowest of its level and of `min(height found, level)` over the cells within `carry` line steps upstream
/// along the channel tree (a tributary is a channel whose last cell is this channel's first cell; `channels` upstream first), so no cell stands above its upstream neighbour within `carry` steps. Lowering
/// only; the disc of a wide river burns to the same target. The heights are those found before any burn (a wide river's disc, or the order of the channels, does not move another line), and the target of a
/// cell is a function of the cells within `carry` steps, so a window with a halo of `carry` cells burns its core as any other window does (partition independence).
pub fn burn_channels_bounded(
    z: &mut [f64],
    w: usize,
    (i0, j0): (i64, i64),
    channels: &[Channel],
    law: Option<(&WidthLaw, f64)>,
    carry: usize,
) -> usize {
    let mut lowered = 0;
    // The disc of a wide river reaches DISC_RADIUS_MAX cells from the line, so a cell of the core is burnt by a line cell up to that far outside it, whose own carry reaches `carry` steps further: the profile
    // carries `carry - DISC_RADIUS_MAX` steps when a law is given, and the half width is capped (GEN-169, partition independence with the discs).
    let carry = if law.is_some() && carry != usize::MAX { carry.saturating_sub(DISC_RADIUS_MAX as usize) } else { carry };
    for ch in channels {
        assert!(ch.points.len() == ch.levels.len() && !ch.points.is_empty(), "a level for every point");
        assert!(ch.q.is_empty() || ch.q.len() == ch.points.len(), "a discharge for every point, or none");
        assert!(
            law.is_none() || ch.q.is_empty() || ch.q_unit == QUnit::Discharge,
            "the width law needs a discharge in m3/s: H1 returned an area in m2 (no runoff); pass the runoff to hydrology"
        );
        assert!(ch.q.iter().all(|q| q.is_finite()), "the discharges of a channel are finite");
        assert!(ch.levels.iter().all(|l| l.is_finite()), "the levels of a channel are finite");
    }
    let in_window = |x: i64, y: i64| x >= i0 && y >= j0 && x < i0 + w as i64 && y < j0 + w as i64;
    // The profile of every channel: the lowest of the level and the height found, carried `carry` steps down the tree. The heights are read before any burn.
    let lines: Vec<Vec<((i64, i64), f64)>> = channels.iter().map(line_with_levels).collect();
    let cells: Vec<Vec<(i64, i64)>> = lines.iter().map(|l| l.iter().map(|&(c, _)| c).collect()).collect();
    let value: Vec<Vec<f64>> = lines
        .iter()
        .map(|l| {
            l.iter()
                .map(|&((x, y), level)| if in_window(x, y) { level.min(z[(y - j0) as usize * w + (x - i0) as usize]) } else { level })
                .collect()
        })
        .collect();
    let target = tree_carry(&cells, &value, carry);
    for (ci, ch) in channels.iter().enumerate() {
        let bounds = (i0, i0 + w as i64 - 1, j0, j0 + w as i64 - 1);
        let width_cells = |q: f64| law.map_or(0.0, |(l, cell_m)| (l.width_m(q) / cell_m).min(2.0 * DISC_RADIUS_MAX as f64));
        let mut burn = |x: i64, y: i64, level: f64| {
            if in_window(x, y) {
                let c = (y - j0) as usize * w + (x - i0) as usize;
                if level < z[c] {
                    z[c] = level;
                    lowered += 1;
                }
            }
        };
        let q_at = |k: usize, t: f64| -> f64 {
            if ch.q.is_empty() {
                0.0
            } else {
                let k1 = (k + 1).min(ch.q.len() - 1);
                ch.q[k] + (ch.q[k1] - ch.q[k]) * t
            }
        };
        // The index of the sample's cell in the line (a cell repeated in a row is one entry).
        let (mut kidx, mut last_cell) = (0usize, None);
        let mut at_cell = |cell: (i64, i64)| -> f64 {
            if last_cell.is_some() && last_cell != Some(cell) {
                kidx += 1;
            }
            last_cell = Some(cell);
            target[ci][kidx]
        };
        if ch.points.len() == 1 {
            let wc = width_cells(q_at(0, 0.0));
            let cell = (libm::round(ch.points[0].0) as i64, libm::round(ch.points[0].1) as i64);
            let level = at_cell(cell);
            burn(cell.0, cell.1, level);
            if wc > 1.0 {
                disc(&mut burn, ch.points[0].0, ch.points[0].1, wc / 2.0, level, bounds);
            }
        }
        for k in 0..ch.points.len().saturating_sub(1) {
            let ((x0, y0), (x1, y1)) = (ch.points[k], ch.points[k + 1]);
            let steps = libm::fabs(x1 - x0).max(libm::fabs(y1 - y0)).max(1.0).ceil() as i64;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (px, py) = if s == steps { (x1, y1) } else { (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t) };
                let wc = width_cells(q_at(k, t));
                // The one-cell line is always burnt (a trench is never narrower than the line: max(1 cell, w)); a wider river adds the cells within w / 2 of the point.
                let cell = (libm::round(px) as i64, libm::round(py) as i64);
                let level = at_cell(cell);
                burn(cell.0, cell.1, level);
                if wc > 1.0 {
                    disc(&mut burn, px, py, wc / 2.0, level, bounds);
                }
            }
        }
    }
    lowered
}

/// The largest half width of a trench in cells (GEN-169): a wider river is burnt as one of `2 * DISC_RADIUS_MAX` cells (3.3 km at 101.8 m cells; the Amazon's mouth is about 16 cells). The tile halo must hold
/// the carry and this radius.
pub const DISC_RADIUS_MAX: i64 = 16;

/// The smallest carry of the profile in line steps, after the trench radius is taken from the halo (GEN-169): `k7_tile` refuses a halo under `DISC_RADIUS_MAX + PROFILE_CARRY_MIN` when it has channels.
pub const PROFILE_CARRY_MIN: usize = 8;

/// What the final channel pass did (GEN-169).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FinalPass {
    /// The path cells lowered, and those lowered by more than 1 m.
    pub lowered: usize,
    pub over_1m: usize,
    /// The largest lowering in metres, and the removed volume in cubic metres (land cells only, `cell_area_m2` a cell): booked as eroded and exported.
    pub largest_m: f64,
    pub removed_m3: f64,
    /// The land path cells whose target differs from the unbounded carry (the cutoff binds: a hole more than `carry` steps upstream is not carried to them).
    pub cutoff_binds: usize,
}

/// The final channel pass (GEN-169): every land cell of a channel's line is lowered to the lowest height over the cells within `carry` steps upstream along the channel tree (the heights as the pass
/// finds them, so the order of the channels does not matter), lowering path cells only. A cell outside the window or at or under the sea level neither is lowered nor carries. `origin` is the face cell
/// `(i0, j0)` of the window's cell `(0, 0)`; `channels` upstream first. A function of the cells within `carry` steps: a window with a halo of `carry` cells returns the same bits for its core.
pub fn final_channel_pass(
    z: &mut [f64],
    w: usize,
    (i0, j0): (i64, i64),
    channels: &[Channel],
    sea_level: f64,
    cell_area_m2: f64,
    carry: usize,
) -> FinalPass {
    let in_window = |x: i64, y: i64| x >= i0 && y >= j0 && x < i0 + w as i64 && y < j0 + w as i64;
    let cells: Vec<Vec<(i64, i64)>> = channels.iter().map(line_cells).collect();
    let value: Vec<Vec<f64>> = cells
        .iter()
        .map(|l| {
            l.iter()
                .map(|&(x, y)| {
                    if in_window(x, y) && z[(y - j0) as usize * w + (x - i0) as usize] > sea_level {
                        z[(y - j0) as usize * w + (x - i0) as usize]
                    } else {
                        f64::INFINITY
                    }
                })
                .collect()
        })
        .collect();
    let target = tree_carry(&cells, &value, carry);
    let unbounded = tree_min_unbounded(&cells, &value);
    let mut report = FinalPass::default();
    for (ci, line) in cells.iter().enumerate() {
        for (k, &(x, y)) in line.iter().enumerate() {
            if !in_window(x, y) || value[ci][k] == f64::INFINITY {
                continue;
            }
            report.cutoff_binds += usize::from(target[ci][k] != unbounded[ci][k]);
            let c = (y - j0) as usize * w + (x - i0) as usize;
            if z[c] > target[ci][k] {
                let d = z[c] - target[ci][k];
                report.lowered += 1;
                report.over_1m += usize::from(d > 1.0);
                report.largest_m = report.largest_m.max(d);
                report.removed_m3 += d * cell_area_m2;
                z[c] = target[ci][k];
            }
        }
    }
    report
}

/// The cells whose centres lie within `half` cells of `(px, py)` (cell-index space) inside the window box `(x_lo, x_hi, y_lo, y_hi)` (inclusive, in face cell indices), burned to `level`.
fn disc(burn: &mut impl FnMut(i64, i64, f64), px: f64, py: f64, half: f64, level: f64, (x_lo, x_hi, y_lo, y_hi): (i64, i64, i64, i64)) {
    let r = libm::ceil(half) as i64;
    let (cx, cy) = (libm::round(px) as i64, libm::round(py) as i64);
    // Only the cells of the window: a river far outside it costs nothing.
    for y in (cy - r).max(y_lo)..=(cy + r).min(y_hi) {
        for x in (cx - r).max(x_lo)..=(cx + r).min(x_hi) {
            let (dx, dy) = (x as f64 - px, y as f64 - py);
            if dx * dx + dy * dy <= half * half {
                burn(x, y, level);
            }
        }
    }
}

/// The pits of a surface: land cells whose fill depth reaches `min_m`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PitStats {
    pub land_cells: usize,
    pub pit_cells: usize,
    pub p50_m: f64,
    pub p99_m: f64,
    pub max_m: f64,
}

impl PitStats {
    /// The share of the land cells that are pits.
    pub fn share(&self) -> f64 {
        self.pit_cells as f64 / self.land_cells.max(1) as f64
    }
}

/// The pits of `z` on `grid` (outlets: the sea and the window's border) at the depth `min_m`; only the cells at least `halo` cells in from the border of the square window are counted (the flood runs on the whole window).
pub fn pit_stats(grid: &super::Grid, z: &[f64], sea_level: f64, epsilon: f64, min_m: f64, halo: usize) -> PitStats {
    pit_stats_with(grid, z, sea_level, epsilon, min_m, halo, false)
}

/// [`pit_stats`] with the flood chosen: `canonical` fills by (level, cell id) (GEN-160), off it is the plain flood.
pub fn pit_stats_with(grid: &super::Grid, z: &[f64], sea_level: f64, epsilon: f64, min_m: f64, halo: usize, canonical: bool) -> PitStats {
    let w = (z.len() as f64).sqrt().round() as usize;
    assert_eq!(w * w, z.len(), "a square window");
    let outlet = grid.outlets(z, sea_level);
    let filled = priority_flood_with(grid, z, &outlet, epsilon, canonical);
    let mut depths: Vec<f64> = Vec::new();
    let mut land = 0usize;
    for p in 0..z.len() {
        let (x, y) = (p % w, p / w);
        let in_core = x >= halo && y >= halo && x < w - halo && y < w - halo;
        if in_core && z[p] > sea_level && !outlet[p] {
            land += 1;
            let d = filled[p] - z[p];
            if d >= min_m {
                depths.push(d);
            }
        }
    }
    depths.sort_by(|a, b| a.total_cmp(b));
    let at = |f: f64| if depths.is_empty() { 0.0 } else { depths[((depths.len() - 1) as f64 * f) as usize] };
    PitStats { land_cells: land, pit_cells: depths.len(), p50_m: at(0.5), p99_m: at(0.99), max_m: depths.last().copied().unwrap_or(0.0) }
}

/// The parameters of K7 besides the geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct K7Params {
    /// Pass 2's parameters; the local carve's breach is pass 1's rule at the local grid (`breach_max_m`, `breach_slope`).
    pub pass2: Pass2Params,
    /// The flood of K7's pit statistics, its incising carve and its deposition sweep: the canonical flood (cells visited by level, then cell id; A3 section 6.4, GEN-160) when on, the plain flood of pass 2 when off
    /// (the bits of K7 before GEN-160).
    pub canonical_flood: bool,
    /// The river width law of the burn-in (and of the drawing): a bake parameter, part of the key.
    pub width: WidthLaw,
}

impl K7Params {
    /// Pass 2's start values with the breach of pass 1 (500 m limit, 0.1 % slope) in the local carve.
    pub fn standard(seed: u64, sea_level: f64) -> K7Params {
        let mut pass2 = Pass2Params::standard(seed, sea_level);
        pass2.carve.breach_max_m = CarveParams::default().breach_max_m;
        K7Params { pass2, canonical_flood: true, width: WidthLaw::STANDARD }
    }
}

/// What K7 did on a tile.
pub struct K7Result {
    /// The window's heights after the pass, and `B` at the window's cells.
    pub z: Vec<f64>,
    pub b: Vec<f64>,
    /// The core's change against `B`, as stored (f32, before the 0.25 m quantisation).
    pub change: Vec<f32>,
    /// The cells lowered by the burn-in.
    pub burned: usize,
    /// The window's cells the burn-in lowered.
    pub burn_mask: Vec<bool>,
    /// The pits (at `pit_min_m`) after each stage: the start, the burn-in, the breach alone, the incising carve, the whole pass.
    pub pits: Vec<(&'static str, PitStats)>,
    /// The deposition sweep's ledger with the final channel pass's removed volume booked as eroded and exported (G-SED closes: eroded = deposited + exported).
    pub sediment: DepositReport,
    /// The final channel pass (GEN-169).
    pub final_pass: FinalPass,
}

/// K7 on one tile. `base` is `B` (read at every cell of the window), `regional` the regional change (a C1 read of the regional solve, `RegionField::at`), `channels` the channel segments on the local
/// grid (cell-index space of the face, see [`Channel`]).
pub fn k7_tile(
    g: &TileGrid,
    id: TileId,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    regional: &(dyn Fn(Vec3) -> f64 + Sync),
    channels: &[Channel],
    p: &K7Params,
    threads: usize,
) -> K7Result {
    g.check().expect("a usable tile geometry");
    let q = &p.pass2;
    assert!(q.carve.round_rules.is_default() && !q.carve.cell_metric, "K7 runs the plane kernels of pass 2");
    let grid = window_grid(g, id);
    let b = sample_window(g, id, base, threads);
    let dr = sample_window(g, id, regional, threads);
    let mut z: Vec<f64> = b.iter().zip(&dr).map(|(b, c)| b + c).collect();
    drop(dr);
    let eps = q.carve.epsilon;
    let stats = |z: &[f64]| pit_stats_with(&grid, z, q.sea_level, eps, q.pit_min_m, g.halo, p.canonical_flood);
    let mut pits = vec![("base + regional change", stats(&z))];
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let origin = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let z_start = z.clone();
    assert!(
        channels.is_empty() || g.halo >= DISC_RADIUS_MAX as usize + PROFILE_CARRY_MIN,
        "the tile halo of {} cells is smaller than the trench radius cap of {} cells plus the smallest profile carry of {} cells ({}): the profile would carry nothing across the halo (GEN-169)",
        g.halo,
        DISC_RADIUS_MAX,
        PROFILE_CARRY_MIN,
        DISC_RADIUS_MAX as usize + PROFILE_CARRY_MIN
    );
    let channels = &upstream_first(channels.to_vec());
    let burned = burn_channels_bounded(&mut z, w, origin, channels, Some((&p.width, g.dx())), g.halo);
    let burn_mask: Vec<bool> = z.iter().zip(&z_start).map(|(a, b)| a != b).collect();
    drop(z_start);
    pits.push(("channels burned in", stats(&z)));
    let z_burned = z.clone();
    let outlet = grid.outlets(&z, q.sea_level);
    let stream = super::face::face_stream(q.seed, id.face);
    let rules = RoundRules { clip_mfd: MfdClip::Off, canonical_flood: p.canonical_flood };
    let local = CarveParams { max_area: q.local_area_cells * g.dx() * g.dx(), round_rules: rules, ..q.carve };
    // The breach alone: the carve with no incision (`k_dt` 0) runs the breach of pass 1 and leaves the rest.
    let mut breach_only = z.clone();
    carve(&grid, &mut breach_only, &outlet, &CarveParams { k_dt: 0.0, steps: 1, reroute_every: 1, ..local }, None, stream);
    pits.push(("breach", stats(&breach_only)));
    drop(breach_only);
    let _ = carve(&grid, &mut z, &outlet, &local, None, stream);
    pits.push(("breach and incision", stats(&z)));
    thermal(&grid, &mut z, &q.thermal, threads);
    let mut sediment = deposit_on_own_route_flood(
        &grid,
        &mut z,
        &z_burned,
        eps,
        stream,
        &DepositParams { kc: Kc::Uniform(q.kc), zeta: q.zeta, sea_level: q.sea_level },
        p.canonical_flood,
    );
    spread_pits(&grid, &mut z, q.sea_level, q.pit_min_m, q.pit_passes);
    let final_pass = final_channel_pass(&mut z, w, origin, channels, q.sea_level, g.dx() * g.dx(), g.halo);
    sediment.eroded += final_pass.removed_m3;
    sediment.exported += final_pass.removed_m3;
    pits.push(("the whole pass", stats(&z)));
    // The stage probe (GEN-161) reads the finished window: off by default, read only.
    super::probe::emit(super::probe::Stage::K7 { face: id.face.0, ti: id.ti, tj: id.tj }, |s| {
        super::probe::StageView::new(s, &grid)
            .f64("z", super::probe::FieldKind::Height, &z)
            .number("pits_after_pass", pits.last().map_or(0.0, |p| p.1.pit_cells as f64))
    });
    let (tile, halo) = (g.tile, g.halo);
    let mut change = Vec::with_capacity(tile * tile);
    for j in 0..tile {
        for i in 0..tile {
            let c = (j + halo) * w + i + halo;
            change.push((z[c] - b[c]) as f32);
        }
    }
    K7Result { z, b, change, burned, burn_mask, pits, sediment, final_pass }
}

/// The result of G-SURF at the lattice nodes of a tile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceGate {
    /// Lattice nodes read.
    pub nodes: usize,
    /// The largest `|runtime - bake|` before quantisation and with `L` quantised to steps of `QUANT_STEP_M`, over the nodes.
    pub max_unquantised_m: f64,
    pub max_quantised_m: f64,
    /// Nodes over 0.25 m after quantisation, at most 4,096 of them: the window coordinates (cell-index space) and the difference.
    pub failing: Vec<(f64, f64, f64)>,
    /// The two other ways of reading the surface at a node, against the bake's `C1(z)`, with `L` quantised: `B` plus the unclamped Catmull-Rom read of `L` (largest difference), and the clamped read of
    /// the composed `B + L` at the four cells around the point (largest difference).
    pub max_unclamped_l_quantised_m: f64,
    pub max_compose_quantised_m: f64,
    /// The largest overshoot of the unclamped read of `L` beyond its clamped read at the nodes.
    pub max_l_overshoot_m: f64,
    /// The 50th and 99th percentile of the quantised difference over the nodes, and the number of nodes over the quantised bound of A3 (0.25 m).
    pub p50_quantised_m: f64,
    pub p99_quantised_m: f64,
    pub over_bound: usize,
    /// The largest `|B(node) - C1(B at the cells)(node)|`: what re-reading `B` from its values at the local cells' centres loses against `B` itself (part of the two above).
    pub max_base_resample_m: f64,
    /// The same at the centres of the core's cells (where the C1 read returns the stored value).
    pub max_centres_unquantised_m: f64,
    pub max_centres_quantised_m: f64,
    /// G-SURF-I (A3 section 12.1): the 95th percentile of the quantised difference at the nodes; the nodes and the nodes over 0.25 m by the local step height of the bake's surface (the largest `|z - z(here)|` within 2 cells:
    /// up to 5 m, over 5 m up to 20 m, over 20 m); the largest quotient of the difference and the step height over the nodes with a step of at least 1 m; and the spatial concentration: the bins of 64 cells that hold nodes, those that
    /// hold a node over 0.25 m, and the most such nodes in one bin.
    pub p95_quantised_m: f64,
    pub nodes_by_step: [usize; 3],
    pub over_by_step: [usize; 3],
    pub max_error_over_step: f64,
    pub bins: usize,
    pub bins_over: usize,
    pub busiest_bin_over: usize,
    /// G-SURF-I, error against the local base slope: the largest and the 99th percentile of `|difference| / rise` over the nodes whose base `B` rises by at least 1 m across one cell (the rise is the largest `|B(neighbour) - B(here)|` over the 8 neighbours, diagonals included, of the lower-left of the four cells around the node), and their number.
    pub max_error_over_rise: f64,
    pub p99_error_over_rise: f64,
    pub nodes_with_rise: usize,
    /// G-SURF-I, overshoot: the largest amount by which the runtime read `B(node) + C1(L)(node)` lies outside the range of the composed values `B + L` (quantised) at the four cells around the node, and the nodes over 0.25 m.
    pub max_runtime_overshoot_m: f64,
    pub runtime_overshoot_nodes: usize,
}

/// The Catmull-Rom read of `v` at `(x, y)` without the clamp to the four cells (linear in `v`).
fn read_c1_unclamped(v: &[f64], w: usize, x: f64, y: f64) -> f64 {
    let (i, j) = ((x as usize).min(w - 2), (y as usize).min(w - 2));
    let (fx, fy) = (x - i as f64, y - j as f64);
    let at = |a: i64, b: i64| v[b.clamp(0, w as i64 - 1) as usize * w + a.clamp(0, w as i64 - 1) as usize];
    let (ii, jj) = (i as i64, j as i64);
    let row = |dj: i64| catmull_rom([at(ii - 1, jj + dj), at(ii, jj + dj), at(ii + 1, jj + dj), at(ii + 2, jj + dj)], fx);
    catmull_rom([row(-1), row(0), row(1), row(2)], fy)
}

/// A C1 read of the window field `v` (`w x w`, centres at the integer coordinates) at `(x, y)`, with the clamp of [`RegionField`].
fn read_c1(v: &[f64], w: usize, x: f64, y: f64) -> f64 {
    let (i, j) = ((x as usize).min(w - 2), (y as usize).min(w - 2));
    let (fx, fy) = (x - i as f64, y - j as f64);
    let at = |a: i64, b: i64| v[b.clamp(0, w as i64 - 1) as usize * w + a.clamp(0, w as i64 - 1) as usize];
    let (ii, jj) = (i as i64, j as i64);
    let row = |dj: i64| catmull_rom([at(ii - 1, jj + dj), at(ii, jj + dj), at(ii + 1, jj + dj), at(ii + 2, jj + dj)], fx);
    let r = catmull_rom([row(-1), row(0), row(1), row(2)], fy);
    let around = [at(ii, jj), at(ii + 1, jj), at(ii, jj + 1), at(ii + 1, jj + 1)];
    r.clamp(around.iter().copied().fold(f64::INFINITY, f64::min), around.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// G-SURF on a tile: the runtime's surface `B + C1(L)` against the bake's `C1(z)` at the lattice nodes (the vertices of the store lattice, every `lattice_step` local cells) inside the core, two cells in from its
/// edge (the read needs its neighbours), with `L = z - B` as stored (quantised to `QUANT_STEP_M`) and unquantised; and at the centres of the same region's cells.
pub fn surface_gate(g: &TileGrid, id: TileId, base: &(dyn Fn(Vec3) -> f64 + Sync), r: &K7Result, lattice_step: usize) -> SurfaceGate {
    let (w, t, h) = (g.window(), g.tile as i64, g.halo as i64);
    let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let l: Vec<f64> = r.z.iter().zip(&r.b).map(|(z, b)| z - b).collect();
    let step = f64::from(QUANT_STEP_M);
    let lq: Vec<f64> = l.iter().map(|v| (libm::round(v / step).clamp(f64::from(i16::MIN), f64::from(i16::MAX))) * step).collect();
    let mut out = SurfaceGate::default();
    let mut diffs: Vec<f64> = Vec::new();
    let mut over_rise: Vec<f64> = Vec::new();
    let mut bins: std::collections::BTreeMap<(i64, i64), usize> = std::collections::BTreeMap::new();
    let zq: Vec<f64> = r.b.iter().zip(&lq).map(|(b, l)| b + l).collect();
    // The store lattice is vertex-based: vertex k lies at the local edge coordinate E = lattice_step * k (cell-index coordinate E - 0.5).
    let half = 0.0;
    let (lo, hi) = (h + 2, h + t - 3);
    let n = g.cells as f64;
    let e_first = (((i0 + lo) as f64 - half) / lattice_step as f64).ceil() as i64;
    let e_last = (((i0 + hi) as f64 - half) / lattice_step as f64).floor() as i64;
    let f_first = (((j0 + lo) as f64 - half) / lattice_step as f64).ceil() as i64;
    let f_last = (((j0 + hi) as f64 - half) / lattice_step as f64).floor() as i64;
    for fb in f_first..=f_last {
        for ea in e_first..=e_last {
            let (ex, ey) = ((ea * lattice_step as i64) as f64 + half, (fb * lattice_step as i64) as f64 + half);
            let (x, y) = (ex - 0.5 - i0 as f64, ey - 0.5 - j0 as f64);
            let d = face_to_direction(&TangentWarp, id.face, 2.0 * ex / n - 1.0, 2.0 * ey / n - 1.0);
            let b_node = base(d);
            let bake = read_c1(&r.z, w, x, y);
            out.nodes += 1;
            out.max_base_resample_m = out.max_base_resample_m.max(libm::fabs(b_node - read_c1(&r.b, w, x, y)));
            out.max_unquantised_m = out.max_unquantised_m.max(libm::fabs(b_node + read_c1(&l, w, x, y) - bake));
            let dq = libm::fabs(b_node + read_c1(&lq, w, x, y) - bake);
            out.max_quantised_m = out.max_quantised_m.max(dq);
            diffs.push(dq);
            let (cx, cy) = ((x as i64).clamp(0, w as i64 - 1), (y as i64).clamp(0, w as i64 - 1));
            let here = r.z[cy as usize * w + cx as usize];
            let mut step = 0.0f64;
            for dy in -2..=2i64 {
                for dx in -2..=2i64 {
                    let (a, b) = ((cx + dx).clamp(0, w as i64 - 1) as usize, (cy + dy).clamp(0, w as i64 - 1) as usize);
                    step = step.max(libm::fabs(r.z[b * w + a] - here));
                }
            }
            let class = if step <= 5.0 {
                0
            } else if step <= 20.0 {
                1
            } else {
                2
            };
            out.nodes_by_step[class] += 1;
            if step >= 1.0 {
                out.max_error_over_step = out.max_error_over_step.max(dq / step);
            }
            let rise = (-1..=1i64)
                .flat_map(|dy| (-1..=1i64).map(move |dx| (dx, dy)))
                .map(|(dx, dy)| {
                    let (a, b) = ((cx + dx).clamp(0, w as i64 - 1) as usize, (cy + dy).clamp(0, w as i64 - 1) as usize);
                    libm::fabs(r.b[b * w + a] - r.b[cy as usize * w + cx as usize])
                })
                .fold(0.0f64, f64::max);
            if rise >= 1.0 {
                over_rise.push(dq / rise);
            }
            let (ci, cj) = ((x as usize).min(w - 2), (y as usize).min(w - 2));
            let around = [zq[cj * w + ci], zq[cj * w + ci + 1], zq[(cj + 1) * w + ci], zq[(cj + 1) * w + ci + 1]];
            let (alo, ahi) =
                (around.iter().copied().fold(f64::INFINITY, f64::min), around.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            let runtime = b_node + read_c1(&lq, w, x, y);
            let over = (alo - runtime).max(runtime - ahi).max(0.0);
            out.max_runtime_overshoot_m = out.max_runtime_overshoot_m.max(over);
            if over > f64::from(QUANT_STEP_M) {
                out.runtime_overshoot_nodes += 1;
            }
            let bin = bins.entry((cx / 64, cy / 64)).or_insert(0);
            if dq > f64::from(QUANT_STEP_M) {
                out.over_by_step[class] += 1;
                *bin += 1;
            }
            if dq > f64::from(QUANT_STEP_M) && out.failing.len() < 4096 {
                out.failing.push((x, y, dq));
            }
            out.max_unclamped_l_quantised_m =
                out.max_unclamped_l_quantised_m.max(libm::fabs(b_node + read_c1_unclamped(&lq, w, x, y) - bake));
            out.max_compose_quantised_m = out.max_compose_quantised_m.max(libm::fabs(read_c1(&zq, w, x, y) - bake));
            out.max_l_overshoot_m = out.max_l_overshoot_m.max(libm::fabs(read_c1_unclamped(&lq, w, x, y) - read_c1(&lq, w, x, y)));
        }
    }
    diffs.sort_by(|a, b| a.total_cmp(b));
    if !diffs.is_empty() {
        out.p50_quantised_m = diffs[diffs.len() / 2];
        out.p95_quantised_m = diffs[(diffs.len() - 1) * 95 / 100];
        out.p99_quantised_m = diffs[(diffs.len() - 1) * 99 / 100];
        out.over_bound = diffs.iter().filter(|d| **d > f64::from(QUANT_STEP_M)).count();
        out.bins = bins.len();
        out.bins_over = bins.values().filter(|c| **c > 0).count();
        out.busiest_bin_over = bins.values().copied().max().unwrap_or(0);
        over_rise.sort_by(|a, b| a.total_cmp(b));
        out.nodes_with_rise = over_rise.len();
        if let Some(m) = over_rise.last() {
            out.max_error_over_rise = *m;
            out.p99_error_over_rise = over_rise[(over_rise.len() - 1) * 99 / 100];
        }
    }
    for j in lo..=hi {
        for i in lo..=hi {
            let c = j as usize * w + i as usize;
            // The C1 reads at a centre (the coordinates are the integer cell indices): the stored value comes back.
            let (x, y) = (i as f64, j as f64);
            let bake = read_c1(&r.z, w, x, y);
            out.max_centres_unquantised_m = out.max_centres_unquantised_m.max(libm::fabs(r.b[c] + read_c1(&l, w, x, y) - bake));
            out.max_centres_quantised_m = out.max_centres_quantised_m.max(libm::fabs(r.b[c] + read_c1(&lq, w, x, y) - bake));
        }
    }
    out
}
