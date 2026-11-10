//! The lake water mask of a tile of L (GEN-167, architect D17/D18/D20; design section 13): which cells of a tile's core are the water of which lake, decided at bake time on the final surface `zl = B + L`.
//!
//! The rule is a pure function of the window, with a reach of at most [`RING_STEPS`] local cells, so two tiles that meet at an edge or a corner decide the cells next to it the same way in any order and on any
//! thread count (CON-14):
//! 1. **Water blocks**: the local cells whose H1 parent is a water cell of a lake ([`Status::Water`]).
//! 2. **Core wet**: a cell of a water block with `zl` under the lake's level.
//! 3. **Ring wet**: a cell with `zl` under the level, reached from a core wet cell through at most [`RING_STEPS`] steps (8-neighbours) over such cells, entering only cells whose H1 parent is in the lake's ring
//!    ([`Status::Ring`]: the H1 cells next to the water that drain into this lake and belong to no other basin).
//!
//! A cell is in one lake's water or ring only (the producer's lists give a cell one lake), so one label per cell needs no tie-break.
//!
//! The mask is stored as runs per row in the tile record's water section ([`encode`], [`decode`]): `n lakes (u8) | lake ids (u64 LE each) | per row: run count (u16 LE), runs (start u16, length u16, lake index u8)`.

use std::collections::VecDeque;

/// The ring's depth in local cells: one H1 cell is 8 local cells, so the lake's edge can lie one H1 cell beyond its blocks.
pub const RING_STEPS: u8 = 8;
/// No lake: the label of a dry cell.
pub const NO_LAKE: u64 = u64::MAX;
/// The most lakes one tile lists (the run label is a byte).
pub const MAX_LAKES_PER_TILE: usize = 255;

/// What the lake producer says about an H1 cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Not water and not in a ring: nothing here is wet.
    Dry,
    /// A water cell of the lake (basin cell under the level, or a shore cell that drains into it).
    Water(u64),
    /// A cell next to the lake's water that drains into the lake and is in no other basin.
    Ring(u64),
}

/// The window the rule reads: `zl` is `w x w` heights of the final surface, cell `(0, 0)` is the face's local cell `origin`; `ratio` local cells make one H1 cell.
/// A window at a face edge holds cells outside the face (negative or past the edge): `zl` there is the surface read across the seam on the neighbouring face, and `status` must answer for the H1 cell
/// the same way (resolved on the neighbouring face); a window quadrant at a cube corner with no face cells reads as dry. The caller (the writer) owns that contract.
pub struct Window<'a> {
    pub zl: &'a [f64],
    pub w: usize,
    pub origin: (i64, i64),
    pub ratio: i64,
}

/// The mask of the core of a tile: the label of every cell of the core (`core x core` cells at `halo` in from the window's edge), [`NO_LAKE`] for a dry one.
/// `status` answers for an H1 cell `(i, j)` (face cell-index space of the H1 grid), `level` for a lake id.
pub fn lake_mask(win: &Window, halo: usize, core: usize, status: &dyn Fn(i64, i64) -> Status, level: &dyn Fn(u64) -> f64) -> Vec<u64> {
    let w = win.w;
    assert_eq!(win.zl.len(), w * w, "the window is w x w");
    assert!(win.ratio > 0, "local cells per H1 cell is positive, it is {}", win.ratio);
    assert!(halo + core + halo == w, "a window is the core with a halo around it");
    assert!(halo >= usize::from(RING_STEPS), "the halo reaches at least the ring depth ({RING_STEPS} cells), it is {halo}");
    let reach = usize::from(RING_STEPS);
    // The region the rule reads for the core: the core and the reach around it (the sources of a ring that ends in the core lie within the reach).
    let (lo, hi) = (halo - reach, halo + core + reach);
    let side = hi - lo;
    let st = |x: usize, y: usize| {
        let (gi, gj) = (win.origin.0 + x as i64, win.origin.1 + y as i64);
        status(gi.div_euclid(win.ratio), gj.div_euclid(win.ratio))
    };
    // Per region cell: the lake it is wet for (NO_LAKE), and the ring depth for the breadth-first pass.
    let mut label = vec![NO_LAKE; side * side];
    let mut depth = vec![u8::MAX; side * side];
    let mut frontier: VecDeque<(usize, usize)> = VecDeque::new();
    for y in lo..hi {
        for x in lo..hi {
            if let Status::Water(l) = st(x, y) {
                if win.zl[y * w + x] < level(l) {
                    let r = (y - lo) * side + (x - lo);
                    label[r] = l;
                    depth[r] = 0;
                    frontier.push_back((x, y));
                }
            }
        }
    }
    // The ring: breadth-first from every core wet cell at once; a cell's depth is the least number of steps from any of them, so the result does not depend on the order.
    while let Some((x, y)) = frontier.pop_front() {
        let r = (y - lo) * side + (x - lo);
        let (d, l) = (depth[r], label[r]);
        if d >= RING_STEPS {
            continue;
        }
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if (dx, dy) == (0, 0) || nx < lo as i64 || ny < lo as i64 || nx >= hi as i64 || ny >= hi as i64 {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                let nr = (ny - lo) * side + (nx - lo);
                if depth[nr] != u8::MAX {
                    continue;
                }
                if st(nx, ny) == Status::Ring(l) && win.zl[ny * w + nx] < level(l) {
                    label[nr] = l;
                    depth[nr] = d + 1;
                    frontier.push_back((nx, ny));
                }
            }
        }
    }
    let off = halo - lo;
    let mut out = Vec::with_capacity(core * core);
    for y in 0..core {
        for x in 0..core {
            out.push(label[(y + off) * side + x + off]);
        }
    }
    out
}

/// The lake ids of a mask, sorted and without repeats (the tile's lake list).
pub fn lakes_of(labels: &[u64]) -> Vec<u64> {
    let mut v: Vec<u64> = labels.iter().copied().filter(|&l| l != NO_LAKE).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Why a mask could not be encoded or decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaskError(pub String);

impl std::fmt::Display for MaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The water section of a tile: empty when no cell is wet, else the runs of the mask (`labels` is `core x core`). Refused above [`MAX_LAKES_PER_TILE`] lakes or for a core wider than a `u16` run.
pub fn encode(labels: &[u64], core: usize) -> Result<Vec<u8>, MaskError> {
    assert_eq!(labels.len(), core * core, "a mask is core x core");
    let lakes = lakes_of(labels);
    if lakes.is_empty() {
        return Ok(Vec::new());
    }
    if lakes.len() > MAX_LAKES_PER_TILE {
        return Err(MaskError(format!("the tile lists {} lakes, at most {MAX_LAKES_PER_TILE} (a run label is a byte)", lakes.len())));
    }
    if core > usize::from(u16::MAX) {
        return Err(MaskError(format!("a core of {core} cells a side does not fit the u16 run starts")));
    }
    let mut out = vec![lakes.len() as u8];
    for l in &lakes {
        out.extend(l.to_le_bytes());
    }
    for row in labels.chunks(core) {
        let mut runs: Vec<(u16, u16, u8)> = Vec::new();
        let mut x = 0;
        while x < core {
            if row[x] == NO_LAKE {
                x += 1;
                continue;
            }
            let l = row[x];
            let start = x;
            while x < core && row[x] == l {
                x += 1;
            }
            let idx = lakes.binary_search(&l).expect("a listed lake") as u8;
            runs.push((start as u16, (x - start) as u16, idx));
        }
        out.extend((runs.len() as u16).to_le_bytes());
        for (s, n, i) in runs {
            out.extend(s.to_le_bytes());
            out.extend(n.to_le_bytes());
            out.push(i);
        }
    }
    Ok(out)
}

/// The mask of a stored water section (`core x core` labels): an empty section is all dry. A section that does not fit the layout is an error naming what was found.
pub fn decode(bytes: &[u8], core: usize) -> Result<Vec<u64>, MaskError> {
    let mut labels = vec![NO_LAKE; core * core];
    if bytes.is_empty() {
        return Ok(labels);
    }
    // Canonical form only: the lake ids strictly increasing, the runs of a row in order, apart and not adjacent for one lake, so that a damaged section is not read as another valid mask.
    let bad = |why: String| MaskError(format!("water section of {} bytes: {why}", bytes.len()));
    let n = usize::from(bytes[0]);
    let mut at = 1 + 8 * n;
    if n == 0 || bytes.len() < at {
        return Err(bad(format!("it lists {n} lakes but holds {} bytes", bytes.len())));
    }
    let lakes: Vec<u64> = (0..n).map(|k| u64::from_le_bytes(bytes[1 + 8 * k..9 + 8 * k].try_into().expect("eight bytes"))).collect();
    if lakes.windows(2).any(|p| p[0] >= p[1]) || lakes.contains(&NO_LAKE) {
        return Err(bad("its lake list is not strictly increasing or holds the dry label".into()));
    }
    for y in 0..core {
        if at + 2 > bytes.len() {
            return Err(bad(format!("row {y} has no run count")));
        }
        let runs = usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
        at += 2;
        if at + 5 * runs > bytes.len() {
            return Err(bad(format!("row {y} claims {runs} runs, {} bytes are left", bytes.len() - at)));
        }
        let mut prev: Option<(usize, usize)> = None; // end and lake of the previous run
        for _ in 0..runs {
            let s = usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
            let len = usize::from(u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]));
            let idx = usize::from(bytes[at + 4]);
            at += 5;
            if idx >= n || s + len > core || len == 0 {
                return Err(bad(format!(
                    "row {y} has a run (start {s}, length {len}, lake {idx}) outside the tile or the list of {n} lakes"
                )));
            }
            if prev.is_some_and(|(end, l)| s < end || (s == end && l == idx)) {
                return Err(bad(format!("row {y} has runs that overlap, are out of order or could be one run (start {s})")));
            }
            prev = Some((s + len, idx));
            labels[y * core + s..y * core + s + len].fill(lakes[idx]);
        }
    }
    if at != bytes.len() {
        return Err(bad(format!("{} bytes follow the last row", bytes.len() - at)));
    }
    Ok(labels)
}

/// The diagnostic of design section 13 (D20 point 2), reported and not gated: the wet cells of each lake that are not 8-connected, inside the tile, to the lake's largest wet component, as
/// `(lake id, cells)`; the area is the cells times the cell area.
pub fn disconnected_wet(labels: &[u64], core: usize) -> Vec<(u64, usize)> {
    let mut comp = vec![u32::MAX; labels.len()];
    let mut sizes: Vec<(u64, usize)> = Vec::new(); // per component: lake and size
    for start in 0..labels.len() {
        if labels[start] == NO_LAKE || comp[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut size = 0;
        let mut stack = vec![start];
        comp[start] = id;
        while let Some(c) = stack.pop() {
            size += 1;
            let (x, y) = ((c % core) as i64, (c / core) as i64);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= core as i64 || ny >= core as i64 {
                        continue;
                    }
                    let n = ny as usize * core + nx as usize;
                    if comp[n] == u32::MAX && labels[n] == labels[start] {
                        comp[n] = id;
                        stack.push(n);
                    }
                }
            }
        }
        sizes.push((labels[start], size));
    }
    let mut out: Vec<(u64, usize)> = Vec::new();
    for lake in lakes_of(labels) {
        let (total, largest) = sizes.iter().filter(|s| s.0 == lake).fold((0, 0), |(t, m), s| (t + s.1, m.max(s.1)));
        if total > largest {
            out.push((lake, total - largest));
        }
    }
    out
}
