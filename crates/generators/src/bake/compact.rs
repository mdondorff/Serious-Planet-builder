//! The cells of a landmass of the cube as a compact index (GEN-147, A3 section 6.6): a grid of only the active cells, so that the arrays of a solve cost `n_active` and not `6 n^2`. The compact index
//! of an active cell is its rank among the active cells in the order of the cube's cell ids, so every kernel that visits cells by index (the flood's ties, the area's order) visits them as it does on the
//! masked cube grid, and the results are the same bits. A lookup is a bitset rank (a popcount and a table of counts a word); the inverse is a table of cube ids.

/// The active cells of a cube grid of `6 n^2` cells.
pub struct ActiveIndex {
    bits: Vec<u64>,
    /// The number of set bits before each word.
    prefix: Vec<u32>,
    /// The cube id of each active cell, ascending.
    pos: Vec<u32>,
}

impl std::fmt::Debug for ActiveIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ActiveIndex {{ cells: {} }}", self.pos.len())
    }
}

/// Two indexes are equal when their bitsets are (the rank table and the ids are derived from them).
impl PartialEq for ActiveIndex {
    fn eq(&self, other: &ActiveIndex) -> bool {
        self.bits == other.bits
    }
}

impl ActiveIndex {
    /// From the bitset of the active cube ids (bit `g % 64` of word `g / 64`).
    pub fn new(bits: Vec<u64>) -> ActiveIndex {
        let mut prefix = Vec::with_capacity(bits.len());
        let mut count = 0u32;
        for &w in &bits {
            prefix.push(count);
            count += w.count_ones();
        }
        let mut pos = Vec::with_capacity(count as usize);
        for (k, &w) in bits.iter().enumerate() {
            let mut w = w;
            while w != 0 {
                pos.push((k * 64 + w.trailing_zeros() as usize) as u32);
                w &= w - 1;
            }
        }
        ActiveIndex { bits, prefix, pos }
    }

    /// The number of active cells.
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    /// The cube id of compact cell `p`.
    pub fn cube_id(&self, p: usize) -> usize {
        self.pos[p] as usize
    }

    /// The compact index of the cube cell `g`, if it is active.
    pub fn rank(&self, g: usize) -> Option<usize> {
        let (w, b) = (g >> 6, g & 63);
        let word = *self.bits.get(w)?;
        (word >> b & 1 == 1).then(|| self.prefix[w] as usize + (word & ((1u64 << b) - 1)).count_ones() as usize)
    }
}
