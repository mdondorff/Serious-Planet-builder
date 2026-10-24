//! The coarse horizon of the global run as a stored channel (M8-5, STRM-045): blocks of the canonical layout of the observer lattice
//! (`GlobalWorld::layout(m)`), one byte per observer cell and azimuth, LZ4. The map is a pure function of the coupled run's stored heights and
//! the march (`generators::coarsehorizon`), and the key carries both.

use std::path::Path;

use planet_core::{Face, TileId};
use planet_generators::coarsehorizon::MAX_AZIMUTHS;
use planet_generators::global::MAX_BLOCK_CELLS;

use crate::global_store::{channel_record, stored_channel, BlockCodecError, GlobalBuildError, GlobalWorld, TileChannel};
use crate::key::{CacheKey, Kind};
use crate::pack::{PackWriter, Record, TileStore};

/// The kind byte of a block of the coarse-horizon channel.
pub const KIND_GLOBAL_HORIZON: u8 = Kind::GlobalHorizon as u8;

/// Version of the coarse-horizon block payload layout. History: 1 = `codec version (u8) | B (u32 LE) | azimuths (u32 LE) | raw length (u32 LE)`,
/// then an LZ4 block of `B x B x azimuths` bytes (row `j` outer, then `i`, then the azimuth).
pub const HORIZON_CODEC_VERSION: u8 = 1;

const HEADER_LEN: usize = 13;

/// A block of the coarse horizon: `cells x cells` observer cells, `azimuths` bytes each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HorizonBlock {
    pub cells: u32,
    pub azimuths: u32,
    pub bytes: Vec<u8>,
}

fn valid_cells(cells: u32) -> bool {
    cells.is_power_of_two() && cells <= MAX_BLOCK_CELLS
}

fn raw_len(cells: u32, azimuths: u32) -> u64 {
    u64::from(cells) * u64::from(cells) * u64::from(azimuths)
}

/// Encode a block; refused when it is not what the decoder would read back.
pub fn encode_horizon_block(block: &HorizonBlock) -> Result<Vec<u8>, BlockCodecError> {
    if !valid_cells(block.cells) {
        return Err(BlockCodecError::Cells(block.cells));
    }
    if block.azimuths == 0 || block.azimuths > MAX_AZIMUTHS {
        return Err(BlockCodecError::HorizonAzimuths(block.azimuths));
    }
    let expected = raw_len(block.cells, block.azimuths);
    if block.bytes.len() as u64 != expected {
        return Err(BlockCodecError::Samples { got: block.bytes.len(), expected });
    }
    let mut out = Vec::with_capacity(HEADER_LEN + block.bytes.len() / 2);
    out.push(HORIZON_CODEC_VERSION);
    out.extend_from_slice(&block.cells.to_le_bytes());
    out.extend_from_slice(&block.azimuths.to_le_bytes());
    out.extend_from_slice(&u32::try_from(expected).expect("at most 1024^2 x 256 bytes").to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&block.bytes));
    Ok(out)
}

/// Decode a block payload: refuses what [`encode_horizon_block`] would not write, without allocating more than `B` and the azimuth limit allow.
pub fn decode_horizon_block(bytes: &[u8]) -> Result<HorizonBlock, BlockCodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(BlockCodecError::Short);
    }
    if bytes[0] != HORIZON_CODEC_VERSION {
        return Err(BlockCodecError::HorizonVersion(bytes[0]));
    }
    let cells = u32::from_le_bytes(bytes[1..5].try_into().expect("4 bytes"));
    let azimuths = u32::from_le_bytes(bytes[5..9].try_into().expect("4 bytes"));
    if !valid_cells(cells) {
        return Err(BlockCodecError::Cells(cells));
    }
    if azimuths == 0 || azimuths > MAX_AZIMUTHS {
        return Err(BlockCodecError::HorizonAzimuths(azimuths));
    }
    let claimed = u32::from_le_bytes(bytes[9..13].try_into().expect("4 bytes"));
    let expected = raw_len(cells, azimuths);
    if u64::from(claimed) != expected {
        return Err(BlockCodecError::RawLength { claimed, expected });
    }
    let raw = lz4_flex::decompress(&bytes[HEADER_LEN..], claimed as usize).map_err(|e| BlockCodecError::Lz4(e.to_string()))?;
    if raw.len() as u64 != expected {
        return Err(BlockCodecError::Truncated { got: raw.len(), expected });
    }
    Ok(HorizonBlock { cells, azimuths, bytes: raw })
}

/// The coarse-horizon channel (STRM-045).
pub struct HorizonChannel;

impl TileChannel for HorizonChannel {
    const KIND: u8 = KIND_GLOBAL_HORIZON;
    const LABEL: &'static str = "coarse horizon block";
    type Payload = HorizonBlock;
    fn key(world: &GlobalWorld, tile: TileId, cells: u32) -> CacheKey {
        world.horizon_key(tile, cells)
    }
    fn encode(_cells: u32, payload: &Self::Payload) -> Result<Vec<u8>, BlockCodecError> {
        encode_horizon_block(payload)
    }
    fn decode(bytes: &[u8]) -> Result<(u32, Self::Payload), BlockCodecError> {
        decode_horizon_block(bytes).map(|b| (b.cells, b))
    }
}

/// The stored coarse horizon reassembled over the observer lattice: `6 m m azimuths` bytes, faces in order, rows `j` outer, then `i`, then the
/// azimuth (the layout of `generators::coarsehorizon::coarse_horizon`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredHorizon {
    pub m: u32,
    pub azimuths: u32,
    pub bytes: Vec<u8>,
}

impl GlobalWorld {
    /// The records of a coarse horizon `map` (the output of `coarse_horizon` for the world's spec): one per tile of the observer lattice's layout.
    pub fn horizon_records(&self, map: &[u8]) -> Result<Vec<Record>, BlockCodecError> {
        let spec = self.horizon_spec();
        let (m, a) = (spec.m as usize, spec.params.azimuths as usize);
        // The observer lattice is laid over the coupled run's: its size must divide the run's, and the azimuth count must be one a block holds.
        let run_n = self.coupled_spec().n;
        if spec.m == 0 || spec.m > run_n || !run_n.is_multiple_of(spec.m) {
            return Err(BlockCodecError::Grid { level: 0, cells: spec.m });
        }
        if spec.params.azimuths == 0 || spec.params.azimuths > MAX_AZIMUTHS {
            return Err(BlockCodecError::HorizonAzimuths(spec.params.azimuths));
        }
        let (level, cells) = self.layout(spec.m).map_err(|_| BlockCodecError::Grid { level: 0, cells: spec.m })?;
        let expected = 6 * (m as u64) * (m as u64) * a as u64;
        if map.len() as u64 != expected {
            return Err(BlockCodecError::Samples { got: map.len(), expected });
        }
        let per_edge = 1u32 << level;
        let b = cells as usize;
        let mut out = Vec::new();
        for face in 0..6u8 {
            for ty in 0..per_edge {
                for tx in 0..per_edge {
                    let tile = TileId::new(Face(face), level, tx, ty).expect("a tile of the layout");
                    let mut block = HorizonBlock { cells, azimuths: spec.params.azimuths, bytes: Vec::with_capacity(b * b * a) };
                    for j in 0..b {
                        let row = ((usize::from(face) * m + ty as usize * b + j) * m + tx as usize * b) * a;
                        block.bytes.extend_from_slice(&map[row..row + b * a]);
                    }
                    out.push(channel_record::<HorizonChannel>(self, tile, cells, &block)?);
                }
            }
        }
        Ok(out)
    }
}

/// Write the records of a coarse horizon and flush the store.
pub fn store_horizon(dir: &Path, world: &GlobalWorld, map: &[u8], max_pack_bytes: u64) -> Result<usize, GlobalBuildError> {
    let records = world.horizon_records(map)?;
    let mut writer = PackWriter::open(dir, world.params(), max_pack_bytes)?;
    for r in &records {
        writer.append(r)?;
    }
    writer.finish()?;
    Ok(records.len())
}

/// The stored coarse horizon reassembled: an error naming the block when one is missing, damaged, of another `B` or of another azimuth count.
pub fn load_horizon(store: &TileStore, world: &GlobalWorld) -> Result<StoredHorizon, GlobalBuildError> {
    let spec = world.horizon_spec();
    let (m, a) = (spec.m as usize, spec.params.azimuths as usize);
    let (level, cells) = world.layout(spec.m)?;
    let (per_edge, b) = (1u32 << level, cells as usize);
    let mut bytes = vec![0u8; 6 * m * m * a];
    for face in 0..6u8 {
        for ty in 0..per_edge {
            for tx in 0..per_edge {
                let tile = TileId::new(Face(face), level, tx, ty).expect("a tile of the layout");
                let block = stored_channel::<HorizonChannel>(store, world, tile, cells)?;
                if block.azimuths != spec.params.azimuths {
                    return Err(GlobalBuildError::MissingBlock(format!(
                        "{} {:#018x}: it has {} azimuths, the spec says {}",
                        HorizonChannel::LABEL,
                        tile.raw(),
                        block.azimuths,
                        spec.params.azimuths
                    )));
                }
                for j in 0..b {
                    let row = ((usize::from(face) * m + ty as usize * b + j) * m + tx as usize * b) * a;
                    bytes[row..row + b * a].copy_from_slice(&block.bytes[j * b * a..(j + 1) * b * a]);
                }
            }
        }
    }
    Ok(StoredHorizon { m: spec.m, azimuths: spec.params.azimuths, bytes })
}
