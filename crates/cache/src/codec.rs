//! The payload codec of a height tile (ADR 0019): a lossless byte shuffle of the `f32` plane, then an LZ4 block.
//!
//! Layout: `codec version (u8) | cells per edge `res` (u32 LE) | raw length (u32 LE) | LZ4 block`. The raw bytes are
//! the `(res + 1)^2` heights (`res` cells, so `res + 1` samples, per edge) as little-endian `f32`, shuffled so that all first bytes come first, then all second bytes
//! and so on: neighbouring heights share their high bytes, which LZ4 then finds as long runs. Everything is safe Rust
//! (`lz4_flex` without its `unsafe` paths). Equality of tiles is judged on the decoded heights, never on the compressed
//! bytes, so a change of the LZ4 implementation cannot make a stored tile look different.

use planet_core::TileId;
use planet_generators::height::TileData;

/// Version of the payload layout. History: 1 = shuffled f32 plane, LZ4 block.
pub const CODEC_VERSION: u8 = 1;

/// Largest `res` a payload may claim: a hostile header must not make a decoder allocate gigabytes.
pub const MAX_RES: u32 = 4096;

const HEADER_LEN: usize = 9;

/// Why a payload could not be encoded or decoded. Every variant says what was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    Short,
    Version(u8),
    Res(u32),
    RawLength {
        claimed: u32,
        expected: u64,
    },
    /// The LZ4 block decompressed to fewer bytes than the header says.
    Truncated {
        got: usize,
        expected: u64,
    },
    Lz4(String),
    /// Encoding: the tile has a number of heights that does not match its resolution.
    Heights {
        got: usize,
        expected: u64,
    },
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Short => write!(f, "the payload is shorter than its header"),
            CodecError::Version(v) => write!(f, "codec version {v}, this build reads {CODEC_VERSION}"),
            CodecError::Res(r) => write!(f, "{r} cells per edge is not a power of two up to {MAX_RES}"),
            CodecError::RawLength { claimed, expected } => {
                write!(f, "the payload claims {claimed} raw bytes, {expected} are needed for its resolution")
            }
            CodecError::Truncated { got, expected } => {
                write!(f, "the LZ4 block gives {got} raw bytes, {expected} are needed for its resolution")
            }
            CodecError::Lz4(e) => write!(f, "the LZ4 block is damaged: {e}"),
            CodecError::Heights { got, expected } => write!(f, "the tile has {got} heights, {expected} are needed for its resolution"),
        }
    }
}

impl std::error::Error for CodecError {}

/// Whether `res` is a resolution a payload may have.
pub fn valid_res(res: u32) -> bool {
    res.is_power_of_two() && res <= MAX_RES
}

fn shuffle(heights: &[f32]) -> Vec<u8> {
    let n = heights.len();
    let mut out = vec![0u8; n * 4];
    for (i, h) in heights.iter().enumerate() {
        let b = h.to_le_bytes();
        for (k, byte) in b.iter().enumerate() {
            out[k * n + i] = *byte;
        }
    }
    out
}

fn unshuffle(bytes: &[u8]) -> Vec<f32> {
    let n = bytes.len() / 4;
    (0..n).map(|i| f32::from_le_bytes([bytes[i], bytes[n + i], bytes[2 * n + i], bytes[3 * n + i]])).collect()
}

/// Encode a height tile. A tile whose resolution the decoder would refuse, or whose number of heights does not match its
/// resolution, is an error here, so nothing is stored that cannot be read back.
pub fn encode_height_tile(t: &TileData) -> Result<Vec<u8>, CodecError> {
    if !valid_res(t.res) {
        return Err(CodecError::Res(t.res));
    }
    let expected = (u64::from(t.res) + 1) * (u64::from(t.res) + 1);
    if t.heights.len() as u64 != expected {
        return Err(CodecError::Heights { got: t.heights.len(), expected });
    }
    let raw = shuffle(&t.heights);
    let mut out = Vec::with_capacity(HEADER_LEN + raw.len() / 2);
    out.push(CODEC_VERSION);
    out.extend_from_slice(&t.res.to_le_bytes());
    out.extend_from_slice(&u32::try_from(raw.len()).expect("at most 4 * 4097^2 bytes").to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&raw));
    Ok(out)
}

/// The resolution a payload claims, if its header is well formed.
pub fn payload_res(bytes: &[u8]) -> Result<u32, CodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(CodecError::Short);
    }
    if bytes[0] != CODEC_VERSION {
        return Err(CodecError::Version(bytes[0]));
    }
    let res = u32::from_le_bytes(bytes[1..5].try_into().expect("4 bytes"));
    if !valid_res(res) {
        return Err(CodecError::Res(res));
    }
    Ok(res)
}

/// Decode the payload of the height tile `id`.
pub fn decode_height_tile(id: TileId, bytes: &[u8]) -> Result<TileData, CodecError> {
    let res = payload_res(bytes)?;
    let claimed = u32::from_le_bytes(bytes[5..9].try_into().expect("4 bytes"));
    let expected = 4 * u64::from(res + 1) * u64::from(res + 1);
    if u64::from(claimed) != expected {
        return Err(CodecError::RawLength { claimed, expected });
    }
    let raw = lz4_flex::decompress(&bytes[HEADER_LEN..], claimed as usize).map_err(|e| CodecError::Lz4(e.to_string()))?;
    if raw.len() as u64 != expected {
        return Err(CodecError::Truncated { got: raw.len(), expected });
    }
    Ok(TileData { id, res, heights: unshuffle(&raw) })
}
