//! The six cube faces as one grid for the kernels (GEN-144, A3 sections 4.1 and 6.1): `6 n^2` cells stacked as a `n x 6n` grid (cell `(face, i, j)` is `face n^2 + j n + i`, the id of
//! [`GlobalGrid`]), neighbours joined across the cube edges as H0 joins them (GEN-139): a cell has its 8 neighbours inside its face and, across an edge, the one axis neighbour of the face it
//! meets; a diagonal across an edge or a corner is not an edge of the graph. Direction slots are those of [`super::NEIGHBOURS`]; a missing diagonal is a missing neighbour. Areas and distances are
//! the sphere's (the great circle between the cell centres).

use super::h0::GlobalGrid;
use super::NEIGHBOURS;

/// The cube topology of a [`super::Grid`].
#[derive(Clone)]
pub struct Cube {
    n: usize,
    radius_m: f64,
    g: GlobalGrid,
    /// The neighbour across an edge: `[face][side][k]`, side 0 = `i = 0` (k = j), 1 = `i = n - 1` (k = j), 2 = `j = 0` (k = i), 3 = `j = n - 1` (k = i).
    links: Vec<u32>,
}

impl std::fmt::Debug for Cube {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cube {{ n: {}, radius_m: {} }}", self.n, self.radius_m)
    }
}

impl PartialEq for Cube {
    fn eq(&self, other: &Cube) -> bool {
        self.n == other.n && self.radius_m.to_bits() == other.radius_m.to_bits() && self.links == other.links
    }
}

impl Cube {
    pub fn new(n: usize, radius_m: f64) -> Cube {
        let g = GlobalGrid::new(n, radius_m);
        let mut links = Vec::with_capacity(6 * 4 * n);
        for f in 0..6u8 {
            for side in 0..4 {
                for k in 0..n {
                    let (i, j, di, dj) = match side {
                        0 => (0, k, -1, 0),
                        1 => (n - 1, k, 1, 0),
                        2 => (k, 0, 0, -1),
                        _ => (k, n - 1, 0, 1),
                    };
                    links.push(g.axis_neighbour(g.id(f, i, j), di, dj));
                }
            }
        }
        Cube { n, radius_m, g, links }
    }

    pub fn n(&self) -> usize {
        self.n
    }

    /// The neighbour `d` (a slot of [`NEIGHBOURS`]) of cell `p`.
    pub fn neighbour(&self, p: usize, d: usize) -> Option<usize> {
        let n = self.n;
        let (f, r) = (p / (n * n), p % (n * n));
        let (i, j) = ((r % n) as i64, (r / n) as i64);
        let (ni, nj) = (i + NEIGHBOURS[d].0, j + NEIGHBOURS[d].1);
        if ni >= 0 && nj >= 0 && ni < n as i64 && nj < n as i64 {
            return Some(f * n * n + nj as usize * n + ni as usize);
        }
        if d >= 4 {
            return None;
        }
        let (side, k) = match d {
            0 => (0, j),
            1 => (1, j),
            2 => (2, i),
            _ => (3, i),
        };
        Some(self.links[(f * 4 + side) * n + k as usize] as usize)
    }

    /// The area of cell `p` in square metres.
    pub fn area(&self, p: usize) -> f64 {
        self.g.area(p as u32)
    }

    /// The great-circle distance between the centres of cells `p` and `q`.
    pub fn distance(&self, p: usize, q: usize) -> f64 {
        self.g.arc(self.g.direction(p as u32), q as u32)
    }

    /// The face of cell `p`.
    pub fn face(&self, p: usize) -> usize {
        p / (self.n * self.n)
    }
}
